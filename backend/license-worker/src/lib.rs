use api_contracts::{
    LeasePayload, LicenseLease, LicenseState, RiskLevel, RuntimeGrant, LEASE_KIND_LICENSE,
    LICENSE_TASK_BATCH_DELIVERY, LICENSE_TASK_CACHE_MANAGE, LICENSE_TASK_QUALITY_REFUND,
    LICENSE_TASK_REVIEW_FIND, LICENSE_TASK_REVIEW_FULL_SCAN,
};
use base64::engine::general_purpose::{STANDARD, URL_SAFE_NO_PAD};
use base64::Engine;
use chrono::{DateTime, Utc};
use ed25519_dalek::pkcs8::DecodePrivateKey;
use ed25519_dalek::{Signer, SigningKey};
use license_service::{
    authorize_task_local, ActivationInput, LicenseRepository, LicenseService,
    LicenseServiceResponse, VerifyInput,
};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::sync::atomic::{AtomicU64, Ordering};

#[cfg(target_arch = "wasm32")]
use sha2::{Digest, Sha256};

static NEXT_GRANT_SEQ: AtomicU64 = AtomicU64::new(1);

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

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct SignedLicenseApiResponse {
    pub success: bool,
    #[serde(default)]
    pub message: String,
    pub license_state: LicenseState,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub license_lease: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub license_expires_at: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub activated_at: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub device_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub license_key: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub lease_expires_at: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub renew_after: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub issued_at: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub license_status: Option<LicenseState>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub task_policy: Option<Vec<String>>,
}

#[derive(Debug, Clone)]
pub struct LeaseTokenSigner {
    signing_key: SigningKey,
}

impl LeaseTokenSigner {
    pub fn from_private_key_b64(private_key_b64: &str) -> anyhow::Result<Self> {
        let raw = STANDARD.decode(private_key_b64.trim())?;
        if let Ok(text) = String::from_utf8(raw.clone()) {
            if text.contains("BEGIN PRIVATE KEY") {
                let body = text
                    .lines()
                    .filter(|line| !line.starts_with("-----"))
                    .collect::<String>();
                let der = STANDARD.decode(body.as_bytes())?;
                return Ok(Self {
                    signing_key: SigningKey::from_pkcs8_der(&der)?,
                });
            }
        }
        Ok(Self {
            signing_key: SigningKey::from_pkcs8_der(&raw)?,
        })
    }

    pub fn public_key_b64(&self) -> String {
        URL_SAFE_NO_PAD.encode(self.signing_key.verifying_key().as_bytes())
    }

    pub fn sign_license_lease(&self, lease: &LicenseLease) -> anyhow::Result<String> {
        let payload = lease_to_payload(lease)?;
        let payload_bytes = serde_json::to_vec(&payload)?;
        let payload_b64 = URL_SAFE_NO_PAD.encode(payload_bytes);
        let signature = self.signing_key.sign(payload_b64.as_bytes());
        let signature_b64 = URL_SAFE_NO_PAD.encode(signature.to_bytes());
        Ok(format!("{payload_b64}.{signature_b64}"))
    }
}

fn parse_iso_epoch(value: &str) -> anyhow::Result<i64> {
    Ok(DateTime::parse_from_rfc3339(value)?.with_timezone(&Utc).timestamp())
}

fn lease_to_payload(lease: &LicenseLease) -> anyhow::Result<LeasePayload> {
    Ok(LeasePayload {
        kind: LEASE_KIND_LICENSE.to_string(),
        license_key: lease.license_key.clone(),
        device_id: lease.device_id.clone(),
        issued_at: parse_iso_epoch(&lease.issued_at)?,
        exp: parse_iso_epoch(&lease.lease_expires_at)?,
        renew_after: parse_iso_epoch(&lease.renew_after)?,
        task_policy: lease.task_policy.clone(),
        risk_level: "low".to_string(),
    })
}

fn task_risk_level(task_type: &str) -> Option<RiskLevel> {
    match task_type {
        LICENSE_TASK_BATCH_DELIVERY => Some(RiskLevel::High),
        LICENSE_TASK_REVIEW_FULL_SCAN | LICENSE_TASK_CACHE_MANAGE => Some(RiskLevel::Medium),
        LICENSE_TASK_REVIEW_FIND | LICENSE_TASK_QUALITY_REFUND => Some(RiskLevel::Low),
        _ => None,
    }
}

fn next_grant_id() -> String {
    let seq = NEXT_GRANT_SEQ.fetch_add(1, Ordering::Relaxed);
    format!("worker-grant-{}-{seq}", Utc::now().timestamp_millis())
}

