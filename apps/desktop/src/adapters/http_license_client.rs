use std::future::Future;
use std::time::Duration;

use serde::{Deserialize, Serialize};
use thiserror::Error;

use crate::app_settings::LICENSE_API_TIMEOUT_SECS;

/// 授权服务 HTTP 调用的结构化错误。
///
/// 与 PRD M2-01 对齐：网络层失败会自动切换下一个备用域名，业务层失败
/// （HTTP 4xx/5xx 或协议错误）则立即上抛，不再浪费时间重试无意义的请求。
#[derive(Debug, Error)]
pub enum LicenseHttpError {
    #[error("授权服务返回 HTTP {0}")]
    HttpError(u16),
    #[error("授权服务响应格式错误：{0}")]
    InvalidResponse(String),
    #[error("授权服务网络错误：{0}")]
    NetworkError(String),
    #[error("所有授权域名均无法连接：{0}")]
    AllDomainsFailed(String),
}

/// 单次域名尝试的控制流结果。
///
/// - `Final(Ok)` / `Final(Err)`：拿到最终业务结果，停止切域名。
/// - `Network(msg)`：连接/超时层面的失败，可以尝试下一个域名。
pub enum DomainAttempt<R> {
    Final(Result<R, LicenseHttpError>),
    Network(String),
}

/// 顺序尝试多个备用域名，封装容灾逻辑。
///
/// 设计理念：把「容灾切换」与具体的 HTTP 客户端解耦——调用方在 `op` 内完成
/// 请求、解析、错误归类，只需把结果映射为 `DomainAttempt` 即可享受统一的
/// 切换与最后错误记录能力。
///
/// 语义：
/// - 任何 `Final` 直接返回（业务错误也不再尝试下一个）。
/// - `Network` 累积 `last_network_err` 并尝试下一个。
/// - 所有域名都是 `Network` → 返回 `AllDomainsFailed(last_error)`。
/// - `urls` 为空 → 返回 `AllDomainsFailed("")`。
pub async fn try_each_domain<R, F, Fut>(
    urls: &[String],
    mut op: F,
) -> Result<R, LicenseHttpError>
where
    F: FnMut(&str) -> Fut,
    Fut: Future<Output = DomainAttempt<R>>,
{
    let mut last_network_err: Option<String> = None;
    for url in urls {
        match op(url).await {
            DomainAttempt::Final(result) => return result,
            DomainAttempt::Network(msg) => {
                tracing::warn!("授权域名 {url} 网络失败：{msg}");
                last_network_err = Some(msg);
                continue;
            }
        }
    }
    Err(LicenseHttpError::AllDomainsFailed(
        last_network_err.unwrap_or_default(),
    ))
}

/// 把一次 `reqwest` 请求的结果映射成 `DomainAttempt`。
///
/// 分类规则：
/// - 连接失败 / 超时 / 请求发送失败 → `Network`，继续下一个域名
/// - HTTP 非 2xx → `Final(Err(HttpError(status)))`，直接返回
/// - 响应体读取或 JSON 解析失败 → `Final(Err(InvalidResponse(...)))`
/// - JSON 解析成功 → `Final(Ok(value))`
async fn response_to_attempt<R: serde::de::DeserializeOwned>(
    send_result: Result<reqwest::Response, reqwest::Error>,
) -> DomainAttempt<R> {
    match send_result {
        Ok(resp) => {
            let status = resp.status();
            if !status.is_success() {
                return DomainAttempt::Final(Err(LicenseHttpError::HttpError(status.as_u16())));
            }
            let body = match resp.text().await {
                Ok(body) => body,
                Err(e) => {
                    return DomainAttempt::Final(Err(LicenseHttpError::InvalidResponse(
                        format!("读取响应失败：{e}"),
                    )))
                }
            };
            match serde_json::from_str::<R>(&body) {
                Ok(value) => DomainAttempt::Final(Ok(value)),
                Err(e) => {
                    let snippet: String =
                        body.chars().take(160).collect::<String>().replace('\n', " ");
                    DomainAttempt::Final(Err(LicenseHttpError::InvalidResponse(format!(
                        "JSON 解析失败：{e}；片段：{snippet}"
                    ))))
                }
            }
        }
        Err(e) if e.is_timeout() || e.is_connect() || e.is_request() => {
            DomainAttempt::Network(e.to_string())
        }
        Err(e) => DomainAttempt::Final(Err(LicenseHttpError::NetworkError(e.to_string()))),
    }
}

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
pub struct LeaseRefreshRequest {
    pub license_key: String,
    pub device_id: String,
    pub current_issued_at: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LeaseRefreshResponse {
    pub success: bool,
    #[serde(default)]
    pub message: String,
    #[serde(default)]
    pub new_token: String,
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
    ) -> Result<LicenseApiResponse, LicenseHttpError> {
        try_each_domain(&self.base_urls, |base_url| {
            let url = format!("{base_url}{path}");
            let client = self.client.clone();
            let payload_value = serde_json::to_value(payload).ok();
            async move {
                // 单请求硬超时：即便 client-level 已有 builder timeout，
                // 这里再显式设置一次防止迁移过程中的配置回退。
                let request = client
                    .post(&url)
                    .timeout(Duration::from_secs(LICENSE_API_TIMEOUT_SECS));
                let request = match payload_value {
                    Some(body) => request.json(&body),
                    None => {
                        return DomainAttempt::Final(Err(LicenseHttpError::InvalidResponse(
                            "请求体无法序列化".into(),
                        )))
                    }
                };
                response_to_attempt(request.send().await).await
            }
        })
        .await
    }

