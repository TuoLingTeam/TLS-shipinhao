use regex::Regex;
use serde::{Deserialize, Serialize};
use std::cmp::{max, min};
use std::sync::OnceLock;

const SIMILARITY_PENALTY_BANDS: &[(i32, i32)] = &[
    (100, 0),
    (90, 5),
    (80, 10),
    (70, 15),
    (60, 20),
    (50, 25),
    (40, 30),
    (30, 35),
    (20, 40),
    (10, 45),
    (0, 50),
];

const PRODUCT_ID_WEIGHT: i32 = 40;
const PRODUCT_SKU_WEIGHT: i32 = 40;
const PRODUCT_TITLE_WEIGHT: i32 = 20;
const PRODUCT_SIMILARITY_WEIGHT_TOTAL: i32 =
    PRODUCT_ID_WEIGHT + PRODUCT_SKU_WEIGHT + PRODUCT_TITLE_WEIGHT;
const MIN_MATCH_SCORE: i32 = 50;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Default)]
#[serde(rename_all = "camelCase")]
pub struct ProductSimilarityResult {
    pub product_exact: bool,
    pub product_id_exact: bool,
    pub sku_id_exact: bool,
    pub title_exact: bool,
    pub title_similarity: i32,
    pub product_similarity: i32,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Default)]
#[serde(rename_all = "camelCase")]
pub struct MatchScoreResult {
    pub buyer_nickname_exact: bool,
    pub buyer_nickname_similarity: i32,
    pub buyer_nickname_penalty: i32,
    pub product_penalty: i32,
    pub score: i32,
    #[serde(flatten)]
    pub product: ProductSimilarityResult,
}

pub fn clamp_percent(value: f64) -> i32 {
    if !value.is_finite() {
        return 0;
    }
    min(100, max(0, value.round() as i32))
}

pub fn similarity_percent(left: Option<&str>, right: Option<&str>) -> i32 {
    let left_text = left.unwrap_or("");
    let right_text = right.unwrap_or("");
    if left_text == right_text {
        return 100;
    }
    if left_text.is_empty() || right_text.is_empty() {
        return 0;
    }

    let left_trimmed = left_text.trim();
    let right_trimmed = right_text.trim();
    if !left_trimmed.is_empty() && left_trimmed == right_trimmed {
        return 95;
    }

    if let Some(similarity) = nickname_similarity_by_rename_patterns(left_trimmed, right_trimmed) {
        return similarity;
    }

    sequence_similarity(left_text, right_text)
}

pub fn normalize_product_title_for_similarity(title: Option<&str>) -> String {
    static RE: OnceLock<Regex> = OnceLock::new();
    let re = RE.get_or_init(|| Regex::new(r"[\s，,、/\-_|（）()]+").expect("valid regex"));
    let title = title.unwrap_or("");
    if title.is_empty() {
        return String::new();
    }
    re.replace_all(&title.to_lowercase(), "").into_owned()
}

pub fn title_similarity_percent(left: Option<&str>, right: Option<&str>) -> i32 {
    let left_text = left.unwrap_or("");
    let right_text = right.unwrap_or("");
    if left_text == right_text {
        return 100;
    }
    let left_norm = normalize_product_title_for_similarity(Some(left_text));
    let right_norm = normalize_product_title_for_similarity(Some(right_text));
    if left_norm.is_empty() || right_norm.is_empty() {
        return 0;
    }
    if left_norm == right_norm {
        return 100;
    }
    sequence_similarity(&left_norm, &right_norm)
}

pub fn penalty_from_similarity(similarity: i32) -> i32 {
    let value = clamp_percent(similarity as f64);
    for (minimum, penalty) in SIMILARITY_PENALTY_BANDS {
        if value >= *minimum {
            return *penalty;
        }
    }
    50
}

pub fn compute_product_similarity(
    evaluation_product_id: Option<&str>,
    evaluation_sku_id: Option<&str>,
    evaluation_title: Option<&str>,
    order_product_id: Option<&str>,
    order_sku_id: Option<&str>,
    order_title: Option<&str>,
) -> ProductSimilarityResult {
    let eval_product_id = evaluation_product_id.unwrap_or("");
    let eval_sku_id = evaluation_sku_id.unwrap_or("");
    let eval_title = evaluation_title.unwrap_or("");
    let order_product_id = order_product_id.unwrap_or("");
    let order_sku_id = order_sku_id.unwrap_or("");
    let order_title = order_title.unwrap_or("");

    let product_id_exact = !eval_product_id.is_empty() && eval_product_id == order_product_id;
    let sku_id_exact = !eval_sku_id.is_empty() && eval_sku_id == order_sku_id;
    let title_exact = !eval_title.is_empty() && eval_title == order_title;

    let product_id_similarity = if product_id_exact { 100 } else { 0 };
    let sku_id_similarity = if sku_id_exact { 100 } else { 0 };
    let title_similarity = title_similarity_percent(Some(eval_title), Some(order_title));

    let weighted_similarity =
        weighted_product_similarity(product_id_similarity, sku_id_similarity, title_similarity);

    let product_exact = product_id_exact && sku_id_exact && title_exact;
    let product_similarity = if product_exact {
        100
    } else {
        min(99, weighted_similarity)
    };

    ProductSimilarityResult {
        product_exact,
        product_id_exact,
        sku_id_exact,
        title_exact,
        title_similarity,
        product_similarity,
    }
}