fn denied_grant(task_type: &str, message: impl Into<String>) -> RuntimeGrant {
    RuntimeGrant {
        task_type: task_type.to_string(),
        granted: false,
        grant_id: String::new(),
        valid_until: String::new(),
        risk_level: task_risk_level(task_type),
        degraded_reason: Some(message.into()),
    }
}

fn map_service_response(
    response: LicenseServiceResponse,
    signer: &LeaseTokenSigner,
) -> anyhow::Result<SignedLicenseApiResponse> {
    let signed_lease = match response.license_lease.as_ref() {
        Some(lease) => Some(signer.sign_license_lease(lease)?),
        None => None,
    };
    let lease = response.license_lease;
    Ok(SignedLicenseApiResponse {
        success: response.success,
        message: response.message,
        license_state: response.license_state,
        license_lease: signed_lease,
        license_expires_at: response.license_expires_at,
        activated_at: response.activated_at,
        device_id: lease.as_ref().map(|value| value.device_id.clone()),
        license_key: lease.as_ref().map(|value| value.license_key.clone()),
        lease_expires_at: lease.as_ref().map(|value| value.lease_expires_at.clone()),
        renew_after: lease.as_ref().map(|value| value.renew_after.clone()),
        issued_at: lease.as_ref().map(|value| value.issued_at.clone()),
        license_status: lease.as_ref().map(|value| value.license_status),
        task_policy: lease.as_ref().map(|value| value.task_policy.clone()),
    })
}

fn handle_lease_refresh<R: LicenseRepository>(
    service: &LicenseService<R>,
    input: LeaseRefreshRequest,
    signer: &LeaseTokenSigner,
    now: DateTime<Utc>,
) -> anyhow::Result<LeaseRefreshResponse> {
    let verify = service.verify_at(
        VerifyInput {
            license_key: input.license_key,
            device_id: input.device_id,
            client_version: "worker_refresh".into(),
        },
        now,
    )?;
    let Some(lease) = verify.license_lease else {
        return Ok(LeaseRefreshResponse {
            success: false,
            message: verify.message,
            new_token: String::new(),
        });
    };
    let token = signer.sign_license_lease(&lease)?;
    Ok(LeaseRefreshResponse {
        success: true,
        message: "lease_refreshed".into(),
        new_token: token,
    })
}

fn handle_task_authorize<R: LicenseRepository>(
    service: &LicenseService<R>,
    input: TaskAuthorizeRequest,
    now: DateTime<Utc>,
) -> anyhow::Result<RuntimeGrant> {
    let verify = service.verify_at(
        VerifyInput {
            license_key: input.license_key,
            device_id: input.device_id,
            client_version: if input.client_version.is_empty() {
                "worker_task_authorize".into()
            } else {
                format!("worker_task_authorize:{}", input.client_version)
            },
        },
        now,
    )?;
    let Some(lease) = verify.license_lease else {
        return Ok(denied_grant(&input.task_type, verify.message));
    };
    let payload = lease_to_payload(&lease)?;
    match authorize_task_local(&payload, &input.task_type, now.timestamp(), next_grant_id) {
        Ok(mut grant) => {
            grant.risk_level = task_risk_level(&input.task_type);
            Ok(grant)
        }
        Err(err) => Ok(denied_grant(&input.task_type, err.to_string())),
    }
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
    signer: &LeaseTokenSigner,
) -> anyhow::Result<String> {
    let route = parse_route(path);
    let payload: Value = serde_json::from_str(body)?;
    match route {
        WorkerRoute::Activate => {
            let input: ActivationInput = serde_json::from_value(payload)?;
            let resp = handle_activate(service, input)?;
            Ok(serde_json::to_string(&map_service_response(resp, signer)?)?)
        }
        WorkerRoute::Verify => {
            let input: VerifyInput = serde_json::from_value(payload)?;
            let resp = handle_verify(service, input)?;
            Ok(serde_json::to_string(&map_service_response(resp, signer)?)?)
        }
        WorkerRoute::LeaseRefresh => {
            let input: LeaseRefreshRequest = serde_json::from_value(payload)?;
            Ok(serde_json::to_string(&handle_lease_refresh(
                service,
                input,
                signer,
                Utc::now(),
            )?)?)
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
            let input: TaskAuthorizeRequest = serde_json::from_value(payload)?;
            Ok(serde_json::to_string(&handle_task_authorize(
                service,
                input,
                Utc::now(),
            )?)?)
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
            Ok(serde_json::to_string(&map_service_response(resp, signer)?)?)
        }
    }
}

