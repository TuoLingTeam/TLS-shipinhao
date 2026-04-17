//! 订单评分引擎：围绕"买家昵称 + 商品信息"合成 0~100 分。
//!
//! 昵称侧相似度算法由 [`crate::matching::nickname`] 提供；本模块聚焦商品侧相似度
//! 与综合评分裁剪，同时 re-export [`similarity_percent`] 等给既有调用方保持不动。

use crate::matching::nickname::{clamp_percent, sequence_similarity};
use regex::Regex;
use serde::{Deserialize, Serialize};
use std::cmp::{max, min};
use std::sync::OnceLock;

pub use crate::matching::nickname::similarity_percent;

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
