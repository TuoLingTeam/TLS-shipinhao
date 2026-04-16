use tauri::State;

use crate::error::AppError;
use crate::state::AppState;
use domain_core::{OrderCacheEntry, TimeWindow};

#[tauri::command]
pub async fn load_order_cache(
    state: State<'_, AppState>,
    start_at: String,
    end_at: String,
) -> Result<Vec<OrderCacheEntry>, AppError> {
    let window = TimeWindow { start_at, end_at };
    let services = state.services.clone();
    tokio::task::spawn_blocking(move || {
        services.refresh_cache(&window, &[])
    })
    .await
    .map_err(|e| AppError::Message(e.to_string()))?
    .map_err(AppError::Internal)
}