#[cfg(target_arch = "wasm32")]
mod cloudflare_entry {
    use super::*;
    use wasm_bindgen::JsValue;
    use worker::{event, D1Database, Env, Method, Request, Response, Result};

    #[derive(Debug, Clone, Deserialize)]
    struct GeneratedKeyRow {
        license_key: String,
        plan_days: i64,
        status: String,
        #[serde(default)]
        created_at: Option<String>,
        #[serde(default)]
        note: Option<String>,
    }

    #[derive(Debug, Clone, Deserialize)]
    struct ActivationRow {
        license_key: String,
        device_id: String,
        #[serde(default)]
        device_fingerprint: Option<String>,
        plan_days: i64,
        activated_at: String,
        expires_at: String,
        updated_at: String,
        binding_version: u32,
        status: String,
        #[serde(default)]
        last_verify_at: Option<String>,
        #[serde(default)]
        last_session_issued_at: Option<String>,
        #[serde(default)]
        last_offline_grant_issued_at: Option<String>,
    }

    #[derive(Debug, Clone, Deserialize)]
    struct DeviceRegistrationRow {
        id: i64,
        license_key: String,
        #[serde(default)]
        device_id: String,
        #[serde(default)]
        device_fingerprint_hash: Option<String>,
        #[serde(default)]
        registered_at: Option<String>,
        #[serde(default)]
        last_seen_at: Option<String>,
        #[serde(default)]
        registration_status: Option<String>,
    }

    fn normalize_key(value: &str) -> String {
        value.trim().to_uppercase()
    }

    fn now_iso(now: DateTime<Utc>) -> String {
        now.to_rfc3339_opts(chrono::SecondsFormat::Secs, true)
    }

    fn sha256_hex(value: &str) -> String {
        format!("{:x}", Sha256::digest(value.as_bytes()))
    }

    fn missing_secret(name: &str) -> Result<Response> {
        Response::from_json(&serde_json::json!({
            "success": false,
            "message": format!("{name} 未配置"),
        }))
        .map(|resp| resp.with_status(503))
    }

    fn load_signer(env: &Env) -> anyhow::Result<LeaseTokenSigner> {
        let value = env.secret("LICENSE_SIGNING_PRIVATE_KEY_B64")?;
        LeaseTokenSigner::from_private_key_b64(&value.to_string())
    }

    fn compatibility_payload(path: &str) -> String {
        serde_json::json!({
            "success": false,
            "message": "rust_worker_repository_pending",
            "path": path,
        })
        .to_string()
    }

    async fn load_generated_key(
        db: &D1Database,
        license_key: &str,
    ) -> anyhow::Result<Option<GeneratedKeyRow>> {
        let stmt = db
            .prepare(
                "SELECT license_key, plan_days, status, created_at, note FROM generated_keys WHERE license_key = ? LIMIT 1",
            )
            .bind(&[JsValue::from_str(license_key)])?;
        let result = stmt.all().await?;
        let mut rows: Vec<GeneratedKeyRow> = result.results().unwrap_or_default();
        Ok(rows.pop())
    }

    async fn load_activation(
        db: &D1Database,
        license_key: &str,
    ) -> anyhow::Result<Option<ActivationRow>> {
        let stmt = db
            .prepare(
                "SELECT license_key, device_id, device_fingerprint, plan_days, activated_at, expires_at, updated_at, binding_version, status, last_verify_at, last_session_issued_at, last_offline_grant_issued_at FROM activations WHERE license_key = ? LIMIT 1",
            )
            .bind(&[JsValue::from_str(license_key)])?;
        let result = stmt.all().await?;
        let mut rows: Vec<ActivationRow> = result.results().unwrap_or_default();
        Ok(rows.pop())
    }

    async fn load_device_registration(
        db: &D1Database,
        license_key: &str,
        device_id: &str,
    ) -> anyhow::Result<Option<DeviceRegistrationRow>> {
        let stmt = db
            .prepare(
                "SELECT id, license_key, device_id, device_fingerprint_hash, registered_at, last_seen_at, registration_status FROM device_registrations WHERE license_key = ? AND device_id = ? LIMIT 1",
            )
            .bind(&[JsValue::from_str(license_key), JsValue::from_str(device_id)])?;
        let result = stmt.all().await?;
        let mut rows: Vec<DeviceRegistrationRow> = result.results().unwrap_or_default();
        Ok(rows.pop())
    }

