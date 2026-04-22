//! `license_worker` 整体集成测试：把从 `lib.rs` 抽离出来的 `#[cfg(test)]`
//! 测试单独放到本文件，让 `lib.rs` 的生产代码不被千行测试淹没。
//!
//! `use super::*;` 拉入 crate root 的 pub 符号（`runtime_*` / `LeaseTokenSigner`
//! / DTO / 契约常量等）；下面再显式从外部 crate import `api_contracts` /
//! `chrono` / `license_service` 的类型，避免依赖"lib.rs 以前顺带 `use` 的
//! 私有符号通过同一个模块传递"这种脆弱的耦合。

use super::*;
use crate::runtime::{device_id_matches_fingerprint, now_iso};
use api_contracts::{LicenseState, Rg, LICENSE_TASK_REVIEW_FIND};
use base64::engine::general_purpose::STANDARD;
use base64::Engine;
use chrono::Utc;
use ed25519_dalek::pkcs8::EncodePrivateKey;
use ed25519_dalek::SigningKey;
use license_service::LeaseVerifier;
use license_service::{
    ActivationInput, AuditEvent, DeviceRegistration, GeneratedKeyRecord, GeneratedKeyStatus,
    LicenseRecord, VerifyInput,
};
use std::collections::HashMap;
use std::sync::Mutex;

#[derive(Default)]
struct Repo {
    generated_keys: Mutex<HashMap<String, GeneratedKeyRecord>>,
    licenses: Mutex<HashMap<String, LicenseRecord>>,
    registrations: Mutex<HashMap<(String, String), DeviceRegistration>>,
    sessions: Mutex<HashMap<String, String>>,
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

    async fn save_device_registration(&self, record: &DeviceRegistration) -> anyhow::Result<()> {
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
        self.sessions
            .lock()
            .unwrap()
            .insert(license_key.to_string(), revoked_at.to_string());
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
        self.sessions
            .lock()
            .unwrap()
            .insert(license_key.to_string(), revoked_at.to_string());
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
    assert_eq!(
        revoke_error_contract("secret_missing"),
        (503, "secret_missing")
    );
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

#[test]
fn revoke_generated_key_sql_covers_metadata_and_fallback_paths() {
    assert_eq!(
        revoke_generated_key_update_sql(true),
        REVOKE_GENERATED_KEY_SQL_WITH_METADATA
    );
    assert_eq!(
        revoke_generated_key_update_sql(false),
        REVOKE_GENERATED_KEY_SQL_FALLBACK
    );
    assert!(revoke_generated_key_update_sql(true).contains("revoked_at = ?"));
    assert!(!revoke_generated_key_update_sql(false).contains("revoked_at = ?"));
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
            device_id: "858c06cf9c505c9f".into(),
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
            device_id: "858c06cf9c505c9f".into(),
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
            device_id: "858c06cf9c505c9f".into(),
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
            device_id: "858c06cf9c505c9f".into(),
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
            device_id: "858c06cf9c505c9f".into(),
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
            device_id: "858c06cf9c505c9f".into(),
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
        r#"{"license_key":"TLS-TEST","device_id":"858c06cf9c505c9f","device_fingerprint":"fp-1","client_version":"5.0.0"}"#,
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
            Some("858c06cf9c505c9f"),
            now.timestamp(),
            false,
        )
        .unwrap();
    assert_eq!(verified_lease.device_id, "858c06cf9c505c9f");

    let grant = handle_async_runtime_json(
        &repo,
        "/api/task/authorize",
        r#"{"license_key":"TLS-TEST","device_id":"858c06cf9c505c9f","task_type":"review_find","client_version":"5.2.0"}"#,
        Some(&signer),
        now,
    )
    .await
    .unwrap();
    let grant_payload: Rg = serde_json::from_str(&grant).unwrap();
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
        r#"{"license_key":"TLS-TEST","device_id":"858c06cf9c505c9f","device_fingerprint":"fp-1","client_version":"5.0.0"}"#,
        Some(&signer),
        now,
    )
    .await
    .unwrap();

