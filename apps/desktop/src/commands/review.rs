use tauri::{AppHandle, State};

use crate::adapters::http_order_search::parse_iso_window;
use crate::adapters::http_order_search::HttpOrderCacheFinder;
use crate::adapters::http_quality_refund_source::HttpQualityRefundSource;
use crate::adapters::http_review_source::HttpReviewSource;
use crate::commands::license::ensure_feature_authorized;
use crate::commands::order::{emit_order_sync_progress, recent_order_cache_status};
use crate::error::AppError;
use crate::state::AppState;
use desktop_services::order_cache_repository::{CacheOrderRecord, OrderCacheRepository};
use desktop_services::order_cache_storage::SqliteOrderCacheRepository;
use desktop_services::order_sync_planner::ORDER_CACHE_COVERAGE_DAYS;
use desktop_services::order_sync_service::OrderSyncService;
use desktop_services::review_batch_match::{match_orders_with_evaluations, EvaluationRecord};
use desktop_services::review_candidate_scoring::CandidateOrder;
use desktop_services::review_match_flow::MatchStrategy;
use desktop_services::ReviewQuery;
use domain_core::{MatchSource, OrderMatchResult, TimeWindow};
use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use std::sync::Arc;

fn cache_data_dir() -> std::path::PathBuf {
    dirs::data_local_dir()
        .unwrap_or_else(|| std::path::PathBuf::from("."))
        .join("TLS-shipinhao")
}

fn rich_order_cache_path() -> PathBuf {
    cache_data_dir().join("order_cache.sqlite3")
}


