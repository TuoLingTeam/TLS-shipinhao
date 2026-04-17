use license_service::{
    ActivationInput, LicenseRepository, LicenseService, LicenseServiceResponse, VerifyInput,
};
use serde::{Deserialize, Serialize};
use serde_json::Value;

#[cfg(target_arch = "wasm32")]
mod admin_d1;

/// Worker 支持的路由枚举。
///
/// 新增路由必须同时更新 `parse_route` / `route_request` / `handle_json_request`
/// 与对应的请求/响应结构（`LeaseRefreshRequest` 等）。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WorkerRoute {
    Activate,
    Verify,
    /// M2-04 续约端点：input = LeaseRefreshRequest, output = LeaseRefreshResponse
    LeaseRefresh,
    /// 管理员吊销端点：由后台管理 UI 触发
    LeaseRevoke,
    /// M2-08 任务级授权：input = TaskAuthorizeRequest, output = RuntimeGrant
    TaskAuthorize,
    NotFound,
}

/// `/api/lease/refresh` 入参。
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct LeaseRefreshRequest {
    pub license_key: String,
    pub device_id: String,
    /// 原 Lease 的 issued_at，Worker 用它做乐观并发控制。
    pub current_issued_at: i64,
}

/// `/api/lease/refresh` 响应。
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct LeaseRefreshResponse {
    pub success: bool,
    #[serde(default)]
    pub message: String,
    /// 新签发的 Lease Token（base64url(payload).base64url(signature)），
    /// 客户端需本地验签后才能写回 Keychain。
    pub new_token: String,
}

/// `/api/lease/revoke` 入参（管理员使用）。
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct LeaseRevokeRequest {
    pub license_key: String,
    pub device_id: String,
    #[serde(default)]
    pub reason: String,
}

/// `/api/task/authorize` 入参。
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct TaskAuthorizeRequest {
    pub license_key: String,
    pub device_id: String,
    pub task_type: String,
    #[serde(default)]
    pub client_version: String,
}

pub fn handle_activate<R: LicenseRepository>(
    service: &LicenseService<R>,
    input: ActivationInput,
) -> anyhow::Result<LicenseServiceResponse> {
    service.activate(input)
}

pub fn handle_verify<R: LicenseRepository>(
    service: &LicenseService<R>,
    input: VerifyInput,
) -> anyhow::Result<LicenseServiceResponse> {
    service.verify(input)
}

pub fn parse_route(path: &str) -> WorkerRoute {
    match path {
        "/api/activate" => WorkerRoute::Activate,
        "/api/verify" => WorkerRoute::Verify,
        "/api/lease/refresh" => WorkerRoute::LeaseRefresh,
        "/api/lease/revoke" => WorkerRoute::LeaseRevoke,
        "/api/task/authorize" => WorkerRoute::TaskAuthorize,
        _ => WorkerRoute::NotFound,
    }
}

pub fn route_request(path: &str) -> &'static str {
    match parse_route(path) {
        WorkerRoute::Activate => "activate",
        WorkerRoute::Verify => "verify",
        WorkerRoute::LeaseRefresh => "lease_refresh",
        WorkerRoute::LeaseRevoke => "lease_revoke",
        WorkerRoute::TaskAuthorize => "task_authorize",
        WorkerRoute::NotFound => "not_found",
    }
}

pub fn handle_json_request<R: LicenseRepository>(
    service: &LicenseService<R>,
    path: &str,
    body: &str,
) -> anyhow::Result<String> {
    let route = parse_route(path);
    let payload: Value = serde_json::from_str(body)?;
    match route {
        WorkerRoute::Activate => {
            let input: ActivationInput = serde_json::from_value(payload)?;
            let resp = handle_activate(service, input)?;
            Ok(serde_json::to_string(&resp)?)
        }
        WorkerRoute::Verify => {
            let input: VerifyInput = serde_json::from_value(payload)?;
            let resp = handle_verify(service, input)?;
            Ok(serde_json::to_string(&resp)?)
        }
        WorkerRoute::LeaseRefresh => {
            // 真正的续约签名待 D1 repository + Ed25519 私钥接入后完成；
            // 这里先做契约 pinning，保证客户端可以按 `LeaseRefreshResponse` 解析。
            let _input: LeaseRefreshRequest = serde_json::from_value(payload)?;
            Ok(serde_json::to_string(&LeaseRefreshResponse {
                success: false,
                message: "lease_refresh_pending".into(),
                new_token: String::new(),
            })?)
        }
        WorkerRoute::LeaseRevoke => {
            let _input: LeaseRevokeRequest = serde_json::from_value(payload)?;
            Ok(serde_json::json!({
                "success": false,
                "message": "lease_revoke_pending",
            })
            .to_string())
        }
        WorkerRoute::TaskAuthorize => {
            let _input: TaskAuthorizeRequest = serde_json::from_value(payload)?;
            Ok(serde_json::json!({
                "success": false,
                "message": "task_authorize_pending",
            })
            .to_string())
        }
        WorkerRoute::NotFound => {
            let resp = LicenseServiceResponse {
                success: false,
                message: "not_found".into(),
                license_state: api_contracts::LicenseState::Invalid,
                expired: false,
                activated_at: None,
                license_expires_at: None,
                license_lease: None,
            };
            Ok(serde_json::to_string(&resp)?)
        }
    }
}

