use crate::adapters::order::{parse_iso_window, HttpOrderCacheFinder};
use crate::adapters::order_cache::SqliteOrderCache;
use crate::commands::license::{authorize_runtime_task, ensure_feature_authorized};
use crate::commands::shared::{current_store_paths, require_store_runtime_context};
use crate::error::AppError;
use crate::state::AppState;
use api_contracts::LICENSE_TASK_CACHE_MANAGE;
use desktop_services::day_window::{
    end_of_day_timestamp, recent_day_range_timestamps, start_of_day_timestamp,
};
use desktop_services::order_cache_repository::OrderCacheRepository;
use desktop_services::order_cache_storage::SqliteOrderCacheRepository;
use desktop_services::order_sync_service::{
    OrderSyncService, MERGE_TOLERANCE_SECONDS, MIN_GAP_WIDTH_SECONDS, ORDER_CACHE_SCOPE,
};
use domain_core::{OrderCacheEntry, TimeWindow};
use serde::{Deserialize, Serialize};
use std::path::Path;
use std::sync::Arc;
use tauri::{AppHandle, Emitter, State};

fn recent_window() -> TimeWindow {
    let (start, end) = recent_day_range_timestamps(30, Some(chrono::Utc::now()));
    TimeWindow {
        start_at: timestamp_to_iso(start).unwrap_or_default(),
        end_at: timestamp_to_iso(end).unwrap_or_default(),
    }
}

fn timestamp_to_iso(timestamp: i64) -> Option<String> {
    chrono::DateTime::from_timestamp(timestamp, 0).map(|dt| dt.to_rfc3339())
}

pub(crate) fn mask_order_cache_error(
    operation: &str,
    window: Option<(&str, &str)>,
    user_message: &str,
    error: anyhow::Error,
) -> AppError {
    match window {
        Some((start_at, end_at)) => {
            tracing::error!(
                target: "desktop::order_cache",
                operation,
                start_at,
                end_at,
                error = %error,
                error_dbg = ?error,
                "{user_message}"
            );
        }
        None => {
            tracing::error!(
                target: "desktop::order_cache",
                operation,
                error = %error,
                error_dbg = ?error,
                "{user_message}"
            );
        }
    }
    AppError::Message(user_message.to_string())
}

#[derive(Debug, Serialize, Deserialize, Clone, Default)]
pub struct OrderSyncProgressEvent {
    pub source: String,
    pub phase: String,
    pub progress: u8,
    pub message: String,
}

#[derive(Debug, Serialize, Deserialize, Clone, Default)]
#[serde(rename_all = "snake_case")]
pub struct OrderSyncResult {
    pub orders_saved: usize,
    pub cache_sync_performed: bool,
    pub cache_coverage_start: Option<String>,
    pub cache_coverage_end: Option<String>,
    pub cache_warnings: Vec<String>,
}

#[derive(Debug, Serialize, Deserialize, Clone, Default)]
#[serde(rename_all = "snake_case")]
pub struct OrderCacheStatus {
    pub cached_order_count: usize,
    pub today_count: usize,
    pub yesterday_count: usize,
    pub last_7_days_count: usize,
    pub last_30_days_count: usize,
    pub today_latest_order_at: Option<String>,
    pub last_sync_at: Option<String>,
    pub coverage_start: Option<String>,
    pub coverage_end: Option<String>,
    pub coverage_complete: bool,
    pub missing_segment_count: usize,
}

#[derive(Debug, Serialize, Deserialize, Clone, Default)]
#[serde(rename_all = "snake_case")]
pub struct OrderCacheCounts {
    pub today_count: usize,
    pub yesterday_count: usize,
    pub last_7_days_count: usize,
    pub last_30_days_count: usize,
    pub today_latest_order_at: Option<String>,
}

#[derive(Debug, Clone, Copy)]
struct OrderCacheCountWindows {
    today: (i64, i64),
    yesterday: (i64, i64),
    last_7_days: (i64, i64),
    last_30_days: (i64, i64),
}

