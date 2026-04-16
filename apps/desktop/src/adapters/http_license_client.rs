use serde::{Deserialize, Serialize};

use crate::app_settings::LICENSE_API_TIMEOUT_SECS;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ActivateRequest {
    pub key: String,
    pub device_id: String,
    pub device_fingerprint: String,
    pub client_version: String,
    pub platform: String,
    pub build_channel: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VerifyRequest {
    pub key: String,
    pub device_id: String,
    pub license_version: u32,
    pub client_version: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LicenseApiResponse {
    pub success: bool,
    #[serde(default)]
    pub message: String,
    #[serde(default)]
    pub license_state: String,
    #[serde(default)]
    pub license_lease: Option<String>,
    #[serde(default)]
    pub license_expires_at: Option<String>,
    #[serde(default)]
    pub activated_at: Option<String>,
    #[serde(default)]
    pub device_id: Option<String>,
    #[serde(default)]
    pub license_key: Option<String>,
    #[serde(default)]
    pub lease_expires_at: Option<String>,
    #[serde(default)]
    pub renew_after: Option<String>,
    #[serde(default)]
    pub issued_at: Option<String>,
    #[serde(default)]
    pub license_status: Option<String>,
    #[serde(default)]
    pub task_policy: Option<Vec<String>>,
}

pub fn normalize_license_state(raw: &str) -> String {
    match raw.trim().to_ascii_lowercase().as_str() {
        "" => "invalid".to_string(),
        "ok" => "active".to_string(),
        other => other.to_string(),
    }
}

impl LicenseApiResponse {
    pub fn normalized_state(&self) -> String {
        if !self.license_state.trim().is_empty() {
            return normalize_license_state(&self.license_state);
        }
        if let Some(status) = self.license_status.as_deref() {
            return normalize_license_state(status);
        }
        "invalid".to_string()
    }
}

fn response_allows_activation(resp: &LicenseApiResponse) -> bool {
    if !resp.success {
        return false;
    }
    if resp.license_lease.is_some() {
        return true;
    }
    matches!(resp.normalized_state().as_str(), "active" | "renewal_due")
}

pub struct HttpLicenseClient {
    base_urls: Vec<String>,
    client: reqwest::Client,
}

impl HttpLicenseClient {
    pub fn new(base_urls: Vec<String>) -> Self {
        let client = reqwest::Client::builder()
            .timeout(std::time::Duration::from_secs(LICENSE_API_TIMEOUT_SECS))
            .build()
            .unwrap_or_default();
        Self { base_urls, client }
    }

    async fn request_json<T: Serialize>(
        &self,
        path: &str,
        payload: &T,
    ) -> anyhow::Result<LicenseApiResponse> {
        let mut last_err: Option<anyhow::Error> = None;
        for base_url in &self.base_urls {
            let url = format!("{}{}", base_url, path);
            match self.client.post(&url).json(payload).send().await {
                Ok(resp) => {
                    let status = resp.status();
                    match resp.text().await {
                        Ok(body) => match serde_json::from_str::<LicenseApiResponse>(&body) {
                            Ok(data) => return Ok(data),
                            Err(e) => {
                                let snippet = body
                                    .chars()
                                    .take(160)
                                    .collect::<String>()
                                    .replace("\n", " ");
                                last_err = Some(anyhow::anyhow!(
                                    "服务器返回了非 JSON 响应（HTTP {} {}）：{}；片段：{}",
                                    status.as_u16(),
                                    status.canonical_reason().unwrap_or(""),
                                    e,
                                    snippet
                                ));
                            }
                        },
                        Err(e) => {
                            last_err = Some(anyhow::anyhow!(
                                "读取服务器响应失败（HTTP {}）：{}",
                                status.as_u16(),
                                e
                            ));
                        }
                    }
                }
                Err(e) => {
                    tracing::warn!("API 请求失败 {}: {}", url, e);
                    last_err = Some(e.into());
                }
            }
        }
        Err(last_err.unwrap_or_else(|| anyhow::anyhow!("无法连接服务器")))
    }

    pub async fn activate(
        &self,
        key: &str,
        device_id: &str,
        device_fingerprint: &str,
        client_version: &str,
    ) -> anyhow::Result<LicenseApiResponse> {
        let req = ActivateRequest {
            key: key.trim().to_uppercase(),
            device_id: device_id.to_string(),
            device_fingerprint: device_fingerprint.to_string(),
            client_version: client_version.to_string(),
            platform: std::env::consts::OS.to_string(),
            build_channel: "desktop".to_string(),
        };
        let resp = self.request_json("/api/activate", &req).await?;
        if !response_allows_activation(&resp) {
            anyhow::bail!("激活失败：{}", resp.message);
        }
        Ok(resp)
    }

    pub async fn verify(
        &self,
        key: &str,
        device_id: &str,
        license_version: u32,
        client_version: &str,
    ) -> anyhow::Result<LicenseApiResponse> {
        let req = VerifyRequest {
            key: key.to_string(),
            device_id: device_id.to_string(),
            license_version,
            client_version: client_version.to_string(),
        };
        self.request_json("/api/verify", &req).await
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn normalize_license_state_maps_ok_to_active() {
        assert_eq!(normalize_license_state("ok"), "active");
        assert_eq!(normalize_license_state("active"), "active");
        assert_eq!(normalize_license_state("renewal_due"), "renewal_due");
        assert_eq!(normalize_license_state(""), "invalid");
    }

    #[test]
    fn activation_accepts_success_without_lease_when_state_is_ok() {
        let resp = LicenseApiResponse {
            success: true,
            message: "重新激活成功".into(),
            license_state: "ok".into(),
            license_lease: None,
            license_expires_at: Some("2120-01-01T00:00:00Z".into()),
            activated_at: None,
            device_id: None,
            license_key: Some("TLS-TEST".into()),
            lease_expires_at: None,
            renew_after: None,
            issued_at: None,
            license_status: None,
            task_policy: None,
        };
        assert!(response_allows_activation(&resp));
        assert_eq!(resp.normalized_state(), "active");
    }
}