    pub async fn activate(
        &self,
        key: &str,
        device_id: &str,
        device_fingerprint: &str,
        client_version: &str,
    ) -> Result<LicenseApiResponse, LicenseHttpError> {
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
            return Err(LicenseHttpError::InvalidResponse(format!(
                "激活失败：{}",
                resp.message
            )));
        }
        Ok(resp)
    }

    pub async fn verify(
        &self,
        key: &str,
        device_id: &str,
        license_version: u32,
        client_version: &str,
    ) -> Result<LicenseApiResponse, LicenseHttpError> {
        let req = VerifyRequest {
            key: key.to_string(),
            device_id: device_id.to_string(),
            license_version,
            client_version: client_version.to_string(),
        };
        self.request_json("/api/verify", &req).await
    }

    pub async fn refresh_lease(
        &self,
        license_key: &str,
        device_id: &str,
        current_issued_at: i64,
    ) -> Result<LeaseRefreshResponse, LicenseHttpError> {
        let req = LeaseRefreshRequest {
            license_key: license_key.trim().to_uppercase(),
            device_id: device_id.to_string(),
            current_issued_at,
        };
        try_each_domain(&self.base_urls, |base_url| {
            let url = format!("{base_url}/api/lease/refresh");
            let client = self.client.clone();
            let payload_value = serde_json::to_value(&req).ok();
            async move {
                let request = client
                    .post(&url)
                    .timeout(Duration::from_secs(LICENSE_API_TIMEOUT_SECS));
                let request = match payload_value {
                    Some(body) => request.json(&body),
                    None => {
                        return DomainAttempt::Final(Err(LicenseHttpError::InvalidResponse(
                            "请求体无法序列化".into(),
                        )))
                    }
                };
                response_to_attempt(request.send().await).await
            }
        })
        .await
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::cell::RefCell;

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

    #[test]
    fn lease_refresh_response_roundtrip_keeps_new_token() {
        let response = LeaseRefreshResponse {
            success: true,
            message: "ok".into(),
            new_token: "lease.token.next".into(),
        };
        let json = serde_json::to_string(&response).unwrap();
        let parsed: LeaseRefreshResponse = serde_json::from_str(&json).unwrap();
        assert!(parsed.success);
        assert_eq!(parsed.new_token, "lease.token.next");
    }

    // --- try_each_domain（M2-01） ---

    fn domains() -> Vec<String> {
        vec![
            "https://d1.example".into(),
            "https://d2.example".into(),
            "https://d3.example".into(),
            "https://d4.example".into(),
        ]
    }

    #[tokio::test]
    async fn first_domain_success_stops_iteration() {
        let attempts = RefCell::new(Vec::<String>::new());
        let out: Result<u32, LicenseHttpError> = try_each_domain(&domains(), |url| {
            attempts.borrow_mut().push(url.to_string());
            async move { DomainAttempt::Final(Ok(42)) }
        })
        .await;
        assert_eq!(out.unwrap(), 42);
        assert_eq!(attempts.borrow().len(), 1);
    }

    #[tokio::test]
    async fn network_failure_falls_through_to_next_domain() {
        let attempts = RefCell::new(Vec::<String>::new());
        let out: Result<u32, LicenseHttpError> = try_each_domain(&domains(), |url| {
            attempts.borrow_mut().push(url.to_string());
            let is_first = attempts.borrow().len() == 1;
            async move {
                if is_first {
                    DomainAttempt::Network("timeout".into())
                } else {
                    DomainAttempt::Final(Ok(99))
                }
            }
        })
        .await;
        assert_eq!(out.unwrap(), 99);
        assert_eq!(attempts.borrow().len(), 2);
        assert!(attempts.borrow()[0].ends_with("d1.example"));
        assert!(attempts.borrow()[1].ends_with("d2.example"));
    }

    #[tokio::test]
    async fn business_error_stops_iteration_immediately() {
        // HTTP 4xx 属于业务错误，不应切换下一个域名。
        let attempts = RefCell::new(0u32);
        let out: Result<u32, LicenseHttpError> = try_each_domain(&domains(), |_url| {
            *attempts.borrow_mut() += 1;
            async move { DomainAttempt::Final(Err(LicenseHttpError::HttpError(401))) }
        })
        .await;
        match out {
            Err(LicenseHttpError::HttpError(401)) => {}
            other => panic!("预期 HttpError(401)，实际 {other:?}"),
        }
        assert_eq!(*attempts.borrow(), 1, "业务错误后不应继续尝试下一个域名");
    }

    #[tokio::test]
    async fn invalid_response_stops_iteration_immediately() {
        let attempts = RefCell::new(0u32);
        let out: Result<u32, LicenseHttpError> = try_each_domain(&domains(), |_url| {
            *attempts.borrow_mut() += 1;
            async move {
                DomainAttempt::Final(Err(LicenseHttpError::InvalidResponse("bad json".into())))
            }
        })
        .await;
        assert!(matches!(out, Err(LicenseHttpError::InvalidResponse(_))));
        assert_eq!(*attempts.borrow(), 1);
    }

    #[tokio::test]
    async fn all_network_failures_produce_all_domains_failed_with_last_error() {
        let attempts = RefCell::new(Vec::<String>::new());
        let out: Result<u32, LicenseHttpError> = try_each_domain(&domains(), |url| {
            attempts.borrow_mut().push(url.to_string());
            let n = attempts.borrow().len();
            async move { DomainAttempt::Network(format!("e{n}")) }
        })
        .await;
        assert_eq!(attempts.borrow().len(), 4);
        match out {
            Err(LicenseHttpError::AllDomainsFailed(msg)) => {
                assert_eq!(msg, "e4", "AllDomainsFailed 应携带最后一次的错误");
            }
            other => panic!("预期 AllDomainsFailed，实际 {other:?}"),
        }
    }

    #[tokio::test]
    async fn empty_url_list_returns_all_domains_failed_without_calling_op() {
        let attempts = RefCell::new(0u32);
        let urls: Vec<String> = vec![];
        let out: Result<u32, LicenseHttpError> = try_each_domain(&urls, |_| {
            *attempts.borrow_mut() += 1;
            async move { DomainAttempt::Final(Ok(1)) }
        })
        .await;
        match out {
            Err(LicenseHttpError::AllDomainsFailed(msg)) => assert!(msg.is_empty()),
            other => panic!("预期 AllDomainsFailed，实际 {other:?}"),
        }
        assert_eq!(*attempts.borrow(), 0);
    }

    #[tokio::test]
    async fn mixed_network_then_http_error_surfaces_http_error() {
        // 先 2 个域名网络失败，第 3 个返回 HTTP 500（业务错误）→ 立即上抛。
        let counter = RefCell::new(0u32);
        let out: Result<u32, LicenseHttpError> = try_each_domain(&domains(), |_| {
            *counter.borrow_mut() += 1;
            let n = *counter.borrow();
            async move {
                if n <= 2 {
                    DomainAttempt::Network(format!("net{n}"))
                } else {
                    DomainAttempt::Final(Err(LicenseHttpError::HttpError(500)))
                }
            }
        })
        .await;
        match out {
            Err(LicenseHttpError::HttpError(500)) => {}
            other => panic!("预期 HttpError(500)，实际 {other:?}"),
        }
        assert_eq!(*counter.borrow(), 3);
    }

    #[test]
    fn http_error_display_includes_status_code() {
        let err = LicenseHttpError::HttpError(503);
        assert!(err.to_string().contains("503"));
    }

    #[test]
    fn all_domains_failed_display_includes_last_error() {
        let err = LicenseHttpError::AllDomainsFailed("connection refused".into());
        let msg = err.to_string();
        assert!(msg.contains("connection refused"));
    }
}