    let verified = handle_async_runtime_json(
        &repo,
        "/api/verify",
        r#"{"license_key":"TLS-TEST","device_id":"858c06cf9c505c9f","client_version":"5.1.0"}"#,
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
        r#"{"license_key":"TLS-TEST","device_id":"858c06cf9c505c9f","current_issued_at":1713312000}"#,
        Some(&signer),
        now,
    )
    .await
    .unwrap();
    let refreshed_payload: Lrr = serde_json::from_str(&refreshed).unwrap();
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
        r#"{"license_key":"TLS-TEST","device_id":"858c06cf9c505c9f","reason":"admin"}"#,
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
        r#"{"license_key":"TLS-TEST","device_id":"858c06cf9c505c9f","device_fingerprint":"fp-1","client_version":"5.0.0"}"#,
        Some(&signer),
        now,
    )
    .await
    .unwrap();

    let revoked = handle_async_runtime_json(
        &repo,
        "/api/lease/revoke",
        r#"{"license_key":"TLS-TEST","device_id":"858c06cf9c505c9f","reason":"admin"}"#,
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
        r#"{"license_key":"TLS-TEST","device_id":"858c06cf9c505c9f","client_version":"5.1.0"}"#,
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
        r#"{"license_key":"TLS-TEST","device_id":"858c06cf9c505c9f","current_issued_at":1713312000}"#,
        Some(&signer),
        now,
    )
    .await
    .unwrap();
    let refreshed_payload: Lrr = serde_json::from_str(&refreshed).unwrap();
    assert!(!refreshed_payload.success);
    assert_eq!(refreshed_payload.message, "该卡密已被吊销");

    let grant = handle_async_runtime_json(
        &repo,
        "/api/task/authorize",
        r#"{"license_key":"TLS-TEST","device_id":"858c06cf9c505c9f","task_type":"review_find","client_version":"5.2.0"}"#,
        None,
        now,
    )
    .await
    .unwrap();
    let grant_payload: Rg = serde_json::from_str(&grant).unwrap();
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
        r#"{"license_key":"TLS-TEST","device_id":"858c06cf9c505c9f","device_fingerprint":"fp-1","client_version":"5.0.0"}"#,
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
    assert_eq!(
        revoked_payload.device_id.as_deref(),
        Some("858c06cf9c505c9f")
    );

    let verified = handle_async_runtime_json(
        &repo,
        "/api/verify",
        r#"{"license_key":"TLS-TEST","device_id":"858c06cf9c505c9f","client_version":"5.1.0"}"#,
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
    assert_eq!(
        revoke_error_contract(&err.to_string()),
        (400, "invalid_json")
    );
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
        r#"{"license_key":"TLS-TEST","device_id":"858c06cf9c505c9f","device_fingerprint":"fp-1","client_version":"5.0.0"}"#,
        Some(&signer),
        now,
    )
    .await
    .unwrap();

    repo.save_device_registration(&DeviceRegistration {
        license_key: "TLS-TEST".into(),
        device_id: "f333ed59bc52e256".into(),
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
            .get(&("TLS-TEST".into(), "858c06cf9c505c9f".into()))
            .map(|value| value.registration_status.as_str()),
        Some("revoked")
    );
    assert_eq!(
        registrations
            .get(&("TLS-TEST".into(), "f333ed59bc52e256".into()))
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
        r#"{"license_key":"TLS-MISSING","device_id":"858c06cf9c505c9f","reason":"admin"}"#,
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
async fn runtime_revoke_persists_repo_side_effects_and_audit_event() {
    let repo = Repo::seeded();
    let signer = test_signer();
    let now = chrono::DateTime::parse_from_rfc3339("2026-04-17T00:00:00Z")
        .unwrap()
        .with_timezone(&Utc);

    let _ = handle_async_runtime_json(
        &repo,
        "/api/activate",
        r#"{"license_key":"TLS-TEST","device_id":"858c06cf9c505c9f","device_fingerprint":"fp-1","client_version":"5.0.0"}"#,
        Some(&signer),
        now,
    )
    .await
    .unwrap();

    repo.sessions
        .lock()
        .unwrap()
        .insert("TLS-TEST".into(), String::new());

    let revoked = handle_async_runtime_json(
        &repo,
        "/api/lease/revoke",
        r#"{"license_key":"TLS-TEST","device_id":"858c06cf9c505c9f","reason":"admin"}"#,
        None,
        now,
    )
    .await
    .unwrap();
    let payload: SignedLicenseApiResponse = serde_json::from_str(&revoked).unwrap();
    assert!(payload.success);
    let revoked_at = now_iso(now);

    let generated = repo.generated_keys.lock().unwrap();
    assert_eq!(
        generated.get("TLS-TEST").map(|value| value.status.clone()),
        Some(GeneratedKeyStatus::Revoked)
    );
    drop(generated);

    let licenses = repo.licenses.lock().unwrap();
    let record = licenses.get("TLS-TEST").unwrap();
    assert_eq!(record.status, LicenseState::Revoked);
    assert_eq!(record.updated_at, revoked_at);
    assert_eq!(record.last_verify_at, revoked_at);
    drop(licenses);

    let registrations = repo.registrations.lock().unwrap();
    let registration = registrations
        .get(&("TLS-TEST".into(), "858c06cf9c505c9f".into()))
        .unwrap();
    assert_eq!(registration.registration_status, "revoked");
    assert_eq!(registration.last_seen_at, revoked_at);
    drop(registrations);

    let sessions = repo.sessions.lock().unwrap();
    assert_eq!(
        sessions.get("TLS-TEST").map(|value| value.as_str()),
        Some(revoked_at.as_str())
    );
    drop(sessions);

    let audits = repo.audits.lock().unwrap();
    let audit = audits.last().unwrap();
    assert_eq!(audit.action, "lease_revoke");
    assert_eq!(audit.license_key, "TLS-TEST");
    assert_eq!(audit.device_id, "858c06cf9c505c9f");
    assert_eq!(audit.reason, "admin");
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
        r#"{"license_key":"   ","device_id":"858c06cf9c505c9f","reason":"admin"}"#,
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

// ---- runtime_task_authorize：拒绝路径集成回归（Pass 4 · T15） -----------

/// 公共 helper：激活一把默认许可证，返回 (repo, signer, now)。
async fn activate_default_license() -> (Repo, LeaseTokenSigner, chrono::DateTime<Utc>) {
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
            device_id: "858c06cf9c505c9f".into(),
            device_fingerprint: "fp-1".into(),
            client_version: "5.0.0".into(),
        },
        now,
    )
    .await
    .unwrap();
    assert!(activated.success);
    (repo, signer, now)
}

#[tokio::test]
async fn task_authorize_rejects_unknown_task_type_with_degraded_reason() {
    let (repo, _signer, now) = activate_default_license().await;
    let grant = runtime_task_authorize(
        &repo,
        TaskAuthorizeRequest {
            license_key: "TLS-TEST".into(),
            device_id: "858c06cf9c505c9f".into(),
            task_type: "malicious_unknown_task".into(),
            client_version: "5.2.0".into(),
        },
        now,
    )
    .await
    .unwrap();
    assert!(!grant.granted, "未在 policy 白名单内的 task 必须被拒");
    assert_eq!(grant.task_type, "malicious_unknown_task");
    assert!(grant.grant_id.is_empty());
    assert!(grant.valid_until.is_empty());
    assert!(
        grant.degraded_reason.is_some(),
        "拒绝原因必须附在 degraded_reason 上方便前端显示",
    );
}

#[tokio::test]
async fn task_authorize_rejects_when_device_id_does_not_match_activated_binding() {
    let (repo, _signer, now) = activate_default_license().await;
    let grant = runtime_task_authorize(
        &repo,
        TaskAuthorizeRequest {
            license_key: "TLS-TEST".into(),
            device_id: "device-other".into(),
            task_type: LICENSE_TASK_REVIEW_FIND.into(),
            client_version: "5.2.0".into(),
        },
        now,
    )
    .await
    .unwrap();
    assert!(
        !grant.granted,
        "device 绑定不一致必须拒绝（防止 Lease 被移植到其它设备）",
    );
    assert_eq!(grant.task_type, LICENSE_TASK_REVIEW_FIND);
    assert!(grant.grant_id.is_empty());
    assert!(grant.degraded_reason.is_some());
}

#[tokio::test]
async fn task_authorize_rejects_after_license_is_revoked() {
    let (repo, _signer, now) = activate_default_license().await;

    // 先调用一次确认活跃状态
    let granted = runtime_task_authorize(
        &repo,
        TaskAuthorizeRequest {
            license_key: "TLS-TEST".into(),
            device_id: "858c06cf9c505c9f".into(),
            task_type: LICENSE_TASK_REVIEW_FIND.into(),
            client_version: "5.2.0".into(),
        },
        now,
    )
    .await
    .unwrap();
    assert!(granted.granted);

    // 管理员吊销
    runtime_revoke(
        &repo,
        LeaseRevokeRequest {
            license_key: "TLS-TEST".into(),
            device_id: "858c06cf9c505c9f".into(),
            reason: "admin_revoke_test".into(),
        },
        now,
    )
    .await
    .unwrap();

    // 吊销后再申请任务授权应被拒
    let after_revoke = runtime_task_authorize(
        &repo,
        TaskAuthorizeRequest {
            license_key: "TLS-TEST".into(),
            device_id: "858c06cf9c505c9f".into(),
            task_type: LICENSE_TASK_REVIEW_FIND.into(),
            client_version: "5.2.0".into(),
        },
        now,
    )
    .await
    .unwrap();
    assert!(
        !after_revoke.granted,
        "license 吊销后再次 authorize 必须拒绝",
    );
    assert!(after_revoke.grant_id.is_empty());
    assert!(after_revoke.degraded_reason.is_some());
}

// ---- 激活时 device_id / device_fingerprint 自洽校验（审计报告 H2） -----

#[test]
fn device_id_matches_fingerprint_accepts_canonical_pair() {
    // "fp-1" 的 SHA-256 前 8 字节是 858c06cf9c505c9f
    assert!(device_id_matches_fingerprint("858c06cf9c505c9f", "fp-1"));
    // 大小写 / 前后空白均容忍
    assert!(device_id_matches_fingerprint(
        "  858C06CF9C505C9F  ",
        "fp-1"
    ));
}

#[test]
fn device_id_matches_fingerprint_rejects_tampered_pair() {
    // device_id 与 fingerprint 不匹配
    assert!(!device_id_matches_fingerprint("0000000000000000", "fp-1"));
    // fingerprint 非空时 device_id 空也应拒
    assert!(!device_id_matches_fingerprint("", "fp-1"));
    // device_id 长度不足 16
    assert!(!device_id_matches_fingerprint("858c06cf", "fp-1"));
}

#[test]
fn device_id_matches_fingerprint_bypasses_empty_fingerprint() {
    // 空指纹的兜底兼容：client 采集失败场景不被硬拒
    assert!(device_id_matches_fingerprint("whatever", ""));
    assert!(device_id_matches_fingerprint("", ""));
}

#[tokio::test]
async fn runtime_activate_rejects_mismatched_device_id_fingerprint() {
    let repo = Repo::seeded();
    let signer = test_signer();
    let now = chrono::DateTime::parse_from_rfc3339("2026-04-17T00:00:00Z")
        .unwrap()
        .with_timezone(&Utc);

    // 攻击者声称 device_id = 858c06cf9c505c9f（本应对应 fp="fp-1"），
    // 但实际发送 device_fingerprint = "tampered-fp"（其 SHA[..16] 完全不同）
    let response = runtime_activate(
        &repo,
        &signer,
        ActivationInput {
            license_key: "TLS-TEST".into(),
            device_id: "858c06cf9c505c9f".into(),
            device_fingerprint: "tampered-fp".into(),
            client_version: "5.2.0".into(),
        },
        now,
    )
    .await
    .unwrap();

    assert!(!response.success, "不自洽的设备凭证必须拒绝");
    assert_eq!(response.license_state, LicenseState::DeviceMismatch);
    assert!(response.license_lease.is_none());
    assert!(
        response.message.contains("不自洽"),
        "拒绝原因应明确指示 device_id/device_fingerprint 不匹配：{}",
        response.message
    );

    // 确认 D1 侧未创建任何污染数据
    assert!(
        repo.licenses.lock().unwrap().get("TLS-TEST").is_none(),
        "未通过自洽校验的激活不应写入 activations"
    );
    assert!(
        repo.audits.lock().unwrap().is_empty(),
        "未通过自洽校验的激活不应记录审计事件"
    );
}

#[tokio::test]
async fn runtime_activate_still_accepts_canonical_pair_after_h2() {
    // H2 校验不能误杀正常 client 的激活流程（正向回归）
    let repo = Repo::seeded();
    let signer = test_signer();
    let now = chrono::DateTime::parse_from_rfc3339("2026-04-17T00:00:00Z")
        .unwrap()
        .with_timezone(&Utc);

    let response = runtime_activate(
        &repo,
        &signer,
        ActivationInput {
            license_key: "TLS-TEST".into(),
            device_id: "858c06cf9c505c9f".into(),
            device_fingerprint: "fp-1".into(),
            client_version: "5.2.0".into(),
        },
        now,
    )
    .await
    .unwrap();

    assert!(response.success);
    assert!(response.license_lease.is_some());
    assert_eq!(response.license_state, LicenseState::Active);
}