fn parse_iso_timestamp(value: &str) -> Option<i64> {
    chrono::DateTime::parse_from_rfc3339(value.trim())
        .map(|dt| dt.timestamp())
        .ok()
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

fn map_match_source(strategy: Option<MatchStrategy>) -> MatchSource {
    match strategy.unwrap_or_default() {
        MatchStrategy::ExactMatch | MatchStrategy::HighConfidence => MatchSource::ExactOrderId,
        MatchStrategy::ProbableMatch => MatchSource::ReceiverAndTimeWindow,
        MatchStrategy::Fallback | MatchStrategy::None => MatchSource::ManualFallback,
    }
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
        .map(|matched| OrderMatchResult {
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
            confidence_score: matched.match_score.max(0) as u32,
            match_reasons: matched.match_reasons,
            candidate_count: matched.candidate_count,
            top_score: matched.top_score,
        })
        .collect()
}

fn run_review_match_flow(
    app: AppHandle,
    cookie: String,
    magic: String,
    query: ReviewQuery,
) -> Result<ReviewMatchResponse, AppError> {
    let (start_unix, end_unix) = parse_iso_window(&query.time_window.start_at, &query.time_window.end_at)
        .map_err(|e| AppError::Message(e.to_string()))?;
    let source = HttpReviewSource::new(cookie.clone(), magic.clone());
    let evaluations = source
        .fetch_evaluation_records(&query)
        .map_err(AppError::Internal)?;

    let status_before = recent_order_cache_status().ok();
    let cached_window_ready = status_before.as_ref().and_then(|status| {
        if !status.coverage_complete {
            return None;
        }
        let coverage_start = status
            .coverage_start
            .as_deref()
            .and_then(parse_iso_timestamp);
        let coverage_end = status
            .coverage_end
            .as_deref()
            .and_then(parse_iso_timestamp);
        match (coverage_start, coverage_end) {
            (Some(coverage_start), Some(coverage_end))
                if start_unix >= coverage_start && end_unix <= coverage_end =>
            {
                Some((coverage_start, coverage_end))
            }
            _ => None,
        }
    });

    let mut cache_sync_performed = false;
    let mut sync_written_count = 0usize;
    let (orders, warnings) = if let Some((_coverage_start, coverage_end)) = cached_window_ready {
        emit_order_sync_progress(
            &app,
            "review_query",
            "read_cached_orders",
            22,
            "最近 30 天缓存完整，直接读取本地订单并进入评分匹配…",
        );
        let repository = SqliteOrderCacheRepository::open(&rich_order_cache_path())
            .map_err(AppError::Internal)?;
        let orders = repository
            .fetch_orders_in_range(start_unix, end_unix.min(coverage_end))
            .map_err(AppError::Internal)?;
        (orders, Vec::new())
    } else {
        emit_order_sync_progress(
            &app,
            "review_query",
            "ensure_recent_cache",
            18,
            "正在确保最近 30 天订单缓存可用…",
        );

        let finder = HttpOrderCacheFinder::new(cookie, magic);
        let repository = SqliteOrderCacheRepository::open(&rich_order_cache_path())
            .map_err(AppError::Internal)?;
        let repository: Arc<dyn OrderCacheRepository> = Arc::new(repository);
        let mut service = OrderSyncService::new(finder, repository);
        let now = chrono::Utc::now();
        let retention_start = now - chrono::Duration::days(ORDER_CACHE_COVERAGE_DAYS);
        let earliest = start_unix;
        let (orders, warnings) = if earliest < retention_start.timestamp() {
            let (orders, warnings) = service
                .fetch_full_scan_orders(earliest, Some(now))
                .map_err(AppError::Internal)?;
            (orders, warnings)
        } else {
            let (orders, ensure_warnings) = service
                .ensure_orders(earliest, Some(now))
                .map_err(AppError::Internal)?;
            (orders, ensure_warnings)
        };

        let status = recent_order_cache_status().map_err(AppError::Internal)?;
        let before_count = status_before
            .as_ref()
            .map(|item| item.cached_order_count)
            .unwrap_or(0);
        sync_written_count = status.cached_order_count.saturating_sub(before_count);
        cache_sync_performed = sync_written_count > 0
            || status_before.is_none()
            || status_before
                .as_ref()
                .map(|item| !item.coverage_complete)
                .unwrap_or(false);
        (orders, warnings)
    };

    emit_order_sync_progress(
        &app,
        "review_query",
        "match_reviews",
        76,
        format!("订单缓存已就绪，正在对 {} 条差评执行评分匹配…", evaluations.len()),
    );

    let results = match_reviews_with_cache_records(&evaluations, &orders);
    let status = recent_order_cache_status().map_err(AppError::Internal)?;

    emit_order_sync_progress(
        &app,
        "review_query",
        "completed",
        100,
        format!("评分匹配完成，返回 {} 条差评结果。", results.len()),
    );

    Ok(ReviewMatchResponse {
        results,
        cache_warnings: warnings,
        cache_coverage_start: status.coverage_start,
        cache_coverage_end: status.coverage_end,
        cache_sync_performed,
        cache_sync_written_count: sync_written_count,
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
    let cookie_profile = state.cookie_profile.lock().await;
    if cookie_profile.cookie_header.is_empty() {
        return Err(AppError::Message("请先在设置中配置 Cookie".to_string()));
    }
    let cookie = cookie_profile.cookie_header.clone();
    let magic = cookie_profile.biz_magic.clone().unwrap_or_default();
    drop(cookie_profile);

    let query = ReviewQuery {
        days,
        time_window: TimeWindow { start_at, end_at },
        runtime_grant: None,
    };

    tokio::task::spawn_blocking(move || run_review_match_flow(app, cookie, magic, query))
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
    let cookie_profile = state.cookie_profile.lock().await;
    if cookie_profile.cookie_header.is_empty() {
        return Err(AppError::Message("请先在设置中配置 Cookie".to_string()));
    }
    let cookie = cookie_profile.cookie_header.clone();
    let magic = cookie_profile.biz_magic.clone().unwrap_or_default();
    drop(cookie_profile);

    let query = ReviewQuery {
        days,
        time_window: TimeWindow { start_at, end_at },
        runtime_grant: None,
    };

    let results = tokio::task::spawn_blocking(move || {
        let source = HttpQualityRefundSource::new(cookie, magic);
        source.fetch_quality_refund_orders(&query.time_window)
    })
    .await
    .map_err(|e| AppError::Message(e.to_string()))?
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
                receiver_name: String::new(),
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
                receiver_name: String::new(),
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
        assert_eq!(results[0].candidate_count, 2);
        assert_eq!(results[0].top_score, 100);
    }
}
