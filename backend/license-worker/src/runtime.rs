//! 异步运行时业务层：`/api/*` 路由入口的纯业务实现 + Cloudflare D1 的仓储实现。
//!
//! 历史上这两层代码分散在 `lib.rs` 与 `runtime/repo_d1.rs` 里，`lib.rs` 因此
//! 既像"Worker 入口"又像"runtime service"的 God File（~900 行）。本文件把:
//!
//! - 协议常量（`WORKER_RUNTIME_ERROR_MESSAGE` / `REVOKE_GENERATED_KEY_SQL_*`）
//! - 纯 helper（时间 / 哈希 / 响应体组装 / 错误契约 / device_id 自洽校验）
//! - [`AsyncRuntimeRepository`] trait
//! - `runtime_activate / verify / refresh_lease / revoke / task_authorize /
//!   handle_admin_revoke_json / handle_async_runtime_json`
//!
//! 都收敛到这里。[`D1RuntimeRepo`]（仅 `wasm32` target 下编译的 Cloudflare D1
//! 实现）已拆到 `runtime_d1.rs`，通过 `#[path]` 属性加载，保持对外访问路径
//! `crate::runtime::D1RuntimeRepo` 不变；这样主文件只保留业务主线，便于阅读。
//!
//! `lib.rs` 只保留三件事：[`crate::LeaseTokenSigner`]（签发工具）、
//! `cloudflare_entry`（wasm `fetch` 事件入口）以及对 `messages` 与本模块的
//! `pub use` 转发，保证外部测试与调用方的 import 路径（`use super::*;` /
//! `use license_worker::…`）不变。

use api_contracts::{
    LicenseLease, LicenseState, Lp, Rg, RiskLevel, LEASE_KIND_LICENSE, LICENSE_TASK_BATCH_DELIVERY,
    LICENSE_TASK_CACHE_MANAGE, LICENSE_TASK_QUALITY_REFUND, LICENSE_TASK_REVIEW_FIND,
    LICENSE_TASK_REVIEW_FULL_SCAN,
};
use async_trait::async_trait;
use chrono::{DateTime, Utc};
use license_service::{
    authorize_task_local, ActivationInput, AuditEvent, DeviceRegistration, GeneratedKeyRecord,
    GeneratedKeyStatus, LicenseRecord, VerifyInput,
};
use serde_json::Value;
use sha2::{Digest, Sha256};
use std::sync::atomic::{AtomicU64, Ordering};

use crate::messages::{
    parse_route, AdminRevokeRequest, LeaseRefreshRequest, LeaseRevokeRequest, Lrr,
    SignedLicenseApiResponse, TaskAuthorizeRequest, WorkerRoute,
};
use crate::LeaseTokenSigner;

// --- 协议常量 ---------------------------------------------------------------

pub const WORKER_RUNTIME_ERROR_MESSAGE: &str = "worker_runtime_error";
pub const REVOKE_GENERATED_KEY_SQL_WITH_METADATA: &str =
    "UPDATE generated_keys SET status = 'revoked', revoked_at = ?, revoke_reason = ? WHERE license_key = ?";
pub const REVOKE_GENERATED_KEY_SQL_FALLBACK: &str =
    "UPDATE generated_keys SET status = 'revoked' WHERE license_key = ?";

static NEXT_GRANT_SEQ: AtomicU64 = AtomicU64::new(1);

// --- 纯 helper --------------------------------------------------------------

fn parse_iso_epoch(value: &str) -> anyhow::Result<i64> {
    Ok(DateTime::parse_from_rfc3339(value)?
        .with_timezone(&Utc)
        .timestamp())
}

