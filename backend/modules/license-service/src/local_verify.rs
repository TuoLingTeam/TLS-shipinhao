//! 本地离线授权校验（PRD §5.10 / M2-10）。
//!
//! 组合 `LeaseVerifier` + 本地存储读出的 Lease Token，产出前端可直接消费的
//! `RuntimeState`；整个链路不发起任何网络请求，确保飞行模式下 72 小时内
//! 仍能让用户正常使用。
//!
//! 本模块不直接依赖 Keychain/文件后备——调用方（`apps/desktop` 的 Tauri
//! 命令层）负责读取 Token 后作为 `Option<&str>` 传入。这样 license-service
//! 无需依赖任何 IO，方便纯函数测试。

use api_contracts::{LeasePayload, LicenseState, RuntimeState};
use chrono::{DateTime, Utc};

use crate::lease::{LeaseError, LeaseVerifier};

const RUNTIME_BACKEND: &str = "rust";

/// 组合本地 Lease + 设备指纹 + 当前时间，产出 `RuntimeState`。
///
/// 语义（与 PRD §5.10 对齐）：
/// - `lease_token` 为 None / 空 → `RuntimeState.reason = NotFound`
/// - 设备指纹不匹配 → `DeviceMismatch`
/// - 签名/格式/kind 错误 → `Invalid`
/// - 硬过期 → `Expired`
/// - 未过期但进入软刷新窗口 → `Active`（主状态） + `RenewalDue`（UI 提示）
/// - 正常 → `Active`（reason = status_hint）
///
/// 返回的 RuntimeState 永远带有完整字段；错误分支只填 reason，其余为默认值。
pub fn verify_stored_lease_local(
    lease_token: Option<&str>,
    device_id: &str,
    now_epoch: i64,
    verifier: &LeaseVerifier,
) -> RuntimeState {
    let token = match lease_token.map(str::trim) {
        Some(t) if !t.is_empty() => t,
        _ => return RuntimeState::reason_only(LicenseState::NotFound),
    };

    match verifier.verify(token, Some(device_id), now_epoch, false) {
        Ok(payload) => {
            let status_hint = if payload.is_renewal_due_at(now_epoch) {
                LicenseState::RenewalDue
            } else {
                LicenseState::Active
            };
            runtime_state_from_payload(payload, LicenseState::Active, status_hint)
        }
        Err(LeaseError::DeviceMismatch) => RuntimeState::reason_only(LicenseState::DeviceMismatch),
        Err(LeaseError::Expired) => {
            // 过期的 Lease 也应该尽量带出 license_key / renew_after 等字段给 UI 展示
            match verifier.verify(token, Some(device_id), now_epoch, true) {
                Ok(payload) => runtime_state_from_payload(
                    payload,
                    LicenseState::Expired,
                    LicenseState::Expired,
                ),
                Err(_) => RuntimeState::reason_only(LicenseState::Expired),
            }
        }
        Err(LeaseError::InvalidSignature)
        | Err(LeaseError::InvalidFormat(_))
        | Err(LeaseError::InvalidKind)
        | Err(LeaseError::InvalidPublicKey(_)) => RuntimeState::reason_only(LicenseState::Invalid),
    }
}

fn runtime_state_from_payload(
    payload: LeasePayload,
    reason: LicenseState,
    status_hint: LicenseState,
) -> RuntimeState {
    RuntimeState {
        license_key: payload.license_key,
        device_id: payload.device_id,
        reason,
        status_hint,
        license_expires_at: epoch_to_iso(payload.exp),
        lease_expires_at: epoch_to_iso(payload.exp),
        renew_after: epoch_to_iso(payload.renew_after),
        task_policy: payload.task_policy,
        risk_level: payload.risk_level,
        runtime_backend: RUNTIME_BACKEND.to_string(),
        compromised: false,
        last_verify_at: String::new(),
    }
}

fn epoch_to_iso(epoch: i64) -> String {
    DateTime::<Utc>::from_timestamp(epoch, 0)
        .map(|dt| dt.to_rfc3339_opts(chrono::SecondsFormat::Secs, true))
        .unwrap_or_default()
}

#[cfg(test)]
mod tests {
    use super::*;
    use api_contracts::LEASE_KIND_LICENSE;
    use base64::engine::general_purpose::URL_SAFE_NO_PAD;
    use base64::Engine;
    use ed25519_dalek::{Signer, SigningKey};
    use rand::rngs::OsRng;
    use serde_json::json;

    fn keypair() -> (SigningKey, LeaseVerifier, String) {
        let sk = SigningKey::generate(&mut OsRng);
        let vk_b64 = URL_SAFE_NO_PAD.encode(sk.verifying_key().as_bytes());
        let verifier = LeaseVerifier::from_public_key_b64(&vk_b64).unwrap();
        (sk, verifier, vk_b64)
    }

    fn sign(sk: &SigningKey, payload: &serde_json::Value) -> String {
        let payload_bytes = serde_json::to_vec(payload).unwrap();
        let payload_b64 = URL_SAFE_NO_PAD.encode(&payload_bytes);
        let sig = sk.sign(payload_b64.as_bytes());
        format!("{payload_b64}.{}", URL_SAFE_NO_PAD.encode(sig.to_bytes()))
    }

