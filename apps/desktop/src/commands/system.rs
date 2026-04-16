use tauri::State;

use crate::error::AppError;
use crate::state::AppState;

#[tauri::command]
pub async fn get_app_info() -> Result<serde_json::Value, AppError> {
    Ok(serde_json::json!({
        "name": "TLS-shipinhao",
        "version": env!("CARGO_PKG_VERSION"),
        "runtime": "tauri-2.0",
    }))
}

#[tauri::command]
pub async fn set_cookie(
    state: State<'_, AppState>,
    cookie_header: String,
) -> Result<serde_json::Value, AppError> {
    let biz_magic = cookie_header
        .split(';')
        .map(str::trim)
        .find_map(|seg| seg.strip_prefix("biz_magic="))
        .map(str::to_string);

    let mut profile = state.cookie_profile.lock().await;
    profile.cookie_header = cookie_header;
    profile.biz_magic = biz_magic.clone();
    drop(profile);

    Ok(serde_json::json!({
        "success": true,
        "biz_magic": biz_magic,
    }))
}

#[tauri::command]
pub async fn get_cookie_status(
    state: State<'_, AppState>,
) -> Result<serde_json::Value, AppError> {
    let profile = state.cookie_profile.lock().await;
    Ok(serde_json::json!({
        "configured": !profile.cookie_header.is_empty(),
        "has_biz_magic": profile.biz_magic.is_some(),
    }))
}
