use crate::review_candidate_scoring::{
    score_candidate_order, CandidateOrder, EvaluationMatchContext,
};
use crate::review_matcher_helpers::pick_best_match;
use chrono::{DateTime, TimeZone, Utc};
use serde::{Deserialize, Serialize};

pub const AUTO_FILL_SCORE_THRESHOLD: i32 = 100;

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, Default)]
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
    pub candidate_count: usize,
    pub top_score: i32,
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

pub fn is_evaluation_replyable(can_reply_expire_time: i64, now_ts: i64) -> bool {
    if can_reply_expire_time == 0 {
        return true;
    }
    let days_until_expire = (can_reply_expire_time - now_ts) / 86_400;
    days_until_expire >= -30
}

pub fn reply_deadline(can_reply_expire_time: i64) -> Option<DateTime<Utc>> {
    (can_reply_expire_time > 0)
        .then(|| Utc.timestamp_opt(can_reply_expire_time, 0).single())
        .flatten()
}

pub fn match_single_evaluation(
    evaluation_context: &EvaluationMatchContext,
    candidate_orders: &[CandidateOrder],
) -> SingleEvaluationMatch {
    if candidate_orders.is_empty() {
        return SingleEvaluationMatch::default();
    }

    // 同时收集评分成功和被时间过滤掉的候选，方便诊断「真买家订单明明存在
    // 却没进入候选评分」类问题（eval_time < create_time 一票否决是唯一
    // 入口过滤，必须能观测到）。
    let mut best_matches = Vec::with_capacity(candidate_orders.len());
    let mut time_filtered_out: Vec<&CandidateOrder> = Vec::new();
    for order in candidate_orders.iter() {
        match score_candidate_order(order, evaluation_context) {
            Some(scored) => best_matches.push(scored),
            None => time_filtered_out.push(order),
        }
    }

    log_short_nickname_diagnostics(
        evaluation_context,
        candidate_orders,
        &best_matches,
        &time_filtered_out,
    );

    if best_matches.is_empty() {
        return SingleEvaluationMatch {
            candidate_count: candidate_orders.len(),
            ..SingleEvaluationMatch::default()
        };
    }

    let top_score = best_matches
        .iter()
        .map(|item| item.score)
        .max()
        .unwrap_or(0);
    let candidate_count = best_matches.len();

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
        return SingleEvaluationMatch {
            candidate_count,
            top_score,
            ..SingleEvaluationMatch::default()
        };
    };

    let best_match = best_matches
        .into_iter()
        .find(|item| {
            item.score == best_key.score
                && item.confirm_diff == best_key.confirm_diff
                && item.time_diff == best_key.time_diff
        })
        .expect("best match should exist");

    if best_match.score < crate::review_candidate_scoring::MATCH_MIN_SCORE {
        return SingleEvaluationMatch {
            matched_order: None,
            match_strategy: MatchStrategy::None,
            match_score: 0,
            match_reasons: best_match.reasons,
            candidate_count,
            top_score,
        };
    }

    SingleEvaluationMatch {
        matched_order: Some(best_match.order),
        match_strategy: match_strategy_by_score(best_match.score),
        match_score: best_match.score,
        match_reasons: best_match.reasons,
        candidate_count,
        top_score,
    }
}

/// 短昵称评价的诊断日志门槛（字符数）。
///
/// 业务经验：像「梦云」「李长喜」这类 ≤4 字的昵称在小店评价里高频出现，
/// 也最容易因候选订单提取字段偏差导致"候选 325 条却没真梦云"。对这类
/// 场景做 warn 级别的定向诊断，便于运营/研发快速复盘，不会污染长昵称
/// 的正常日志。
const DIAGNOSTIC_SHORT_NICKNAME_MAX_CHARS: usize = 4;
/// 单条评价日志里最多列出多少候选订单，避免 325 条全量打印淹没终端。
const DIAGNOSTIC_MAX_SAMPLES: usize = 5;

