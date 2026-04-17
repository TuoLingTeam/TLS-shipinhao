use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Default)]
#[serde(rename_all = "camelCase")]
pub struct CandidateMatch {
    pub score: i32,
    pub confirm_diff: i64,
    pub time_diff: i64,
}

pub fn build_product_reason(
    product_exact: bool,
    product_similarity: i32,
    title_similarity: i32,
    product_id_exact: bool,
    sku_id_exact: bool,
    product_penalty: i32,
) -> String {
    if product_exact {
        return "商品标题/商品ID/SKU 完全匹配".to_string();
    }
    format!(
        "商品信息相似度 {}%(标题 {}%，ID {}，SKU {})(扣 {} 分)",
        product_similarity,
        title_similarity,
        if product_id_exact {
            "命中"
        } else {
            "未命中"
        },
        if sku_id_exact { "命中" } else { "未命中" },
        product_penalty,
    )
}

pub fn build_nickname_reason(
    evaluation_buyer_nickname: &str,
    order_buyer_nickname: &str,
    similarity: i32,
    penalty: i32,
) -> String {
    let eval_name = evaluation_buyer_nickname.trim();
    let order_name = order_buyer_nickname.trim();
    let (shorter, longer) = if eval_name.chars().count() <= order_name.chars().count() {
        (eval_name, order_name)
    } else {
        (order_name, eval_name)
    };

    if shorter.chars().count() == 1 && !shorter.is_empty() && longer.contains(shorter) {
        return format!(
            "昵称仅单字重合，歧义较高！(相似度 {}%，扣 {} 分)",
            similarity, penalty
        );
    }

    format!(
        "昵称相似度较高，疑似改名！(相似度 {}%，扣 {} 分)",
        similarity, penalty
    )
}

pub fn pick_best_match(best_matches: &mut [CandidateMatch]) -> Option<CandidateMatch> {
    if best_matches.is_empty() {
        return None;
    }
    best_matches.sort_by(|left, right| {
        right
            .score
            .cmp(&left.score)
            .then_with(|| left.confirm_diff.cmp(&right.confirm_diff))
            .then_with(|| left.time_diff.cmp(&right.time_diff))
    });
    best_matches.first().cloned()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn builds_exact_product_reason() {
        assert_eq!(
            build_product_reason(true, 100, 100, true, true, 0),
            "商品标题/商品ID/SKU 完全匹配"
        );
    }

    #[test]
    fn builds_partial_product_reason() {
        assert_eq!(
            build_product_reason(false, 88, 75, true, false, 10),
            "商品信息相似度 88%(标题 75%，ID 命中，SKU 未命中)(扣 10 分)"
        );
    }

    #[test]
    fn builds_ambiguous_single_character_reason() {
        let reason = build_nickname_reason("度", "城市轻度假酒店-杨", 11, 45);
        assert!(reason.contains("昵称仅单字重合，歧义较高！"));
        assert!(reason.contains("相似度 11%"));
    }

    #[test]
    fn builds_rename_reason_for_multi_character_similarity() {
        let reason = build_nickname_reason("赵亮6057", "赵亮", 95, 5);
        assert!(reason.contains("昵称相似度较高，疑似改名！"));
        assert!(reason.contains("扣 5 分"));
    }

    #[test]
    fn picks_best_match_by_score_then_confirm_then_time() {
        let mut matches = vec![
            CandidateMatch {
                score: 90,
                confirm_diff: 30,
                time_diff: 20,
            },
            CandidateMatch {
                score: 95,
                confirm_diff: 50,
                time_diff: 10,
            },
            CandidateMatch {
                score: 95,
                confirm_diff: 40,
                time_diff: 100,
            },
        ];
        let picked = pick_best_match(&mut matches).expect("picked");
        assert_eq!(picked.score, 95);
        assert_eq!(picked.confirm_diff, 40);
    }
}
