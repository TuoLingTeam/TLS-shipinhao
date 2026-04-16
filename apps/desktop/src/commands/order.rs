use crate::adapters::http_order_search::{parse_iso_window, HttpOrderCacheFinder, HttpOrderSearchClient};
use crate::adapters::sqlite_order_cache::SqliteOrderCache;
use crate::commands::license::ensure_feature_authorized;
use crate::error::AppError;
use crate::state::AppState;
use desktop_services::order_cache_storage::OrderCacheRepository;
use desktop_services::order_sync_service::{
    OrderSyncService, ORDER_CACHE_SCOPE, MERGE_TOLERANCE_SECONDS, MIN_GAP_WIDTH_SECONDS,
};
use domain_core::{OrderCacheEntry, TimeWindow};
use serde::{Deserialize, Serialize};
use tauri::{AppHandle, Emitter, State};

fn cache_data_dir() -> std::path::PathBuf {
    dirs::data_local_dir()
        .unwrap_or_else(|| std::path::PathBuf::from("."))
        .join("TLS-shipinhao")
}

fn rich_order_cache_path() -> std::path::PathBuf {
    cache_data_dir().join("order_cache.sqlite3")
}

fn recent_window() -> TimeWindow {
    let end = chrono::Utc::now();
    let start = end - chrono::Duration::days(30);
    TimeWindow {
        start_at: start.to_rfc3339(),
        end_at: end.to_rfc3339(),
    }
}

fn timestamp_to_iso(timestamp: i64) -> Option<String> {
    chrono::DateTime::from_timestamp(timestamp, 0).map(|dt| dt.to_rfc3339())
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
    pub last_sync_at: Option<String>,
    pub coverage_start: Option<String>,
    pub coverage_end: Option<String>,
    pub coverage_complete: bool,
    pub missing_segment_count: usize,
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

pub(crate) fn recent_order_cache_status() -> anyhow::Result<OrderCacheStatus> {
    let rich_cache_path = rich_order_cache_path();
    let repository = OrderCacheRepository::open(&rich_cache_path)?;
    repository.initialize()?;
    let count = repository.count_orders()?;
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
        last_sync_at,
        coverage_start,
        coverage_end,
        coverage_complete,
        missing_segment_count,
    })
}

fn write_lightweight_recent_cache() -> anyhow::Result<Vec<OrderCacheEntry>> {
    use desktop_services::OrderCacheStore;

    let status_window = recent_window();
    let repository = OrderCacheRepository::open(&rich_order_cache_path())?;
    repository.initialize()?;
    let (start_unix, end_unix) = parse_iso_window(&status_window.start_at, &status_window.end_at)?;
    let orders = repository.fetch_orders_in_range(start_unix, end_unix)?;

    let light_entries = orders
        .into_iter()
        .map(|order| OrderCacheEntry {
            order_id: order.order_id,
            buyer_name: order.buyer_nickname,
            receiver_name: order.receiver_name,
            amount_cent: order.amount_cent,
            created_at: timestamp_to_iso(order.create_time).unwrap_or_default(),
            updated_at: timestamp_to_iso(order.updated_at).unwrap_or_default(),
        })
        .collect::<Vec<_>>();

    let cache = SqliteOrderCache::new(cache_data_dir());
    cache.save_orders(&light_entries)?;
    Ok(light_entries)
}

#[tauri::command(rename_all = "snake_case")]
pub async fn load_order_cache(
    start_at: String,
    end_at: String,
) -> Result<Vec<OrderCacheEntry>, AppError> {
    let window = TimeWindow { start_at, end_at };
    tokio::task::spawn_blocking(move || {
        use desktop_services::OrderCacheStore;
        let cache = SqliteOrderCache::new(cache_data_dir());
        cache.load_recent_orders(&window)
    })
    .await
    .map_err(|e| AppError::Message(e.to_string()))?
    .map_err(AppError::Internal)
}

