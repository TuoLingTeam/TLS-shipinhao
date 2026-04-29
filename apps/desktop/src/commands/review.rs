use tauri::{AppHandle, State};

use crate::adapters::order::parse_iso_window;
use crate::adapters::order::HttpOrderCacheFinder;
use crate::adapters::quality_refund::HttpQualityRefundSource;
use crate::adapters::review::HttpReviewSource;
use crate::commands::license::{authorize_runtime_task, ensure_feature_authorized};
use crate::commands::order::{
    emit_order_sync_progress, mask_order_cache_error, recent_order_cache_status,
};
use crate::commands::shared::{require_cookie_credentials, require_store_runtime_context};
use crate::error::AppError;
use crate::state::AppState;
use api_contracts::{LICENSE_TASK_QUALITY_REFUND, LICENSE_TASK_REVIEW_FIND};
use desktop_services::order_cache_repository::{CacheOrderRecord, OrderCacheRepository};
use desktop_services::order_cache_storage::SqliteOrderCacheRepository;
use desktop_services::order_sync_service::OrderSyncService;
use desktop_services::review_batch_match::{match_orders_with_evaluations, EvaluationRecord};
use desktop_services::review_candidate_scoring::CandidateOrder;
use desktop_services::review_match_flow::{
    is_evaluation_replyable, reply_deadline, MatchStrategy as ServiceMatchStrategy,
};
use desktop_services::ReviewQuery;
use domain_core::{MatchSource, MatchStrategy as ApiMatchStrategy, OrderMatchResult, TimeWindow};
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};
use std::sync::Arc;

fn parse_iso_timestamp(value: &str) -> Option<i64> {
    chrono::DateTime::parse_from_rfc3339(value.trim())
        .map(|dt| dt.timestamp())
        .ok()
}

fn candidate_window_from_recent_cache(
    review_start_unix: i64,
    review_end_unix: i64,
    cache_start_unix: i64,
    cache_end_unix: i64,
) -> (i64, i64) {
    let candidate_start = if review_start_unix >= cache_start_unix {
        cache_start_unix
    } else {
        review_start_unix
    };
    let candidate_end = review_end_unix.min(cache_end_unix);
    (candidate_start, candidate_end)
}

#[derive(Debug, Serialize, Deserialize, Clone, Default)]
#[serde(rename_all = "snake_case")]
pub struct ReviewMatchResponse {
    pub results: Vec<OrderMatchResult>,
    pub cache_warnings: Vec<String>,
    pub cache_coverage_start: Option<String>,
    pub cache_coverage_end: Option<String>,
    pub cache_sync_performed: bool,
    pub cache_sync_written_count: usize,
}

fn map_match_source(strategy: Option<ServiceMatchStrategy>) -> MatchSource {
    match strategy.unwrap_or_default() {
        ServiceMatchStrategy::ExactMatch | ServiceMatchStrategy::HighConfidence => {
            MatchSource::ExactOrderId
        }
        ServiceMatchStrategy::ProbableMatch => MatchSource::ReceiverAndTimeWindow,
        ServiceMatchStrategy::Fallback | ServiceMatchStrategy::None => MatchSource::ManualFallback,
    }
}

fn map_match_strategy(strategy: Option<ServiceMatchStrategy>) -> ApiMatchStrategy {
    match strategy.unwrap_or(ServiceMatchStrategy::Fallback) {
        ServiceMatchStrategy::ExactMatch => ApiMatchStrategy::ExactMatch,
        ServiceMatchStrategy::HighConfidence => ApiMatchStrategy::HighConfidence,
        ServiceMatchStrategy::ProbableMatch => ApiMatchStrategy::ProbableMatch,
        ServiceMatchStrategy::Fallback | ServiceMatchStrategy::None => ApiMatchStrategy::Fallback,
    }
}

fn map_reply_state(can_reply_expire_time: i64) -> (bool, Option<String>) {
    (
        is_evaluation_replyable(can_reply_expire_time, chrono::Utc::now().timestamp()),
        reply_deadline(can_reply_expire_time).map(|dt| dt.to_rfc3339()),
    )
}

