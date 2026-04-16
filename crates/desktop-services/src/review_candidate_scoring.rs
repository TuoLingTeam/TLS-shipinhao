use crate::order_match_scoring::{compute_match_score, MatchScoreResult};
use crate::review_matcher_helpers::{build_nickname_reason, build_product_reason};
use serde::{Deserialize, Serialize};

pub const MATCH_MIN_SCORE: i32 = 50;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Default)]
#[serde(rename_all = "camelCase")]
pub struct EvaluationMatchContext {
    pub buyer_nickname: String,
    pub product_id: String,
    pub sku_id: String,
    pub product_name: String,
    pub eval_time: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Default)]
#[serde(rename_all = "camelCase")]
pub struct CandidateOrder {
    pub order_id: String,
    pub buyer_nickname: String,
    pub product_id: String,
    pub sku_id: String,
    pub product_name: String,
    pub create_time: i64,
    pub confirm_receipt_time: i64,
    pub is_waybill_received: bool,
    pub waybill_received_time: i64,
    pub sale_param: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Default)]
#[serde(rename_all = "camelCase")]
pub struct ScoredCandidateOrder {
    pub order: CandidateOrder,
    pub score: i32,
    pub reasons: Vec<String>,
    pub time_diff: i64,
    pub confirm_diff: i64,
    pub match_result: MatchScoreResult,
}

pub fn resolve_reference_time(order: &CandidateOrder) -> i64 {
    if order.confirm_receipt_time > 0 {
        return order.confirm_receipt_time;
    }
    if order.is_waybill_received && order.waybill_received_time > 0 {
        return order.waybill_received_time;
    }
    0
}

pub fn score_candidate_order(
    order: &CandidateOrder,
    evaluation_context: &EvaluationMatchContext,
) -> Option<ScoredCandidateOrder> {
    let mut reasons = Vec::new();
    let reference_time = resolve_reference_time(order);

    if evaluation_context.eval_time > 0
        && order.create_time > 0
        && evaluation_context.eval_time < order.create_time
    {
        return None;
    }

    let match_result = compute_match_score(
        Some(&evaluation_context.buyer_nickname),
        Some(&evaluation_context.product_id),
        Some(&evaluation_context.sku_id),
        Some(&evaluation_context.product_name),
        Some(&order.buyer_nickname),
        Some(&order.product_id),
        Some(&order.sku_id),
        Some(&order.product_name),
    );
    let score = match_result.score;

    if match_result.buyer_nickname_exact {
        reasons.push("买家昵称完全匹配".to_string());
    } else {
        reasons.push(build_nickname_reason(
            &evaluation_context.buyer_nickname,
            &order.buyer_nickname,
            match_result.buyer_nickname_similarity,
            match_result.buyer_nickname_penalty,
        ));
    }

    reasons.push(build_product_reason(
        match_result.product.product_exact,
        match_result.product.product_similarity,
        match_result.product.title_similarity,
        match_result.product.product_id_exact,
        match_result.product.sku_id_exact,
        match_result.product_penalty,
    ));

    if score < MATCH_MIN_SCORE {
        return None;
    }

    Some(ScoredCandidateOrder {
        order: order.clone(),
        score,
        reasons,
        time_diff: if evaluation_context.eval_time > 0 && order.create_time > 0 {
            evaluation_context.eval_time - order.create_time
        } else {
            i64::MAX
        },
        confirm_diff: if evaluation_context.eval_time > 0 && reference_time > 0 {
            evaluation_context.eval_time - reference_time
        } else {
            i64::MAX
        },
        match_result,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    const PRODUCT_ID: &str = "7982968968";
    const SKU_ID: &str = "7982968968";
    const TITLE: &str = "仁和二硫化硒去屑洗发水止痒除螨控油清爽蓬松柔顺头屑清洁水润男女";

    fn context() -> EvaluationMatchContext {
        EvaluationMatchContext {
            buyer_nickname: "赵亮6057".into(),
            product_id: PRODUCT_ID.into(),
            sku_id: SKU_ID.into(),
            product_name: TITLE.into(),
            eval_time: 1_712_910_000,
        }
    }

    fn candidate() -> CandidateOrder {
        CandidateOrder {
            order_id: "3735325366963804672".into(),
            buyer_nickname: "赵亮".into(),
            product_id: PRODUCT_ID.into(),
            sku_id: SKU_ID.into(),
            product_name: TITLE.into(),
            create_time: 1_712_910_000 - 172800,
            confirm_receipt_time: 0,
            is_waybill_received: false,
            waybill_received_time: 0,
            sale_param: "默认规格".into(),
        }
    }

    #[test]
    fn resolve_reference_time_prefers_confirm_receipt() {
        let mut order = candidate();
        order.confirm_receipt_time = 100;
        order.is_waybill_received = true;
        order.waybill_received_time = 80;
        assert_eq!(resolve_reference_time(&order), 100);
    }

    #[test]
    fn resolve_reference_time_falls_back_to_waybill() {
        let mut order = candidate();
        order.is_waybill_received = true;
        order.waybill_received_time = 80;
        assert_eq!(resolve_reference_time(&order), 80);
    }

    #[test]
    fn score_candidate_order_builds_reasons_and_score() {
        let scored = score_candidate_order(&candidate(), &context()).expect("scored");
        assert_eq!(scored.score, 95);
        assert!(scored.reasons[0].contains("昵称相似度较高，疑似改名！"));
        assert_eq!(scored.reasons[1], "商品标题/商品ID/SKU 完全匹配");
        assert_eq!(scored.time_diff, 172800);
        assert_eq!(scored.confirm_diff, i64::MAX);
    }

    #[test]
    fn score_candidate_order_rejects_future_order_creation_time() {
        let mut future_order = candidate();
        future_order.create_time = context().eval_time + 1;
        assert!(score_candidate_order(&future_order, &context()).is_none());
    }

    #[test]
    fn score_candidate_order_uses_exact_nickname_reason_when_exact() {
        let mut exact_order = candidate();
        exact_order.buyer_nickname = "赵亮6057".into();
        let scored = score_candidate_order(&exact_order, &context()).expect("scored");
        assert_eq!(scored.reasons[0], "买家昵称完全匹配");
        assert_eq!(scored.score, 100);
    }
}