pub fn compute_match_score(
    evaluation_buyer_nickname: Option<&str>,
    evaluation_product_id: Option<&str>,
    evaluation_sku_id: Option<&str>,
    evaluation_title: Option<&str>,
    order_buyer_nickname: Option<&str>,
    order_product_id: Option<&str>,
    order_sku_id: Option<&str>,
    order_title: Option<&str>,
) -> MatchScoreResult {
    let eval_buyer = evaluation_buyer_nickname.unwrap_or("");
    let order_buyer = order_buyer_nickname.unwrap_or("");

    let buyer_nickname_exact = !eval_buyer.is_empty() && eval_buyer == order_buyer;
    let buyer_nickname_similarity = similarity_percent(Some(eval_buyer), Some(order_buyer));

    let product = compute_product_similarity(
        evaluation_product_id,
        evaluation_sku_id,
        evaluation_title,
        order_product_id,
        order_sku_id,
        order_title,
    );

    let buyer_penalty = penalty_from_similarity(buyer_nickname_similarity);
    let product_penalty = penalty_from_similarity(product.product_similarity);

    let score = if buyer_nickname_exact && product.product_exact {
        100
    } else if product.product_exact {
        max(MIN_MATCH_SCORE, 100 - buyer_penalty)
    } else if buyer_nickname_exact {
        max(MIN_MATCH_SCORE, 100 - product_penalty)
    } else {
        max(
            MIN_MATCH_SCORE,
            100 - ((buyer_penalty + product_penalty) as f64 / 2.0).round() as i32,
        )
    };

    MatchScoreResult {
        buyer_nickname_exact,
        buyer_nickname_similarity,
        buyer_nickname_penalty: buyer_penalty,
        product_penalty,
        score,
        product,
    }
}

fn strip_trailing_digit_tail(text: &str) -> String {
    static RE: OnceLock<Regex> = OnceLock::new();
    let re =
        RE.get_or_init(|| Regex::new(r"[0-9０-９⁰¹²³⁴⁵⁶⁷⁸⁹₀₁₂₃₄₅₆₇₈₉\s]+$").expect("valid regex"));
    re.replace(text, "").trim().to_string()
}

fn is_subsequence(shorter: &str, longer: &str) -> bool {
    if shorter.is_empty() {
        return false;
    }
    let shorter_chars: Vec<char> = shorter.chars().collect();
    let mut pos = 0usize;
    for ch in longer.chars() {
        if pos < shorter_chars.len() && ch == shorter_chars[pos] {
            pos += 1;
            if pos == shorter_chars.len() {
                return true;
            }
        }
    }
    false
}

fn single_char_containment_similarity(longer: &str) -> i32 {
    let normalized_length = max(longer.chars().count(), 3);
    clamp_percent(100.0 / normalized_length as f64)
}

fn subsequence_similarity_by_length(text: &str) -> Option<i32> {
    let length = text.chars().count();
    if length >= 4 {
        Some(85)
    } else if length == 3 {
        Some(80)
    } else if length == 2 {
        Some(70)
    } else {
        None
    }
}

fn nickname_similarity_by_rename_patterns(left: &str, right: &str) -> Option<i32> {
    if left.is_empty() || right.is_empty() {
        return None;
    }

    let left_core = strip_trailing_digit_tail(left);
    let right_core = strip_trailing_digit_tail(right);

    if !left_core.is_empty() && !right_core.is_empty() && left_core == right_core && left != right {
        if left_core.chars().count() >= 2 {
            return Some(95);
        }
        return Some(80);
    }

    let (shorter, longer) = if left.chars().count() <= right.chars().count() {
        (left, right)
    } else {
        (right, left)
    };
    let (shorter_core, longer_core) = if left_core.chars().count() <= right_core.chars().count() {
        (left_core.as_str(), right_core.as_str())
    } else {
        (right_core.as_str(), left_core.as_str())
    };

    if !shorter.is_empty() && longer.contains(shorter) {
        let len = shorter.chars().count();
        if len >= 3 {
            return Some(90);
        }
        if len == 2 {
            return Some(80);
        }
        return Some(single_char_containment_similarity(longer));
    }

    if !shorter_core.is_empty() && longer_core.contains(shorter_core) {
        let len = shorter_core.chars().count();
        if len >= 3 {
            return Some(90);
        }
        if len == 2 {
            return Some(80);
        }
        return Some(single_char_containment_similarity(longer_core));
    }

    if let Some(similarity) = subsequence_similarity_by_length(shorter) {
        if is_subsequence(shorter, longer) {
            return Some(similarity);
        }
    }

    if let Some(similarity) = subsequence_similarity_by_length(shorter_core) {
        if is_subsequence(shorter_core, longer_core) {
            return Some(similarity);
        }
    }

    None
}

