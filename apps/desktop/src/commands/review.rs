use tauri::State;

use crate::error::AppError;
use crate::state::AppState;
use desktop_services::ReviewQuery;
use domain_core::{OrderMatchResult, TimeWindow};

#[tauri::command]
pub async fn find_reviews(
    state: State<'_, AppState>,
    days: u32,
    start_at: String,
    end_at: String,
) -> Result<Vec<OrderMatchResult>, AppError> {
    let query = ReviewQuery {
        days,
        time_window: TimeWindow { start_at, end_at },
        runtime_grant: None,
    };
    let services = state.services.clone();
    tokio::task::spawn_blocking(move || services.find_reviews(&query))
        .await
        .map_err(|e| AppError::Message(e.to_string()))?
        .map_err(AppError::Internal)
}
