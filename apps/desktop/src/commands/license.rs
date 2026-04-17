use crate::adapters::http_license_client::{
    normalize_license_state, HttpLicenseClient, LicenseApiResponse,
};
use crate::app_settings::LICENSE_API_BASE_URLS;
use crate::error::AppError;
use crate::state::{self, AppState, StoredLicenseProfile};
use api_contracts::{LeasePayload, LicenseState, RiskLevel, RuntimeGrant, RuntimeState};
use license_service::{authorize_task_local, lease::RefreshOutcome};
use sha2::{Digest, Sha256};
use std::ffi::CStr;
use tauri::State;

const LICENSE_PROTOCOL_VERSION: u32 = 3;

fn runtime_state_allows_feature(runtime: &RuntimeState) -> bool {
    runtime.reason.is_locally_allowed()
}

pub async fn ensure_feature_authorized(
    state: &AppState,
    feature_name: &str,
) -> Result<(), AppError> {
    ensure_runtime_integrity(state).await?;
    let _ = refresh_runtime_license_if_needed(state).await;
    let runtime = state.runtime_license_state.lock().await.clone();
    if runtime_state_allows_feature(&runtime) {
        return Ok(());
    }

    let detail = license_state_detail(runtime.reason);
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

fn license_state_detail(state: LicenseState) -> &'static str {
    match state {
        LicenseState::Expired => "当前授权已过期，请续费后再试",
        LicenseState::Revoked => "当前授权已吊销，请联系管理员",
        LicenseState::DeviceMismatch => "当前设备与授权不匹配，请重新激活",
        LicenseState::ReactivationRequired => "当前设备授权需要重新激活，请重新绑定",
        LicenseState::Compromised => "当前授权状态异常，请联系管理员",
        LicenseState::Invalid | LicenseState::NotFound => "请先激活授权后再使用此功能",
        LicenseState::OnlineRefreshRequired => "当前授权需要联网刷新，请稍后重试",
        _ => "当前授权不可用，请检查授权状态后重试",
    }
}

fn parse_license_state(raw: &str) -> LicenseState {
    match normalize_license_state(raw).as_str() {
        "active" => LicenseState::Active,
        "renewal_due" => LicenseState::RenewalDue,
        "expired" => LicenseState::Expired,
        "revoked" => LicenseState::Revoked,
        "device_mismatch" => LicenseState::DeviceMismatch,
        "reactivation_required" => LicenseState::ReactivationRequired,
        "online_refresh_required" => LicenseState::OnlineRefreshRequired,
        "compromised" => LicenseState::Compromised,
        "not_found" => LicenseState::NotFound,
        _ => LicenseState::Invalid,
    }
}

async fn persist_runtime_profile(
    state: &AppState,
    runtime: RuntimeState,
    fallback_license_key: String,
    fallback_license_expires_at: Option<String>,
) -> Result<StoredLicenseProfile, AppError> {
    let last_verified_at = current_timestamp();
    let profile = StoredLicenseProfile {
        license_key: if runtime.license_key.trim().is_empty() {
            fallback_license_key
        } else {
            runtime.license_key.clone()
        },
        license_state: state::runtime_state_to_license_state(&runtime),
        license_expires_at: if runtime.license_expires_at.trim().is_empty() {
            fallback_license_expires_at
        } else {
            Some(runtime.license_expires_at.clone())
        },
        last_verified_at: Some(last_verified_at),
    };

    {
        let mut current = state.runtime_license_state.lock().await;
        *current = runtime;
    }
    persist_license_profile(state, profile.clone()).await?;
    Ok(profile)
}

async fn sync_license_state_from_response(
    state: &AppState,
    requested_key: &str,
    response: &LicenseApiResponse,
) -> Result<StoredLicenseProfile, AppError> {
    let normalized_key = response
        .license_key
        .clone()
        .unwrap_or_else(|| requested_key.trim().to_uppercase());

    if let Some(token) = response
        .license_lease
        .as_deref()
        .map(str::trim)
        .filter(|token| !token.is_empty())
    {
        let runtime = state::verify_and_store_license_token(
            state.lease_store.as_ref(),
            token,
            &state.device_id,
            chrono::Utc::now().timestamp(),
            &state.lease_verifier,
        )
        .map_err(|err| AppError::Message(format!("授权 Lease 校验失败：{err}")))?;

        return persist_runtime_profile(
            state,
            runtime,
            normalized_key,
            response.license_expires_at.clone(),
        )
        .await;
    }

    let reason = parse_license_state(&response.normalized_state());
    if reason.is_locally_allowed() {
        return Err(AppError::Message(
            "授权服务未返回签名 Lease，已拒绝信任裸授权状态".to_string(),
        ));
    }

    state
        .lease_store
        .delete()
        .map_err(|err| AppError::Message(format!("清理本地授权材料失败：{err}")))?;

    persist_runtime_profile(
        state,
        RuntimeState::reason_only(reason),
        normalized_key,
        response.license_expires_at.clone(),
    )
    .await
}