#[tauri::command(rename_all = "snake_case")]
pub async fn get_order_cache_status() -> Result<OrderCacheStatus, AppError> {
    tokio::task::spawn_blocking(recent_order_cache_status)
        .await
        .map_err(|e| AppError::Message(e.to_string()))?
        .map_err(AppError::Internal)
}

/// 兼容旧接口：按窗口抓单并写入两套缓存。
#[tauri::command(rename_all = "snake_case")]
pub async fn sync_orders(
    state: State<'_, AppState>,
    start_at: String,
    end_at: String,
) -> Result<OrderSyncResult, AppError> {
    ensure_feature_authorized(&state, "订单同步").await?;
    let cookie_profile = state.cookie_profile.lock().await;
    if cookie_profile.cookie_header.is_empty() {
        return Err(AppError::Message("请先在设置中配置 Cookie".to_string()));
    }
    let cookie = cookie_profile.cookie_header.clone();
    let magic = cookie_profile.biz_magic.clone().unwrap_or_default();
    drop(cookie_profile);

    let (start_unix, end_unix) =
        parse_iso_window(&start_at, &end_at).map_err(|e| AppError::Message(e.to_string()))?;

    let client = HttpOrderSearchClient::new(cookie, magic);
    let snapshot = client
        .fetch_order_snapshots_in_window(start_unix, end_unix)
        .await
        .map_err(|e| AppError::Message(e.to_string()))?;

    let orders_saved = snapshot.ui_entries.len();
    let data_dir = cache_data_dir();
    let rich_cache_path = rich_order_cache_path();
    tokio::task::spawn_blocking(move || {
        use desktop_services::OrderCacheStore;
        let cache = SqliteOrderCache::new(data_dir);
        cache.save_orders(&snapshot.ui_entries)?;

        let mut repository = OrderCacheRepository::open(&rich_cache_path)?;
        repository.initialize()?;
        repository.upsert_orders(&snapshot.cache_records)?;
        Ok::<(), anyhow::Error>(())
    })
    .await
    .map_err(|e| AppError::Message(e.to_string()))?
    .map_err(AppError::Internal)?;

    Ok(OrderSyncResult {
        orders_saved,
        cache_sync_performed: true,
        cache_coverage_start: Some(start_at),
        cache_coverage_end: Some(end_at),
        cache_warnings: Vec::new(),
    })
}

#[tauri::command(rename_all = "snake_case")]
pub async fn sync_recent_order_cache(
    app: AppHandle,
    state: State<'_, AppState>,
) -> Result<OrderSyncResult, AppError> {
    ensure_feature_authorized(&state, "订单同步").await?;
    let cookie_profile = state.cookie_profile.lock().await;
    if cookie_profile.cookie_header.is_empty() {
        return Err(AppError::Message("请先在设置中配置 Cookie".to_string()));
    }
    let cookie = cookie_profile.cookie_header.clone();
    let magic = cookie_profile.biz_magic.clone().unwrap_or_default();
    drop(cookie_profile);

    emit_order_sync_progress(
        &app,
        "manual",
        "ensure_recent_cache",
        15,
        "正在维护最近 30 天订单缓存…",
    );

    let app_clone = app.clone();
    let result = tokio::task::spawn_blocking(move || -> Result<OrderSyncResult, AppError> {
        let finder = HttpOrderCacheFinder::new(cookie, magic);
        let repository = OrderCacheRepository::open(&rich_order_cache_path())
            .map_err(AppError::Internal)?;
        let mut service = OrderSyncService::new(finder, repository);
        let (written, warnings, coverage_start, coverage_end) = service
            .ensure_recent_cache(Some(chrono::Utc::now()))
            .map_err(AppError::Internal)?;

        emit_order_sync_progress(
            &app_clone,
            "manual",
            "refresh_light_cache",
            78,
            "最近 30 天富缓存已更新，正在刷新订单列表视图…",
        );

        let light_entries = write_lightweight_recent_cache().map_err(AppError::Internal)?;
        emit_order_sync_progress(
            &app_clone,
            "manual",
            "completed",
            100,
            format!("缓存维护完成，当前最近 30 天可见 {} 条订单。", light_entries.len()),
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
