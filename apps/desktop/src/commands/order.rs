use crate::adapters::http_order_search::{parse_iso_window, HttpOrderSearchClient};
use crate::adapters::sqlite_order_cache::SqliteOrderCache;
use crate::error::AppError;
use crate::state::AppState;
use domain_core::{OrderCacheEntry, TimeWindow};
use serde::Serialize;
use tauri::State;

fn cache_data_dir() -> std::path::PathBuf {
    dirs::data_local_dir()
        .unwrap_or_else(|| std::path::PathBuf::from("."))
        .join("TLS-shipinhao")
}

#[derive(Debug, Serialize)]
pub struct OrderSyncResult {
    pub orders_saved: usize,
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

/// 从微信 `orderSearch` 拉单并写入本地 SQLite（与 Python `get_orders_for_cache` 同数据源）。
#[tauri::command(rename_all = "snake_case")]
pub async fn sync_orders(
    state: State<'_, AppState>,
    start_at: String,
    end_at: String,
) -> Result<OrderSyncResult, AppError> {
    let cookie_profile = state.cookie_profile.lock().await;
    if cookie_profile.cookie_header.is_empty() {
        return Err(AppError::Message("请先在设置中配置 Cookie".to_string()));
    }
    let cookie = cookie_profile.cookie_header.clone();
    let magic = cookie_profile
        .biz_magic
        .clone()
        .unwrap_or_default();
    drop(cookie_profile);

    let (start_unix, end_unix) =
        parse_iso_window(&start_at, &end_at).map_err(|e| AppError::Message(e.to_string()))?;

    let client = HttpOrderSearchClient::new(cookie, magic);
    let orders = client
        .fetch_orders_in_window(start_unix, end_unix)
        .await
        .map_err(|e| AppError::Message(e.to_string()))?;

    let orders_saved = orders.len();
    let data_dir = cache_data_dir();
    tokio::task::spawn_blocking(move || {
        use desktop_services::OrderCacheStore;
        let cache = SqliteOrderCache::new(data_dir);
        cache.save_orders(&orders)
    })
    .await
    .map_err(|e| AppError::Message(e.to_string()))?
    .map_err(AppError::Internal)?;

    Ok(OrderSyncResult { orders_saved })
}