/// 当评价买家昵称较短且未拿到 100 分精确匹配时，向 tracing 输出候选诊断。
///
/// 打印内容：
/// - 评价字段（buyer_nickname/product_id/sku_id/product_name/eval_time）
/// - 总候选数、被"评价时间早于下单时间"过滤掉的候选数
/// - 最多 5 条「昵称有重合（子串或被包含）」的未过滤候选，附 buyer/product 字段
/// - 最多 5 条被时间过滤掉且昵称重合的候选 —— 这是定位"真订单存在但未参评"
///   的黄金线索（比如用户缓存窗口外的早期订单被漂到缓存窗口后）
/// - 评分 Top-3 的候选摘要
fn log_short_nickname_diagnostics(
    evaluation_context: &EvaluationMatchContext,
    candidate_orders: &[CandidateOrder],
    best_matches: &[crate::review_candidate_scoring::ScoredCandidateOrder],
    time_filtered_out: &[&CandidateOrder],
) {
    let eval_name = evaluation_context.buyer_nickname.trim();
    if eval_name.is_empty() {
        return;
    }
    if eval_name.chars().count() > DIAGNOSTIC_SHORT_NICKNAME_MAX_CHARS {
        return;
    }
    let top_score = best_matches
        .iter()
        .map(|item| item.score)
        .max()
        .unwrap_or(0);
    // 只在"有缺口"的场景打印：要么没拿到精确匹配，要么最高基分未达 100。
    if top_score >= 100 {
        return;
    }

    let overlap_in_scored: Vec<&CandidateOrder> = best_matches
        .iter()
        .map(|scored| &scored.order)
        .filter(|order| nickname_has_overlap(eval_name, &order.buyer_nickname))
        .take(DIAGNOSTIC_MAX_SAMPLES)
        .collect();
    let overlap_in_filtered: Vec<&CandidateOrder> = time_filtered_out
        .iter()
        .copied()
        .filter(|order| nickname_has_overlap(eval_name, &order.buyer_nickname))
        .take(DIAGNOSTIC_MAX_SAMPLES)
        .collect();

    let mut top_three = best_matches
        .iter()
        .take(DIAGNOSTIC_MAX_SAMPLES)
        .collect::<Vec<_>>();
    top_three.sort_by(|a, b| b.score.cmp(&a.score));
    top_three.truncate(3);

    tracing::warn!(
        target: "review.match.diagnostic",
        "短昵称评价未拿满分 · eval={{nickname='{}', product_id='{}', sku_id='{}', product_name='{}', eval_time={}}} | 候选={} 时间过滤掉={} 最高基分={}",
        eval_name,
        evaluation_context.product_id,
        evaluation_context.sku_id,
        evaluation_context.product_name,
        evaluation_context.eval_time,
        candidate_orders.len(),
        time_filtered_out.len(),
        top_score,
    );

    for order in &overlap_in_scored {
        tracing::warn!(
            target: "review.match.diagnostic",
            "  候选·昵称重合(已参评): order_id={} buyer='{}' product_id='{}' sku_id='{}' product_name='{}' create_time={} confirm_receipt_time={}",
            order.order_id,
            order.buyer_nickname,
            order.product_id,
            order.sku_id,
            order.product_name,
            order.create_time,
            order.confirm_receipt_time,
        );
    }
    for order in &overlap_in_filtered {
        tracing::warn!(
            target: "review.match.diagnostic",
            "  候选·昵称重合(被时间过滤): order_id={} buyer='{}' product_id='{}' sku_id='{}' create_time={} eval_time={} 差={}s",
            order.order_id,
            order.buyer_nickname,
            order.product_id,
            order.sku_id,
            order.create_time,
            evaluation_context.eval_time,
            order.create_time - evaluation_context.eval_time,
        );
    }
    for scored in &top_three {
        tracing::warn!(
            target: "review.match.diagnostic",
            "  Top: score={} order_id={} buyer='{}' product_id='{}' sku_id='{}'",
            scored.score,
            scored.order.order_id,
            scored.order.buyer_nickname,
            scored.order.product_id,
            scored.order.sku_id,
        );
    }
}

/// 判断订单 buyer_nickname 是否与评价 buyer_nickname 有"明显字符重合"，
/// 用于诊断日志挑选有分析价值的样本。规则：
/// - 任一方为空 → false
/// - 任一方是另一方的子串 → true
/// - 共享至少 2 个 Unicode 字符（交集长度 ≥ 2）→ true
fn nickname_has_overlap(eval_name: &str, order_name: &str) -> bool {
    let order = order_name.trim();
    if eval_name.is_empty() || order.is_empty() {
        return false;
    }
    if order.contains(eval_name) || eval_name.contains(order) {
        return true;
    }
    let eval_chars: std::collections::HashSet<char> = eval_name.chars().collect();
    let order_chars: std::collections::HashSet<char> = order.chars().collect();
    eval_chars.intersection(&order_chars).count() >= 2
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

    fn order(
        order_id: &str,
        buyer: &str,
        confirm_time: i64,
        status_variant: i64,
    ) -> CandidateOrder {
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
        assert_eq!(
            result.matched_order.unwrap().order_id,
            "3735563912835389952"
        );
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

    #[test]
    fn maps_thresholds_to_expected_strategies() {
        assert_eq!(match_strategy_by_score(100), MatchStrategy::ExactMatch);
        assert_eq!(match_strategy_by_score(80), MatchStrategy::ProbableMatch);
        assert_eq!(match_strategy_by_score(40), MatchStrategy::Fallback);
    }

    #[test]
    fn reply_window_keeps_30_day_grace_period_and_missing_values() {
        let now = 1_776_324_243;
        assert!(is_evaluation_replyable(0, now));
        assert!(is_evaluation_replyable(now - 15 * 86_400, now));
        assert!(!is_evaluation_replyable(now - 45 * 86_400, now));
        assert!(reply_deadline(now + 86_400).is_some());
        assert!(reply_deadline(0).is_none());
    }
}