#[cfg(target_arch = "wasm32")]
mod cloudflare_entry {
    use super::*;
    use worker::{event, Env, Method, Request, Response, Result};

    fn compatibility_payload(path: &str) -> String {
        serde_json::json!({
            "success": false,
            "message": "rust_worker_repository_pending",
            "path": path,
        })
        .to_string()
    }

    async fn route_fetch(req: Request, env: Env) -> Result<Response> {
        let path = req.path();
        let method = req.method();

        if method == Method::Get && path == "/admin" {
            return crate::admin_d1::serve_admin_html().await;
        }

        if path.starts_with("/api/admin/") {
            return crate::admin_d1::handle_admin_request(req, &env).await;
        }

        if method != Method::Post {
            return Response::error("Method Not Allowed", 405);
        }

        let _ = req.text().await;
        match parse_route(&path) {
            WorkerRoute::Activate
            | WorkerRoute::Verify
            | WorkerRoute::LeaseRefresh
            | WorkerRoute::LeaseRevoke
            | WorkerRoute::TaskAuthorize => {
                // D1 Repository 与 Ed25519 签发逻辑尚未接通，统一返回占位响应。
                Response::from_json(&serde_json::json!({
                    "success": false,
                    "message": "rust_worker_repository_pending",
                    "path": path,
                }))
            }
            WorkerRoute::NotFound => Response::error("not_found", 404),
        }
    }

