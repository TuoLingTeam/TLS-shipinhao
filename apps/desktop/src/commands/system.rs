use crate::error::AppError;

#[tauri::command]
pub async fn get_app_info() -> Result<serde_json::Value, AppError> {
    Ok(serde_json::json!({
        "name": "TLS-shipinhao",
        "version": env!("CARGO_PKG_VERSION"),
        "runtime": "tauri-2.0",
    }))
}