    fn payload_for(device_id: &str, renew_after: i64, exp: i64) -> serde_json::Value {
        json!({
            "kind": LEASE_KIND_LICENSE,
            "license_key": "ABCD-1234",
            "device_id": device_id,
            "issued_at": 1_000,
            "exp": exp,
            "renew_after": renew_after,
            "task_policy": ["review_find", "batch_delivery"],
            "risk_level": "low",
        })
    }

    #[test]
    fn missing_token_returns_not_found() {
        let (_, verifier, _) = keypair();
        let state = verify_stored_lease_local(None, "dev-1", 1_500, &verifier);
        assert_eq!(state.reason, LicenseState::NotFound);
        assert_eq!(state.runtime_backend, "rust");
    }

    #[test]
    fn empty_or_whitespace_token_returns_not_found() {
        let (_, verifier, _) = keypair();
        assert_eq!(
            verify_stored_lease_local(Some(""), "dev-1", 1_500, &verifier).reason,
            LicenseState::NotFound
        );
        assert_eq!(
            verify_stored_lease_local(Some("   "), "dev-1", 1_500, &verifier).reason,
            LicenseState::NotFound
        );
    }

    #[test]
    fn valid_unreached_window_returns_active_active() {
        let (sk, verifier, _) = keypair();
        let token = sign(&sk, &payload_for("dev-1", 2_000, 3_000));
        let state = verify_stored_lease_local(Some(&token), "dev-1", 1_500, &verifier);
        assert_eq!(state.reason, LicenseState::Active);
        assert_eq!(state.status_hint, LicenseState::Active);
        assert_eq!(state.license_key, "ABCD-1234");
        assert_eq!(state.device_id, "dev-1");
        assert_eq!(state.task_policy, vec!["review_find", "batch_delivery"]);
        assert_eq!(state.risk_level, "low");
        assert_eq!(state.runtime_backend, "rust");
        assert!(!state.compromised);
        assert!(state.license_expires_at.ends_with("Z"));
        assert!(state.renew_after.ends_with("Z"));
    }

    #[test]
    fn within_soft_refresh_window_returns_active_renewal_due() {
        let (sk, verifier, _) = keypair();
        let token = sign(&sk, &payload_for("dev-1", 2_000, 3_000));
        let state = verify_stored_lease_local(Some(&token), "dev-1", 2_500, &verifier);
        assert_eq!(state.reason, LicenseState::Active);
        assert_eq!(state.status_hint, LicenseState::RenewalDue);
        // 仍然允许离线使用
        assert!(state.reason.is_locally_allowed());
    }

    #[test]
    fn hard_expired_returns_expired_with_details_when_possible() {
        let (sk, verifier, _) = keypair();
        let token = sign(&sk, &payload_for("dev-1", 2_000, 3_000));
        let state = verify_stored_lease_local(Some(&token), "dev-1", 3_500, &verifier);
        assert_eq!(state.reason, LicenseState::Expired);
        // 过期情况下仍带出 license_key / lease_expires_at 供 UI 展示
        assert_eq!(state.license_key, "ABCD-1234");
        assert!(state.license_expires_at.ends_with("Z"));
        assert!(!state.reason.is_locally_allowed());
    }

    #[test]
    fn device_mismatch_returns_device_mismatch_without_details() {
        let (sk, verifier, _) = keypair();
        let token = sign(&sk, &payload_for("dev-A", 2_000, i64::MAX));
        let state = verify_stored_lease_local(Some(&token), "dev-B", 1_500, &verifier);
        assert_eq!(state.reason, LicenseState::DeviceMismatch);
        // 设备不匹配不应泄漏 license_key
        assert!(state.license_key.is_empty());
    }

    #[test]
    fn tampered_token_returns_invalid() {
        let (sk, verifier, _) = keypair();
        let token = sign(&sk, &payload_for("dev-1", 2_000, i64::MAX));
        let bad = format!("{}X", &token[..token.len() - 1]);
        let state = verify_stored_lease_local(Some(&bad), "dev-1", 1_500, &verifier);
        assert_eq!(state.reason, LicenseState::Invalid);
    }

    #[test]
    fn malformed_token_returns_invalid() {
        let (_, verifier, _) = keypair();
        let state = verify_stored_lease_local(Some("no-dot-format"), "dev-1", 1_500, &verifier);
        assert_eq!(state.reason, LicenseState::Invalid);
    }

    #[test]
    fn wrong_kind_returns_invalid() {
        let (sk, verifier, _) = keypair();
        let bad_payload = json!({
            "kind": "task_grant",
            "license_key": "X",
            "device_id": "dev-1",
            "issued_at": 0,
            "exp": i64::MAX,
            "renew_after": 0,
            "task_policy": [],
            "risk_level": "low",
        });
        let token = sign(&sk, &bad_payload);
        let state = verify_stored_lease_local(Some(&token), "dev-1", 1_500, &verifier);
        assert_eq!(state.reason, LicenseState::Invalid);
    }

    #[test]
    fn iso8601_fields_include_z_suffix() {
        let (sk, verifier, _) = keypair();
        let token = sign(&sk, &payload_for("dev-1", 2_000, 3_000));
        let state = verify_stored_lease_local(Some(&token), "dev-1", 1_500, &verifier);
        for field in [
            &state.license_expires_at,
            &state.lease_expires_at,
            &state.renew_after,
        ] {
            assert!(field.contains('T'));
            assert!(field.ends_with('Z'), "字段必须以 Z 结尾：{field}");
        }
    }
}
