use crate::adapters::sqlite_order_cache::SqliteOrderCache;
use crate::error::AppError;
use domain_core::{OrderCacheEntry, TimeWindow};

fn cache_data_dir() -> std::path::PathBuf {
    dirs::data_local_dir()
        .unwrap_or_else(|| std::path::PathBuf::from("."))
        .join("TLS-shipinhao")
}

#[tauri::command]
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
