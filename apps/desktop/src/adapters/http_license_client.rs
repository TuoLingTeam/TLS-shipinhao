use serde::{Deserialize, Serialize};

const LICENSE_API_TIMEOUT_SECS: u64 = 30;

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
                Ok(resp) => match resp.json::<LicenseApiResponse>().await {
                    Ok(data) => return Ok(data),
                    Err(e) => {
                        last_err = Some(anyhow::anyhow!(
                            "服务器返回了非 JSON 响应：{}",
                            e
                        ));
                    }
                },
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
        if !resp.success || resp.license_lease.is_none() {
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