fn cache_record_to_candidates(record: &CacheOrderRecord) -> Vec<CandidateOrder> {
    record
        .products
        .iter()
        .map(|product| CandidateOrder {
            order_id: record.order_id.clone(),
            buyer_nickname: if record.normalized_nickname.trim().is_empty() {
                record.buyer_nickname.clone()
            } else {
                record.normalized_nickname.clone()
            },
            product_id: product.product_id.clone(),
            sku_id: product.sku_id.clone(),
            product_name: product.product_name.clone(),
            create_time: record.create_time,
            confirm_receipt_time: record.confirm_receipt_time,
            is_waybill_received: record.is_waybill_received,
            waybill_received_time: record.waybill_received_time,
            sale_param: product.sale_param.clone(),
        })
        .collect()
}

fn match_reviews_with_cache_records(
    evaluations: &[EvaluationRecord],
    orders: &[CacheOrderRecord],
) -> Vec<OrderMatchResult> {
    let candidates = orders
        .iter()
        .flat_map(cache_record_to_candidates)
        .collect::<Vec<_>>();

    match_orders_with_evaluations(evaluations, &candidates)
        .into_iter()
        .map(|matched| {
            let (replyable, reply_deadline) = map_reply_state(matched.can_reply_expire_time);
            OrderMatchResult {
                evaluation_id: matched.evaluation_id,
                order_id: matched.order_id.unwrap_or_default(),
                buyer_nickname: matched.buyer_nickname,
                evaluation_content: if matched.evaluation_content.trim().is_empty() {
                    matched.default_content
                } else {
                    matched.evaluation_content
                },
                product_id: matched.product_id,
                sku_id: matched.sku_id,
                sku_name: if matched.sku_name.trim().is_empty() {
                    matched.sale_param
                } else {
                    matched.sku_name
                },
                product_name: matched.product_name,
                matched: matched.matched,
                source: map_match_source(matched.match_strategy),
                strategy: map_match_strategy(matched.match_strategy),
                replyable,
                reply_deadline,
                confidence_score: matched.match_score.max(0) as u32,
                quality_refund_info: None,
                match_reasons: matched.match_reasons,
                candidate_count: matched.candidate_count,
                top_score: matched.top_score,
            }
        })
        .collect()
}

struct OrderCacheResult {
    orders: Vec<CacheOrderRecord>,
    warnings: Vec<String>,
    cache_sync_performed: bool,
    sync_written_count: usize,
}

fn read_orders_from_ready_cache(
    app: &AppHandle,
    rich_order_cache_path: &Path,
    start_unix: i64,
    end_unix: i64,
    coverage_start: i64,
    coverage_end: i64,
) -> Result<(Vec<CacheOrderRecord>, Vec<String>), AppError> {
    emit_order_sync_progress(
        app,
        "review_query",
        "read_cached_orders",
        22,
        "订单缓存完整，直接读取本地订单并进入评分匹配…",
    );
    let repository = SqliteOrderCacheRepository::open(rich_order_cache_path).map_err(|error| {
        mask_order_cache_error(
            "review_query.open_repository_cached_window",
            None,
            "订单缓存读取失败，请稍后重试",
            error,
        )
    })?;
    let (candidate_start, candidate_end) =
        candidate_window_from_recent_cache(start_unix, end_unix, coverage_start, coverage_end);
    let orders = repository
        .fetch_orders_in_range(candidate_start, candidate_end)
        .map_err(|error| {
            mask_order_cache_error(
                "review_query.fetch_orders_in_range_cached_window",
                None,
                "订单缓存读取失败，请稍后重试",
                error,
            )
        })?;
    Ok((orders, Vec::new()))
}