    async fn append_audit(
        db: &D1Database,
        license_key: &str,
        device_id: &str,
        action: &str,
        reason: &str,
        now_iso: &str,
    ) -> anyhow::Result<()> {
        let sql = r#"
            INSERT INTO license_audit_logs
                (license_key, device_id, action, action_reason, created_at, operator, meta_json)
            VALUES (?, ?, ?, ?, ?, 'worker', '{}')
        "#;
        db.prepare(sql)
            .bind(&[
                JsValue::from_str(license_key),
                JsValue::from_str(device_id),
                JsValue::from_str(action),
                JsValue::from_str(reason),
                JsValue::from_str(now_iso),
            ])?
            .run()
            .await?;
        Ok(())
    }

    async fn update_activation_markers(
        db: &D1Database,
        license_key: &str,
        now_iso: &str,
        session_issued: bool,
        grant_issued: bool,
        new_status: Option<&str>,
    ) -> anyhow::Result<()> {
        let session_sql = if session_issued {
            ", last_session_issued_at = ?"
        } else {
            ""
        };
        let grant_sql = if grant_issued {
            ", last_offline_grant_issued_at = ?"
        } else {
            ""
        };
        let status_sql = if new_status.is_some() { ", status = ?" } else { "" };
        let sql = format!(
            "UPDATE activations SET updated_at = ?, last_verify_at = ?{session_sql}{grant_sql}{status_sql} WHERE license_key = ?"
        );
        let mut binds = vec![JsValue::from_str(now_iso), JsValue::from_str(now_iso)];
        if session_issued {
            binds.push(JsValue::from_str(now_iso));
        }
        if grant_issued {
            binds.push(JsValue::from_str(now_iso));
        }
        if let Some(status) = new_status {
            binds.push(JsValue::from_str(status));
        }
        binds.push(JsValue::from_str(license_key));
        db.prepare(&sql).bind(&binds)?.run().await?;
        Ok(())
    }

    async fn upsert_device_registration(
        db: &D1Database,
        license_key: &str,
        device_id: &str,
        device_fingerprint: &str,
        now_iso: &str,
    ) -> anyhow::Result<()> {
        let fingerprint_hash = sha256_hex(device_fingerprint);
        if let Some(existing) = load_device_registration(db, license_key, device_id).await? {
            let _ = (
                &existing.license_key,
                &existing.device_id,
                &existing.device_fingerprint_hash,
                &existing.registered_at,
                &existing.last_seen_at,
                &existing.registration_status,
            );
            db.prepare(
                "UPDATE device_registrations SET device_fingerprint_hash = ?, last_seen_at = ?, registration_status = 'active' WHERE id = ?",
            )
            .bind(&[
                JsValue::from_str(&fingerprint_hash),
                JsValue::from_str(now_iso),
                JsValue::from_f64(existing.id as f64),
            ])?
            .run()
            .await?;
            return Ok(());
        }

        db.prepare(
            "INSERT INTO device_registrations (license_key, device_id, device_fingerprint_hash, registered_at, last_seen_at, registration_status) VALUES (?, ?, ?, ?, ?, 'active')",
        )
        .bind(&[
            JsValue::from_str(license_key),
            JsValue::from_str(device_id),
            JsValue::from_str(&fingerprint_hash),
            JsValue::from_str(now_iso),
            JsValue::from_str(now_iso),
        ])?
        .run()
        .await?;
        Ok(())
    }

    fn build_license_lease_from_activation(row: &ActivationRow, now: DateTime<Utc>) -> LicenseLease {
        let _ = (
            &row.license_key,
            &row.device_fingerprint,
            row.plan_days,
            &row.updated_at,
            &row.last_verify_at,
            &row.last_session_issued_at,
            &row.last_offline_grant_issued_at,
        );
        let lease = license_service::issue_license_lease(
            &row.license_key,
            &row.device_id,
            LicenseState::Active,
            &row.expires_at,
            &(now + chrono::Duration::hours(license_service::LEASE_HARD_EXPIRY_HOURS))
                .to_rfc3339_opts(chrono::SecondsFormat::Secs, true),
            &(now + chrono::Duration::hours(license_service::LEASE_RENEWAL_HOURS))
                .to_rfc3339_opts(chrono::SecondsFormat::Secs, true),
            &now_iso(now),
        );
        LicenseLease {
            binding_version: row.binding_version,
            ..lease
        }
    }

    fn signed_success_response(
        message: &str,
        state: LicenseState,
        activation: &ActivationRow,
        signer: &LeaseTokenSigner,
        now: DateTime<Utc>,
    ) -> anyhow::Result<SignedLicenseApiResponse> {
        let lease = build_license_lease_from_activation(activation, now);
        let token = signer.sign_license_lease(&lease)?;
        Ok(SignedLicenseApiResponse {
            success: true,
            message: message.to_string(),
            license_state: state,
            license_lease: Some(token),
            license_expires_at: Some(activation.expires_at.clone()),
            activated_at: Some(activation.activated_at.clone()),
            device_id: Some(activation.device_id.clone()),
            license_key: Some(activation.license_key.clone()),
            lease_expires_at: Some(lease.lease_expires_at.clone()),
            renew_after: Some(lease.renew_after.clone()),
            issued_at: Some(lease.issued_at.clone()),
            license_status: Some(state),
            task_policy: Some(lease.task_policy.clone()),
        })
    }

