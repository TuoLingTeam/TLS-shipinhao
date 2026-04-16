use tauri::State;

use crate::adapters::http_review_source::HttpReviewSource;
use crate::error::AppError;
use crate::state::AppState;
use desktop_services::ReviewQuery;
use desktop_services::ReviewSource;
use domain_core::{OrderMatchResult, TimeWindow};

#[tauri::command]
pub async fn find_reviews(
    state: State<'_, AppState>,
    days: u32,
    start_at: String,
    end_at: String,
) -> Result<Vec<OrderMatchResult>, AppError> {
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
        source.fetch_reviews(&query)
    })
    .await
    .map_err(|e| AppError::Message(e.to_string()))?
    .map_err(AppError::Internal)
}
