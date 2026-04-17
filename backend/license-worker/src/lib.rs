use api_contracts::{
    LeasePayload, LicenseLease, LicenseState, RiskLevel, RuntimeGrant, LEASE_KIND_LICENSE,
    LICENSE_TASK_BATCH_DELIVERY, LICENSE_TASK_CACHE_MANAGE, LICENSE_TASK_QUALITY_REFUND,
    LICENSE_TASK_REVIEW_FIND, LICENSE_TASK_REVIEW_FULL_SCAN,
};
use async_trait::async_trait;
use base64::engine::general_purpose::{STANDARD, URL_SAFE_NO_PAD};
use base64::Engine;
use chrono::{DateTime, Utc};
use ed25519_dalek::pkcs8::DecodePrivateKey;
use ed25519_dalek::{Signer, SigningKey};
use license_service::{
    authorize_task_local, ActivationInput, AuditEvent, DeviceRegistration, GeneratedKeyRecord,
    GeneratedKeyStatus, LicenseRecord, VerifyInput,
};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use sha2::{Digest, Sha256};
use std::sync::atomic::{AtomicU64, Ordering};

static NEXT_GRANT_SEQ: AtomicU64 = AtomicU64::new(1);
pub const WORKER_RUNTIME_ERROR_MESSAGE: &str = "worker_runtime_error";

#[cfg(target_arch = "wasm32")]
mod admin_d1;