pub(crate) fn emit_order_sync_progress(
    app: &AppHandle,
    source: &str,
    phase: &str,
    progress: u8,
    message: impl Into<String>,
) {
    let _ = app.emit(
        "order-sync-progress",
        OrderSyncProgressEvent {
            source: source.to_string(),
            phase: phase.to_string(),
            progress,
            message: message.into(),
        },
    );
}

pub(crate) fn recent_order_cache_status(
    rich_cache_path: &Path,
) -> anyhow::Result<OrderCacheStatus> {
    let repository = SqliteOrderCacheRepository::open(rich_cache_path)?;
    repository.initialize()?;
    let count = repository.count_orders()?;
    let counts = order_cache_counts_from_repository(&repository)?;
    let state = repository.get_state(ORDER_CACHE_SCOPE)?;
    let (coverage_start, coverage_end, last_sync_at, coverage_complete, missing_segment_count) =
        if let Some(state) = state {
            let missing_segments = repository.get_missing_segments(
                ORDER_CACHE_SCOPE,
                state.coverage_start,
                state.coverage_end,
                MERGE_TOLERANCE_SECONDS,
                MIN_GAP_WIDTH_SECONDS,
            )?;
            (
                timestamp_to_iso(state.coverage_start),
                timestamp_to_iso(state.coverage_end),
                timestamp_to_iso(state.last_success_at),
                missing_segments.is_empty(),
                missing_segments.len(),
            )
        } else {
            (None, None, None, false, 0)
        };

    Ok(OrderCacheStatus {
        cached_order_count: count,
        today_count: counts.today_count,
        yesterday_count: counts.yesterday_count,
        last_7_days_count: counts.last_7_days_count,
        last_30_days_count: counts.last_30_days_count,
        today_latest_order_at: counts.today_latest_order_at,
        last_sync_at,
        coverage_start,
        coverage_end,
        coverage_complete,
        missing_segment_count,
    })
}

pub(crate) fn order_cache_counts(rich_cache_path: &Path) -> anyhow::Result<OrderCacheCounts> {
    let repository = SqliteOrderCacheRepository::open(rich_cache_path)?;
    repository.initialize()?;
    order_cache_counts_from_repository(&repository)
}

fn order_cache_counts_from_repository(
    repository: &dyn OrderCacheRepository,
) -> anyhow::Result<OrderCacheCounts> {
    let windows = order_cache_count_windows(chrono::Utc::now());

    Ok(OrderCacheCounts {
        today_count: repository.count_orders_in_range(windows.today.0, windows.today.1)?,
        yesterday_count: repository
            .count_orders_in_range(windows.yesterday.0, windows.yesterday.1)?,
        last_7_days_count: repository
            .count_orders_in_range(windows.last_7_days.0, windows.last_7_days.1)?,
        last_30_days_count: repository
            .count_orders_in_range(windows.last_30_days.0, windows.last_30_days.1)?,
        today_latest_order_at: repository
            .max_order_create_time_in_range(windows.today.0, windows.today.1)?
            .and_then(timestamp_to_iso),
    })
}

fn order_cache_count_windows(now: chrono::DateTime<chrono::Utc>) -> OrderCacheCountWindows {
    let today = (
        start_of_day_timestamp(Some(now)),
        end_of_day_timestamp(Some(now)),
    );
    let yesterday = recent_day_range_timestamps(1, Some(now));
    let last_7_days = recent_day_range_timestamps(7, Some(now));
    let last_30_days = recent_day_range_timestamps(30, Some(now));

    OrderCacheCountWindows {
        today,
        yesterday,
        last_7_days,
        last_30_days,
    }
}

fn write_lightweight_recent_cache(
    data_dir: &Path,
    rich_cache_path: &Path,
) -> anyhow::Result<Vec<OrderCacheEntry>> {
    use desktop_services::OrderCacheStore;

    let status_window = recent_window();
    let repository = SqliteOrderCacheRepository::open(rich_cache_path)?;
    repository.initialize()?;
    let (start_unix, end_unix) = parse_iso_window(&status_window.start_at, &status_window.end_at)?;
    let orders = repository.fetch_orders_in_range(start_unix, end_unix)?;

    let light_entries = orders
        .into_iter()
        .map(|order| OrderCacheEntry {
            order_id: order.order_id,
            buyer_name: order.buyer_nickname,
            amount_cent: order.amount_cent,
            created_at: timestamp_to_iso(order.create_time).unwrap_or_default(),
            updated_at: timestamp_to_iso(order.updated_at).unwrap_or_default(),
        })
        .collect::<Vec<_>>();

    let cache = SqliteOrderCache::new(data_dir.to_path_buf());
    cache.save_orders(&light_entries)?;
    Ok(light_entries)
}

