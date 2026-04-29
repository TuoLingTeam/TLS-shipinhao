//! `HttpOrderSearchClient` / `OrderRateLimitGate` 等 adapter 入口的回归测试。
//!
//! 历史上和 `mod.rs` 同文件，2026 年起按 A1 大文件拆分外移到本文件，
//! `super::*` 仍指向 adapter::order 模块顶层（含 #[cfg(test)] 暴露的
//! `OrderRateLimitGate` 测试专用方法）。

use super::*;
use serde_json::json;
use std::sync::atomic::{AtomicU32, Ordering};
use std::sync::Arc;

#[test]
fn recent_cache_fetch_uses_two_workers() {
    assert_eq!(cache_fetch_worker_count(), 2);
}

#[test]
fn gate_schedules_expanded_2_4_8_16_32_sequence_then_exhausts() {
    let gate = OrderRateLimitGate::default();
    let mut scheduled = Vec::<u64>::new();
    for _ in 0..ORDER_RATE_LIMIT_RETRY_COUNT {
        match gate.try_schedule_backoff() {
            BackoffSchedule::Scheduled(secs) => scheduled.push(secs),
            other => panic!("期望 Scheduled，实际 {other:?}"),
        }
        gate.force_expire_pause();
    }
    assert_eq!(scheduled, vec![2, 4, 8, 16, 32]);
    assert_eq!(gate.attempt_count(), ORDER_RATE_LIMIT_RETRY_COUNT);

    match gate.try_schedule_backoff() {
        BackoffSchedule::Exhausted => (),
        other => panic!("超过上限应 Exhausted，实际 {other:?}"),
    }
}

#[test]
fn concurrent_gate_calls_share_backoff_budget() {
    let gate = OrderRateLimitGate::default();
    match gate.try_schedule_backoff() {
        BackoffSchedule::Scheduled(secs) => assert_eq!(secs, 2),
        other => panic!("首次期望 Scheduled(2)，实际 {other:?}"),
    }
    match gate.try_schedule_backoff() {
        BackoffSchedule::Waiting(_) => (),
        other => panic!("同窗口内第二次应 Waiting，实际 {other:?}"),
    }
    assert_eq!(gate.attempt_count(), 1, "并发调用不应重复消耗 attempt 配额");
}

#[test]
fn gate_record_success_resets_state() {
    let gate = OrderRateLimitGate::default();
    let _ = gate.try_schedule_backoff();
    gate.force_expire_pause();
    let _ = gate.try_schedule_backoff();
    assert_eq!(gate.attempt_count(), 2);
    assert!(gate.total_wait_secs_snapshot() > 0);

    gate.record_success();
    assert_eq!(gate.attempt_count(), 0);
    assert_eq!(gate.total_wait_secs_snapshot(), 0);
}

#[tokio::test(start_paused = true)]
async fn order_search_retry_succeeds_after_three_rate_limits() {
    let call_count = Arc::new(AtomicU32::new(0));
    let cc = call_count.clone();
    let gate = Arc::new(OrderRateLimitGate::default());

    let result = retry_order_search_with_gate(
        move || {
            let cc = cc.clone();
            async move {
                let n = cc.fetch_add(1, Ordering::SeqCst);
                if n < 3 {
                    Ok(OrderSearchRequestOutcome::<serde_json::Value>::RetryRateLimited)
                } else {
                    Ok(OrderSearchRequestOutcome::Ready(
                        serde_json::json!({"code": 0}),
                    ))
                }
            }
        },
        Arc::clone(&gate),
    )
    .await
    .expect("should eventually succeed");

    assert_eq!(
        result.get("code").and_then(serde_json::Value::as_i64),
        Some(0)
    );
    assert_eq!(call_count.load(Ordering::SeqCst), 4);
    assert_eq!(gate.attempt_count(), 0, "成功后应归零 attempt");
    assert_eq!(gate.total_wait_secs_snapshot(), 0, "成功后应归零累计等待");
}