// 参数略超 clippy 默认阈值（8/7）：每一个都是同步 + 读取合并流程里的独立上下文，
// 打包成 struct 会让调用点语义变绕且无复用价值，短期内显式抑制
#[allow(clippy::too_many_arguments)]
fn sync_and_read_orders(
    app: &AppHandle,
    cookie: String,
    magic: String,
    rich_order_cache_path: &Path,
    start_unix: i64,
    end_unix: i64,
    before_count: usize,
    had_coverage: bool,
) -> Result<OrderCacheResult, AppError> {
    emit_order_sync_progress(
        app,
        "review_query",
        "ensure_window_covered",
        18,
        "正在按业务日期范围补齐订单缓存缺口…",
    );

    let finder = HttpOrderCacheFinder::new(cookie, magic);
    let repository = SqliteOrderCacheRepository::open(rich_order_cache_path).map_err(|error| {
        mask_order_cache_error(
            "review_query.open_repository_ensure_window_covered",
            None,
            "订单缓存读取失败，请稍后重试",
            error,
        )
    })?;
    let repository: Arc<dyn OrderCacheRepository> = Arc::new(repository);
    let mut service = OrderSyncService::new(finder, repository);
    let now = chrono::Utc::now();
    let sync_now = service.sync_now_timestamp(Some(now));
    let retention_start = service.retention_start_timestamp(sync_now);

    let (orders, warnings) = if start_unix < retention_start {
        service
            .fetch_full_scan_orders(start_unix, Some(now))
            .map_err(|error| {
                mask_order_cache_error(
                    "review_query.fetch_full_scan_orders",
                    None,
                    "订单缓存同步失败，请稍后重试",
                    error,
                )
            })?
    } else {
        // 评价匹配候选以缓存保留窗口为基础；query 决定拉哪段评价。
        // 若用户选「今天」，订单候选需要扩展到今天，避免今天评价找不到同日订单。
        let target_end = end_unix.max(sync_now);
        let (_, ensure_warnings, candidate_start, candidate_end) = service
            .ensure_window_covered(retention_start, target_end, Some(now))
            .map_err(|error| {
                mask_order_cache_error(
                    "review_query.ensure_window_covered",
                    None,
                    "订单缓存同步失败，请稍后重试",
                    error,
                )
            })?;
        let repo = SqliteOrderCacheRepository::open(rich_order_cache_path).map_err(|error| {
            mask_order_cache_error(
                "review_query.open_repository_after_ensure_window",
                None,
                "订单缓存读取失败，请稍后重试",
                error,
            )
        })?;
        let (final_start, final_end) = candidate_window_from_recent_cache(
            start_unix,
            end_unix,
            candidate_start,
            candidate_end,
        );
        let orders = repo
            .fetch_orders_in_range(final_start, final_end)
            .map_err(|error| {
                mask_order_cache_error(
                    "review_query.fetch_orders_in_range_after_ensure_window",
                    None,
                    "订单缓存读取失败，请稍后重试",
                    error,
                )
            })?;
        (orders, ensure_warnings)
    };

    let status = recent_order_cache_status(rich_order_cache_path).map_err(|error| {
        mask_order_cache_error(
            "review_query.recent_order_cache_status_after_refresh",
            None,
            "订单缓存读取失败，请稍后重试",
            error,
        )
    })?;
    let sync_written_count = status.cached_order_count.saturating_sub(before_count);
    let cache_sync_performed = sync_written_count > 0 || !had_coverage;

    Ok(OrderCacheResult {
        orders,
        warnings,
        cache_sync_performed,
        sync_written_count,
    })
}

