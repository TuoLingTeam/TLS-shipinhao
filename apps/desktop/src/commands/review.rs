use tauri::State;

use crate::adapters::http_quality_refund_source::HttpQualityRefundSource;
use crate::adapters::http_review_source::HttpReviewSource;
use crate::adapters::sqlite_order_cache::SqliteOrderCache;
use crate::commands::license::ensure_feature_authorized;
use crate::error::AppError;
use crate::state::AppState;
use desktop_services::OrderCacheStore;
use desktop_services::ReviewQuery;
use desktop_services::ReviewSource;
use domain_core::{MatchSource, OrderMatchResult, TimeWindow};
use std::collections::HashSet;

fn cache_data_dir() -> std::path::PathBuf {
    dirs::data_local_dir()
        .unwrap_or_else(|| std::path::PathBuf::from("."))
        .join("TLS-shipinhao")
}

fn apply_cache_order_match(
    mut results: Vec<OrderMatchResult>,
    window: &TimeWindow,
) -> anyhow::Result<Vec<OrderMatchResult>> {
    let cache = SqliteOrderCache::new(cache_data_dir());
    let cache_ids: HashSet<String> = cache
        .load_recent_orders(window)?
        .into_iter()
        .map(|item| item.order_id)
        .collect();
    apply_cache_order_match_with_ids(&mut results, &cache_ids);
    Ok(results)
}

fn apply_cache_order_match_with_ids(results: &mut [OrderMatchResult], cache_ids: &HashSet<String>) {
    for item in results.iter_mut() {
        let order_id = item.order_id.trim();
        if !order_id.is_empty() && cache_ids.contains(order_id) {
            item.matched = true;
            item.source = MatchSource::ExactOrderId;
            item.confidence_score = 100;
        } else {
            item.matched = false;
            item.source = MatchSource::ManualFallback;
            item.confidence_score = 0;
        }
    }
}

#[tauri::command(rename_all = "snake_case")]
pub async fn find_reviews(
    state: State<'_, AppState>,
    days: u32,
    start_at: String,
    end_at: String,
) -> Result<Vec<OrderMatchResult>, AppError> {
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

    tokio::task::spawn_blocking(move || {
        let source = HttpReviewSource::new(cookie, magic);
        let results = source.fetch_reviews(&query)?;
        apply_cache_order_match(results, &query.time_window)
    })
    .await
    .map_err(|e| AppError::Message(e.to_string()))?
    .map_err(AppError::Internal)
}

#[tauri::command(rename_all = "snake_case")]
pub async fn find_quality_refund_orders(
    state: State<'_, AppState>,
    days: u32,
    start_at: String,
    end_at: String,
) -> Result<Vec<OrderMatchResult>, AppError> {
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

    tokio::task::spawn_blocking(move || {
        let source = HttpQualityRefundSource::new(cookie, magic);
        source.fetch_quality_refund_orders(&query.time_window)
    })
    .await
    .map_err(|e| AppError::Message(e.to_string()))?
    .map_err(AppError::Internal)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn cache_match_only_keeps_cached_order_ids_as_matched() {
        let mut results = vec![
            OrderMatchResult {
                evaluation_id: "a".into(),
                order_id: "cached-order".into(),
                matched: true,
                source: MatchSource::ExactOrderId,
                confidence_score: 100,
                ..Default::default()
            },
            OrderMatchResult {
                evaluation_id: "b".into(),
                order_id: "missing-order".into(),
                matched: true,
                source: MatchSource::ExactOrderId,
                confidence_score: 100,
                ..Default::default()
            },
        ];
        let cache_ids = HashSet::from(["cached-order".to_string()]);
        apply_cache_order_match_with_ids(&mut results, &cache_ids);

        assert!(results[0].matched);
        assert!(!results[1].matched);
        assert_eq!(results[1].source, MatchSource::ManualFallback);
    }
}
