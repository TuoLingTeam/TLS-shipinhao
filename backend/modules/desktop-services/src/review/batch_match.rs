use crate::review::nickname_index::{build_nickname_index, try_nickname_first_match};
use crate::review_candidate_scoring::{CandidateOrder, EvaluationMatchContext};
use crate::review_index::{build_product_sku_index, collect_candidate_orders};
use crate::review_match_flow::{match_single_evaluation, MatchStrategy};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Default)]
#[serde(rename_all = "camelCase")]
pub struct EvaluationRecord {
    pub evaluation_id: String,
    pub buyer_nickname: String,
    pub product_id: String,
    pub sku_id: String,
    pub sku_name: String,
    pub product_name: String,
    pub eval_time: i64,
    pub attitude_name: String,
    pub evaluation_content: String,
    pub default_content: String,
    pub evaluation_star: i32,
    pub can_reply_expire_time: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct MatchedEvaluationResult {
    pub evaluation_id: String,
    pub order_id: Option<String>,
    pub product_id: String,
    pub sku_id: String,
    pub sku_name: String,
    pub sale_param: String,
    pub buyer_nickname: String,
    pub order_buyer_nickname: String,
    pub match_strategy: Option<MatchStrategy>,
    pub match_score: i32,
    pub match_reasons: Vec<String>,
    pub time_diff_hours: Option<f64>,
    pub confirm_diff_hours: Option<f64>,
    pub attitude_name: String,
    pub evaluation_content: String,
    pub default_content: String,
    pub evaluation_star: i32,
    pub product_name: String,
    pub can_reply_expire_time: i64,
    pub matched: bool,
    pub candidate_count: usize,
    pub top_score: i32,
}

pub fn match_orders_with_evaluations(
    evaluations: &[EvaluationRecord],
    orders: &[CandidateOrder],
) -> Vec<MatchedEvaluationResult> {
    // 主路径索引：按买家昵称直接定位订单（爆品场景下 O(1) 命中真匹配）。
    let nickname_index = build_nickname_index(orders);
    // 兜底路径索引：按 product_id+sku_id / 规范化 title+skuName 建立（Python 原版行为）。
    let product_index = build_product_sku_index(orders);

    evaluations
        .iter()
        .map(|evaluation| {
            let context = EvaluationMatchContext {
                buyer_nickname: evaluation.buyer_nickname.clone(),
                product_id: evaluation.product_id.clone(),
                sku_id: evaluation.sku_id.clone(),
                product_name: evaluation.product_name.clone(),
                eval_time: evaluation.eval_time,
            };

            // ① 主路径：昵称精确 → SKU 精确 → 时间对齐。命中即为 100 分 ExactMatch。
            let single = if let Some(primary) = try_nickname_first_match(&nickname_index, &context)
            {
                primary
            } else {
                // ② 兜底：SKU 优先的模糊匹配（保留 Python 原版全部行为 + 时间辅助加分）。
                let candidates =
                    collect_candidate_orders(&product_index, &context, &evaluation.sku_name);
                match_single_evaluation(&context, &candidates)
            };

            build_match_result(
                evaluation,
                single.matched_order.as_ref(),
                single.match_strategy,
                single.match_score,
                single.match_reasons,
                single.candidate_count,
                single.top_score,
            )
        })
        .collect()
}

pub fn build_match_result(
    evaluation: &EvaluationRecord,
    matched_order: Option<&CandidateOrder>,
    match_strategy: MatchStrategy,
    match_score: i32,
    match_reasons: Vec<String>,
    candidate_count: usize,
    top_score: i32,
) -> MatchedEvaluationResult {
    let reference_time = matched_order.map(resolve_reference_time).unwrap_or(0);
    MatchedEvaluationResult {
        evaluation_id: evaluation.evaluation_id.clone(),
        order_id: matched_order.map(|order| order.order_id.clone()),
        product_id: evaluation.product_id.clone(),
        sku_id: evaluation.sku_id.clone(),
        sku_name: evaluation.sku_name.clone(),
        sale_param: matched_order
            .map(|order| order.sale_param.clone())
            .unwrap_or_default(),
        buyer_nickname: evaluation.buyer_nickname.clone(),
        order_buyer_nickname: matched_order
            .map(|order| order.buyer_nickname.clone())
            .unwrap_or_default(),
        match_strategy: matched_order.map(|_| match_strategy),
        match_score: if matched_order.is_some() {
            match_score
        } else {
            0
        },
        match_reasons: if matched_order.is_some() {
            match_reasons
        } else {
            Vec::new()
        },
        time_diff_hours: matched_order.and_then(|order| {
            (evaluation.eval_time > 0 && order.create_time > 0)
                .then_some((evaluation.eval_time - order.create_time) as f64 / 3600.0)
        }),
        confirm_diff_hours: matched_order.and_then(|_| {
            (evaluation.eval_time > 0 && reference_time > 0)
                .then_some((evaluation.eval_time - reference_time) as f64 / 3600.0)
        }),
        attitude_name: evaluation.attitude_name.clone(),
        evaluation_content: evaluation.evaluation_content.clone(),
        default_content: evaluation.default_content.clone(),
        evaluation_star: evaluation.evaluation_star,
        product_name: evaluation.product_name.clone(),
        can_reply_expire_time: evaluation.can_reply_expire_time,
        matched: matched_order.is_some(),
        candidate_count,
        top_score,
    }
}

fn resolve_reference_time(order: &CandidateOrder) -> i64 {
    if order.confirm_receipt_time > 0 {
        order.confirm_receipt_time
    } else if order.is_waybill_received && order.waybill_received_time > 0 {
        order.waybill_received_time
    } else {
        0
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const PRODUCT_ID: &str = "7982968968";
    const SKU_ID: &str = "7982968968";
    const TITLE: &str = "仁和二硫化硒去屑洗发水止痒除螨控油清爽蓬松柔顺头屑清洁水润男女";

    fn evaluation() -> EvaluationRecord {
        EvaluationRecord {
            evaluation_id: "eval-1".into(),
            buyer_nickname: "无锡农膜¹³⁸⁶¹⁸²¹¹⁷⁵".into(),
            product_id: PRODUCT_ID.into(),
            sku_id: SKU_ID.into(),
            sku_name: "默认规格".into(),
            product_name: TITLE.into(),
            eval_time: 1_712_910_000,
            attitude_name: String::new(),
            evaluation_content: String::new(),
            default_content: String::new(),
            evaluation_star: 1,
            can_reply_expire_time: 0,
        }
    }

    fn order(
        order_id: &str,
        buyer: &str,
        confirm_receipt_time: i64,
        create_time: i64,
    ) -> CandidateOrder {
        CandidateOrder {
            order_id: order_id.into(),
            buyer_nickname: buyer.into(),
            product_id: PRODUCT_ID.into(),
            sku_id: SKU_ID.into(),
            product_name: TITLE.into(),
            create_time,
            confirm_receipt_time,
            is_waybill_received: false,
            waybill_received_time: 0,
            sale_param: "默认规格".into(),
        }
    }

    #[test]
    fn batch_match_returns_unmatched_when_no_candidates() {
        let results = match_orders_with_evaluations(&[evaluation()], &[]);
        assert_eq!(results.len(), 1);
        assert!(!results[0].matched);
        assert_eq!(results[0].order_id, None);
    }

    #[test]
    fn batch_match_picks_best_candidate_and_builds_result() {
        let wrong = order(
            "3735582233824220672",
            "Y",
            1_712_910_000 - 86400,
            1_712_910_000 - 172800,
        );
        let right = order(
            "3735563912835389952",
            "无锡农膜¹³⁸⁶¹⁸²¹¹⁷⁵",
            0,
            1_712_910_000 - 172800,
        );
        let results = match_orders_with_evaluations(&[evaluation()], &[wrong, right]);
        assert_eq!(results.len(), 1);
        assert!(results[0].matched);
        assert_eq!(results[0].order_id.as_deref(), Some("3735563912835389952"));
        assert_eq!(results[0].match_score, 100);
        assert_eq!(results[0].match_strategy, Some(MatchStrategy::ExactMatch));
    }

    #[test]
    fn build_match_result_calculates_hour_diffs() {
        let eval = evaluation();
        let matched = order(
            "order-1",
            "buyer",
            eval.eval_time - 3600,
            eval.eval_time - 7200,
        );
        let result = build_match_result(
            &eval,
            Some(&matched),
            MatchStrategy::ProbableMatch,
            90,
            vec!["ok".into()],
            2,
            90,
        );
        assert_eq!(result.time_diff_hours, Some(2.0));
        assert_eq!(result.confirm_diff_hours, Some(1.0));
        assert_eq!(result.candidate_count, 2);
        assert_eq!(result.top_score, 90);
    }
}