#[tauri::command(rename_all = "snake_case")]
pub async fn load_order_cache(
    state: State<'_, AppState>,
    start_at: String,
    end_at: String,
) -> Result<Vec<OrderCacheEntry>, AppError> {
    let log_start_at = start_at.clone();
    let log_end_at = end_at.clone();
    let window = TimeWindow { start_at, end_at };
    let store_paths = current_store_paths(&state).await;
    let data_dir = store_paths.data_dir;
    tokio::task::spawn_blocking(move || {
        use desktop_services::OrderCacheStore;
        let cache = SqliteOrderCache::new(data_dir);
        cache.load_recent_orders(&window)
    })
    .await
    .map_err(|e| AppError::Message(e.to_string()))?
    .map_err(|error| {
        mask_order_cache_error(
            "load_order_cache",
            Some((&log_start_at, &log_end_at)),
            "订单缓存读取失败，请稍后重试",
            error,
        )
    })
}

#[tauri::command(rename_all = "snake_case")]
pub async fn get_order_cache_status(
    state: State<'_, AppState>,
) -> Result<OrderCacheStatus, AppError> {
    let store_paths = current_store_paths(&state).await;
    let rich_cache_path = store_paths.rich_order_cache_path;
    tokio::task::spawn_blocking(move || recent_order_cache_status(&rich_cache_path))
        .await
        .map_err(|e| AppError::Message(e.to_string()))?
        .map_err(AppError::Internal)
}

#[tauri::command(rename_all = "snake_case")]
pub async fn get_order_cache_counts(
    state: State<'_, AppState>,
) -> Result<OrderCacheCounts, AppError> {
    let store_paths = current_store_paths(&state).await;
    let rich_cache_path = store_paths.rich_order_cache_path;
    tokio::task::spawn_blocking(move || order_cache_counts(&rich_cache_path))
        .await
        .map_err(|e| AppError::Message(e.to_string()))?
        .map_err(AppError::Internal)
}

// NOTE: 原 sync_orders(start_at, end_at) 兼容旧接口接窗口抓单已废弃；
// 前端统一走 sync_recent_order_cache 增量同步。删除以收缩 invoke 面。