/// Worker 支持的路由枚举。
///
/// 新增路由必须同时更新 `parse_route` / `route_request` / `handle_async_runtime_json`
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

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct AdminRevokeRequest {
    pub key: String,
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
    Ok(DateTime::parse_from_rfc3339(value)?
        .with_timezone(&Utc)
        .timestamp())
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

#[async_trait(?Send)]
pub trait AsyncRuntimeRepository {
    async fn load_generated_key(
        &self,
        license_key: &str,
    ) -> anyhow::Result<Option<GeneratedKeyRecord>>;
    async fn save_generated_key(&self, record: &GeneratedKeyRecord) -> anyhow::Result<()>;
    async fn load_license(&self, license_key: &str) -> anyhow::Result<Option<LicenseRecord>>;
    async fn save_license(&self, record: &LicenseRecord) -> anyhow::Result<()>;
    async fn load_device_registration(
        &self,
        license_key: &str,
        device_id: &str,
    ) -> anyhow::Result<Option<DeviceRegistration>>;
    async fn save_device_registration(&self, record: &DeviceRegistration) -> anyhow::Result<()>;
    async fn append_audit_event(&self, event: &AuditEvent) -> anyhow::Result<()>;
    async fn update_runtime_markers(
        &self,
        license_key: &str,
        now_iso: &str,
        session_issued: bool,
        grant_issued: bool,
        new_status: Option<LicenseState>,
    ) -> anyhow::Result<()>;
    async fn revoke_license(
        &self,
        license_key: &str,
        device_id: &str,
        reason: &str,
        revoked_at: &str,
    ) -> anyhow::Result<bool>;
    async fn revoke_license_by_key(
        &self,
        license_key: &str,
        reason: &str,
        revoked_at: &str,
    ) -> anyhow::Result<bool>;
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

fn issue_license_lease_for_record(record: &LicenseRecord, now: DateTime<Utc>) -> LicenseLease {
    let lease = license_service::issue_license_lease(
        &record.license_key,
        &record.device_id,
        record.status,
        &record.license_expires_at,
        &(now + chrono::Duration::hours(license_service::LEASE_HARD_EXPIRY_HOURS))
            .to_rfc3339_opts(chrono::SecondsFormat::Secs, true),
        &(now + chrono::Duration::hours(license_service::LEASE_RENEWAL_HOURS))
            .to_rfc3339_opts(chrono::SecondsFormat::Secs, true),
        &now_iso(now),
    );
    LicenseLease {
        binding_version: record.binding_version,
        ..lease
    }
}

fn signed_success_response_for_record(
    message: &str,
    state: LicenseState,
    record: &LicenseRecord,
    signer: &LeaseTokenSigner,
    now: DateTime<Utc>,
) -> anyhow::Result<SignedLicenseApiResponse> {
    let lease = issue_license_lease_for_record(record, now);
    let token = signer.sign_license_lease(&lease)?;
    Ok(SignedLicenseApiResponse {
        success: true,
        message: message.to_string(),
        license_state: state,
        license_lease: Some(token),
        license_expires_at: Some(record.license_expires_at.clone()),
        activated_at: Some(record.activated_at.clone()),
        device_id: Some(record.device_id.clone()),
        license_key: Some(record.license_key.clone()),
        lease_expires_at: Some(lease.lease_expires_at.clone()),
        renew_after: Some(lease.renew_after.clone()),
        issued_at: Some(lease.issued_at.clone()),
        license_status: Some(state),
        task_policy: Some(lease.task_policy.clone()),
    })
}

fn signed_failure_response_for_record(
    message: &str,
    state: LicenseState,
    record: Option<&LicenseRecord>,
) -> SignedLicenseApiResponse {
    SignedLicenseApiResponse {
        success: false,
        message: message.to_string(),
        license_state: state,
        license_lease: None,
        license_expires_at: record.map(|value| value.license_expires_at.clone()),
        activated_at: record.map(|value| value.activated_at.clone()),
        device_id: record.map(|value| value.device_id.clone()),
        license_key: record.map(|value| value.license_key.clone()),
        lease_expires_at: None,
        renew_after: None,
        issued_at: None,
        license_status: record.map(|_| state),
        task_policy: None,
    }
}

fn looks_like_invalid_json_error(message: &str) -> bool {
    message.starts_with("missing field")
        || message.starts_with("expected")
        || message.contains("EOF while parsing")
        || message.contains("trailing characters")
        || message.contains("expected value")
}

pub fn revoke_error_contract(message: &str) -> (u16, &'static str) {
    match message {
        "empty_key" => (400, "empty_key"),
        "not_found" => (404, "not_found"),
        "unauthorized" => (401, "unauthorized"),
        "secret_missing" => (503, "secret_missing"),
        _ if looks_like_invalid_json_error(message) => (400, "invalid_json"),
        _ => (500, "revoke_failed"),
    }
}

pub fn revoke_response_status(payload: &SignedLicenseApiResponse) -> u16 {
    if payload.success {
        200
    } else {
        revoke_error_contract(&payload.message).0
    }
}

pub fn admin_auth_error_contract(
    secret_configured: bool,
    provided_secret_matches: bool,
) -> Option<(u16, &'static str)> {
    if !secret_configured {
        Some((503, "secret_missing"))
    } else if !provided_secret_matches {
        Some((401, "unauthorized"))
    } else {
        None
    }
}

async fn runtime_upsert_device_registration<R: AsyncRuntimeRepository + ?Sized>(
    repo: &R,
    license_key: &str,
    device_id: &str,
    device_fingerprint: &str,
    now: DateTime<Utc>,
) -> anyhow::Result<()> {
    let now_iso_str = now_iso(now);
    let hash = sha256_hex(device_fingerprint);
    let mut registration = repo
        .load_device_registration(license_key, device_id)
        .await?
        .unwrap_or(DeviceRegistration {
            license_key: license_key.to_string(),
            device_id: device_id.to_string(),
            device_fingerprint_hash: hash.clone(),
            registered_at: now_iso_str.clone(),
            last_seen_at: now_iso_str.clone(),
            registration_status: "active".into(),
        });
    registration.device_fingerprint_hash = hash;
    registration.last_seen_at = now_iso_str;
    registration.registration_status = "active".into();
    repo.save_device_registration(&registration).await
}

async fn runtime_load_usable_license<R: AsyncRuntimeRepository + ?Sized>(
    repo: &R,
    license_key: &str,
    device_id: &str,
    now: DateTime<Utc>,
    expired_audit_action: &str,
) -> anyhow::Result<Result<LicenseRecord, (String, LicenseState, Option<LicenseRecord>)>> {
    let normalized_key = normalize_key(license_key);
    let Some(key_record) = repo.load_generated_key(&normalized_key).await? else {
        return Ok(Err(("该卡密已被吊销".into(), LicenseState::Revoked, None)));
    };
    if key_record.status == GeneratedKeyStatus::Revoked {
        return Ok(Err(("该卡密已被吊销".into(), LicenseState::Revoked, None)));
    }

    let Some(mut record) = repo.load_license(&normalized_key).await? else {
        return Ok(Err(("该卡密尚未激活".into(), LicenseState::Invalid, None)));
    };
    if record.device_id != device_id {
        return Ok(Err((
            "设备不匹配：该卡密已绑定其他设备".into(),
            LicenseState::DeviceMismatch,
            Some(record),
        )));
    }
    if record.status == LicenseState::Revoked {
        return Ok(Err((
            "该卡密已被吊销".into(),
            LicenseState::Revoked,
            Some(record),
        )));
    }

    let expires_at = DateTime::parse_from_rfc3339(&record.license_expires_at)?.with_timezone(&Utc);
    if now >= expires_at {
        let now_iso_str = now_iso(now);
        record.status = LicenseState::Expired;
        record.updated_at = now_iso_str.clone();
        record.last_verify_at = now_iso_str.clone();
        repo.save_license(&record).await?;
        repo.append_audit_event(&AuditEvent {
            action: expired_audit_action.to_string(),
            license_key: normalized_key,
            device_id: device_id.to_string(),
            reason: "expired".into(),
            created_at: now_iso_str,
        })
        .await?;
        return Ok(Err((
            "授权已过期".into(),
            LicenseState::Expired,
            Some(record),
        )));
    }

    Ok(Ok(record))
}

pub async fn runtime_activate<R: AsyncRuntimeRepository + ?Sized>(
    repo: &R,
    signer: &LeaseTokenSigner,
    input: ActivationInput,
    now: DateTime<Utc>,
) -> anyhow::Result<SignedLicenseApiResponse> {
    let normalized_key = normalize_key(&input.license_key);
    let Some(mut key_record) = repo.load_generated_key(&normalized_key).await? else {
        return Ok(signed_failure_response_for_record(
            "该卡密不存在或已被吊销",
            LicenseState::Revoked,
            None,
        ));
    };
    if key_record.status == GeneratedKeyStatus::Revoked {
        return Ok(signed_failure_response_for_record(
            "该卡密已被吊销，无法使用",
            LicenseState::Revoked,
            None,
        ));
    }
    if key_record.plan_days == 0 {
        return Ok(signed_failure_response_for_record(
            "卡密无效：有效期异常",
            LicenseState::Invalid,
            None,
        ));
    }

    let now_iso_str = now_iso(now);
    let existing = repo.load_license(&normalized_key).await?;
    let (record, message) = if let Some(mut record) = existing {
        if record.device_id != input.device_id {
            return Ok(signed_failure_response_for_record(
                "该卡密已在其他设备激活，不允许更换设备。如需帮助请联系作者。",
                LicenseState::DeviceMismatch,
                Some(&record),
            ));
        }
        record.device_fingerprint = input.device_fingerprint.clone();
        record.updated_at = now_iso_str.clone();
        record.binding_version = license_service::LICENSE_PROTOCOL_VERSION;
        record.status = LicenseState::Active;
        record.last_verify_at = now_iso_str.clone();
        repo.save_license(&record).await?;
        (record, "重新激活成功")
    } else {
        let record = LicenseRecord {
            license_key: normalized_key.clone(),
            device_id: input.device_id.clone(),
            device_fingerprint: input.device_fingerprint.clone(),
            plan_days: key_record.plan_days,
            activated_at: now_iso_str.clone(),
            license_expires_at: now_iso(now + chrono::Duration::days(key_record.plan_days as i64)),
            updated_at: now_iso_str.clone(),
            binding_version: license_service::LICENSE_PROTOCOL_VERSION,
            status: LicenseState::Active,
            last_verify_at: now_iso_str.clone(),
        };
        repo.save_license(&record).await?;
        key_record.status = GeneratedKeyStatus::Activated;
        repo.save_generated_key(&key_record).await?;
        (record, "激活成功")
    };

    runtime_upsert_device_registration(
        repo,
        &normalized_key,
        &input.device_id,
        &input.device_fingerprint,
        now,
    )
    .await?;
    repo.append_audit_event(&AuditEvent {
        action: "activate".into(),
        license_key: normalized_key,
        device_id: input.device_id,
        reason: if input.client_version.is_empty() {
            "client_activate".into()
        } else {
            format!("client_activate:{}", input.client_version)
        },
        created_at: now_iso_str,
    })
    .await?;

    signed_success_response_for_record(message, LicenseState::Active, &record, signer, now)
}

pub async fn runtime_verify<R: AsyncRuntimeRepository + ?Sized>(
    repo: &R,
    signer: &LeaseTokenSigner,
    input: VerifyInput,
    now: DateTime<Utc>,
) -> anyhow::Result<SignedLicenseApiResponse> {
    let normalized_key = normalize_key(&input.license_key);
    let mut record =
        match runtime_load_usable_license(repo, &normalized_key, &input.device_id, now, "verify")
            .await?
        {
            Ok(record) => record,
            Err((message, state, record)) => {
                return Ok(signed_failure_response_for_record(
                    &message,
                    state,
                    record.as_ref(),
                ))
            }
        };

    let now_iso_str = now_iso(now);
    record.status = LicenseState::Active;
    record.updated_at = now_iso_str.clone();
    record.last_verify_at = now_iso_str.clone();
    repo.save_license(&record).await?;
    repo.append_audit_event(&AuditEvent {
        action: "verify".into(),
        license_key: normalized_key,
        device_id: input.device_id,
        reason: if input.client_version.is_empty() {
            "client_verify".into()
        } else {
            format!("client_verify:{}", input.client_version)
        },
        created_at: now_iso_str,
    })
    .await?;

    signed_success_response_for_record("授权有效", LicenseState::Active, &record, signer, now)
}

pub async fn runtime_refresh_lease<R: AsyncRuntimeRepository + ?Sized>(
    repo: &R,
    signer: &LeaseTokenSigner,
    input: LeaseRefreshRequest,
    now: DateTime<Utc>,
) -> anyhow::Result<LeaseRefreshResponse> {
    let record = match runtime_load_usable_license(
        repo,
        &input.license_key,
        &input.device_id,
        now,
        "verify",
    )
    .await?
    {
        Ok(record) => record,
        Err((message, _, _)) => {
            return Ok(LeaseRefreshResponse {
                success: false,
                message,
                new_token: String::new(),
            })
        }
    };
    let now_iso_str = now_iso(now);
    repo.update_runtime_markers(
        &record.license_key,
        &now_iso_str,
        true,
        false,
        Some(LicenseState::Active),
    )
    .await?;
    repo.append_audit_event(&AuditEvent {
        action: "lease_refresh".into(),
        license_key: record.license_key.clone(),
        device_id: input.device_id,
        reason: "ok".into(),
        created_at: now_iso_str,
    })
    .await?;
    Ok(LeaseRefreshResponse {
        success: true,
        message: "lease_refreshed".into(),
        new_token: signer.sign_license_lease(&issue_license_lease_for_record(&record, now))?,
    })
}

pub async fn runtime_task_authorize<R: AsyncRuntimeRepository + ?Sized>(
    repo: &R,
    input: TaskAuthorizeRequest,
    now: DateTime<Utc>,
) -> anyhow::Result<RuntimeGrant> {
    let record = match runtime_load_usable_license(
        repo,
        &input.license_key,
        &input.device_id,
        now,
        "verify",
    )
    .await?
    {
        Ok(record) => record,
        Err((message, _, _)) => return Ok(denied_grant(&input.task_type, message)),
    };
    let payload = lease_to_payload(&issue_license_lease_for_record(&record, now))?;
    let mut grant =
        match authorize_task_local(&payload, &input.task_type, now.timestamp(), next_grant_id) {
            Ok(grant) => grant,
            Err(err) => return Ok(denied_grant(&input.task_type, err.to_string())),
        };
    grant.risk_level = task_risk_level(&input.task_type);
    let now_iso_str = now_iso(now);
    repo.update_runtime_markers(
        &record.license_key,
        &now_iso_str,
        false,
        true,
        Some(LicenseState::Active),
    )
    .await?;
    repo.append_audit_event(&AuditEvent {
        action: "task_authorize".into(),
        license_key: record.license_key,
        device_id: input.device_id,
        reason: input.task_type,
        created_at: now_iso_str,
    })
    .await?;
    Ok(grant)
}

pub async fn runtime_revoke<R: AsyncRuntimeRepository + ?Sized>(
    repo: &R,
    input: LeaseRevokeRequest,
    now: DateTime<Utc>,
) -> anyhow::Result<SignedLicenseApiResponse> {
    let normalized_key = normalize_key(&input.license_key);
    if normalized_key.is_empty() {
        return Ok(signed_failure_response_for_record(
            "empty_key",
            LicenseState::Invalid,
            None,
        ));
    }
    let now_iso_str = now_iso(now);
    let reason = if input.reason.trim().is_empty() {
        "admin_revoke".to_string()
    } else {
        input.reason.trim().to_string()
    };

    let existed = repo
        .revoke_license(&normalized_key, &input.device_id, &reason, &now_iso_str)
        .await?;
    if !existed {
        return Ok(signed_failure_response_for_record(
            "not_found",
            LicenseState::NotFound,
            None,
        ));
    }

    repo.append_audit_event(&AuditEvent {
        action: "lease_revoke".into(),
        license_key: normalized_key.clone(),
        device_id: input.device_id,
        reason,
        created_at: now_iso_str,
    })
    .await?;

    let record = repo.load_license(&normalized_key).await?;
    Ok(SignedLicenseApiResponse {
        success: true,
        message: "license_revoked".into(),
        license_state: LicenseState::Revoked,
        license_lease: None,
        license_expires_at: record
            .as_ref()
            .map(|value| value.license_expires_at.clone()),
        activated_at: record.as_ref().map(|value| value.activated_at.clone()),
        device_id: record.as_ref().map(|value| value.device_id.clone()),
        license_key: Some(normalized_key),
        lease_expires_at: None,
        renew_after: None,
        issued_at: None,
        license_status: Some(LicenseState::Revoked),
        task_policy: None,
    })
}

pub async fn handle_admin_revoke_json<R: AsyncRuntimeRepository + ?Sized>(
    repo: &R,
    body: &str,
    now: DateTime<Utc>,
) -> anyhow::Result<String> {
    let request: AdminRevokeRequest = serde_json::from_str(body)?;
    let normalized_key = normalize_key(&request.key);
    if normalized_key.is_empty() {
        anyhow::bail!("empty_key");
    }
    let record = repo
        .load_license(&normalized_key)
        .await?
        .map(Some)
        .unwrap_or(None);
    let device_id = record
        .as_ref()
        .map(|value| value.device_id.clone())
        .unwrap_or_default();
    let now_iso_str = now_iso(now);
    let existed = repo
        .revoke_license_by_key(&normalized_key, "admin_revoke", &now_iso_str)
        .await?;
    let payload = if !existed {
        signed_failure_response_for_record("not_found", LicenseState::NotFound, None)
    } else {
        repo.append_audit_event(&AuditEvent {
            action: "lease_revoke".into(),
            license_key: normalized_key.clone(),
            device_id,
            reason: "admin_revoke".into(),
            created_at: now_iso_str,
        })
        .await?;
        let refreshed = repo.load_license(&normalized_key).await?;
        SignedLicenseApiResponse {
            success: true,
            message: "license_revoked".into(),
            license_state: LicenseState::Revoked,
            license_lease: None,
            license_expires_at: refreshed
                .as_ref()
                .map(|value| value.license_expires_at.clone()),
            activated_at: refreshed.as_ref().map(|value| value.activated_at.clone()),
            device_id: refreshed.as_ref().map(|value| value.device_id.clone()),
            license_key: Some(normalized_key),
            lease_expires_at: None,
            renew_after: None,
            issued_at: None,
            license_status: Some(LicenseState::Revoked),
            task_policy: None,
        }
    };
    Ok(serde_json::to_string(&payload)?)
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

pub fn route_requires_signer(route: WorkerRoute) -> bool {
    matches!(
        route,
        WorkerRoute::Activate | WorkerRoute::Verify | WorkerRoute::LeaseRefresh
    )
}

pub async fn handle_async_runtime_json<R: AsyncRuntimeRepository + ?Sized>(
    repo: &R,
    path: &str,
    body: &str,
    signer: Option<&LeaseTokenSigner>,
    now: DateTime<Utc>,
) -> anyhow::Result<String> {
    let route = parse_route(path);
    let payload: Value = serde_json::from_str(body)?;
    match route {
        WorkerRoute::Activate => {
            let input: ActivationInput = serde_json::from_value(payload)?;
            let signer = signer.ok_or_else(|| anyhow::anyhow!("missing signer for activate"))?;
            Ok(serde_json::to_string(
                &runtime_activate(repo, signer, input, now).await?,
            )?)
        }
        WorkerRoute::Verify => {
            let input: VerifyInput = serde_json::from_value(payload)?;
            let signer = signer.ok_or_else(|| anyhow::anyhow!("missing signer for verify"))?;
            Ok(serde_json::to_string(
                &runtime_verify(repo, signer, input, now).await?,
            )?)
        }
        WorkerRoute::LeaseRefresh => {
            let input: LeaseRefreshRequest = serde_json::from_value(payload)?;
            let signer =
                signer.ok_or_else(|| anyhow::anyhow!("missing signer for lease_refresh"))?;
            Ok(serde_json::to_string(
                &runtime_refresh_lease(repo, signer, input, now).await?,
            )?)
        }
        WorkerRoute::LeaseRevoke => {
            let input: LeaseRevokeRequest = serde_json::from_value(payload)?;
            Ok(serde_json::to_string(
                &runtime_revoke(repo, input, now).await?,
            )?)
        }
        WorkerRoute::TaskAuthorize => {
            let input: TaskAuthorizeRequest = serde_json::from_value(payload)?;
            Ok(serde_json::to_string(
                &runtime_task_authorize(repo, input, now).await?,
            )?)
        }
        WorkerRoute::NotFound => {
            let resp = SignedLicenseApiResponse {
                success: false,
                message: "not_found".into(),
                license_state: LicenseState::Invalid,
                license_lease: None,
                license_expires_at: None,
                activated_at: None,
                device_id: None,
                license_key: None,
                lease_expires_at: None,
                renew_after: None,
                issued_at: None,
                license_status: None,
                task_policy: None,
            };
            Ok(serde_json::to_string(&resp)?)
        }
    }
}

#[cfg(target_arch = "wasm32")]
mod cloudflare_entry {
    use super::*;
    use wasm_bindgen::JsValue;
    use worker::{event, D1Database, Env, Method, Request, Response, Result};

    fn missing_secret(name: &str) -> Result<Response> {
        Response::from_json(&serde_json::json!({
            "success": false,
            "message": format!("{name} 未配置"),
        }))
        .map(|resp| resp.with_status(503))
    }

    fn response_from_revoke_payload(payload: &str) -> Result<Response> {
        let value: SignedLicenseApiResponse =
            serde_json::from_str(payload).map_err(worker_error)?;
        Response::from_json(&value).map(|resp| resp.with_status(revoke_response_status(&value)))
    }

    fn load_signer(env: &Env) -> anyhow::Result<LeaseTokenSigner> {
        let value = env.secret("LICENSE_SIGNING_PRIVATE_KEY_B64")?;
        LeaseTokenSigner::from_private_key_b64(&value.to_string())
    }

    fn compatibility_payload(path: &str) -> String {
        serde_json::json!({
            "success": false,
            "message": WORKER_RUNTIME_ERROR_MESSAGE,
            "path": path,
        })
        .to_string()
    }

    pub(crate) struct D1RuntimeRepo<'a> {
        db: &'a D1Database,
    }

    impl<'a> D1RuntimeRepo<'a> {
        pub(crate) fn new(db: &'a D1Database) -> Self {
            Self { db }
        }
    }

    fn enum_text<T: serde::Serialize>(value: &T) -> anyhow::Result<String> {
        Ok(serde_json::to_string(value)?.trim_matches('"').to_string())
    }

    #[async_trait(?Send)]
    impl AsyncRuntimeRepository for D1RuntimeRepo<'_> {
        async fn load_generated_key(
            &self,
            license_key: &str,
        ) -> anyhow::Result<Option<GeneratedKeyRecord>> {
            let stmt = self
                .db
                .prepare(
                    "SELECT license_key, CAST(plan_days AS INTEGER) AS plan_days, status, COALESCE(created_at, '') AS created_at, COALESCE(note, '') AS note FROM generated_keys WHERE license_key = ? LIMIT 1",
                )
                .bind(&[JsValue::from_str(license_key)])?;
            let result = stmt.all().await?;
            let mut rows: Vec<GeneratedKeyRecord> = result.results().unwrap_or_default();
            Ok(rows.pop())
        }

        async fn save_generated_key(&self, record: &GeneratedKeyRecord) -> anyhow::Result<()> {
            self.db
                .prepare(
                    "INSERT INTO generated_keys (license_key, plan_days, status, created_at, note) VALUES (?, ?, ?, ?, ?) \
                     ON CONFLICT(license_key) DO UPDATE SET plan_days = excluded.plan_days, status = excluded.status, created_at = excluded.created_at, note = excluded.note",
                )
                .bind(&[
                    JsValue::from_str(&record.license_key),
                    JsValue::from_f64(record.plan_days as f64),
                    JsValue::from_str(&enum_text(&record.status)?),
                    JsValue::from_str(&record.created_at),
                    JsValue::from_str(&record.note),
                ])?
                .run()
                .await?;
            Ok(())
        }

        async fn load_license(&self, license_key: &str) -> anyhow::Result<Option<LicenseRecord>> {
            let stmt = self
                .db
                .prepare(
                    "SELECT license_key, device_id, COALESCE(device_fingerprint, '') AS device_fingerprint, CAST(plan_days AS INTEGER) AS plan_days, activated_at, expires_at AS license_expires_at, updated_at, binding_version, status, COALESCE(last_verify_at, '') AS last_verify_at FROM activations WHERE license_key = ? LIMIT 1",
                )
                .bind(&[JsValue::from_str(license_key)])?;
            let result = stmt.all().await?;
            let mut rows: Vec<LicenseRecord> = result.results().unwrap_or_default();
            Ok(rows.pop())
        }

        async fn save_license(&self, record: &LicenseRecord) -> anyhow::Result<()> {
            self.db
                .prepare(
                    "INSERT INTO activations (license_key, device_id, device_fingerprint, plan_days, activated_at, expires_at, updated_at, binding_version, status, last_verify_at, last_session_issued_at, last_offline_grant_issued_at) \
                     VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, COALESCE((SELECT last_session_issued_at FROM activations WHERE license_key = ?), ''), COALESCE((SELECT last_offline_grant_issued_at FROM activations WHERE license_key = ?), '')) \
                     ON CONFLICT(license_key) DO UPDATE SET device_id = excluded.device_id, device_fingerprint = excluded.device_fingerprint, plan_days = excluded.plan_days, activated_at = excluded.activated_at, expires_at = excluded.expires_at, updated_at = excluded.updated_at, binding_version = excluded.binding_version, status = excluded.status, last_verify_at = excluded.last_verify_at",
                )
                .bind(&[
                    JsValue::from_str(&record.license_key),
                    JsValue::from_str(&record.device_id),
                    JsValue::from_str(&record.device_fingerprint),
                    JsValue::from_f64(record.plan_days as f64),
                    JsValue::from_str(&record.activated_at),
                    JsValue::from_str(&record.license_expires_at),
                    JsValue::from_str(&record.updated_at),
                    JsValue::from_f64(record.binding_version as f64),
                    JsValue::from_str(&enum_text(&record.status)?),
                    JsValue::from_str(&record.last_verify_at),
                    JsValue::from_str(&record.license_key),
                    JsValue::from_str(&record.license_key),
                ])?
                .run()
                .await?;
            Ok(())
        }

        async fn load_device_registration(
            &self,
            license_key: &str,
            device_id: &str,
        ) -> anyhow::Result<Option<DeviceRegistration>> {
            let stmt = self
                .db
                .prepare(
                    "SELECT license_key, device_id, COALESCE(device_fingerprint_hash, '') AS device_fingerprint_hash, COALESCE(registered_at, '') AS registered_at, COALESCE(last_seen_at, '') AS last_seen_at, COALESCE(registration_status, 'active') AS registration_status FROM device_registrations WHERE license_key = ? AND device_id = ? LIMIT 1",
                )
                .bind(&[JsValue::from_str(license_key), JsValue::from_str(device_id)])?;
            let result = stmt.all().await?;
            let mut rows: Vec<DeviceRegistration> = result.results().unwrap_or_default();
            Ok(rows.pop())
        }

        async fn save_device_registration(
            &self,
            record: &DeviceRegistration,
        ) -> anyhow::Result<()> {
            if self
                .load_device_registration(&record.license_key, &record.device_id)
                .await?
                .is_some()
            {
                self.db
                    .prepare(
                        "UPDATE device_registrations SET device_fingerprint_hash = ?, registered_at = ?, last_seen_at = ?, registration_status = ? WHERE license_key = ? AND device_id = ?",
                    )
                    .bind(&[
                        JsValue::from_str(&record.device_fingerprint_hash),
                        JsValue::from_str(&record.registered_at),
                        JsValue::from_str(&record.last_seen_at),
                        JsValue::from_str(&record.registration_status),
                        JsValue::from_str(&record.license_key),
                        JsValue::from_str(&record.device_id),
                    ])?
                    .run()
                    .await?;
            } else {
                self.db
                    .prepare(
                        "INSERT INTO device_registrations (license_key, device_id, device_fingerprint_hash, registered_at, last_seen_at, registration_status) VALUES (?, ?, ?, ?, ?, ?)",
                    )
                    .bind(&[
                        JsValue::from_str(&record.license_key),
                        JsValue::from_str(&record.device_id),
                        JsValue::from_str(&record.device_fingerprint_hash),
                        JsValue::from_str(&record.registered_at),
                        JsValue::from_str(&record.last_seen_at),
                        JsValue::from_str(&record.registration_status),
                    ])?
                    .run()
                    .await?;
            }
            Ok(())
        }

        async fn append_audit_event(&self, event: &AuditEvent) -> anyhow::Result<()> {
            self.db
                .prepare(
                    "INSERT INTO license_audit_logs (license_key, device_id, action, action_reason, created_at, operator, meta_json) VALUES (?, ?, ?, ?, ?, 'worker', '{}')",
                )
                .bind(&[
                    JsValue::from_str(&event.license_key),
                    JsValue::from_str(&event.device_id),
                    JsValue::from_str(&event.action),
                    JsValue::from_str(&event.reason),
                    JsValue::from_str(&event.created_at),
                ])?
                .run()
                .await?;
            Ok(())
        }

        async fn update_runtime_markers(
            &self,
            license_key: &str,
            now_iso: &str,
            session_issued: bool,
            grant_issued: bool,
            new_status: Option<LicenseState>,
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
            let status_sql = if new_status.is_some() {
                ", status = ?"
            } else {
                ""
            };
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
                binds.push(JsValue::from_str(&enum_text(&status)?));
            }
            binds.push(JsValue::from_str(license_key));
            self.db.prepare(&sql).bind(&binds)?.run().await?;
            Ok(())
        }

        async fn revoke_license(
            &self,
            license_key: &str,
            device_id: &str,
            reason: &str,
            revoked_at: &str,
        ) -> anyhow::Result<bool> {
            let Some(_key_record) = self.load_generated_key(license_key).await? else {
                return Ok(false);
            };

            let advanced_update = self
                .db
                .prepare(
                    "UPDATE generated_keys SET status = 'revoked', revoked_at = ?, revoke_reason = ? WHERE license_key = ?",
                )
                .bind(&[
                    JsValue::from_str(revoked_at),
                    JsValue::from_str(reason),
                    JsValue::from_str(license_key),
                ])?
                .run()
                .await;
            if advanced_update.is_err() {
                self.db
                    .prepare("UPDATE generated_keys SET status = 'revoked' WHERE license_key = ?")
                    .bind(&[JsValue::from_str(license_key)])?
                    .run()
                    .await?;
            }

            self.db
                .prepare(
                    "UPDATE activations SET status = 'revoked', updated_at = ?, last_verify_at = ? WHERE license_key = ?",
                )
                .bind(&[
                    JsValue::from_str(revoked_at),
                    JsValue::from_str(revoked_at),
                    JsValue::from_str(license_key),
                ])?
                .run()
                .await?;

            self.db
                .prepare(
                    "UPDATE device_sessions SET revoked_at = ? WHERE license_key = ? AND (revoked_at IS NULL OR revoked_at = '')",
                )
                .bind(&[JsValue::from_str(revoked_at), JsValue::from_str(license_key)])?
                .run()
                .await?;

            self.db
                .prepare(
                    "UPDATE device_registrations SET registration_status = 'revoked', last_seen_at = ? WHERE license_key = ? AND device_id = ?",
                )
                .bind(&[
                    JsValue::from_str(revoked_at),
                    JsValue::from_str(license_key),
                    JsValue::from_str(device_id),
                ])?
                .run()
                .await?;
            Ok(true)
        }

        async fn revoke_license_by_key(
            &self,
            license_key: &str,
            reason: &str,
            revoked_at: &str,
        ) -> anyhow::Result<bool> {
            let Some(_key_record) = self.load_generated_key(license_key).await? else {
                return Ok(false);
            };

            let advanced_update = self
                .db
                .prepare(
                    "UPDATE generated_keys SET status = 'revoked', revoked_at = ?, revoke_reason = ? WHERE license_key = ?",
                )
                .bind(&[
                    JsValue::from_str(revoked_at),
                    JsValue::from_str(reason),
                    JsValue::from_str(license_key),
                ])?
                .run()
                .await;
            if advanced_update.is_err() {
                self.db
                    .prepare("UPDATE generated_keys SET status = 'revoked' WHERE license_key = ?")
                    .bind(&[JsValue::from_str(license_key)])?
                    .run()
                    .await?;
            }

            self.db
                .prepare(
                    "UPDATE activations SET status = 'revoked', updated_at = ?, last_verify_at = ? WHERE license_key = ?",
                )
                .bind(&[
                    JsValue::from_str(revoked_at),
                    JsValue::from_str(revoked_at),
                    JsValue::from_str(license_key),
                ])?
                .run()
                .await?;

            self.db
                .prepare(
                    "UPDATE device_sessions SET revoked_at = ? WHERE license_key = ? AND (revoked_at IS NULL OR revoked_at = '')",
                )
                .bind(&[JsValue::from_str(revoked_at), JsValue::from_str(license_key)])?
                .run()
                .await?;

            self.db
                .prepare(
                    "UPDATE device_registrations SET registration_status = 'revoked', last_seen_at = ? WHERE license_key = ?",
                )
                .bind(&[JsValue::from_str(revoked_at), JsValue::from_str(license_key)])?
                .run()
                .await?;
            Ok(true)
        }
    }

    fn worker_error(err: impl ToString) -> worker::Error {
        worker::Error::RustError(err.to_string())
    }

    fn response_from_json_string(payload: String) -> Result<Response> {
        let value: Value = serde_json::from_str(&payload).map_err(worker_error)?;
        Response::from_json(&value)
    }

    async fn route_fetch(mut req: Request, env: Env) -> Result<Response> {
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

        let route = parse_route(&path);
        if route == WorkerRoute::NotFound {
            return Response::error("not_found", 404);
        }

        if route == WorkerRoute::LeaseRevoke {
            if let Some(resp) = crate::admin_d1::check_admin(req.headers(), &env)? {
                return Ok(resp);
            }
        }

        let body = req.text().await.unwrap_or_default();
        match route {
            WorkerRoute::Activate
            | WorkerRoute::Verify
            | WorkerRoute::LeaseRefresh
            | WorkerRoute::TaskAuthorize
            | WorkerRoute::LeaseRevoke => {
                let signer = if route_requires_signer(route) {
                    match load_signer(&env) {
                        Ok(signer) => Some(signer),
                        Err(_) => return missing_secret("LICENSE_SIGNING_PRIVATE_KEY_B64"),
                    }
                } else {
                    None
                };
                let db = env.d1("DB")?;
                let repo = D1RuntimeRepo::new(&db);
                let payload = match handle_async_runtime_json(
                    &repo,
                    &path,
                    &body,
                    signer.as_ref(),
                    Utc::now(),
                )
                .await
                {
                    Ok(payload) => payload,
                    Err(err) if route == WorkerRoute::LeaseRevoke => {
                        let (status, message) = revoke_error_contract(&err.to_string());
                        return Response::from_json(&serde_json::json!({
                            "success": false,
                            "message": message,
                        }))
                        .map(|resp| resp.with_status(status));
                    }
                    Err(err) => return Err(worker_error(err)),
                };
                if route == WorkerRoute::LeaseRevoke {
                    response_from_revoke_payload(&payload)
                } else {
                    response_from_json_string(payload)
                }
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

    #[async_trait(?Send)]
    impl AsyncRuntimeRepository for Repo {
        async fn load_generated_key(
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

        async fn save_generated_key(&self, record: &GeneratedKeyRecord) -> anyhow::Result<()> {
            self.generated_keys
                .lock()
                .unwrap()
                .insert(record.license_key.clone(), record.clone());
            Ok(())
        }

        async fn load_license(&self, license_key: &str) -> anyhow::Result<Option<LicenseRecord>> {
            Ok(self.licenses.lock().unwrap().get(license_key).cloned())
        }

        async fn save_license(&self, record: &LicenseRecord) -> anyhow::Result<()> {
            self.licenses
                .lock()
                .unwrap()
                .insert(record.license_key.clone(), record.clone());
            Ok(())
        }

        async fn load_device_registration(
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

        async fn save_device_registration(
            &self,
            record: &DeviceRegistration,
        ) -> anyhow::Result<()> {
            self.registrations.lock().unwrap().insert(
                (record.license_key.clone(), record.device_id.clone()),
                record.clone(),
            );
            Ok(())
        }

        async fn append_audit_event(&self, event: &AuditEvent) -> anyhow::Result<()> {
            self.audits.lock().unwrap().push(event.clone());
            Ok(())
        }

        async fn update_runtime_markers(
            &self,
            license_key: &str,
            now_iso: &str,
            _session_issued: bool,
            _grant_issued: bool,
            new_status: Option<LicenseState>,
        ) -> anyhow::Result<()> {
            let mut licenses = self.licenses.lock().unwrap();
            if let Some(record) = licenses.get_mut(license_key) {
                record.updated_at = now_iso.to_string();
                record.last_verify_at = now_iso.to_string();
                if let Some(status) = new_status {
                    record.status = status;
                }
            }
            Ok(())
        }

        async fn revoke_license(
            &self,
            license_key: &str,
            device_id: &str,
            _reason: &str,
            revoked_at: &str,
        ) -> anyhow::Result<bool> {
            let mut generated = self.generated_keys.lock().unwrap();
            let Some(key_record) = generated.get_mut(license_key) else {
                return Ok(false);
            };
            key_record.status = GeneratedKeyStatus::Revoked;
            drop(generated);

            let mut licenses = self.licenses.lock().unwrap();
            if let Some(record) = licenses.get_mut(license_key) {
                record.status = LicenseState::Revoked;
                record.updated_at = revoked_at.to_string();
                record.last_verify_at = revoked_at.to_string();
            }
            drop(licenses);

            if let Some(registration) = self
                .registrations
                .lock()
                .unwrap()
                .get_mut(&(license_key.to_string(), device_id.to_string()))
            {
                registration.registration_status = "revoked".into();
                registration.last_seen_at = revoked_at.to_string();
            }
            Ok(true)
        }

        async fn revoke_license_by_key(
            &self,
            license_key: &str,
            _reason: &str,
            revoked_at: &str,
        ) -> anyhow::Result<bool> {
            let mut generated = self.generated_keys.lock().unwrap();
            let Some(key_record) = generated.get_mut(license_key) else {
                return Ok(false);
            };
            key_record.status = GeneratedKeyStatus::Revoked;
            drop(generated);

            let mut licenses = self.licenses.lock().unwrap();
            if let Some(record) = licenses.get_mut(license_key) {
                record.status = LicenseState::Revoked;
                record.updated_at = revoked_at.to_string();
                record.last_verify_at = revoked_at.to_string();
            }
            drop(licenses);

            for ((key, _), registration) in self.registrations.lock().unwrap().iter_mut() {
                if key == license_key {
                    registration.registration_status = "revoked".into();
                    registration.last_seen_at = revoked_at.to_string();
                }
            }
            Ok(true)
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
    fn signer_requirement_matches_runtime_route_contract() {
        assert!(route_requires_signer(WorkerRoute::Activate));
        assert!(route_requires_signer(WorkerRoute::Verify));
        assert!(route_requires_signer(WorkerRoute::LeaseRefresh));
        assert!(!route_requires_signer(WorkerRoute::LeaseRevoke));
        assert!(!route_requires_signer(WorkerRoute::TaskAuthorize));
        assert!(!route_requires_signer(WorkerRoute::NotFound));
    }

    #[test]
    fn revoke_contract_maps_status_and_message_consistently() {
        assert_eq!(revoke_error_contract("empty_key"), (400, "empty_key"));
        assert_eq!(revoke_error_contract("not_found"), (404, "not_found"));
        assert_eq!(revoke_error_contract("unauthorized"), (401, "unauthorized"));
        assert_eq!(revoke_error_contract("secret_missing"), (503, "secret_missing"));
        assert_eq!(
            revoke_error_contract("missing field `key`"),
            (400, "invalid_json")
        );
    }

    #[test]
    fn worker_runtime_error_message_is_stable() {
        assert_eq!(WORKER_RUNTIME_ERROR_MESSAGE, "worker_runtime_error");
    }

    #[test]
    fn admin_auth_contract_covers_secret_missing_and_unauthorized() {
        assert_eq!(
            admin_auth_error_contract(false, false),
            Some((503, "secret_missing"))
        );
        assert_eq!(
            admin_auth_error_contract(true, false),
            Some((401, "unauthorized"))
        );
        assert_eq!(admin_auth_error_contract(true, true), None);
    }

    #[tokio::test]
    async fn async_runtime_service_activate_verify_refresh_and_authorize_share_repo_path() {
        let repo = Repo::seeded();
        let signer = test_signer();
        let now = chrono::DateTime::parse_from_rfc3339("2026-04-17T00:00:00Z")
            .unwrap()
            .with_timezone(&Utc);

        let activated = runtime_activate(
            &repo,
            &signer,
            ActivationInput {
                license_key: "TLS-TEST".into(),
                device_id: "device-1".into(),
                device_fingerprint: "fp-1".into(),
                client_version: "5.0.0".into(),
            },
            now,
        )
        .await
        .unwrap();
        assert!(activated.success);
        assert!(activated.license_lease.is_some());

        let verified = runtime_verify(
            &repo,
            &signer,
            VerifyInput {
                license_key: "TLS-TEST".into(),
                device_id: "device-1".into(),
                client_version: "5.1.0".into(),
            },
            now,
        )
        .await
        .unwrap();
        assert!(verified.success);
        assert_eq!(verified.license_state, LicenseState::Active);

        let refreshed = runtime_refresh_lease(
            &repo,
            &signer,
            LeaseRefreshRequest {
                license_key: "TLS-TEST".into(),
                device_id: "device-1".into(),
                current_issued_at: now.timestamp(),
            },
            now,
        )
        .await
        .unwrap();
        assert!(refreshed.success);
        assert!(!refreshed.new_token.is_empty());

        let grant = runtime_task_authorize(
            &repo,
            TaskAuthorizeRequest {
                license_key: "TLS-TEST".into(),
                device_id: "device-1".into(),
                task_type: LICENSE_TASK_REVIEW_FIND.into(),
                client_version: "5.2.0".into(),
            },
            now,
        )
        .await
        .unwrap();
        assert!(grant.granted);
        assert_eq!(grant.task_type, LICENSE_TASK_REVIEW_FIND);
    }

    #[tokio::test]
    async fn async_runtime_verify_reports_expired_from_shared_repo_state() {
        let repo = Repo::seeded();
        let signer = test_signer();
        let activated_at = chrono::DateTime::parse_from_rfc3339("2026-04-01T00:00:00Z")
            .unwrap()
            .with_timezone(&Utc);
        runtime_activate(
            &repo,
            &signer,
            ActivationInput {
                license_key: "TLS-TEST".into(),
                device_id: "device-1".into(),
                device_fingerprint: "fp-1".into(),
                client_version: String::new(),
            },
            activated_at,
        )
        .await
        .unwrap();

        let expired_at = chrono::DateTime::parse_from_rfc3339("2026-05-05T00:00:00Z")
            .unwrap()
            .with_timezone(&Utc);
        let verified = runtime_verify(
            &repo,
            &signer,
            VerifyInput {
                license_key: "TLS-TEST".into(),
                device_id: "device-1".into(),
                client_version: String::new(),
            },
            expired_at,
        )
        .await
        .unwrap();

        assert!(!verified.success);
        assert_eq!(verified.license_state, LicenseState::Expired);
        assert_eq!(verified.message, "授权已过期");
    }

    #[tokio::test]
    async fn async_runtime_json_router_uses_shared_repo_flow() {
        let repo = Repo::seeded();
        let signer = test_signer();
        let now = chrono::DateTime::parse_from_rfc3339("2026-04-17T00:00:00Z")
            .unwrap()
            .with_timezone(&Utc);

        let activated = handle_async_runtime_json(
            &repo,
            "/api/activate",
            r#"{"license_key":"TLS-TEST","device_id":"device-1","device_fingerprint":"fp-1","client_version":"5.0.0"}"#,
            Some(&signer),
            now,
        )
        .await
        .unwrap();
        let activated_payload: SignedLicenseApiResponse = serde_json::from_str(&activated).unwrap();
        assert!(activated_payload.success);
        let verifier = LeaseVerifier::from_public_key_b64(&signer.public_key_b64()).unwrap();
        let verified_lease = verifier
            .verify(
                activated_payload.license_lease.as_deref().unwrap(),
                Some("device-1"),
                now.timestamp(),
                false,
            )
            .unwrap();
        assert_eq!(verified_lease.device_id, "device-1");

        let grant = handle_async_runtime_json(
            &repo,
            "/api/task/authorize",
            r#"{"license_key":"TLS-TEST","device_id":"device-1","task_type":"review_find","client_version":"5.2.0"}"#,
            Some(&signer),
            now,
        )
        .await
        .unwrap();
        let grant_payload: RuntimeGrant = serde_json::from_str(&grant).unwrap();
        assert!(grant_payload.granted);
    }

    #[tokio::test]
    async fn async_runtime_json_router_covers_verify_refresh_and_not_found() {
        let repo = Repo::seeded();
        let signer = test_signer();
        let now = chrono::DateTime::parse_from_rfc3339("2026-04-17T00:00:00Z")
            .unwrap()
            .with_timezone(&Utc);

        let _ = handle_async_runtime_json(
            &repo,
            "/api/activate",
            r#"{"license_key":"TLS-TEST","device_id":"device-1","device_fingerprint":"fp-1","client_version":"5.0.0"}"#,
            Some(&signer),
            now,
        )
        .await
        .unwrap();

        let verified = handle_async_runtime_json(
            &repo,
            "/api/verify",
            r#"{"license_key":"TLS-TEST","device_id":"device-1","client_version":"5.1.0"}"#,
            Some(&signer),
            now,
        )
        .await
        .unwrap();
        let verified_payload: SignedLicenseApiResponse = serde_json::from_str(&verified).unwrap();
        assert!(verified_payload.success);
        assert!(verified_payload.license_lease.is_some());

        let refreshed = handle_async_runtime_json(
            &repo,
            "/api/lease/refresh",
            r#"{"license_key":"TLS-TEST","device_id":"device-1","current_issued_at":1713312000}"#,
            Some(&signer),
            now,
        )
        .await
        .unwrap();
        let refreshed_payload: LeaseRefreshResponse = serde_json::from_str(&refreshed).unwrap();
        assert!(refreshed_payload.success);
        assert!(!refreshed_payload.new_token.is_empty());

        let missing = handle_async_runtime_json(&repo, "/missing", "{}", None, now)
            .await
            .unwrap();
        let missing_payload: SignedLicenseApiResponse = serde_json::from_str(&missing).unwrap();
        assert!(!missing_payload.success);
        assert_eq!(missing_payload.message, "not_found");
        assert_eq!(missing_payload.license_state, LicenseState::Invalid);

        let revoke = handle_async_runtime_json(
            &repo,
            "/api/lease/revoke",
            r#"{"license_key":"TLS-TEST","device_id":"device-1","reason":"admin"}"#,
            None,
            now,
        )
        .await
        .unwrap();
        let revoke_payload: SignedLicenseApiResponse = serde_json::from_str(&revoke).unwrap();
        assert!(revoke_payload.success);
        assert_eq!(revoke_payload.message, "license_revoked");
        assert_eq!(revoke_payload.license_state, LicenseState::Revoked);
    }

    #[tokio::test]
    async fn async_runtime_revoke_invalidates_verify_refresh_and_task_authorize() {
        let repo = Repo::seeded();
        let signer = test_signer();
        let now = chrono::DateTime::parse_from_rfc3339("2026-04-17T00:00:00Z")
            .unwrap()
            .with_timezone(&Utc);

        let _ = handle_async_runtime_json(
            &repo,
            "/api/activate",
            r#"{"license_key":"TLS-TEST","device_id":"device-1","device_fingerprint":"fp-1","client_version":"5.0.0"}"#,
            Some(&signer),
            now,
        )
        .await
        .unwrap();

        let revoked = handle_async_runtime_json(
            &repo,
            "/api/lease/revoke",
            r#"{"license_key":"TLS-TEST","device_id":"device-1","reason":"admin"}"#,
            None,
            now,
        )
        .await
        .unwrap();
        let revoked_payload: SignedLicenseApiResponse = serde_json::from_str(&revoked).unwrap();
        assert!(revoked_payload.success);
        assert_eq!(revoked_payload.license_state, LicenseState::Revoked);
        assert_eq!(revoked_payload.message, "license_revoked");

        let verified = handle_async_runtime_json(
            &repo,
            "/api/verify",
            r#"{"license_key":"TLS-TEST","device_id":"device-1","client_version":"5.1.0"}"#,
            Some(&signer),
            now,
        )
        .await
        .unwrap();
        let verified_payload: SignedLicenseApiResponse = serde_json::from_str(&verified).unwrap();
        assert!(!verified_payload.success);
        assert_eq!(verified_payload.license_state, LicenseState::Revoked);

        let refreshed = handle_async_runtime_json(
            &repo,
            "/api/lease/refresh",
            r#"{"license_key":"TLS-TEST","device_id":"device-1","current_issued_at":1713312000}"#,
            Some(&signer),
            now,
        )
        .await
        .unwrap();
        let refreshed_payload: LeaseRefreshResponse = serde_json::from_str(&refreshed).unwrap();
        assert!(!refreshed_payload.success);
        assert_eq!(refreshed_payload.message, "该卡密已被吊销");

        let grant = handle_async_runtime_json(
            &repo,
            "/api/task/authorize",
            r#"{"license_key":"TLS-TEST","device_id":"device-1","task_type":"review_find","client_version":"5.2.0"}"#,
            None,
            now,
        )
        .await
        .unwrap();
        let grant_payload: RuntimeGrant = serde_json::from_str(&grant).unwrap();
        assert!(!grant_payload.granted);
        assert_eq!(
            grant_payload.degraded_reason.as_deref(),
            Some("该卡密已被吊销")
        );
    }

    #[tokio::test]
    async fn admin_revoke_json_reuses_runtime_revoke_flow_without_device_id() {
        let repo = Repo::seeded();
        let signer = test_signer();
        let now = chrono::DateTime::parse_from_rfc3339("2026-04-17T00:00:00Z")
            .unwrap()
            .with_timezone(&Utc);

        let _ = handle_async_runtime_json(
            &repo,
            "/api/activate",
            r#"{"license_key":"TLS-TEST","device_id":"device-1","device_fingerprint":"fp-1","client_version":"5.0.0"}"#,
            Some(&signer),
            now,
        )
        .await
        .unwrap();

        let revoked = handle_admin_revoke_json(&repo, r#"{"key":"TLS-TEST"}"#, now)
            .await
            .unwrap();
        let revoked_payload: SignedLicenseApiResponse = serde_json::from_str(&revoked).unwrap();
        assert!(revoked_payload.success);
        assert_eq!(revoked_payload.license_state, LicenseState::Revoked);
        assert_eq!(revoked_payload.device_id.as_deref(), Some("device-1"));

        let verified = handle_async_runtime_json(
            &repo,
            "/api/verify",
            r#"{"license_key":"TLS-TEST","device_id":"device-1","client_version":"5.1.0"}"#,
            Some(&signer),
            now,
        )
        .await
        .unwrap();
        let verified_payload: SignedLicenseApiResponse = serde_json::from_str(&verified).unwrap();
        assert!(!verified_payload.success);
        assert_eq!(verified_payload.license_state, LicenseState::Revoked);
    }

    #[tokio::test]
    async fn admin_revoke_json_invalid_json_maps_to_400_contract() {
        let repo = Repo::seeded();
        let now = chrono::DateTime::parse_from_rfc3339("2026-04-17T00:00:00Z")
            .unwrap()
            .with_timezone(&Utc);

        let err = handle_admin_revoke_json(&repo, "{", now).await.unwrap_err();
        assert_eq!(revoke_error_contract(&err.to_string()), (400, "invalid_json"));
    }

    #[tokio::test]
    async fn admin_revoke_json_not_found_maps_to_404_contract() {
        let repo = Repo::seeded();
        let now = chrono::DateTime::parse_from_rfc3339("2026-04-17T00:00:00Z")
            .unwrap()
            .with_timezone(&Utc);

        let payload = handle_admin_revoke_json(&repo, r#"{"key":"TLS-MISSING"}"#, now)
            .await
            .unwrap();
        let response: SignedLicenseApiResponse = serde_json::from_str(&payload).unwrap();
        assert!(!response.success);
        assert_eq!(response.message, "not_found");
        assert_eq!(revoke_response_status(&response), 404);
    }

    #[tokio::test]
    async fn admin_revoke_revokes_all_registrations_under_same_license_key() {
        let repo = Repo::seeded();
        let signer = test_signer();
        let now = chrono::DateTime::parse_from_rfc3339("2026-04-17T00:00:00Z")
            .unwrap()
            .with_timezone(&Utc);

        let _ = handle_async_runtime_json(
            &repo,
            "/api/activate",
            r#"{"license_key":"TLS-TEST","device_id":"device-1","device_fingerprint":"fp-1","client_version":"5.0.0"}"#,
            Some(&signer),
            now,
        )
        .await
        .unwrap();

        repo.save_device_registration(&DeviceRegistration {
            license_key: "TLS-TEST".into(),
            device_id: "device-legacy".into(),
            device_fingerprint_hash: "legacy-hash".into(),
            registered_at: "2026-04-01T00:00:00Z".into(),
            last_seen_at: "2026-04-16T00:00:00Z".into(),
            registration_status: "active".into(),
        })
        .await
        .unwrap();

        let _ = handle_admin_revoke_json(&repo, r#"{"key":"TLS-TEST"}"#, now)
            .await
            .unwrap();

        let registrations = repo.registrations.lock().unwrap();
        assert_eq!(
            registrations
                .get(&("TLS-TEST".into(), "device-1".into()))
                .map(|value| value.registration_status.as_str()),
            Some("revoked")
        );
        assert_eq!(
            registrations
                .get(&("TLS-TEST".into(), "device-legacy".into()))
                .map(|value| value.registration_status.as_str()),
            Some("revoked")
        );
    }

    #[tokio::test]
    async fn runtime_revoke_not_found_maps_to_404_contract() {
        let repo = Repo::seeded();
        let now = chrono::DateTime::parse_from_rfc3339("2026-04-17T00:00:00Z")
            .unwrap()
            .with_timezone(&Utc);

        let revoked = handle_async_runtime_json(
            &repo,
            "/api/lease/revoke",
            r#"{"license_key":"TLS-MISSING","device_id":"device-1","reason":"admin"}"#,
            None,
            now,
        )
        .await
        .unwrap();
        let payload: SignedLicenseApiResponse = serde_json::from_str(&revoked).unwrap();
        assert!(!payload.success);
        assert_eq!(payload.message, "not_found");
        assert_eq!(payload.license_state, LicenseState::NotFound);
        assert_eq!(revoke_response_status(&payload), 404);
    }

    #[tokio::test]
    async fn runtime_revoke_with_empty_key_uses_stable_error_contract() {
        let repo = Repo::seeded();
        let now = chrono::DateTime::parse_from_rfc3339("2026-04-17T00:00:00Z")
            .unwrap()
            .with_timezone(&Utc);

        let revoked = handle_async_runtime_json(
            &repo,
            "/api/lease/revoke",
            r#"{"license_key":"   ","device_id":"device-1","reason":"admin"}"#,
            None,
            now,
        )
        .await
        .unwrap();
        let payload: SignedLicenseApiResponse = serde_json::from_str(&revoked).unwrap();
        assert!(!payload.success);
        assert_eq!(payload.message, "empty_key");
        assert_eq!(payload.license_state, LicenseState::Invalid);
        assert_eq!(revoke_response_status(&payload), 400);
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