fn parse_runtime_from_token(
    state: &AppState,
    token: &str,
    now_epoch: i64,
    allow_expired: bool,
) -> Result<LeasePayload, AppError> {
    state
        .lease_verifier
        .verify(token, Some(&state.device_id), now_epoch, allow_expired)
        .map_err(|err| AppError::Message(format!("Lease 解析失败：{err}")))
}

async fn update_runtime_from_token(
    state: &AppState,
    token: &str,
    fallback_license_key: String,
    fallback_license_expires_at: Option<String>,
) -> Result<StoredLicenseProfile, AppError> {
    let runtime = state::verify_and_store_license_token(
        state.lease_store.as_ref(),
        token,
        &state.device_id,
        chrono::Utc::now().timestamp(),
        &state.lease_verifier,
    )
    .map_err(|err| AppError::Message(format!("更新本地 Lease 失败：{err}")))?;

    persist_runtime_profile(
        state,
        runtime,
        fallback_license_key,
        fallback_license_expires_at,
    )
    .await
}

async fn refresh_runtime_license_if_needed(state: &AppState) -> Result<(), AppError> {
    let token = match state.lease_store.get() {
        Ok(Some(token)) if !token.trim().is_empty() => token,
        Ok(_) => return Ok(()),
        Err(err) => return Err(AppError::Message(format!("读取本地 Lease 失败：{err}"))),
    };

    let now_epoch = chrono::Utc::now().timestamp();
    let payload = parse_runtime_from_token(state, &token, now_epoch, true)?;
    let profile = state.license_profile.lock().await.clone();
    let client = make_client();

    let outcome = license_service::lease::refresh_lease_if_due(&payload, now_epoch, |req| async move {
        let response = client
            .refresh_lease(&req.license_key, &req.device_id, req.current_issued_at)
            .await
            .map_err(|err| err.to_string())?;
        if !response.success {
            return Err(response.message);
        }
        Ok(license_service::lease::RefreshResponse {
            new_token: response.new_token,
        })
    })
    .await;

    match outcome {
        Ok(RefreshOutcome::NotDue) => Ok(()),
        Ok(RefreshOutcome::Renewed(new_token)) => {
            update_runtime_from_token(
                state,
                &new_token,
                if payload.license_key.is_empty() {
                    profile.license_key
                } else {
                    payload.license_key
                },
                profile.license_expires_at,
            )
            .await?;
            Ok(())
        }
        Err(license_service::lease::RefreshError::Network(_)) => Ok(()),
        Err(license_service::lease::RefreshError::HardExpired) => {
            persist_runtime_profile(
                state,
                RuntimeState::reason_only(LicenseState::Expired),
                profile.license_key,
                profile.license_expires_at,
            )
            .await?;
            Ok(())
        }
        Err(err) => Err(AppError::Message(format!("Lease 续约失败：{err}"))),
    }
}

async fn mark_runtime_compromised(
    state: &AppState,
    detail: String,
) -> Result<(), AppError> {
    let profile = state.license_profile.lock().await.clone();
    persist_runtime_profile(
        state,
        RuntimeState {
            reason: LicenseState::Compromised,
            status_hint: LicenseState::Compromised,
            compromised: true,
            runtime_backend: "rust".to_string(),
            ..RuntimeState::default()
        },
        profile.license_key,
        profile.license_expires_at,
    )
    .await?;
    Err(AppError::Message(format!("完整性校验失败：{detail}")))
}

pub async fn ensure_runtime_integrity(state: &AppState) -> Result<(), AppError> {
    if let Err(err) = state::validate_integrity_if_present(state.integrity_manifest_path.as_deref()) {
        return mark_runtime_compromised(state, err).await;
    }
    Ok(())
}