#[tauri::command(rename_all = "snake_case")]
pub async fn sync_recent_order_cache(
    app: AppHandle,
    state: State<'_, AppState>,
) -> Result<OrderSyncResult, AppError> {
    ensure_feature_authorized(&state, "订单同步").await?;
    let grant = authorize_runtime_task(&state, LICENSE_TASK_CACHE_MANAGE).await?;
    let context = require_store_runtime_context(&state).await?;
    let crate::commands::shared::StoreRuntimeContext {
        cookie,
        magic,
        data_dir,
        rich_order_cache_path: rich_cache_path,
    } = context;

    emit_order_sync_progress(
        &app,
        "manual",
        "ensure_recent_and_today_cache",
        15,
        "正在维护近 30 天（不含今天）及今天订单缓存…",
    );

    let app_clone = app.clone();
    let result = tokio::task::spawn_blocking(move || -> Result<OrderSyncResult, AppError> {
        let finder = HttpOrderCacheFinder::new_with_grant(cookie, magic, Some(grant.grant_id));
        let repository = SqliteOrderCacheRepository::open(&rich_cache_path).map_err(|error| {
            mask_order_cache_error(
                "sync_recent_order_cache.open_repository",
                None,
                "订单缓存同步失败，请稍后重试",
                error,
            )
        })?;
        let repository: Arc<dyn OrderCacheRepository> = Arc::new(repository);
        let mut service = OrderSyncService::new(finder, repository);
        let (written, warnings, coverage_start, coverage_end) = service
            .ensure_recent_and_today_cache(Some(chrono::Utc::now()))
            .map_err(|error| {
                mask_order_cache_error(
                    "sync_recent_order_cache.ensure_recent_and_today_cache",
                    None,
                    "订单缓存同步失败，请稍后重试",
                    error,
                )
            })?;

        emit_order_sync_progress(
            &app_clone,
            "manual",
            "refresh_light_cache",
            78,
            "近 30 天（不含今天）及今天富缓存已更新，正在刷新订单列表视图…",
        );

        let light_entries =
            write_lightweight_recent_cache(&data_dir, &rich_cache_path).map_err(|error| {
                mask_order_cache_error(
                    "sync_recent_order_cache.write_lightweight_recent_cache",
                    None,
                    "订单缓存同步失败，请稍后重试",
                    error,
                )
            })?;
        emit_order_sync_progress(
            &app_clone,
            "manual",
            "completed",
            100,
            format!(
                "缓存维护完成，当前近 30 天（不含今天）可见 {} 条订单。",
                light_entries.len()
            ),
        );

        Ok(OrderSyncResult {
            orders_saved: written,
            cache_sync_performed: written > 0,
            cache_coverage_start: timestamp_to_iso(coverage_start),
            cache_coverage_end: timestamp_to_iso(coverage_end),
            cache_warnings: warnings,
        })
    })
    .await
    .map_err(|e| AppError::Message(e.to_string()))??;

    Ok(result)
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::TimeZone;

    #[test]
    fn order_cache_load_error_masks_internal_window_details() {
        let masked = mask_order_cache_error(
            "load_order_cache",
            Some(("2026-03-19T16:00:00+00:00", "2026-03-19T23:59:59+00:00")),
            "订单缓存读取失败，请稍后重试",
            anyhow::anyhow!("fetch cache orders for 1773936000..1773964799"),
        );

        assert_eq!(masked.to_string(), "订单缓存读取失败，请稍后重试");
        assert!(!masked.to_string().contains("1773936000"));
        assert!(!masked.to_string().contains("fetch cache orders for"));
    }

    #[test]
    fn order_cache_sync_error_masks_internal_window_details() {
        let masked = mask_order_cache_error(
            "sync_recent_order_cache",
            None,
            "订单缓存同步失败，请稍后重试",
            anyhow::anyhow!("fetch cache orders for 1773936000..1773964799"),
        );

        assert_eq!(masked.to_string(), "订单缓存同步失败，请稍后重试");
        assert!(!masked.to_string().contains("1773964799"));
        assert!(!masked.to_string().contains("fetch cache orders for"));
    }

    #[test]
    fn order_cache_count_windows_follow_business_day_rules() {
        let now = chrono::DateTime::parse_from_rfc3339("2026-04-20T04:00:00Z")
            .unwrap()
            .with_timezone(&chrono::Utc);
        let windows = order_cache_count_windows(now);

        assert_eq!(
            chrono::Utc
                .timestamp_opt(windows.today.0, 0)
                .unwrap()
                .to_rfc3339(),
            "2026-04-19T16:00:00+00:00"
        );
        assert_eq!(
            chrono::Utc
                .timestamp_opt(windows.today.1, 0)
                .unwrap()
                .to_rfc3339(),
            "2026-04-20T15:59:59+00:00"
        );
        assert_eq!(
            chrono::Utc
                .timestamp_opt(windows.yesterday.0, 0)
                .unwrap()
                .to_rfc3339(),
            "2026-04-18T16:00:00+00:00"
        );
        assert_eq!(
            chrono::Utc
                .timestamp_opt(windows.yesterday.1, 0)
                .unwrap()
                .to_rfc3339(),
            "2026-04-19T15:59:59+00:00"
        );
        assert_eq!(
            chrono::Utc
                .timestamp_opt(windows.last_7_days.0, 0)
                .unwrap()
                .to_rfc3339(),
            "2026-04-12T16:00:00+00:00"
        );
        assert_eq!(windows.last_7_days.1, windows.yesterday.1);
        assert_eq!(
            chrono::Utc
                .timestamp_opt(windows.last_30_days.0, 0)
                .unwrap()
                .to_rfc3339(),
            "2026-03-20T16:00:00+00:00"
        );
        assert_eq!(windows.last_30_days.1, windows.yesterday.1);
    }
}
