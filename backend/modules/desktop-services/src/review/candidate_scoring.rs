use crate::order_match_scoring::{compute_match_score, MatchScoreResult};
use crate::review_matcher_helpers::{build_nickname_reason, build_product_reason};
use serde::{Deserialize, Serialize};

pub const MATCH_MIN_SCORE: i32 = 50;

/// 时间辅助评分的单位、阈值与权重。抽成常量便于单测锁死与后续业务微调。
///
/// 业务规则（用户 2026-04-18 提出）：「评价时间肯定是晚于订单时间的，正常情况下
/// 不可能刚下单就评价或者没签收就评价」。因此时间关系被纳入评分作为辅助：
/// - 已签收后评价 → 最正常，加 [`TIME_BONUS_AFTER_CONFIRM`]
/// - 有签收时间但评价早于签收 → 异常（没签收就评价），扣 [`TIME_PENALTY_BEFORE_CONFIRM`]
/// - 无签收时间但下单 <1 天就评价 → 疑似刷单，扣 [`TIME_PENALTY_TOO_EARLY`]
/// - 无签收时间但距下单 >90 天 → 时间跨度异常，扣 [`TIME_PENALTY_TOO_LATE`]
const SECONDS_PER_DAY: i64 = 86_400;
const TIME_BONUS_AFTER_CONFIRM: i32 = 5;
const TIME_PENALTY_BEFORE_CONFIRM: i32 = -10;
const TIME_PENALTY_TOO_EARLY: i32 = -5;
const TIME_PENALTY_TOO_LATE: i32 = -3;
const EARLY_EVAL_THRESHOLD_DAYS: i64 = 1;
const LATE_EVAL_THRESHOLD_DAYS: i64 = 90;

/// 根据评价时间与订单时间的关系返回辅助评分增量。
///
/// 纯函数，不依赖任何类型，便于单测逐分支覆盖。返回范围
/// `[TIME_PENALTY_BEFORE_CONFIRM, TIME_BONUS_AFTER_CONFIRM]`，即
/// [-10, +5]，对原始评分做有界微调。
///
/// 注意：本函数不做"评价早于下单"过滤（那是 [`score_candidate_order`] 的
/// 一票否决职责，与原版 Python `_score_candidate_order` 行为保持一致）。
pub fn compute_time_auxiliary_bonus(
    eval_time: i64,
    create_time: i64,
    reference_time: i64,
) -> i32 {
    if eval_time <= 0 || create_time <= 0 {
        return 0;
    }
    if reference_time > 0 {
        return if eval_time >= reference_time {
            TIME_BONUS_AFTER_CONFIRM
        } else {
            TIME_PENALTY_BEFORE_CONFIRM
        };
    }
    let days_since_create = (eval_time - create_time) / SECONDS_PER_DAY;
    if days_since_create < EARLY_EVAL_THRESHOLD_DAYS {
        return TIME_PENALTY_TOO_EARLY;
    }
    if days_since_create > LATE_EVAL_THRESHOLD_DAYS {
        return TIME_PENALTY_TOO_LATE;
    }
    0
}

/// 生成时间辅助评分的业务原因文案，供 UI 展示。
fn build_time_reason(bonus: i32, has_reference_time: bool) -> Option<String> {
    if bonus == 0 {
        return None;
    }
    let text = match (bonus, has_reference_time) {
        (b, true) if b > 0 => format!("评价时间晚于签收，时间线合理 (加 {} 分)", b),
        (b, true) => format!("评价时间早于签收，异常 (扣 {} 分)", b.abs()),
        (b, false) if b < 0 && b == TIME_PENALTY_TOO_EARLY => {
            format!("下单不足 1 天即评价，疑似异常 (扣 {} 分)", b.abs())
        }
        (b, false) if b < 0 && b == TIME_PENALTY_TOO_LATE => {
            format!("评价距下单超过 90 天，时间跨度异常 (扣 {} 分)", b.abs())
        }
        (b, _) => format!("时间辅助调整 ({} 分)", b),
    };
    Some(text)
}

