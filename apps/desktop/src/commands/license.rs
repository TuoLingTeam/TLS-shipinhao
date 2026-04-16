use crate::adapters::http_license_client::HttpLicenseClient;
use crate::error::AppError;

const LICENSE_API_BASE_URLS: &[&str] = &[
    "https://tls-license.tuolingshe.workers.dev",
];
const LICENSE_PROTOCOL_VERSION: u32 = 3;

fn make_client() -> HttpLicenseClient {
    HttpLicenseClient::new(
        LICENSE_API_BASE_URLS.iter().map(|s| s.to_string()).collect(),
    )
}

fn device_id() -> String {
    // security_core 已经有跨平台设备 ID 采集，直接作为 Rust 调用
    // 回退：使用简化的平台信息
    format!(
        "{}-{}-{}",
        std::env::var("HOSTNAME").unwrap_or_default(),
        std::env::consts::ARCH,
        std::env::consts::OS,
    )
}

fn device_fingerprint() -> String {
    #[cfg(target_os = "macos")]
    {
        if let Ok(output) = std::process::Command::new("ioreg")
            .args(["-rd1", "-c", "IOPlatformExpertDevice"])
            .output()
        {
            let text = String::from_utf8_lossy(&output.stdout);
            for line in text.lines() {
                if line.contains("IOPlatformSerialNumber") {
                    if let Some(last) = line.split('=').last() {
                        return last.trim().trim_matches('"').to_string();
                    }
                }
            }
        }
    }
    format!("{}-{}-{}", hostname(), std::env::consts::ARCH, std::env::consts::OS)
}

fn hostname() -> String {
    std::env::var("HOSTNAME")
        .or_else(|_| std::env::var("COMPUTERNAME"))
        .unwrap_or_else(|_| "unknown".to_string())
}

#[tauri::command]
pub async fn activate_license(
    license_key: String,
) -> Result<serde_json::Value, AppError> {
    let client = make_client();
    let did = device_id();
    let fp = device_fingerprint();
    let version = env!("CARGO_PKG_VERSION").to_string();

    let resp = client
        .activate(&license_key, &did, &fp, &version)
        .await
        .map_err(|e| AppError::Message(e.to_string()))?;

    Ok(serde_json::json!({
        "success": resp.success,
        "message": resp.message,
        "license_key": resp.license_key,
        "device_id": resp.device_id,
        "license_expires_at": resp.license_expires_at,
        "license_lease": resp.license_lease.is_some(),
    }))
}

#[tauri::command]
pub async fn verify_license(
    license_key: String,
) -> Result<serde_json::Value, AppError> {
    let client = make_client();
    let did = device_id();
    let version = env!("CARGO_PKG_VERSION").to_string();

    let resp = client
        .verify(&license_key, &did, LICENSE_PROTOCOL_VERSION, &version)
        .await
        .map_err(|e| AppError::Message(e.to_string()))?;

    Ok(serde_json::json!({
        "success": resp.success,
        "message": resp.message,
        "license_state": resp.license_state,
        "license_key": resp.license_key,
        "license_expires_at": resp.license_expires_at,
        "license_lease": resp.license_lease.is_some(),
    }))
}
