//! 跨 crate 一致性回归：`security_core::verify_lease_impl`（FFI JSON）与
//! `backend::license::lease::LeaseVerifier::verify`（类型化 Rust API）对同一个
//! token 必须返回等价语义。两条路径语义漂移一小格会直接让 FFI Python 桥接
//! 与 Rust 业务层各自"判活"，酿成"一边说签名通过、一边说过期"的诡异现象。
//!
//! 本测试固定一组场景：
//! 1. happy path：签名对、kind 对、device 对、时间内 → 两边都成功
//! 2. 签名被篡改：两边都失败
//! 3. kind 错误：两边都失败
//! 4. device 不匹配：两边都失败（FFI 返回 `device_mismatch` / Rust 返回 `DeviceMismatch`）
//! 5. 已过期 + 不允许过期：两边都失败（FFI `expired` / Rust `Expired`）
//! 6. 已过期 + 允许过期（allow_expired=true）：两边都成功

use backend::contracts::LEASE_KIND_LICENSE;
use backend::license::lease::{LeaseError, LeaseVerifier};
use base64::{engine::general_purpose::URL_SAFE_NO_PAD, Engine};
use ed25519_dalek::{Signer, SigningKey};
use serde_json::{json, Value};

fn generate_keypair() -> (SigningKey, String) {
    // 固定的 32 字节私钥，保证测试结果可重现。
    let sk = SigningKey::from_bytes(&[9u8; 32]);
    let vk_b64 = URL_SAFE_NO_PAD.encode(sk.verifying_key().as_bytes());
    (sk, vk_b64)
}

fn sign_token(sk: &SigningKey, payload: &Value) -> String {
    let payload_bytes = serde_json::to_vec(payload).unwrap();
    let payload_b64 = URL_SAFE_NO_PAD.encode(&payload_bytes);
    let sig = sk.sign(payload_b64.as_bytes());
    let sig_b64 = URL_SAFE_NO_PAD.encode(sig.to_bytes());
    format!("{payload_b64}.{sig_b64}")
}

fn happy_payload(device_id: &str, exp: i64) -> Value {
    json!({
        "kind": LEASE_KIND_LICENSE,
        "license_key": "TLS-CROSS-CHECK",
        "device_id": device_id,
        "issued_at": 1_000_000_000i64,
        "exp": exp,
        "renew_after": 1_000_000_000i64 + 3600,
        "task_policy": ["review_find", "batch_delivery"],
        "risk_level": "low",
    })
}

fn ffi_outcome(
    token: &str,
    vk: &str,
    device: Option<&str>,
    now: i64,
    allow_expired: bool,
) -> (bool, String) {
    let value = security_core::verify_lease_impl(token, vk, device, now, allow_expired);
    let ok = value.get("ok").and_then(Value::as_bool).unwrap_or(false);
    let reason = value
        .get("reason")
        .and_then(Value::as_str)
        .unwrap_or("")
        .to_string();
    (ok, reason)
}

fn rust_outcome(
    token: &str,
    vk: &str,
    device: Option<&str>,
    now: i64,
    allow_expired: bool,
) -> Result<(), LeaseError> {
    let verifier = LeaseVerifier::from_public_key_b64(vk).expect("vk must decode");
    verifier
        .verify(token, device, now, allow_expired)
        .map(|_| ())
}

#[test]
fn happy_path_both_paths_accept_valid_token() {
    let (sk, vk) = generate_keypair();
    let token = sign_token(&sk, &happy_payload("dev-1", i64::MAX));

    let (ffi_ok, ffi_reason) = ffi_outcome(&token, &vk, Some("dev-1"), 1_000_000_500, false);
    assert!(ffi_ok, "FFI 必须通过");
    assert_eq!(ffi_reason, "ok");

    assert!(
        rust_outcome(&token, &vk, Some("dev-1"), 1_000_000_500, false).is_ok(),
        "Rust LeaseVerifier 必须通过",
    );
}

#[test]
fn tampered_signature_rejected_by_both_paths() {
    let (sk, vk) = generate_keypair();
    let token = sign_token(&sk, &happy_payload("dev-1", i64::MAX));
    // 把签名最后一个字符替换为其它字符，使 Ed25519 校验失败
    let mut tampered = token;
    tampered.pop();
    tampered.push('X');

    let (ffi_ok, ffi_reason) = ffi_outcome(&tampered, &vk, Some("dev-1"), 1_000_000_500, false);
    assert!(!ffi_ok, "FFI 必须拒绝");
    assert_eq!(ffi_reason, "invalid");

    let err = rust_outcome(&tampered, &vk, Some("dev-1"), 1_000_000_500, false).unwrap_err();
    assert!(
        matches!(
            err,
            LeaseError::InvalidSignature | LeaseError::InvalidFormat(_)
        ),
        "Rust LeaseVerifier 必须拒绝：{err:?}",
    );
}

#[test]
fn wrong_kind_rejected_by_both_paths() {
    let (sk, vk) = generate_keypair();
    let mut payload = happy_payload("dev-1", i64::MAX);
    payload["kind"] = Value::String("task_grant".into());
    let token = sign_token(&sk, &payload);

    let (ffi_ok, ffi_reason) = ffi_outcome(&token, &vk, Some("dev-1"), 1_000_000_500, false);
    assert!(!ffi_ok);
    assert_eq!(ffi_reason, "invalid");

    let err = rust_outcome(&token, &vk, Some("dev-1"), 1_000_000_500, false).unwrap_err();
    assert!(matches!(err, LeaseError::InvalidKind), "{err:?}");
}

#[test]
fn device_mismatch_rejected_by_both_paths() {
    let (sk, vk) = generate_keypair();
    let token = sign_token(&sk, &happy_payload("dev-A", i64::MAX));

    let (ffi_ok, ffi_reason) = ffi_outcome(&token, &vk, Some("dev-B"), 1_000_000_500, false);
    assert!(!ffi_ok);
    assert_eq!(ffi_reason, "device_mismatch");

    let err = rust_outcome(&token, &vk, Some("dev-B"), 1_000_000_500, false).unwrap_err();
    assert!(matches!(err, LeaseError::DeviceMismatch), "{err:?}");
}

#[test]
fn expired_token_rejected_when_allow_expired_false() {
    let (sk, vk) = generate_keypair();
    // exp 设为一个很早的时间戳
    let token = sign_token(&sk, &happy_payload("dev-1", 1_500));

    let (ffi_ok, ffi_reason) = ffi_outcome(&token, &vk, Some("dev-1"), 2_000, false);
    assert!(!ffi_ok);
    assert_eq!(ffi_reason, "expired");

    let err = rust_outcome(&token, &vk, Some("dev-1"), 2_000, false).unwrap_err();
    assert!(matches!(err, LeaseError::Expired), "{err:?}");
}

#[test]
fn expired_token_accepted_when_allow_expired_true_on_both_paths() {
    let (sk, vk) = generate_keypair();
    let token = sign_token(&sk, &happy_payload("dev-1", 1_500));

    let (ffi_ok, ffi_reason) = ffi_outcome(&token, &vk, Some("dev-1"), 2_000, true);
    assert!(
        ffi_ok,
        "allow_expired=true 时 FFI 应放行：reason={ffi_reason}"
    );
    assert_eq!(ffi_reason, "ok");

    assert!(
        rust_outcome(&token, &vk, Some("dev-1"), 2_000, true).is_ok(),
        "allow_expired=true 时 Rust LeaseVerifier 也应放行",
    );
}
