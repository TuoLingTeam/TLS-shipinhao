use serde::{Deserialize, Serialize};
use serde_json::Value;
use sha2::{Digest, Sha256};
use std::time::Duration;

pub const UPDATE_VERSION_URL: &str =
    "https://gitee.com/tuolingshe/tuoling-shipinhao/raw/master/version.json";
pub const UPDATE_CHECK_DELAY_MS: u64 = 1200;
const UPDATE_REQUEST_TIMEOUT_SECS: u64 = 8;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "snake_case")]
pub struct UpdateInfo {
    pub app: String,
    pub version: String,
    pub build: u32,
    pub mandatory: bool,
    pub platform: String,
    pub download_url: String,
    pub tutorial_url: String,
    pub notes: Vec<String>,
    pub has_update: bool,
    pub rolling_percentage: u8,
    pub rollout_bucket: Option<u8>,
    pub eligible_by_rollout: bool,
    pub raw_payload: Value,
}

#[derive(Debug, thiserror::Error)]
pub enum UpdateError {
    #[error("更新服务请求失败：{0}")]
    Request(#[from] reqwest::Error),
    #[error("更新配置返回非成功状态：{0}")]
    HttpStatus(reqwest::StatusCode),
    #[error("更新配置缺少 version 字段")]
    MissingVersion,
}

pub async fn fetch_latest_version_info(current_version: Option<&str>, device_key: Option<&str>) -> Result<UpdateInfo, UpdateError> {
    let client = reqwest::Client::new();
    let response = client
        .get(UPDATE_VERSION_URL)
        .timeout(Duration::from_secs(UPDATE_REQUEST_TIMEOUT_SECS))
        .send()
        .await?;

    if !response.status().is_success() {
        return Err(UpdateError::HttpStatus(response.status()));
    }

    let payload: Value = response.json().await?;
    build_update_info(payload, current_version, device_key)
}

pub fn detect_platform() -> String {
    match std::env::consts::OS {
        "macos" => "mac".to_string(),
        "windows" => "windows".to_string(),
        other => other.to_string(),
    }
}

pub fn parse_version(version: &str) -> (u32, u32, u32) {
    let mut parts = version
        .trim()
        .split('.')
        .take(3)
        .map(|segment| segment.parse::<u32>().unwrap_or(0))
        .collect::<Vec<_>>();
    parts.resize(3, 0);
    (parts[0], parts[1], parts[2])
}

pub fn is_newer_version(current: &str, latest: &str) -> bool {
    parse_version(latest) > parse_version(current)
}

pub fn normalize_rolling_percentage(payload: &Value) -> u8 {
    payload
        .get("rolling")
        .and_then(|value| value.get("percentage"))
        .and_then(Value::as_u64)
        .map(|value| value.min(100) as u8)
        .unwrap_or(100)
}

pub fn rollout_bucket(device_key: &str) -> u8 {
    let digest = Sha256::digest(device_key.as_bytes());
    let value = u32::from_be_bytes([digest[0], digest[1], digest[2], digest[3]]);
    (value % 100) as u8
}

pub fn is_device_eligible(rolling_percentage: u8, rollout_bucket: Option<u8>) -> bool {
    if rolling_percentage >= 100 {
        return true;
    }
    rollout_bucket.map(|bucket| bucket < rolling_percentage).unwrap_or(false)
}

fn build_update_info(payload: Value, current_version: Option<&str>, device_key: Option<&str>) -> Result<UpdateInfo, UpdateError> {
    let latest_version = payload
        .get("version")
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .ok_or(UpdateError::MissingVersion)?
        .to_string();

    let current = current_version.unwrap_or(env!("CARGO_PKG_VERSION"));
    let rolling_percentage = normalize_rolling_percentage(&payload);
    let rollout_bucket = device_key.filter(|value| !value.trim().is_empty()).map(rollout_bucket);
    let eligible_by_rollout = is_device_eligible(rolling_percentage, rollout_bucket);

    Ok(UpdateInfo {
        app: payload
            .get("app")
            .and_then(Value::as_str)
            .unwrap_or("TLS-shipinhao")
            .to_string(),
        version: latest_version.clone(),
        build: payload.get("build").and_then(Value::as_u64).unwrap_or(0) as u32,
        mandatory: payload
            .get("mandatory")
            .and_then(Value::as_bool)
            .unwrap_or(false),
        platform: detect_platform(),
        download_url: payload
            .get("download_url")
            .and_then(Value::as_str)
            .unwrap_or_default()
            .to_string(),
        tutorial_url: payload
            .get("tutorial_url")
            .and_then(Value::as_str)
            .unwrap_or_default()
            .to_string(),
        notes: normalize_notes(&payload),
        has_update: is_newer_version(current, &latest_version) && eligible_by_rollout,
        rolling_percentage,
        rollout_bucket,
        eligible_by_rollout,
        raw_payload: payload,
    })
}

fn normalize_notes(payload: &Value) -> Vec<String> {
    if let Some(notes) = payload.get("notes") {
        if let Some(list) = notes.as_array() {
            return list
                .iter()
                .filter_map(Value::as_str)
                .map(str::trim)
                .filter(|item| !item.is_empty())
                .map(str::to_string)
                .collect();
        }

        if let Some(text) = notes.as_str() {
            return text
                .lines()
                .map(str::trim)
                .filter(|item| !item.is_empty())
                .map(|item| item.trim_start_matches(['-', '•', '*', ' ']).to_string())
                .filter(|item| !item.is_empty())
                .collect();
        }
    }

    Vec::new()
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn parse_version_pads_missing_segments() {
        assert_eq!(parse_version("5.1"), (5, 1, 0));
        assert_eq!(parse_version("6"), (6, 0, 0));
        assert_eq!(parse_version("5.1.9"), (5, 1, 9));
    }

    #[test]
    fn newer_version_detection_matches_semver_tuple() {
        assert!(is_newer_version("5.1.0", "5.1.1"));
        assert!(!is_newer_version("5.1.0", "5.1.0"));
        assert!(is_newer_version("5.1.0", "6.0.0"));
    }

    #[test]
    fn build_update_info_keeps_required_payload_fields() {
        let payload = json!({
            "app": "TLS-shipinhao",
            "version": "5.1.1",
            "build": 12,
            "mandatory": true,
            "download_url": "https://example.com/app.dmg",
            "tutorial_url": "https://example.com/tutorial",
            "notes": ["修复登录窗口", "优化差评匹配"]
        });

        let info = build_update_info(payload.clone(), Some("5.1.0"), Some("device-A")).expect("update info");
        assert!(info.has_update);
        assert_eq!(info.version, "5.1.1");
        assert_eq!(info.download_url, "https://example.com/app.dmg");
        assert_eq!(info.tutorial_url, "https://example.com/tutorial");
        assert_eq!(info.notes, vec!["修复登录窗口", "优化差评匹配"]);
        assert_eq!(info.rolling_percentage, 100);
        assert!(info.eligible_by_rollout);
        assert_eq!(info.raw_payload, payload);
    }

    #[test]
    fn rollout_percentage_limits_visibility_by_bucket() {
        let payload = json!({
            "version": "5.1.1",
            "rolling": { "percentage": 10 }
        });
        let inside = build_update_info(payload.clone(), Some("5.1.0"), Some("device-eligible"))
            .expect("inside");
        let outside = build_update_info(payload, Some("5.1.0"), Some("device-other"))
            .expect("outside");
        assert_eq!(inside.rolling_percentage, 10);
        assert_eq!(outside.rolling_percentage, 10);
        assert_ne!(inside.rollout_bucket, outside.rollout_bucket);
    }

    #[test]
    fn bucket_and_eligibility_are_stable() {
        let bucket = rollout_bucket("device-A");
        assert_eq!(bucket, rollout_bucket("device-A"));
        assert!(is_device_eligible(100, Some(bucket)));
        assert!(!is_device_eligible(0, Some(bucket)));
    }
}