    fn signed_failure_response(
        message: &str,
        state: LicenseState,
        activation: Option<&ActivationRow>,
    ) -> SignedLicenseApiResponse {
        SignedLicenseApiResponse {
            success: false,
            message: message.to_string(),
            license_state: state,
            license_lease: None,
            license_expires_at: activation.map(|value| value.expires_at.clone()),
            activated_at: activation.map(|value| value.activated_at.clone()),
            device_id: activation.map(|value| value.device_id.clone()),
            license_key: activation.map(|value| value.license_key.clone()),
            lease_expires_at: None,
            renew_after: None,
            issued_at: None,
            license_status: activation.map(|_| state),
            task_policy: None,
        }
    }

    async fn verify_license_for_runtime(
        db: &D1Database,
        license_key: &str,
        device_id: &str,
        now: DateTime<Utc>,
    ) -> anyhow::Result<Result<LicenseLease, String>> {
        let Some(key) = load_generated_key(db, license_key).await? else {
            return Ok(Err("该卡密已被吊销".into()));
        };
        let _ = (&key.created_at, &key.note);
        if key.status == "revoked" {
            return Ok(Err("该卡密已被吊销".into()));
        }
        let Some(row) = load_activation(db, license_key).await? else {
            return Ok(Err("该卡密尚未激活".into()));
        };
        if row.device_id != device_id {
            return Ok(Err("设备不匹配：该卡密已绑定其他设备".into()));
        }
        if row.status == "revoked" {
            return Ok(Err("该卡密已被吊销".into()));
        }
        let expires_at = DateTime::parse_from_rfc3339(&row.expires_at)?.with_timezone(&Utc);
        if now >= expires_at {
            let now_iso = now_iso(now);
            update_activation_markers(db, license_key, &now_iso, false, false, Some("expired"))
                .await?;
            append_audit(db, license_key, device_id, "verify", "expired", &now_iso).await?;
            return Ok(Err("授权已过期".into()));
        }
        Ok(Ok(build_license_lease_from_activation(&row, now)))
    }

    async fn handle_refresh_runtime(
        db: &D1Database,
        signer: &LeaseTokenSigner,
        input: LeaseRefreshRequest,
        now: DateTime<Utc>,
    ) -> anyhow::Result<LeaseRefreshResponse> {
        match verify_license_for_runtime(db, &input.license_key, &input.device_id, now).await? {
            Ok(lease) => {
                let now_iso = now.to_rfc3339_opts(chrono::SecondsFormat::Secs, true);
                update_activation_markers(db, &input.license_key, &now_iso, true, false, Some("active"))
                    .await?;
                append_audit(db, &input.license_key, &input.device_id, "lease_refresh", "ok", &now_iso)
                    .await?;
                Ok(LeaseRefreshResponse {
                    success: true,
                    message: "lease_refreshed".into(),
                    new_token: signer.sign_license_lease(&lease)?,
                })
            }
            Err(message) => Ok(LeaseRefreshResponse {
                success: false,
                message,
                new_token: String::new(),
            }),
        }
    }

