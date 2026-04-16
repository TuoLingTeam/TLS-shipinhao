use crate::error::AppError;

#[tauri::command]
pub async fn activate_license(
    license_key: String,
    device_id: String,
) -> Result<serde_json::Value, AppError> {
    Ok(serde_json::json!({
        "success": true,
        "message": "激活功能待接入 Worker API",
        "license_key": license_key,
        "device_id": device_id,
    }))
}

#[tauri::command]
pub async fn verify_license(
    license_key: String,
    device_id: String,
) -> Result<serde_json::Value, AppError> {
    Ok(serde_json::json!({
        "success": true,
        "message": "验证功能待接入 Worker API",
        "license_key": license_key,
        "device_id": device_id,
    }))
}
