//! 昵称优先匹配主路径。
//!
//! # 设计理由
//!
//! 早期候选收集按 `product_id+sku_id` 建索引：先拉出同 SKU 的所有订单，
//! 再靠昵称相似度评分排序挑出真匹配。这套设计在长尾商品（单 SKU 订单数少）
//! 场景没问题，但一旦遇到爆品（单 SKU 数千甚至上万订单）就暴露出两个硬伤：
//!
//! - 候选集爆炸：同 SKU 6000+ 订单全部进入评分，真买家被淹没在噪声里，
//!   任何中间环节（索引构建、时间过滤、哈希冲突等）出一点问题就让真订单丢失。
//! - 业务语义倒置：用户从评价定位订单时，「买家是谁」是最强的唯一键，
//!   SKU 只是辅助确认；算法却反过来把 SKU 当主键，让昵称相似度做排序。
//!
//! 本模块按用户（2026-04-18）指出的正确业务链路实现主路径：
//!
//! ```text
//! ① 买家昵称精确命中           买家是谁 → 订单极少
//! ② 过滤 product_id + sku_id    同一商品 + 同一规格 → 基本唯一
//! ③ 过滤评价时间 ≥ 下单时间      评价必然晚于下单
//! ④ 选距评价时间最近的那条      多次回购时选最近一次
//! ```
//!
//! 命中主路径直接给 `score=100 / strategy=ExactMatch`。主路径未命中
//! （匿名昵称/改名买家/爆品同名撞车等）回退到原有 `match_single_evaluation`
//! 的 SKU 优先模糊匹配，保留 Python 原版全部行为做兜底。

use std::collections::HashMap;

use crate::matching::nickname::is_generic_nickname;
use crate::review_candidate_scoring::{CandidateOrder, EvaluationMatchContext};
use crate::review_match_flow::{MatchStrategy, SingleEvaluationMatch};

/// 按 `buyer_nickname` 分组的订单索引。同名买家的多条订单保留在同一个桶里，
/// 由主路径的 SKU + 时间两步过滤挑出真匹配。
pub type NicknameIndex = HashMap<String, Vec<CandidateOrder>>;

/// 从候选订单列表构建昵称索引。
///
/// 过滤规则（与原版 Python `is_generic_nickname` 对齐）：
/// - `trim` 后为空 → 不入索引（没有匹配信号）
/// - 命中 `is_generic_nickname`（"微信用户" / "匿名" / "默认昵称"）→ 不入索引
///   （通用占位昵称会造成误匹配，必须走兜底模糊路径处理）
pub fn build_nickname_index(orders: &[CandidateOrder]) -> NicknameIndex {
    let mut index: NicknameIndex = HashMap::new();
    for order in orders {
        let key = order.buyer_nickname.trim();
        if key.is_empty() || is_generic_nickname(key) {
            continue;
        }
        index
            .entry(key.to_string())
            .or_default()
            .push(order.clone());
    }
    index
}

/// 主路径匹配：昵称精确 → SKU 精确 → 时间对齐。
///
/// 返回 `Some(SingleEvaluationMatch)` 时表示主路径命中：可直接给
/// `score=100` 和 `MatchStrategy::ExactMatch`，绕过模糊评分。
///
/// 返回 `None` 时让位给兜底的 SKU 优先模糊匹配，调用方（`batch_match`）负责回退。
/// 返回 `None` 的四个触发点：
/// 1. 评价昵称为空或是通用占位（匿名/微信用户）
/// 2. 索引里没有匹配的 `buyer_nickname`
/// 3. 昵称命中但 `product_id + sku_id` 不一致（评价和命中订单不是同一商品）
/// 4. 昵称 + SKU 都命中，但所有候选订单都晚于评价时间（"评价早于下单"是反常）
pub fn try_nickname_first_match(
    nickname_index: &NicknameIndex,
    context: &EvaluationMatchContext,
) -> Option<SingleEvaluationMatch> {
    let eval_nickname = context.buyer_nickname.trim();
    if eval_nickname.is_empty() || is_generic_nickname(eval_nickname) {
        return None;
    }

    let by_nickname = nickname_index.get(eval_nickname)?;
    if by_nickname.is_empty() {
        return None;
    }

    // Step ②：SKU 精确过滤
    let sku_matched: Vec<&CandidateOrder> = by_nickname
        .iter()
        .filter(|order| {
            !context.product_id.is_empty()
                && order.product_id == context.product_id
                && !context.sku_id.is_empty()
                && order.sku_id == context.sku_id
        })
        .collect();
    if sku_matched.is_empty() {
        return None;
    }

    // Step ③：时间过滤（评价必然晚于下单）
    // 缺时间字段（create_time 或 eval_time 为 0）时不丢候选，视为"可能通过"
    // ——此时仍然允许主路径命中，毕竟昵称+SKU 已经极其强的唯一性。
    let time_ok: Vec<&CandidateOrder> = sku_matched
        .iter()
        .copied()
        .filter(|order| {
            if context.eval_time <= 0 || order.create_time <= 0 {
                true
            } else {
                order.create_time <= context.eval_time
            }
        })
        .collect();
    if time_ok.is_empty() {
        return None;
    }

    // Step ④：挑 create_time 距 eval_time 最近的那条；同下单时间再比 confirm_receipt_time
    let best = time_ok.iter().copied().min_by(|a, b| {
        let ta = eval_to_order_gap(context.eval_time, a.create_time);
        let tb = eval_to_order_gap(context.eval_time, b.create_time);
        ta.cmp(&tb).then_with(|| {
            let ca = eval_to_order_gap(context.eval_time, a.confirm_receipt_time);
            let cb = eval_to_order_gap(context.eval_time, b.confirm_receipt_time);
            ca.cmp(&cb)
        })
    })?;

    let candidate_count = sku_matched.len();
    let mut reasons = vec![
        "买家昵称完全匹配".to_string(),
        "商品标题/商品ID/SKU 完全匹配".to_string(),
    ];
    if context.eval_time > 0 && best.create_time > 0 {
        let hours = (context.eval_time - best.create_time) as f64 / 3600.0;
        reasons.push(format!("主路径命中：评价在下单后 {:.1} 小时发出", hours));
    } else {
        reasons.push("主路径命中：昵称+SKU 精确".to_string());
    }

    Some(SingleEvaluationMatch {
        matched_order: Some(best.clone()),
        match_strategy: MatchStrategy::ExactMatch,
        match_score: 100,
        match_reasons: reasons,
        candidate_count,
        top_score: 100,
    })
}