fn run_review_match_flow(
    app: AppHandle,
    cookie: String,
    magic: String,
    rich_order_cache_path: PathBuf,
    query: ReviewQuery,
) -> Result<ReviewMatchResponse, AppError> {
    let (start_unix, end_unix) =
        parse_iso_window(&query.time_window.start_at, &query.time_window.end_at)
            .map_err(|e| AppError::Message(e.to_string()))?;
    let source = HttpReviewSource::new_with_grant(
        cookie.clone(),
        magic.clone(),
        query
            .runtime_grant
            .as_ref()
            .map(|grant| grant.grant_id.clone()),
    );
    let evaluations = source
        .fetch_evaluation_records(&query)
        .map_err(AppError::Internal)?;

    let status_before = recent_order_cache_status(&rich_order_cache_path).ok();
    let cached_window_ready = status_before.as_ref().and_then(|status| {
        if !status.coverage_complete {
            return None;
        }
        let cs = status
            .coverage_start
            .as_deref()
            .and_then(parse_iso_timestamp);
        let ce = status.coverage_end.as_deref().and_then(parse_iso_timestamp);
        match (cs, ce) {
            (Some(cs), Some(ce)) if start_unix >= cs && end_unix <= ce => Some((cs, ce)),
            _ => None,
        }
    });

    let cache_result = if let Some((cs, ce)) = cached_window_ready {
        let (orders, warnings) = read_orders_from_ready_cache(
            &app,
            &rich_order_cache_path,
            start_unix,
            end_unix,
            cs,
            ce,
        )?;
        OrderCacheResult {
            orders,
            warnings,
            cache_sync_performed: false,
            sync_written_count: 0,
        }
    } else {
        let before_count = status_before
            .as_ref()
            .map(|s| s.cached_order_count)
            .unwrap_or(0);
        let had_coverage = status_before
            .as_ref()
            .map(|s| s.coverage_complete)
            .unwrap_or(false);
        sync_and_read_orders(
            &app,
            cookie,
            magic,
            &rich_order_cache_path,
            start_unix,
            end_unix,
            before_count,
            had_coverage,
        )?
    };

    emit_order_sync_progress(
        &app,
        "review_query",
        "match_reviews",
        76,
        format!(
            "订单缓存已就绪，正在对 {} 条差评执行评分匹配…",
            evaluations.len()
        ),
    );

    let results = match_reviews_with_cache_records(&evaluations, &cache_result.orders);
    let status = recent_order_cache_status(&rich_order_cache_path).map_err(|error| {
        mask_order_cache_error(
            "review_query.recent_order_cache_status_before_response",
            None,
            "订单缓存读取失败，请稍后重试",
            error,
        )
    })?;

    emit_order_sync_progress(
        &app,
        "review_query",
        "completed",
        100,
        format!("评分匹配完成，返回 {} 条差评结果。", results.len()),
    );

    Ok(ReviewMatchResponse {
        results,
        cache_warnings: cache_result.warnings,
        cache_coverage_start: status.coverage_start,
        cache_coverage_end: status.coverage_end,
        cache_sync_performed: cache_result.cache_sync_performed,
        cache_sync_written_count: cache_result.sync_written_count,
    })
}

#[tauri::command(rename_all = "snake_case")]
pub async fn find_reviews(
    app: AppHandle,
    state: State<'_, AppState>,
    days: u32,
    start_at: String,
    end_at: String,
) -> Result<ReviewMatchResponse, AppError> {
    ensure_feature_authorized(&state, "评价管理").await?;
    let grant = authorize_runtime_task(&state, LICENSE_TASK_REVIEW_FIND).await?;
    let context = require_store_runtime_context(&state).await?;
    let crate::commands::shared::StoreRuntimeContext {
        cookie,
        magic,
        data_dir: _,
        rich_order_cache_path,
    } = context;

    let query = ReviewQuery {
        days,
        time_window: TimeWindow { start_at, end_at },
        runtime_grant: Some(grant.clone()),
    };

    tokio::task::spawn_blocking(move || {
        run_review_match_flow(app, cookie, magic, rich_order_cache_path, query)
    })
    .await
    .map_err(|e| AppError::Message(e.to_string()))?
}