/// 把 UI 层的 [`LicenseLease`] 结构（时间字段为 ISO8601 字符串）折叠成 Worker
/// 真正签发的 [`Lp`] 结构（时间字段为 Unix 秒）。
///
/// 暴露为 `pub(crate)` 是因为 `crate::LeaseTokenSigner::sign_license_lease`
/// 也需要直接调用它。
pub(crate) fn lease_to_payload(lease: &LicenseLease) -> anyhow::Result<Lp> {
    Ok(Lp {
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

fn denied_grant(task_type: &str, message: impl Into<String>) -> Rg {
    Rg {
        task_type: task_type.to_string(),
        granted: false,
        grant_id: String::new(),
        valid_until: String::new(),
        risk_level: task_risk_level(task_type),
        degraded_reason: Some(message.into()),
    }
}

fn normalize_key(value: &str) -> String {
    value.trim().to_uppercase()
}

pub(crate) fn now_iso(now: DateTime<Utc>) -> String {
    now.to_rfc3339_opts(chrono::SecondsFormat::Secs, true)
}

fn sha256_hex(value: &str) -> String {
    format!("{:x}", Sha256::digest(value.as_bytes()))
}

/// 校验 client 发来的 `device_id` 是否与 `device_fingerprint` 自洽：device_id
/// 必须等于 `SHA256(device_fingerprint)` 前 8 字节（16 个小写 hex 字符）。
///
/// 前后端共享同一个派生规则：
/// - Rust 客户端：`security_core::derive_device_id` = `sha256(raw).iter().take(8).hex()`
/// - Python 旧客户端：`hashlib.sha256(raw).hexdigest()[:16]`
///
/// Worker 激活时加这层校验可以闭合"同 device_id 不同 device_fingerprint 被
/// 静默覆盖"的协议窗口——`activations.device_id` 仅 16 hex（64 位熵），若不
/// 强制 device_fingerprint 与之自洽，攻击者可用相同 device_id 提交任意
/// fingerprint 覆盖 D1 记录，绕过真实的"设备绑定"语义。
///
/// 空串豁免：`device_fingerprint` 为空时不做校验，兼容极端兜底场景；正常
/// client 走 fallback 也会返回 `hostname-arch-os` 这种非空值，因此豁免只
/// 是防御性留门。比对忽略大小写以兼容历史 hex 字符串可能出现的 `to_upper`
/// / `to_lower` 漂移。
pub(crate) fn device_id_matches_fingerprint(device_id: &str, device_fingerprint: &str) -> bool {
    if device_fingerprint.is_empty() {
        return true;
    }
    let expected_full = sha256_hex(device_fingerprint);
    expected_full
        .get(..16)
        .map(|prefix| prefix.eq_ignore_ascii_case(device_id.trim()))
        .unwrap_or(false)
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

// --- 公开的错误契约函数 -----------------------------------------------------

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

pub fn revoke_generated_key_update_sql(with_metadata_columns: bool) -> &'static str {
    if with_metadata_columns {
        REVOKE_GENERATED_KEY_SQL_WITH_METADATA
    } else {
        REVOKE_GENERATED_KEY_SQL_FALLBACK
    }
}

// --- AsyncRuntimeRepository trait ------------------------------------------

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

// --- runtime_* 业务入口 -----------------------------------------------------

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

async fn validate_activation_key<R: AsyncRuntimeRepository + ?Sized>(
    repo: &R,
    normalized_key: &str,
    device_id: &str,
    device_fingerprint: &str,
) -> anyhow::Result<Result<GeneratedKeyRecord, SignedLicenseApiResponse>> {
    if !device_id_matches_fingerprint(device_id, device_fingerprint) {
        return Ok(Err(signed_failure_response_for_record(
            "设备凭证不自洽：device_id 与 device_fingerprint 不匹配，请重新激活",
            LicenseState::DeviceMismatch,
            None,
        )));
    }
    let Some(key_record) = repo.load_generated_key(normalized_key).await? else {
        return Ok(Err(signed_failure_response_for_record(
            "该卡密不存在或已被吊销",
            LicenseState::Revoked,
            None,
        )));
    };
    if key_record.status == GeneratedKeyStatus::Revoked {
        return Ok(Err(signed_failure_response_for_record(
            "该卡密已被吊销，无法使用",
            LicenseState::Revoked,
            None,
        )));
    }
    if key_record.plan_days == 0 {
        return Ok(Err(signed_failure_response_for_record(
            "卡密无效：有效期异常",
            LicenseState::Invalid,
            None,
        )));
    }
    Ok(Ok(key_record))
}

pub async fn runtime_activate<R: AsyncRuntimeRepository + ?Sized>(
    repo: &R,
    signer: &LeaseTokenSigner,
    input: ActivationInput,
    now: DateTime<Utc>,
) -> anyhow::Result<SignedLicenseApiResponse> {
    let normalized_key = normalize_key(&input.license_key);
    let mut key_record = match validate_activation_key(
        repo,
        &normalized_key,
        &input.device_id,
        &input.device_fingerprint,
    )
    .await?
    {
        Ok(record) => record,
        Err(failure) => return Ok(failure),
    };

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
) -> anyhow::Result<Lrr> {
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
            return Ok(Lrr {
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
    Ok(Lrr {
        success: true,
        message: "lease_refreshed".into(),
        new_token: signer.sign_license_lease(&issue_license_lease_for_record(&record, now))?,
    })
}

pub async fn runtime_task_authorize<R: AsyncRuntimeRepository + ?Sized>(
    repo: &R,
    input: TaskAuthorizeRequest,
    now: DateTime<Utc>,
) -> anyhow::Result<Rg> {
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

// --- D1RuntimeRepo：Cloudflare D1 存储层（仅 wasm32） -----------------------
//
// 实现已拆到 runtime_d1.rs；此处 `#[path]` 引用保持对外访问路径 `crate::runtime::D1RuntimeRepo` 不变。

#[cfg(target_arch = "wasm32")]
#[path = "runtime_d1.rs"]
mod d1_repo;

#[cfg(target_arch = "wasm32")]
pub(crate) use d1_repo::D1RuntimeRepo;