    #[event(fetch)]
    pub async fn fetch(req: Request, env: Env, _ctx: worker::Context) -> Result<Response> {
        route_fetch(req, env)
            .await
            .or_else(|_| Response::ok(compatibility_payload("/error")))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use license_service::{
        AuditEvent, DeviceRegistration, GeneratedKeyRecord, GeneratedKeyStatus, LicenseRecord,
    };
    use std::collections::HashMap;
    use std::sync::Mutex;

    #[derive(Default)]
    struct Repo {
        generated_keys: Mutex<HashMap<String, GeneratedKeyRecord>>,
        licenses: Mutex<HashMap<String, LicenseRecord>>,
        registrations: Mutex<HashMap<(String, String), DeviceRegistration>>,
        audits: Mutex<Vec<AuditEvent>>,
    }

    impl Repo {
        fn seeded() -> Self {
            let repo = Self::default();
            repo.generated_keys.lock().unwrap().insert(
                "TLS-TEST".into(),
                GeneratedKeyRecord {
                    license_key: "TLS-TEST".into(),
                    plan_days: 30,
                    status: GeneratedKeyStatus::Unused,
                    created_at: "2026-01-01T00:00:00Z".into(),
                    note: String::new(),
                },
            );
            repo
        }
    }

    impl LicenseRepository for Repo {
        fn load_generated_key(
            &self,
            license_key: &str,
        ) -> anyhow::Result<Option<GeneratedKeyRecord>> {
            Ok(self
                .generated_keys
                .lock()
                .unwrap()
                .get(license_key)
                .cloned())
        }

        fn save_generated_key(&self, record: &GeneratedKeyRecord) -> anyhow::Result<()> {
            self.generated_keys
                .lock()
                .unwrap()
                .insert(record.license_key.clone(), record.clone());
            Ok(())
        }

        fn load_license(&self, license_key: &str) -> anyhow::Result<Option<LicenseRecord>> {
            Ok(self.licenses.lock().unwrap().get(license_key).cloned())
        }

        fn save_license(&self, record: &LicenseRecord) -> anyhow::Result<()> {
            self.licenses
                .lock()
                .unwrap()
                .insert(record.license_key.clone(), record.clone());
            Ok(())
        }

        fn load_device_registration(
            &self,
            license_key: &str,
            device_id: &str,
        ) -> anyhow::Result<Option<DeviceRegistration>> {
            Ok(self
                .registrations
                .lock()
                .unwrap()
                .get(&(license_key.to_string(), device_id.to_string()))
                .cloned())
        }

        fn save_device_registration(
            &self,
            registration: &DeviceRegistration,
        ) -> anyhow::Result<()> {
            self.registrations.lock().unwrap().insert(
                (
                    registration.license_key.clone(),
                    registration.device_id.clone(),
                ),
                registration.clone(),
            );
            Ok(())
        }

        fn append_audit_event(&self, event: &AuditEvent) -> anyhow::Result<()> {
            self.audits.lock().unwrap().push(event.clone());
            Ok(())
        }
    }

    #[test]
    fn parses_routes() {
        assert_eq!(parse_route("/api/activate"), WorkerRoute::Activate);
        assert_eq!(parse_route("/api/verify"), WorkerRoute::Verify);
        assert_eq!(parse_route("/api/lease/refresh"), WorkerRoute::LeaseRefresh);
        assert_eq!(parse_route("/api/lease/revoke"), WorkerRoute::LeaseRevoke);
        assert_eq!(
            parse_route("/api/task/authorize"),
            WorkerRoute::TaskAuthorize
        );
        assert_eq!(parse_route("/missing"), WorkerRoute::NotFound);
    }

    #[test]
    fn route_request_emits_stable_keys() {
        assert_eq!(route_request("/api/activate"), "activate");
        assert_eq!(route_request("/api/verify"), "verify");
        assert_eq!(route_request("/api/lease/refresh"), "lease_refresh");
        assert_eq!(route_request("/api/lease/revoke"), "lease_revoke");
        assert_eq!(route_request("/api/task/authorize"), "task_authorize");
        assert_eq!(route_request("/nope"), "not_found");
    }

    #[test]
    fn handles_activate_json() {
        let repo = Repo::seeded();
        let service = LicenseService::new(repo);
        let response = handle_json_request(
            &service,
            "/api/activate",
            r#"{"license_key":"TLS-TEST","device_id":"device-1","device_fingerprint":"fp-1","client_version":"4.3.0"}"#,
        )
        .unwrap();
        let payload: Value = serde_json::from_str(&response).unwrap();
        assert_eq!(payload["success"], true);
        assert_eq!(payload["license_state"], "active");
        assert_eq!(payload["license_lease"]["device_id"], "device-1");
    }

    #[test]
    fn lease_refresh_returns_pending_contract_response() {
        let service = LicenseService::new(Repo::seeded());
        let resp = handle_json_request(
            &service,
            "/api/lease/refresh",
            r#"{"license_key":"TLS-TEST","device_id":"device-1","current_issued_at":1700000000}"#,
        )
        .unwrap();
        let payload: LeaseRefreshResponse = serde_json::from_str(&resp).unwrap();
        assert!(!payload.success);
        assert_eq!(payload.message, "lease_refresh_pending");
        assert!(payload.new_token.is_empty());
    }

    #[test]
    fn lease_revoke_route_returns_pending_marker() {
        let service = LicenseService::new(Repo::seeded());
        let resp = handle_json_request(
            &service,
            "/api/lease/revoke",
            r#"{"license_key":"TLS-TEST","device_id":"device-1","reason":"admin"}"#,
        )
        .unwrap();
        let payload: Value = serde_json::from_str(&resp).unwrap();
        assert_eq!(payload["message"], "lease_revoke_pending");
    }

    #[test]
    fn task_authorize_route_returns_pending_marker() {
        let service = LicenseService::new(Repo::seeded());
        let resp = handle_json_request(
            &service,
            "/api/task/authorize",
            r#"{"license_key":"TLS-TEST","device_id":"device-1","task_type":"review_find","client_version":"5.1.0"}"#,
        )
        .unwrap();
        let payload: Value = serde_json::from_str(&resp).unwrap();
        assert_eq!(payload["message"], "task_authorize_pending");
    }

    #[test]
    fn new_requests_roundtrip_serde() {
        let refresh = LeaseRefreshRequest {
            license_key: "ABC".into(),
            device_id: "dev".into(),
            current_issued_at: 42,
        };
        let j = serde_json::to_string(&refresh).unwrap();
        assert!(j.contains("\"license_key\""));
        assert!(j.contains("\"current_issued_at\""));
        let parsed: LeaseRefreshRequest = serde_json::from_str(&j).unwrap();
        assert_eq!(parsed, refresh);
    }
}