#[test]
fn order_json_to_entry_maps_buyer_and_string_amount() {
    let raw = json!({
        "commonInfo": {
            "orderId": "3735739244192085760",
            "createTime": 1776324243
        },
        "buyerInfo": {
            "nickName": "琼花🌸若现"
        },
        "priceInfo": {
            "orderPrice": "5990"
        }
    });

    let entry = order_json_to_entry(&raw, "2026-04-16T07:30:00Z").expect("order entry");
    assert_eq!(entry.order_id, "3735739244192085760");
    assert_eq!(entry.buyer_name, "琼花🌸若现");
    assert_eq!(entry.amount_cent, 5990);
    assert_eq!(entry.created_at, "2026-04-16T07:24:03+00:00");
}

#[test]
fn cancelled_order_json_is_hidden_from_lightweight_order_list() {
    let raw = json!({
        "commonInfo": {
            "orderId": "3736036707705178624",
            "createTime": 1777459219,
            "status": 250,
            "statusStr": "已取消"
        },
        "buyerInfo": {
            "nickName": "cancelled-buyer"
        },
        "orderStatus": {
            "cancelReason": "取消原因：买家主动取消"
        }
    });

    assert!(order_json_to_entry(&raw, "2026-04-29T10:40:00Z").is_none());
}

#[test]
fn order_json_to_cache_record_maps_products_and_receipt_fields() {
    let raw = json!({
        "commonInfo": {
            "orderId": "3735739244192085760",
            "createTime": 1776324243,
            "status": 20,
            "openid": "openid-1",
            "isEducationOrder": false
        },
        "buyerInfo": {
            "nickName": "琼花🌸若现"
        },
        "acceptInfo": {
            "confirmReceiptTime": "1776400000"
        },
        "orderStatus": {
            "autoConfirmInfo": {
                "isWaybillReceived": true,
                "waybillReceivedTime": 1776380000
            }
        },
        "orderProductInfo": [
            {
                "productId": "10000496403296",
                "skuId": "400-1",
                "saleParam": ["单瓶", "400ml"],
                "title": "仁和二硫化硒去屑洗发水",
                "thumbImg": "https://img.example.com/1.png"
            }
        ]
    });

    let record = order_json_to_cache_record(&raw, 1776329999).expect("cache record");
    assert_eq!(record.order_id, "3735739244192085760");
    assert_eq!(record.buyer_nickname, "琼花🌸若现");
    assert_eq!(record.amount_cent, 0);
    assert_eq!(record.create_time, 1776324243);
    assert_eq!(record.confirm_receipt_time, 1776400000);
    assert!(record.is_waybill_received);
    assert_eq!(record.waybill_received_time, 1776380000);
    assert_eq!(record.order_status, 20);
    assert_eq!(record.openid, "openid-1");
    assert_eq!(record.products.len(), 1);
    assert_eq!(record.products[0].product_id, "10000496403296");
    assert_eq!(record.products[0].sku_id, "400-1");
    assert_eq!(record.products[0].sale_param, "单瓶|400ml");
    assert_eq!(record.products[0].product_name, "仁和二硫化硒去屑洗发水");
}

#[test]
fn cancelled_order_json_keeps_status_marker_for_cache_deletion() {
    let raw = json!({
        "commonInfo": {
            "orderId": "3736036707705178624",
            "createTime": 1777459219,
            "status": 250,
            "statusStr": "已取消",
            "openid": "openid-cancelled"
        },
        "buyerInfo": {
            "nickName": "cancelled-buyer"
        },
        "orderStatus": {
            "cancelReason": "取消原因：买家主动取消"
        }
    });

    let record = order_json_to_cache_record(&raw, 1777460000).expect("delete marker");
    assert_eq!(record.order_id, "3736036707705178624");
    assert_eq!(record.order_status, 250);
    assert_eq!(record.buyer_nickname, "cancelled-buyer");
}
