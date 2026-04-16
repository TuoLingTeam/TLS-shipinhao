use crate::adapters::http_license_client::{normalize_license_state, HttpLicenseClient};
use crate::app_settings::LICENSE_API_BASE_URLS;
use crate::error::AppError;
use crate::state::{self, AppState, StoredLicenseProfile};
use sha2::{Digest, Sha256};
use std::ffi::CStr;
use tauri::State;

const LICENSE_PROTOCOL_VERSION: u32 = 3;

fn license_state_allows_feature(state: &str) -> bool {
    matches!(state, "active" | "renewal_due" | "ok")
}

pub async fn ensure_feature_authorized(
    state: &AppState,
    feature_name: &str,
) -> Result<(), AppError> {
    let profile = state.license_profile.lock().await.clone();
    if license_state_allows_feature(profile.license_state.as_str()) {
        return Ok(());
    }

    let detail = match profile.license_state.as_str() {
        "expired" => "当前授权已过期，请续费后再试",
        "revoked" => "当前授权已吊销，请联系管理员",
        "device_mismatch" => "当前设备与授权不匹配，请重新激活",
        "compromised" => "当前授权状态异常，请联系管理员",
        _ => "请先激活授权后再使用此功能",
    };
    Err(AppError::Message(format!("{feature_name}：{detail}")))
}

fn make_client() -> HttpLicenseClient {
    HttpLicenseClient::new(
        LICENSE_API_BASE_URLS
            .iter()
            .map(|s| s.to_string())
            .collect(),
    )
}

fn legacy_compatible_device_id_from_raw(raw: &str) -> String {
    let digest = Sha256::digest(raw.as_bytes());
    digest
        .iter()
        .take(8)
        .map(|byte| format!("{byte:02x}"))
        .collect::<String>()
}

fn security_core_device_id() -> Option<String> {
    let ptr = security_core::security_core_collect_device_id();
    if ptr.is_null() {
        return None;
    }
    let value = unsafe { CStr::from_ptr(ptr) }
        .to_string_lossy()
        .trim()
        .to_string();
    security_core::security_core_free_string(ptr);
    if value.is_empty() {
        None
    } else {
        Some(value)
    }
}

fn device_id() -> String {
    if let Some(id) = security_core_device_id() {
        return id;
    }
    legacy_compatible_device_id_from_raw(&device_fingerprint())
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
    format!(
        "{}-{}-{}",
        hostname(),
        std::env::consts::ARCH,
        std::env::consts::OS
    )
}

fn hostname() -> String {
    std::env::var("HOSTNAME")
        .or_else(|_| std::env::var("COMPUTERNAME"))
        .unwrap_or_else(|_| "unknown".to_string())
}

#[tauri::command(rename_all = "snake_case")]
pub async fn activate_license(
    state: State<'_, AppState>,
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

    let normalized_state = normalize_license_state(&resp.normalized_state());

    persist_license_profile(
        &state,
        StoredLicenseProfile {
            license_key: resp
                .license_key
                .clone()
                .unwrap_or_else(|| license_key.trim().to_uppercase()),
            license_state: normalized_state.clone(),
            license_expires_at: resp.license_expires_at.clone(),
            last_verified_at: Some(current_timestamp()),
        },
    )
    .await?;

    Ok(serde_json::json!({
        "success": resp.success,
        "message": resp.message,
        "license_state": normalized_state,
        "license_key": resp.license_key,
        "device_id": resp.device_id,
        "license_expires_at": resp.license_expires_at,
        "license_lease": resp.license_lease.is_some(),
        "last_verified_at": current_timestamp(),
    }))
}

#[tauri::command(rename_all = "snake_case")]
pub async fn verify_license(
    state: State<'_, AppState>,
    license_key: String,
) -> Result<serde_json::Value, AppError> {
    let client = make_client();
    let did = device_id();
    let version = env!("CARGO_PKG_VERSION").to_string();

    let resp = client
        .verify(&license_key, &did, LICENSE_PROTOCOL_VERSION, &version)
        .await
        .map_err(|e| AppError::Message(e.to_string()))?;

    let normalized_state = normalize_license_state(&resp.normalized_state());

    persist_license_profile(
        &state,
        StoredLicenseProfile {
            license_key: resp
                .license_key
                .clone()
                .unwrap_or_else(|| license_key.trim().to_uppercase()),
            license_state: normalized_state.clone(),
            license_expires_at: resp.license_expires_at.clone(),
            last_verified_at: Some(current_timestamp()),
        },
    )
    .await?;

    Ok(serde_json::json!({
        "success": resp.success,
        "message": resp.message,
        "license_state": normalized_state,
        "license_key": resp.license_key,
        "license_expires_at": resp.license_expires_at,
        "license_lease": resp.license_lease.is_some(),
        "last_verified_at": current_timestamp(),
    }))
}

#[tauri::command(rename_all = "snake_case")]
pub async fn get_license_status(state: State<'_, AppState>) -> Result<serde_json::Value, AppError> {
    let profile = state.license_profile.lock().await.clone();
    Ok(serde_json::json!({
        "configured": !profile.license_key.is_empty(),
        "license_key": profile.license_key,
        "license_state": if profile.license_state.is_empty() { "invalid".to_string() } else { profile.license_state },
        "license_expires_at": profile.license_expires_at,
        "last_verified_at": profile.last_verified_at,
    }))
}

async fn persist_license_profile(
    state: &AppState,
    profile: StoredLicenseProfile,
) -> Result<(), AppError> {
    {
        let mut current = state.license_profile.lock().await;
        *current = profile.clone();
    }
    state::save_license_profile(&state.app_home_dir, &profile)
        .map_err(|e| AppError::Message(format!("保存授权状态失败：{e}")))?;
    Ok(())
}

fn current_timestamp() -> String {
    chrono::Utc::now().to_rfc3339_opts(chrono::SecondsFormat::Secs, true)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn license_state_allows_active_and_renewal_due_only() {
        assert!(license_state_allows_feature("active"));
        assert!(license_state_allows_feature("renewal_due"));

        for state in [
            "invalid",
            "expired",
            "revoked",
            "device_mismatch",
            "compromised",
            "",
        ] {
            assert!(
                !license_state_allows_feature(state),
                "state {state} should be blocked"
            );
        }
    }

    #[test]
    fn legacy_compatible_device_id_matches_python_rule() {
        assert_eq!(
            legacy_compatible_device_id_from_raw("SERIAL-123"),
            "0c04dee8a171fce9"
        );
    }
}
