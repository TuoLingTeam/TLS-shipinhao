use crate::review_candidate_scoring::{score_candidate_order, CandidateOrder, EvaluationMatchContext};
use crate::review_matcher_helpers::pick_best_match;
use serde::{Deserialize, Serialize};

pub const AUTO_FILL_SCORE_THRESHOLD: i32 = 100;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Default)]
#[serde(rename_all = "snake_case")]
pub enum MatchStrategy {
    ExactMatch,
    HighConfidence,
    ProbableMatch,
    Fallback,
    #[default]
    None,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Default)]
#[serde(rename_all = "camelCase")]
pub struct SingleEvaluationMatch {
    pub matched_order: Option<CandidateOrder>,
    pub match_strategy: MatchStrategy,
    pub match_score: i32,
    pub match_reasons: Vec<String>,
}

pub fn match_strategy_by_score(score: i32) -> MatchStrategy {
    if score >= 100 {
        MatchStrategy::ExactMatch
    } else if score >= AUTO_FILL_SCORE_THRESHOLD {
        MatchStrategy::HighConfidence
    } else if score >= crate::review_candidate_scoring::MATCH_MIN_SCORE {
        MatchStrategy::ProbableMatch
    } else {
        MatchStrategy::Fallback
    }
}

pub fn match_single_evaluation(
    evaluation_context: &EvaluationMatchContext,
    candidate_orders: &[CandidateOrder],
) -> SingleEvaluationMatch {
    if candidate_orders.is_empty() {
        return SingleEvaluationMatch::default();
    }

    let mut best_matches = candidate_orders
        .iter()
        .filter_map(|order| score_candidate_order(order, evaluation_context))
        .collect::<Vec<_>>();

    if best_matches.is_empty() {
        return SingleEvaluationMatch::default();
    }

    best_matches.sort_by(|left, right| {
        right
            .score
            .cmp(&left.score)
            .then_with(|| left.confirm_diff.cmp(&right.confirm_diff))
            .then_with(|| left.time_diff.cmp(&right.time_diff))
    });

    let mut candidates = best_matches
        .iter()
        .map(|item| crate::review_matcher_helpers::CandidateMatch {
            score: item.score,
            confirm_diff: item.confirm_diff,
            time_diff: item.time_diff,
        })
        .collect::<Vec<_>>();
    let Some(best_key) = pick_best_match(&mut candidates) else {
        return SingleEvaluationMatch::default();
    };

    let best_match = best_matches
        .into_iter()
        .find(|item| {
            item.score == best_key.score
                && item.confirm_diff == best_key.confirm_diff
                && item.time_diff == best_key.time_diff
        })
        .expect("best match should exist");

    SingleEvaluationMatch {
        matched_order: Some(best_match.order),
        match_strategy: match_strategy_by_score(best_match.score),
        match_score: best_match.score,
        match_reasons: best_match.reasons,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const PRODUCT_ID: &str = "7982968968";
    const SKU_ID: &str = "7982968968";
    const TITLE: &str = "仁和二硫化硒去屑洗发水止痒除螨控油清爽蓬松柔顺头屑清洁水润男女";

    fn context() -> EvaluationMatchContext {
        EvaluationMatchContext {
            buyer_nickname: "无锡农膜¹³⁸⁶¹⁸²¹¹⁷⁵".into(),
            product_id: PRODUCT_ID.into(),
            sku_id: SKU_ID.into(),
            product_name: TITLE.into(),
            eval_time: 1_712_910_000,
        }
    }

    fn order(order_id: &str, buyer: &str, confirm_time: i64, status_variant: i64) -> CandidateOrder {
        CandidateOrder {
            order_id: order_id.into(),
            buyer_nickname: buyer.into(),
            product_id: PRODUCT_ID.into(),
            sku_id: SKU_ID.into(),
            product_name: TITLE.into(),
            create_time: 1_712_910_000 - 172800,
            confirm_receipt_time: confirm_time,
            is_waybill_received: false,
            waybill_received_time: 0,
            sale_param: format!("默认规格-{}", status_variant),
        }
    }

    #[test]
    fn returns_empty_when_no_candidates() {
        let result = match_single_evaluation(&context(), &[]);
        assert!(result.matched_order.is_none());
        assert_eq!(result.match_score, 0);
        assert!(result.match_reasons.is_empty());
    }

    #[test]
    fn returns_empty_when_all_candidates_are_filtered_out() {
        let mut future = order("373", "无锡农膜¹³⁸⁶¹⁸²¹¹⁷⁵", 0, 1);
        future.create_time = context().eval_time + 1;
        let result = match_single_evaluation(&context(), &[future]);
        assert!(result.matched_order.is_none());
        assert_eq!(result.match_strategy, MatchStrategy::None);
    }

    #[test]
    fn picks_best_candidate_by_score_and_strategy() {
        let wrong = order("3735582233824220672", "Y", context().eval_time - 86400, 1);
        let right = order("3735563912835389952", "无锡农膜¹³⁸⁶¹⁸²¹¹⁷⁵", 0, 2);
        let result = match_single_evaluation(&context(), &[wrong, right]);
        assert_eq!(result.matched_order.unwrap().order_id, "3735563912835389952");
        assert_eq!(result.match_score, 100);
        assert_eq!(result.match_strategy, MatchStrategy::ExactMatch);
    }

    #[test]
    fn chooses_lower_confirm_diff_when_scores_tie() {
        let mut a = order("A", "赵亮", 100, 1);
        let mut b = order("B", "赵亮", 80, 2);
        let ctx = EvaluationMatchContext {
            buyer_nickname: "赵亮6057".into(),
            product_id: PRODUCT_ID.into(),
            sku_id: SKU_ID.into(),
            product_name: TITLE.into(),
            eval_time: 120,
        };
        a.create_time = 50;
        b.create_time = 40;
        let result = match_single_evaluation(&ctx, &[a, b]);
        assert_eq!(result.matched_order.unwrap().order_id, "A");
        assert_eq!(result.match_strategy, MatchStrategy::ProbableMatch);
    }
}