fn weighted_product_similarity(
    product_id_similarity: i32,
    sku_id_similarity: i32,
    title_similarity: i32,
) -> i32 {
    let weighted = (product_id_similarity * PRODUCT_ID_WEIGHT
        + sku_id_similarity * PRODUCT_SKU_WEIGHT
        + title_similarity * PRODUCT_TITLE_WEIGHT) as f64
        / PRODUCT_SIMILARITY_WEIGHT_TOTAL as f64;
    clamp_percent(weighted)
}

fn sequence_similarity(left: &str, right: &str) -> i32 {
    let left_chars: Vec<char> = left.chars().collect();
    let right_chars: Vec<char> = right.chars().collect();
    if left_chars.is_empty() || right_chars.is_empty() {
        return 0;
    }
    let lcs = lcs_length(&left_chars, &right_chars);
    let ratio = (2.0 * lcs as f64) / (left_chars.len() + right_chars.len()) as f64 * 100.0;
    clamp_percent(ratio)
}

fn lcs_length(left: &[char], right: &[char]) -> usize {
    let mut prev = vec![0usize; right.len() + 1];
    let mut curr = vec![0usize; right.len() + 1];
    for l in left {
        for (j, r) in right.iter().enumerate() {
            curr[j + 1] = if l == r {
                prev[j] + 1
            } else {
                max(prev[j + 1], curr[j])
            };
        }
        std::mem::swap(&mut prev, &mut curr);
        curr.fill(0);
    }
    prev[right.len()]
}

#[cfg(test)]
mod tests {
    use super::*;

    const PRODUCT_ID: &str = "7982968968";
    const SKU_ID: &str = "7982968968";
    const TITLE: &str = "仁和二硫化硒去屑洗发水止痒除螨控油清爽蓬松柔顺头屑清洁水润男女";

    #[test]
    fn exact_match_scores_100() {
        let result = compute_match_score(
            Some("💫其实一个我有故事的人"),
            Some(PRODUCT_ID),
            Some(SKU_ID),
            Some(TITLE),
            Some("💫其实一个我有故事的人"),
            Some(PRODUCT_ID),
            Some(SKU_ID),
            Some(TITLE),
        );
        assert_eq!(result.score, 100);
        assert!(result.buyer_nickname_exact);
        assert!(result.product.product_exact);
    }

    #[test]
    fn floor_score_at_50() {
        let result = compute_match_score(
            Some("甲"),
            Some("1"),
            Some("1"),
            Some("AAA"),
            Some("乙"),
            Some("2"),
            Some("2"),
            Some("ZZZ"),
        );
        assert_eq!(result.score, 50);
    }

    #[test]
    fn nickname_numeric_suffix_keeps_high_similarity() {
        let result = compute_match_score(
            Some("赵亮6057"),
            Some(PRODUCT_ID),
            Some(SKU_ID),
            Some(TITLE),
            Some("赵亮"),
            Some(PRODUCT_ID),
            Some(SKU_ID),
            Some(TITLE),
        );
        assert_eq!(result.score, 95);
        assert_eq!(result.buyer_nickname_similarity, 95);
    }

    #[test]
    fn nickname_subsequence_keeps_high_similarity() {
        let result = compute_match_score(
            Some("潍坊印刷"),
            Some(PRODUCT_ID),
            Some(SKU_ID),
            Some(TITLE),
            Some("潍坊精装印刷王宏杰"),
            Some(PRODUCT_ID),
            Some(SKU_ID),
            Some(TITLE),
        );
        assert_eq!(result.score, 90);
        assert!(result.buyer_nickname_similarity >= 80);
    }

    #[test]
    fn single_character_overlap_remains_low_similarity() {
        let result = compute_match_score(
            Some("我期待"),
            Some(PRODUCT_ID),
            Some(SKU_ID),
            Some(TITLE),
            Some("我"),
            Some(PRODUCT_ID),
            Some(SKU_ID),
            Some(TITLE),
        );
        assert_eq!(result.buyer_nickname_similarity, 33);
        assert_eq!(result.score, 65);
    }

    #[test]
    fn single_character_containment_in_long_name_stays_ambiguous() {
        let result = compute_match_score(
            Some("度"),
            Some(PRODUCT_ID),
            Some(SKU_ID),
            Some(TITLE),
            Some("城市轻度假酒店-杨"),
            Some(PRODUCT_ID),
            Some(SKU_ID),
            Some(TITLE),
        );
        assert!(result.buyer_nickname_similarity <= 15);
        assert_eq!(result.score, 55);
    }
}