/// 计算"评价时间距订单时间"的非负秒差，用于主路径挑最近一次回购。
///
/// 任一方时间为 0 视为 `i64::MAX`（排最后）；时间合法但订单晚于评价视为
/// 不合理（在 time_ok 过滤里已排除，这里只作兜底）。
fn eval_to_order_gap(eval_time: i64, order_time: i64) -> i64 {
    if eval_time > 0 && order_time > 0 {
        (eval_time - order_time).max(0)
    } else {
        i64::MAX
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const PRODUCT_ID: &str = "10000496403296";
    const SKU_ID: &str = "7982968968";
    const OTHER_SKU: &str = "7983537742";
    const TITLE: &str = "仁和二硫化硒去屑洗发水止痒除螨控油清爽蓬松柔顺头屑清洁水润男女";
    const EVAL_TIME: i64 = 1_776_410_556;

    fn order(
        order_id: &str,
        buyer: &str,
        product_id: &str,
        sku_id: &str,
        create_time: i64,
    ) -> CandidateOrder {
        CandidateOrder {
            order_id: order_id.into(),
            buyer_nickname: buyer.into(),
            product_id: product_id.into(),
            sku_id: sku_id.into(),
            product_name: TITLE.into(),
            create_time,
            confirm_receipt_time: create_time + 2 * 86_400,
            is_waybill_received: false,
            waybill_received_time: 0,
            sale_param: "单瓶（体验装）400*1瓶".into(),
        }
    }

    fn eval_context(buyer: &str, product_id: &str, sku_id: &str) -> EvaluationMatchContext {
        EvaluationMatchContext {
            buyer_nickname: buyer.into(),
            product_id: product_id.into(),
            sku_id: sku_id.into(),
            product_name: TITLE.into(),
            eval_time: EVAL_TIME,
        }
    }

    #[test]
    fn build_index_skips_empty_and_generic_nicknames() {
        let orders = vec![
            order("o1", "梦云", PRODUCT_ID, SKU_ID, EVAL_TIME - 86_400),
            order("o2", "", PRODUCT_ID, SKU_ID, EVAL_TIME - 86_400),
            order("o3", "微信用户1234", PRODUCT_ID, SKU_ID, EVAL_TIME - 86_400),
            order("o4", "  梦云  ", PRODUCT_ID, SKU_ID, EVAL_TIME - 7_200),
            order("o5", "匿名", PRODUCT_ID, SKU_ID, EVAL_TIME - 86_400),
        ];
        let index = build_nickname_index(&orders);
        // 只有非空非通用昵称入索引；带空格的被 trim 后合并到同一 key
        assert_eq!(index.len(), 1);
        assert_eq!(index.get("梦云").map(Vec::len), Some(2));
        assert!(!index.contains_key(""));
        assert!(!index.contains_key("微信用户1234"));
        assert!(!index.contains_key("匿名"));
    }

    #[test]
    fn primary_hits_when_nickname_sku_and_time_all_match() {
        let orders = vec![
            order("o-other", "路人甲", PRODUCT_ID, SKU_ID, EVAL_TIME - 86_400),
            order("o-mengyun", "梦云", PRODUCT_ID, SKU_ID, EVAL_TIME - 86_400),
        ];
        let index = build_nickname_index(&orders);
        let result = try_nickname_first_match(&index, &eval_context("梦云", PRODUCT_ID, SKU_ID))
            .expect("primary path should hit");
        assert_eq!(result.match_score, 100);
        assert_eq!(result.match_strategy, MatchStrategy::ExactMatch);
        assert_eq!(result.matched_order.as_ref().unwrap().order_id, "o-mengyun");
        assert_eq!(result.candidate_count, 1);
        assert!(result
            .match_reasons
            .iter()
            .any(|r| r.contains("主路径命中")));
    }

    #[test]
    fn primary_misses_when_nickname_not_in_index() {
        let orders = vec![order(
            "o1",
            "路人甲",
            PRODUCT_ID,
            SKU_ID,
            EVAL_TIME - 86_400,
        )];
        let index = build_nickname_index(&orders);
        let result = try_nickname_first_match(&index, &eval_context("梦云", PRODUCT_ID, SKU_ID));
        assert!(result.is_none(), "昵称不在索引应让位给兜底");
    }

    #[test]
    fn primary_misses_when_nickname_hits_but_sku_mismatch() {
        // 同昵称但不同 SKU（梦云同时买过另一规格却针对这次规格评价）
        let orders = vec![order(
            "o1",
            "梦云",
            PRODUCT_ID,
            OTHER_SKU,
            EVAL_TIME - 86_400,
        )];
        let index = build_nickname_index(&orders);
        let result = try_nickname_first_match(&index, &eval_context("梦云", PRODUCT_ID, SKU_ID));
        assert!(
            result.is_none(),
            "SKU 不等应让位给兜底（兜底能匹配到相似 SKU）"
        );
    }

    #[test]
    fn primary_misses_when_all_orders_after_eval_time() {
        // 只有一条梦云订单，create_time 晚于评价时间（反常）
        let orders = vec![order("o1", "梦云", PRODUCT_ID, SKU_ID, EVAL_TIME + 3_600)];
        let index = build_nickname_index(&orders);
        let result = try_nickname_first_match(&index, &eval_context("梦云", PRODUCT_ID, SKU_ID));
        assert!(result.is_none(), "评价早于下单应让位给兜底去排查");
    }

    #[test]
    fn primary_picks_closest_time_among_repeat_buys() {
        // 梦云回购三次同 SKU：一次很早、一次最近、一次介于中间
        let orders = vec![
            order("o-old", "梦云", PRODUCT_ID, SKU_ID, EVAL_TIME - 10 * 86_400),
            order("o-mid", "梦云", PRODUCT_ID, SKU_ID, EVAL_TIME - 5 * 86_400),
            order("o-recent", "梦云", PRODUCT_ID, SKU_ID, EVAL_TIME - 86_400),
        ];
        let index = build_nickname_index(&orders);
        let result = try_nickname_first_match(&index, &eval_context("梦云", PRODUCT_ID, SKU_ID))
            .expect("primary hit");
        assert_eq!(
            result.matched_order.as_ref().unwrap().order_id,
            "o-recent",
            "应挑距评价时间最近的一次下单"
        );
        assert_eq!(result.candidate_count, 3, "同昵称同 SKU 全部计入候选数");
    }

    #[test]
    fn primary_rejects_generic_eval_nickname() {
        let orders = vec![order("o1", "梦云", PRODUCT_ID, SKU_ID, EVAL_TIME - 86_400)];
        let index = build_nickname_index(&orders);
        // 评价侧是"微信用户"类通用占位 → 主路径不处理
        let result =
            try_nickname_first_match(&index, &eval_context("微信用户abc", PRODUCT_ID, SKU_ID));
        assert!(result.is_none(), "评价昵称是通用占位应让位给兜底");
    }

    #[test]
    fn primary_keeps_candidate_when_time_fields_missing() {
        // create_time=0 的订单（字段缺失）不应因为时间过滤被丢弃，
        // 主路径不比评分模块严格，保留这种 case 让用户看到
        let mut o = order("o1", "梦云", PRODUCT_ID, SKU_ID, 0);
        o.create_time = 0;
        let index = build_nickname_index(&[o]);
        let result = try_nickname_first_match(&index, &eval_context("梦云", PRODUCT_ID, SKU_ID))
            .expect("time 缺失时主路径仍该命中");
        assert_eq!(result.match_score, 100);
    }

    #[test]
    fn primary_reason_includes_hours_after_order() {
        let orders = vec![order(
            "o1",
            "梦云",
            PRODUCT_ID,
            SKU_ID,
            EVAL_TIME - 2 * 3_600,
        )];
        let index = build_nickname_index(&orders);
        let result = try_nickname_first_match(&index, &eval_context("梦云", PRODUCT_ID, SKU_ID))
            .expect("primary hit");
        // reasons 里必须有"评价在下单后 X 小时"方便 UI 展示
        let has_hours = result
            .match_reasons
            .iter()
            .any(|r| r.contains("评价在下单后") && r.contains("小时"));
        assert!(has_hours, "时间说明文案未出现：{:?}", result.match_reasons);
    }
}