    async fn handle_activate_runtime(
        db: &D1Database,
        signer: &LeaseTokenSigner,
        input: ActivationInput,
        now: DateTime<Utc>,
    ) -> anyhow::Result<SignedLicenseApiResponse> {
        let license_key = normalize_key(&input.license_key);
        let Some(key) = load_generated_key(db, &license_key).await? else {
            return Ok(signed_failure_response(
                "该卡密不存在或已被吊销",
                LicenseState::Revoked,
                None,
            ));
        };
        let _ = (&key.created_at, &key.note);
        if key.status == "revoked" {
            return Ok(signed_failure_response(
                "该卡密已被吊销，无法使用",
                LicenseState::Revoked,
                None,
            ));
        }
        if key.plan_days <= 0 {
            return Ok(signed_failure_response(
                "卡密无效：有效期异常",
                LicenseState::Invalid,
                None,
            ));
        }

        let now_iso_str = now_iso(now);
        let existing = load_activation(db, &license_key).await?;
        let was_reactivation = existing.is_some();
        let record = if let Some(existing) = existing {
            if existing.device_id != input.device_id {
                return Ok(signed_failure_response(
                    "该卡密已在其他设备激活，不允许更换设备。如需帮助请联系作者。",
                    LicenseState::DeviceMismatch,
                    Some(&existing),
                ));
            }
            db.prepare(
                "UPDATE activations SET device_fingerprint = ?, updated_at = ?, binding_version = ?, status = 'active', last_verify_at = ? WHERE license_key = ?",
            )
            .bind(&[
                JsValue::from_str(&input.device_fingerprint),
                JsValue::from_str(&now_iso_str),
                JsValue::from_f64(license_service::LICENSE_PROTOCOL_VERSION as f64),
                JsValue::from_str(&now_iso_str),
                JsValue::from_str(&license_key),
            ])?
            .run()
            .await?;
            load_activation(db, &license_key)
                .await?
                .ok_or_else(|| anyhow::anyhow!("激活更新后未找到记录"))?
        } else {
            let expires_at = now
                + chrono::Duration::days(key.plan_days as i64);
            db.prepare(
                "INSERT INTO activations (license_key, device_id, device_fingerprint, plan_days, activated_at, expires_at, updated_at, binding_version, status, last_verify_at, last_session_issued_at, last_offline_grant_issued_at) VALUES (?, ?, ?, ?, ?, ?, ?, ?, 'active', ?, '', '')",
            )
            .bind(&[
                JsValue::from_str(&license_key),
                JsValue::from_str(&input.device_id),
                JsValue::from_str(&input.device_fingerprint),
                JsValue::from_f64(key.plan_days as f64),
                JsValue::from_str(&now_iso_str),
                JsValue::from_str(&now_iso(expires_at)),
                JsValue::from_str(&now_iso_str),
                JsValue::from_f64(license_service::LICENSE_PROTOCOL_VERSION as f64),
                JsValue::from_str(&now_iso_str),
            ])?
            .run()
            .await?;
            db.prepare("UPDATE generated_keys SET status = 'activated' WHERE license_key = ?")
                .bind(&[JsValue::from_str(&license_key)])?
                .run()
                .await?;
            load_activation(db, &license_key)
                .await?
                .ok_or_else(|| anyhow::anyhow!("激活插入后未找到记录"))?
        };

        upsert_device_registration(
            db,
            &license_key,
            &input.device_id,
            &input.device_fingerprint,
            &now_iso_str,
        )
        .await?;
        let audit_reason = if input.client_version.is_empty() {
            "client_activate".to_string()
        } else {
            format!("client_activate:{}", input.client_version)
        };
        append_audit(
            db,
            &license_key,
            &input.device_id,
            "activate",
            &audit_reason,
            &now_iso_str,
        )
        .await?;

        Ok(signed_success_response(
            if was_reactivation { "重新激活成功" } else { "激活成功" },
            LicenseState::Active,
            &record,
            signer,
            now,
        )?)
    }

    async fn handle_verify_runtime(
        db: &D1Database,
        signer: &LeaseTokenSigner,
        input: VerifyInput,
        now: DateTime<Utc>,
    ) -> anyhow::Result<SignedLicenseApiResponse> {
        let license_key = normalize_key(&input.license_key);
        let Some(key) = load_generated_key(db, &license_key).await? else {
            return Ok(signed_failure_response(
                "该卡密已被吊销",
                LicenseState::Revoked,
                None,
            ));
        };
        let _ = (&key.created_at, &key.note);
        if key.status == "revoked" {
            return Ok(signed_failure_response(
                "该卡密已被吊销",
                LicenseState::Revoked,
                None,
            ));
        }

        let Some(record) = load_activation(db, &license_key).await? else {
            return Ok(signed_failure_response(
                "该卡密尚未激活",
                LicenseState::Invalid,
                None,
            ));
        };
        if record.device_id != input.device_id {
            return Ok(signed_failure_response(
                "设备不匹配：该卡密已绑定其他设备",
                LicenseState::DeviceMismatch,
                Some(&record),
            ));
        }
        if record.status == "revoked" {
            return Ok(signed_failure_response(
                "该卡密已被吊销",
                LicenseState::Revoked,
                Some(&record),
            ));
        }

        let now_iso_str = now_iso(now);
        let expires_at = DateTime::parse_from_rfc3339(&record.expires_at)?.with_timezone(&Utc);
        if now >= expires_at {
            update_activation_markers(db, &license_key, &now_iso_str, false, false, Some("expired"))
                .await?;
            append_audit(db, &license_key, &input.device_id, "verify", "expired", &now_iso_str)
                .await?;
            let expired = load_activation(db, &license_key)
                .await?
                .unwrap_or(record);
            return Ok(signed_failure_response(
                "授权已过期",
                LicenseState::Expired,
                Some(&expired),
            ));
        }

        update_activation_markers(db, &license_key, &now_iso_str, false, false, Some("active")).await?;
        let audit_reason = if input.client_version.is_empty() {
            "client_verify".to_string()
        } else {
            format!("client_verify:{}", input.client_version)
        };
        append_audit(
            db,
            &license_key,
            &input.device_id,
            "verify",
            &audit_reason,
            &now_iso_str,
        )
        .await?;
        let active = load_activation(db, &license_key)
            .await?
            .ok_or_else(|| anyhow::anyhow!("校验后未找到激活记录"))?;
        Ok(signed_success_response(
            "授权有效",
            LicenseState::Active,
            &active,
            signer,
            now,
        )?)
    }