#[tauri::command(rename_all = "snake_case")]
pub async fn find_quality_refund_orders(
    state: State<'_, AppState>,
    days: u32,
    start_at: String,
    end_at: String,
) -> Result<ReviewMatchResponse, AppError> {
    ensure_feature_authorized(&state, "品退订单").await?;
    let grant = authorize_runtime_task(&state, LICENSE_TASK_QUALITY_REFUND).await?;
    let creds = require_cookie_credentials(&state).await?;

    let query = ReviewQuery {
        days,
        time_window: TimeWindow { start_at, end_at },
        runtime_grant: Some(grant.clone()),
    };

    let results =
        HttpQualityRefundSource::new_with_grant(creds.cookie, creds.magic, Some(grant.grant_id))
            .fetch_quality_refund_orders(&query.time_window)
            .await
            .map_err(AppError::Internal)?;

    Ok(ReviewMatchResponse {
        results,
        cache_warnings: Vec::new(),
        cache_coverage_start: None,
        cache_coverage_end: None,
        cache_sync_performed: false,
        cache_sync_written_count: 0,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use desktop_services::order_cache_repository::{CacheOrderProduct, CacheOrderRecord};
    use desktop_services::review_batch_match::EvaluationRecord;

    #[test]
    fn recent_cache_candidate_window_expands_small_review_window_to_cache_start() {
        let review_start = 1_776_259_200; // 2026-04-15 00:00:00 +08
        let review_end = 1_776_614_399; // 2026-04-18 23:59:59 +08
        let cache_start = 1_774_051_200; // 2026-03-20 10:40:00 UTC-ish; recent cache起点
        let cache_end = 1_776_614_399;

        let (candidate_start, candidate_end) =
            candidate_window_from_recent_cache(review_start, review_end, cache_start, cache_end);

        assert_eq!(candidate_start, cache_start);
        assert_eq!(candidate_end, review_end);
    }

    #[test]
    fn recent_cache_candidate_window_preserves_older_full_scan_start() {
        let review_start = 1_773_792_000;
        let review_end = 1_776_614_399;
        let cache_start = 1_774_051_200;
        let cache_end = 1_776_614_399;

        let (candidate_start, candidate_end) =
            candidate_window_from_recent_cache(review_start, review_end, cache_start, cache_end);

        assert_eq!(candidate_start, review_start);
        assert_eq!(candidate_end, review_end);
    }

    #[test]
    fn scored_matching_uses_cached_order_product_dimensions() {
        let evaluations = vec![EvaluationRecord {
            evaluation_id: "eval-1".into(),
            buyer_nickname: "无锡农膜¹³⁸⁶¹⁸²¹¹⁷⁵".into(),
            product_id: "7982968968".into(),
            sku_id: "7982968968".into(),
            sku_name: "默认规格".into(),
            product_name: "仁和二硫化硒去屑洗发水止痒除螨控油清爽蓬松柔顺头屑清洁水润男女".into(),
            eval_time: 1_712_910_000,
            attitude_name: "不够好".into(),
            evaluation_content: "没有一点效果".into(),
            default_content: String::new(),
            evaluation_star: 1,
            can_reply_expire_time: chrono::Utc::now().timestamp() + 86_400,
        }];
        let orders = vec![
            CacheOrderRecord {
                order_id: "wrong-order".into(),
                buyer_nickname: "别的买家".into(),
                normalized_nickname: "别的买家".into(),
                amount_cent: 0,
                create_time: 1_712_910_000 - 172800,
                confirm_receipt_time: 0,
                is_waybill_received: false,
                waybill_received_time: 0,
                is_education_order: false,
                order_status: 20,
                openid: String::new(),
                raw_source: "order_api".into(),
                updated_at: 0,
                products: vec![CacheOrderProduct {
                    product_id: "7982968968".into(),
                    sku_id: "7982968968".into(),
                    sale_param: "默认规格".into(),
                    product_name: "仁和二硫化硒去屑洗发水止痒除螨控油清爽蓬松柔顺头屑清洁水润男女"
                        .into(),
                    thumb_img: String::new(),
                }],
            },
            CacheOrderRecord {
                order_id: "3735563912835389952".into(),
                buyer_nickname: "无锡农膜¹³⁸⁶¹⁸²¹¹⁷⁵".into(),
                normalized_nickname: "无锡农膜¹³⁸⁶¹⁸²¹¹⁷⁵".into(),
                amount_cent: 0,
                create_time: 1_712_910_000 - 172800,
                confirm_receipt_time: 0,
                is_waybill_received: false,
                waybill_received_time: 0,
                is_education_order: false,
                order_status: 20,
                openid: String::new(),
                raw_source: "order_api".into(),
                updated_at: 0,
                products: vec![CacheOrderProduct {
                    product_id: "7982968968".into(),
                    sku_id: "7982968968".into(),
                    sale_param: "默认规格".into(),
                    product_name: "仁和二硫化硒去屑洗发水止痒除螨控油清爽蓬松柔顺头屑清洁水润男女"
                        .into(),
                    thumb_img: String::new(),
                }],
            },
        ];

        let results = match_reviews_with_cache_records(&evaluations, &orders);
        assert_eq!(results.len(), 1);
        assert!(results[0].matched);
        assert_eq!(results[0].order_id, "3735563912835389952");
        assert_eq!(results[0].source, MatchSource::ExactOrderId);
        assert_eq!(results[0].strategy, domain_core::MatchStrategy::ExactMatch);
        assert!(results[0].replyable);
        assert!(results[0].reply_deadline.is_some());
        // 主路径（nickname_index）命中：candidate_count 仅表示同昵称+SKU 的候选，
        // 不再是 Python 原版 SKU-first 下的"同商品全集"。此处只有买家"无锡农膜..."
        // 对应的 1 条订单进入主路径候选桶；"别的买家"那条进不了该桶，
        // 由兜底路径评分（本用例主路径直接命中所以兜底未触发）。
        assert_eq!(results[0].candidate_count, 1);
        assert_eq!(results[0].top_score, 100);
    }

    #[test]
    fn older_order_inside_recent_cache_still_exact_matches_recent_review() {
        let evaluations = vec![EvaluationRecord {
            evaluation_id: "55947514874".into(),
            buyer_nickname: "梦云".into(),
            product_id: "10000496403296".into(),
            sku_id: "7982968968".into(),
            sku_name: "单瓶（体验装）400*1瓶".into(),
            product_name: "仁和二硫化硒去屑洗发水止痒除螨控油清爽蓬松柔顺头屑清洁水润男女".into(),
            eval_time: 1_776_410_556,
            attitude_name: "不够好".into(),
            evaluation_content: "越洗越痒".into(),
            default_content: String::new(),
            evaluation_star: 1,
            can_reply_expire_time: chrono::Utc::now().timestamp() + 86_400,
        }];
        let orders = vec![CacheOrderRecord {
            order_id: "3735167246652299776".into(),
            buyer_nickname: "梦云".into(),
            normalized_nickname: "梦云".into(),
            amount_cent: 0,
            create_time: 1_774_142_505,
            confirm_receipt_time: 1_774_355_570,
            is_waybill_received: false,
            waybill_received_time: 0,
            is_education_order: false,
            order_status: 20,
            openid: String::new(),
            raw_source: "order_api".into(),
            updated_at: 0,
            products: vec![CacheOrderProduct {
                product_id: "10000496403296".into(),
                sku_id: "7982968968".into(),
                sale_param: "单瓶（体验装）400*1瓶".into(),
                product_name: "仁和二硫化硒去屑洗发水止痒除螨控油清爽蓬松柔顺头屑清洁水润男女"
                    .into(),
                thumb_img: String::new(),
            }],
        }];

        let results = match_reviews_with_cache_records(&evaluations, &orders);

        assert_eq!(results.len(), 1);
        assert!(results[0].matched);
        assert_eq!(results[0].order_id, "3735167246652299776");
        assert_eq!(results[0].strategy, domain_core::MatchStrategy::ExactMatch);
        assert_eq!(results[0].candidate_count, 1);
        assert_eq!(results[0].top_score, 100);
    }
}