/// 根据原始基分应用时间加权的分数上限。
///
/// - 基分 100（昵称+商品双精确，Python 原版 ExactMatch 语义）：保留 100，不被 +5 虚假抬高也不被 -10 压低。
/// - 基分 <100：clamp 到 `[MATCH_MIN_SCORE, 99]`，避免辅助加分跨越进入 ExactMatch。
fn apply_time_bonus(base_score: i32, bonus: i32) -> i32 {
    if base_score >= 100 {
        return 100;
    }
    (base_score + bonus).clamp(MATCH_MIN_SCORE, 99)
}

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
    let base_score = match_result.score;

    // 维度①②（昵称 / 商品）由 compute_match_score 承担，对齐 Python
    // 原版。维度③时间辅助在这里叠加：不改变原版三分支计算，只对最终分
    // 做 [-10, +5] 的微调，并 clamp 到 [50, 99]，保留 100 = "昵称+商品
    // 双精确" 的权威 ExactMatch 语义。
    let time_bonus = compute_time_auxiliary_bonus(
        evaluation_context.eval_time,
        order.create_time,
        reference_time,
    );
    let score = apply_time_bonus(base_score, time_bonus);

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

    if let Some(reason) = build_time_reason(time_bonus, reference_time > 0) {
        reasons.push(reason);
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

    #[test]
    fn time_bonus_rewards_eval_after_confirm_receipt() {
        // 评价时间 = 签收时间后 1 小时，应触发 +5。
        let reference_time = 1_712_910_000 - 3600;
        let bonus =
            compute_time_auxiliary_bonus(1_712_910_000, 1_712_910_000 - 172800, reference_time);
        assert_eq!(bonus, TIME_BONUS_AFTER_CONFIRM);
    }

    #[test]
    fn time_bonus_penalizes_eval_before_confirm_receipt() {
        // 评价时间 = 签收时间前 1 小时（有签收时间但评价更早），异常 → -10。
        let reference_time = 1_712_910_000 + 3600;
        let bonus =
            compute_time_auxiliary_bonus(1_712_910_000, 1_712_910_000 - 172800, reference_time);
        assert_eq!(bonus, TIME_PENALTY_BEFORE_CONFIRM);
    }

    #[test]
    fn time_bonus_penalizes_eval_within_one_day_of_create_without_confirm() {
        // 无签收时间 + 下单不足 1 天评价 → -5。
        let bonus = compute_time_auxiliary_bonus(1_712_910_000, 1_712_910_000 - 3600, 0);
        assert_eq!(bonus, TIME_PENALTY_TOO_EARLY);
    }

    #[test]
    fn time_bonus_penalizes_eval_far_beyond_create_without_confirm() {
        // 无签收时间 + 距下单 100 天评价 → -3。
        let bonus = compute_time_auxiliary_bonus(
            1_712_910_000,
            1_712_910_000 - 100 * 86_400,
            0,
        );
        assert_eq!(bonus, TIME_PENALTY_TOO_LATE);
    }

    #[test]
    fn time_bonus_returns_zero_when_time_fields_missing() {
        assert_eq!(compute_time_auxiliary_bonus(0, 1_712_910_000, 0), 0);
        assert_eq!(compute_time_auxiliary_bonus(1_712_910_000, 0, 0), 0);
    }

    #[test]
    fn apply_time_bonus_preserves_perfect_score() {
        // 100 分双精确不被 +5 或 -10 扰动，保留 ExactMatch 权威性。
        assert_eq!(apply_time_bonus(100, 5), 100);
        assert_eq!(apply_time_bonus(100, -10), 100);
    }

    #[test]
    fn apply_time_bonus_caps_sub_perfect_score_at_99() {
        // 基分 99 + 5 应被卡在 99，不虚假晋升 ExactMatch。
        assert_eq!(apply_time_bonus(99, 5), 99);
        assert_eq!(apply_time_bonus(95, 5), 99);
    }

    #[test]
    fn apply_time_bonus_floors_at_min_match_score() {
        // 基分 50 - 10 应触底 50，不跌破 ProbableMatch 门槛。
        assert_eq!(apply_time_bonus(50, -10), MATCH_MIN_SCORE);
        assert_eq!(apply_time_bonus(55, -10), MATCH_MIN_SCORE);
    }

    #[test]
    fn score_candidate_order_adds_time_reason_when_bonus_applied() {
        // 昵称相似度较高（95 分）+ 已签收后评价 → +5 → 99 分，附带时间原因。
        let mut order = candidate();
        order.confirm_receipt_time = context().eval_time - 3600;
        let scored = score_candidate_order(&order, &context()).expect("scored");
        assert_eq!(scored.score, 99);
        assert!(scored.reasons.iter().any(|r| r.contains("评价时间晚于签收")));
    }

    #[test]
    fn score_candidate_order_penalizes_eval_before_confirm_in_final_score() {
        // 昵称完全匹配 + 未签收就评价 → 100 保持 100（双精确优先），
        // 但昵称相似非完全匹配（基分 95）+ 异常时间 → 95 - 10 = 85。
        let mut order = candidate();
        order.confirm_receipt_time = context().eval_time + 3600;
        let scored = score_candidate_order(&order, &context()).expect("scored");
        assert_eq!(scored.score, 85);
        assert!(scored
            .reasons
            .iter()
            .any(|r| r.contains("评价时间早于签收")));
    }
}