    async fn handle_remote_task_authorize(
        db: &D1Database,
        input: TaskAuthorizeRequest,
        now: DateTime<Utc>,
    ) -> anyhow::Result<RuntimeGrant> {
        match verify_license_for_runtime(db, &input.license_key, &input.device_id, now).await? {
            Ok(lease) => {
                let payload = lease_to_payload(&lease)?;
                let mut grant = match authorize_task_local(
                    &payload,
                    &input.task_type,
                    now.timestamp(),
                    next_grant_id,
                ) {
                    Ok(grant) => grant,
                    Err(err) => return Ok(denied_grant(&input.task_type, err.to_string())),
                };
                grant.risk_level = task_risk_level(&input.task_type);
                let now_iso = now.to_rfc3339_opts(chrono::SecondsFormat::Secs, true);
                update_activation_markers(db, &input.license_key, &now_iso, false, true, Some("active"))
                    .await?;
                append_audit(
                    db,
                    &input.license_key,
                    &input.device_id,
                    "task_authorize",
                    &input.task_type,
                    &now_iso,
                )
                .await?;
                Ok(grant)
            }
            Err(message) => Ok(denied_grant(&input.task_type, message)),
        }
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

        let body = req.text().await.unwrap_or_default();
        match parse_route(&path) {
            WorkerRoute::Activate => {
                let signer = match load_signer(&env) {
                    Ok(signer) => signer,
                    Err(_) => return missing_secret("LICENSE_SIGNING_PRIVATE_KEY_B64"),
                };
                let db = env.d1("DB")?;
                let input: ActivationInput = serde_json::from_str(&body)
                    .map_err(|e| worker::Error::RustError(e.to_string()))?;
                let resp = handle_activate_runtime(&db, &signer, input, Utc::now())
                    .await
                    .map_err(|e| worker::Error::RustError(e.to_string()))?;
                Response::from_json(&resp)
            }
            WorkerRoute::Verify => {
                let signer = match load_signer(&env) {
                    Ok(signer) => signer,
                    Err(_) => return missing_secret("LICENSE_SIGNING_PRIVATE_KEY_B64"),
                };
                let db = env.d1("DB")?;
                let input: VerifyInput = serde_json::from_str(&body)
                    .map_err(|e| worker::Error::RustError(e.to_string()))?;
                let resp = handle_verify_runtime(&db, &signer, input, Utc::now())
                    .await
                    .map_err(|e| worker::Error::RustError(e.to_string()))?;
                Response::from_json(&resp)
            }
            WorkerRoute::LeaseRefresh => {
                let signer = match load_signer(&env) {
                    Ok(signer) => signer,
                    Err(_) => return missing_secret("LICENSE_SIGNING_PRIVATE_KEY_B64"),
                };
                let db = env.d1("DB")?;
                let input: LeaseRefreshRequest = serde_json::from_str(&body)
                    .map_err(|e| worker::Error::RustError(e.to_string()))?;
                let resp = handle_refresh_runtime(&db, &signer, input, Utc::now())
                    .await
                    .map_err(|e| worker::Error::RustError(e.to_string()))?;
                Response::from_json(&resp)
            }
            WorkerRoute::TaskAuthorize => {
                let db = env.d1("DB")?;
                let input: TaskAuthorizeRequest = serde_json::from_str(&body)
                    .map_err(|e| worker::Error::RustError(e.to_string()))?;
                let resp = handle_remote_task_authorize(&db, input, Utc::now())
                    .await
                    .map_err(|e| worker::Error::RustError(e.to_string()))?;
                Response::from_json(&resp)
            }
            WorkerRoute::LeaseRevoke => {
                // 管理员吊销仍走后台管理接口；公开 API 先保留占位响应。
                Response::from_json(&serde_json::json!({
                    "success": false,
                    "message": "lease_revoke_pending",
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
    use ed25519_dalek::pkcs8::EncodePrivateKey;
    use license_service::LeaseVerifier;
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

    fn test_signer() -> LeaseTokenSigner {
        let signing_key = SigningKey::from_bytes(&[7u8; 32]);
        let der = signing_key.to_pkcs8_der().unwrap();
        LeaseTokenSigner::from_private_key_b64(&STANDARD.encode(der.as_bytes())).unwrap()
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
        let signer = test_signer();
        let response = handle_json_request(
            &service,
            "/api/activate",
            r#"{"license_key":"TLS-TEST","device_id":"device-1","device_fingerprint":"fp-1","client_version":"4.3.0"}"#,
            &signer,
        )
        .unwrap();
        let payload: SignedLicenseApiResponse = serde_json::from_str(&response).unwrap();
        assert!(payload.success);
        assert_eq!(payload.license_state, LicenseState::Active);
        assert!(payload.license_lease.is_some());
        let verifier = LeaseVerifier::from_public_key_b64(&signer.public_key_b64()).unwrap();
        let verified = verifier
            .verify(
                payload.license_lease.as_deref().unwrap(),
                Some("device-1"),
                Utc::now().timestamp(),
                false,
            )
            .unwrap();
        assert_eq!(verified.device_id, "device-1");
    }

    #[test]
    fn lease_refresh_returns_signed_new_token() {
        let service = LicenseService::new(Repo::seeded());
        let signer = test_signer();
        let _ = handle_json_request(
            &service,
            "/api/activate",
            r#"{"license_key":"TLS-TEST","device_id":"device-1","device_fingerprint":"fp-1","client_version":"4.3.0"}"#,
            &signer,
        )
        .unwrap();
        let resp = handle_json_request(
            &service,
            "/api/lease/refresh",
            r#"{"license_key":"TLS-TEST","device_id":"device-1","current_issued_at":1700000000}"#,
            &signer,
        )
        .unwrap();
        let payload: LeaseRefreshResponse = serde_json::from_str(&resp).unwrap();
        assert!(payload.success);
        assert_eq!(payload.message, "lease_refreshed");
        assert!(!payload.new_token.is_empty());
        let verifier = LeaseVerifier::from_public_key_b64(&signer.public_key_b64()).unwrap();
        let verified = verifier
            .verify(&payload.new_token, Some("device-1"), Utc::now().timestamp(), false)
            .unwrap();
        assert_eq!(verified.license_key, "TLS-TEST");
    }

    #[test]
    fn handles_verify_json_with_signed_lease() {
        let service = LicenseService::new(Repo::seeded());
        let signer = test_signer();
        let _ = handle_json_request(
            &service,
            "/api/activate",
            r#"{"license_key":"TLS-TEST","device_id":"device-1","device_fingerprint":"fp-1","client_version":"4.3.0"}"#,
            &signer,
        )
        .unwrap();
        let response = handle_json_request(
            &service,
            "/api/verify",
            r#"{"license_key":"TLS-TEST","device_id":"device-1","client_version":"5.1.0"}"#,
            &signer,
        )
        .unwrap();
        let payload: SignedLicenseApiResponse = serde_json::from_str(&response).unwrap();
        assert!(payload.success);
        assert_eq!(payload.license_state, LicenseState::Active);
        assert!(payload.license_lease.is_some());
    }

    #[test]
    fn lease_revoke_route_returns_pending_marker() {
        let service = LicenseService::new(Repo::seeded());
        let signer = test_signer();
        let resp = handle_json_request(
            &service,
            "/api/lease/revoke",
            r#"{"license_key":"TLS-TEST","device_id":"device-1","reason":"admin"}"#,
            &signer,
        )
        .unwrap();
        let payload: Value = serde_json::from_str(&resp).unwrap();
        assert_eq!(payload["message"], "lease_revoke_pending");
    }

    #[test]
    fn task_authorize_route_returns_runtime_grant() {
        let service = LicenseService::new(Repo::seeded());
        let signer = test_signer();
        let _ = handle_json_request(
            &service,
            "/api/activate",
            r#"{"license_key":"TLS-TEST","device_id":"device-1","device_fingerprint":"fp-1","client_version":"4.3.0"}"#,
            &signer,
        )
        .unwrap();
        let resp = handle_json_request(
            &service,
            "/api/task/authorize",
            r#"{"license_key":"TLS-TEST","device_id":"device-1","task_type":"review_find","client_version":"5.1.0"}"#,
            &signer,
        )
        .unwrap();
        let payload: RuntimeGrant = serde_json::from_str(&resp).unwrap();
        assert!(payload.granted);
        assert_eq!(payload.task_type, "review_find");
        assert!(!payload.grant_id.is_empty());
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