fn next_grant_id() -> String {
    format!(
        "grant-{}-{}",
        chrono::Utc::now().timestamp_millis(),
        rand::random::<u32>()
    )
}

fn task_requires_remote_authorization(task_type: &str) -> bool {
    matches!(
        task_type,
        api_contracts::LICENSE_TASK_BATCH_DELIVERY
            | api_contracts::LICENSE_TASK_REVIEW_FULL_SCAN
            | api_contracts::LICENSE_TASK_CACHE_MANAGE
    )
}

pub async fn authorize_runtime_task(
    state: &AppState,
    task_type: &str,
) -> Result<RuntimeGrant, AppError> {
    ensure_runtime_integrity(state).await?;
    let _ = refresh_runtime_license_if_needed(state).await;
    let now_epoch = chrono::Utc::now().timestamp();
    if let Some(grant) = state.task_grant_cache.get_valid(task_type, now_epoch) {
        return Ok(grant);
    }

    let token = state
        .lease_store
        .get()
        .map_err(|err| AppError::Message(format!("读取本地 Lease 失败：{err}")))?
        .ok_or_else(|| AppError::Message("当前缺少有效 Lease，请重新激活".to_string()))?;
    let payload = parse_runtime_from_token(state, &token, now_epoch, false)?;

    let local_grant = authorize_task_local(&payload, task_type, now_epoch, next_grant_id);
    let needs_remote = task_requires_remote_authorization(task_type)
        || match &local_grant {
            Ok(grant) => grant.risk_level == Some(RiskLevel::High),
            Err(_) => true,
        };

    let grant = if needs_remote {
        let client = make_client();
        client
            .authorize_task(
                &payload.license_key,
                &state.device_id,
                task_type,
                env!("CARGO_PKG_VERSION"),
            )
            .await
            .map_err(|err| AppError::Message(format!("任务授权失败：{err}")))?
    } else {
        local_grant.map_err(|err| AppError::Message(format!("任务授权失败：{err}")))?
    };

    state.task_grant_cache.put(grant.clone());
    Ok(grant)
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

    let profile = sync_license_state_from_response(&state, &license_key, &resp).await?;

    Ok(serde_json::json!({
        "success": resp.success,
        "message": resp.message,
        "license_state": profile.license_state,
        "license_key": profile.license_key,
        "device_id": resp.device_id,
        "license_expires_at": profile.license_expires_at,
        "license_lease": resp.license_lease.is_some(),
        "last_verified_at": profile.last_verified_at,
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

    let profile = sync_license_state_from_response(&state, &license_key, &resp).await?;

    Ok(serde_json::json!({
        "success": resp.success,
        "message": resp.message,
        "license_state": profile.license_state,
        "license_key": profile.license_key,
        "license_expires_at": profile.license_expires_at,
        "license_lease": resp.license_lease.is_some(),
        "last_verified_at": profile.last_verified_at,
    }))
}

#[tauri::command(rename_all = "snake_case")]
pub async fn get_license_status(state: State<'_, AppState>) -> Result<serde_json::Value, AppError> {
    let _ = ensure_runtime_integrity(&state).await;
    let _ = refresh_runtime_license_if_needed(&state).await;
    let runtime = state.runtime_license_state.lock().await.clone();
    let profile = state.license_profile.lock().await.clone();
    Ok(build_license_status_payload(&profile, &runtime))
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

fn build_license_status_payload(
    profile: &StoredLicenseProfile,
    runtime: &RuntimeState,
) -> serde_json::Value {
    let license_key = if runtime.license_key.trim().is_empty() {
        profile.license_key.clone()
    } else {
        runtime.license_key.clone()
    };
    let license_state = if matches!(runtime.reason, LicenseState::NotFound)
        && runtime.license_key.trim().is_empty()
        && profile.license_key.trim().is_empty()
    {
        "invalid".to_string()
    } else {
        state::runtime_state_to_license_state(runtime)
    };
    let license_expires_at = if runtime.license_expires_at.trim().is_empty() {
        profile.license_expires_at.clone()
    } else {
        Some(runtime.license_expires_at.clone())
    };
    let last_verified_at = profile
        .last_verified_at
        .clone()
        .filter(|value| !value.trim().is_empty())
        .or_else(|| {
            (!runtime.last_verify_at.trim().is_empty()).then(|| runtime.last_verify_at.clone())
        });

    serde_json::json!({
        "configured": !license_key.is_empty(),
        "license_key": license_key,
        "license_state": license_state,
        "license_expires_at": license_expires_at,
        "last_verified_at": last_verified_at,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::adapters::secure_storage::{InMemorySecretStore, SecretStore};
    use crate::state::AppState;
    use base64::engine::general_purpose::URL_SAFE_NO_PAD;
    use base64::Engine;
    use ed25519_dalek::{Signer, SigningKey};
    use std::path::PathBuf;
    use std::sync::Arc;
    use tokio::sync::Mutex;

    fn signed_lease_token(device_id: &str, renew_after: i64, exp: i64) -> (String, String) {
        let signing_key = SigningKey::generate(&mut rand::rngs::OsRng);
        let verifying_key_b64 = URL_SAFE_NO_PAD.encode(signing_key.verifying_key().as_bytes());
        let payload = serde_json::json!({
            "kind": "license_lease",
            "license_key": "TLS-TEST",
            "device_id": device_id,
            "issued_at": 1_700_000_000i64,
            "exp": exp,
            "renew_after": renew_after,
            "task_policy": ["review_find"],
            "risk_level": "low",
        });
        let payload_bytes = serde_json::to_vec(&payload).unwrap();
        let payload_b64 = URL_SAFE_NO_PAD.encode(payload_bytes);
        let signature = signing_key.sign(payload_b64.as_bytes());
        let signature_b64 = URL_SAFE_NO_PAD.encode(signature.to_bytes());
        (format!("{payload_b64}.{signature_b64}"), verifying_key_b64)
    }

    fn test_state(
        device_id: &str,
        store: Arc<dyn SecretStore>,
        public_key_b64: &str,
    ) -> AppState {
        AppState {
            cookie_profile: Mutex::new(Default::default()),
            cookie_path: Mutex::new(PathBuf::from(".")),
            app_home_dir: std::env::temp_dir(),
            integrity_manifest_path: None,
            device_id: device_id.to_string(),
            lease_store: store,
            lease_verifier: license_service::LeaseVerifier::from_public_key_b64(public_key_b64)
                .unwrap(),
            task_grant_cache: license_service::TaskGrantCache::new(),
            runtime_license_state: Mutex::new(RuntimeState::reason_only(LicenseState::Invalid)),
            license_profile: Mutex::new(StoredLicenseProfile::default()),
        }
    }

    #[test]
    fn runtime_state_allows_active_and_renewal_due_only() {
        assert!(runtime_state_allows_feature(&RuntimeState::reason_only(
            LicenseState::Active
        )));
        assert!(runtime_state_allows_feature(&RuntimeState {
            reason: LicenseState::Active,
            status_hint: LicenseState::RenewalDue,
            ..RuntimeState::default()
        }));

        for state in [
            LicenseState::Invalid,
            LicenseState::Expired,
            LicenseState::Revoked,
            LicenseState::DeviceMismatch,
            LicenseState::Compromised,
        ] {
            assert!(
                !runtime_state_allows_feature(&RuntimeState::reason_only(state)),
                "state {state:?} should be blocked"
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

    #[test]
    fn parse_license_state_supports_reactivation_required() {
        assert_eq!(
            parse_license_state("reactivation_required"),
            LicenseState::ReactivationRequired
        );
        assert_eq!(parse_license_state("ok"), LicenseState::Active);
    }

    #[tokio::test]
    async fn sync_license_state_accepts_signed_lease_and_updates_runtime_gate() {
        let store: Arc<dyn SecretStore> = Arc::new(InMemorySecretStore::new());
        let (token, public_key_b64) = signed_lease_token("dev-1", 1_800_000_000, 1_900_000_000);
        let state = test_state("dev-1", store.clone(), &public_key_b64);
        let response = LicenseApiResponse {
            success: true,
            message: "ok".into(),
            license_state: "active".into(),
            license_lease: Some(token.clone()),
            license_expires_at: Some("2030-01-01T00:00:00Z".into()),
            activated_at: None,
            device_id: Some("dev-1".into()),
            license_key: Some("TLS-TEST".into()),
            lease_expires_at: None,
            renew_after: None,
            issued_at: None,
            license_status: None,
            task_policy: None,
        };

        let profile = sync_license_state_from_response(&state, "tls-test", &response)
            .await
            .expect("signed lease should be accepted");

        assert_eq!(profile.license_state, "active");
        assert_eq!(store.get().unwrap().as_deref(), Some(token.as_str()));
        assert_eq!(
            state.runtime_license_state.lock().await.reason,
            LicenseState::Active
        );
    }

    #[tokio::test]
    async fn sync_license_state_rejects_allowed_state_without_signed_lease() {
        let store: Arc<dyn SecretStore> = Arc::new(InMemorySecretStore::new());
        let state = test_state(
            "dev-1",
            store,
            "H0KTidHIXV0nvzkUNmssrx5t5IrUvEQi1WVelkuCJm8",
        );
        let response = LicenseApiResponse {
            success: true,
            message: "ok".into(),
            license_state: "active".into(),
            license_lease: None,
            license_expires_at: Some("2030-01-01T00:00:00Z".into()),
            activated_at: None,
            device_id: None,
            license_key: Some("TLS-TEST".into()),
            lease_expires_at: None,
            renew_after: None,
            issued_at: None,
            license_status: None,
            task_policy: None,
        };

        let err = sync_license_state_from_response(&state, "tls-test", &response)
            .await
            .expect_err("bare active state must be rejected");
        assert!(err.to_string().contains("未返回签名 Lease"));
    }

    #[tokio::test]
    async fn authorize_runtime_task_uses_local_policy_and_cache() {
        let store: Arc<dyn SecretStore> = Arc::new(InMemorySecretStore::new());
        let (token, public_key_b64) = signed_lease_token("dev-1", 1_900_000_000, 1_999_999_999);
        store.set(&token).unwrap();
        let state = test_state("dev-1", store, &public_key_b64);
        {
            let mut runtime = state.runtime_license_state.lock().await;
            *runtime = RuntimeState::reason_only(LicenseState::Active);
        }

        let first = authorize_runtime_task(&state, api_contracts::LICENSE_TASK_REVIEW_FIND)
            .await
            .expect("local grant should be issued");
        let second = authorize_runtime_task(&state, api_contracts::LICENSE_TASK_REVIEW_FIND)
            .await
            .expect("cached grant should be reused");

        assert!(first.granted);
        assert_eq!(first.grant_id, second.grant_id);
        assert_eq!(first.task_type, api_contracts::LICENSE_TASK_REVIEW_FIND);
    }

    #[test]
    fn high_risk_tasks_require_remote_authorization() {
        assert!(task_requires_remote_authorization(
            api_contracts::LICENSE_TASK_BATCH_DELIVERY
        ));
        assert!(task_requires_remote_authorization(
            api_contracts::LICENSE_TASK_REVIEW_FULL_SCAN
        ));
        assert!(task_requires_remote_authorization(
            api_contracts::LICENSE_TASK_CACHE_MANAGE
        ));
        assert!(!task_requires_remote_authorization(
            api_contracts::LICENSE_TASK_REVIEW_FIND
        ));
    }

    #[test]
    fn build_license_status_prefers_runtime_snapshot_over_legacy_profile() {
        let profile = StoredLicenseProfile {
            license_key: "OLD-KEY".into(),
            license_state: "invalid".into(),
            license_expires_at: Some("2020-01-01T00:00:00Z".into()),
            last_verified_at: Some("2026-04-16T10:00:00Z".into()),
        };
        let runtime = RuntimeState {
            license_key: "TLS-TEST".into(),
            device_id: "dev-1".into(),
            reason: LicenseState::Active,
            status_hint: LicenseState::RenewalDue,
            license_expires_at: "2030-01-01T00:00:00Z".into(),
            lease_expires_at: "2030-01-01T00:00:00Z".into(),
            renew_after: "2029-12-01T00:00:00Z".into(),
            task_policy: vec![],
            risk_level: "low".into(),
            runtime_backend: "rust".into(),
            compromised: false,
            last_verify_at: String::new(),
        };

        let payload = build_license_status_payload(&profile, &runtime);
        assert_eq!(payload["configured"], true);
        assert_eq!(payload["license_key"], "TLS-TEST");
        assert_eq!(payload["license_state"], "renewal_due");
        assert_eq!(payload["license_expires_at"], "2030-01-01T00:00:00Z");
        assert_eq!(payload["last_verified_at"], "2026-04-16T10:00:00Z");
    }
}
