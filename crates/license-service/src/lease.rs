//! Lease Token 的 Ed25519 验签。
//!
//! Token 格式：`base64url(payload_json) . base64url(ed25519_signature)`。
//!
//! 设计目标（与 PRD §5.5、M2-03 对齐）：
//! - 原子化的错误枚举：`InvalidFormat / InvalidSignature / InvalidKind /
//!   DeviceMismatch / Expired`，业务层可以精确决定后续动作（重激/续约/降级）。
//! - 验签 + 业务字段校验一次完成，避免"先解 payload 再二次校验"的双重路径。
//! - 公钥常量来自 `LICENSE_PUBLIC_KEY_B64`（M2-02），轮换密钥时只改一处。
//!
//! 与 `security_core::verify_lease_impl` 的关系：
//! - 本模块是**纯 Rust API**，返回 `Result<LeasePayload, LeaseError>`，方便
//!   在 license-service 内部或 Tauri 命令里直接消费。
//! - security-core 里的 FFI 版本面向 Python 桥接（legacy 兼容），两者共用
//!   相同的语义，但不能简单互相调用——FFI 层把错误扁平化为 JSON，损失了
//!   类型信息。

use api_contracts::{LeasePayload, LEASE_KIND_LICENSE};
use base64::engine::general_purpose::URL_SAFE_NO_PAD;
use base64::Engine;
use ed25519_dalek::{Signature, Verifier, VerifyingKey};
use thiserror::Error;

use crate::LICENSE_PUBLIC_KEY_B64;

/// Lease 校验失败的分类错误。
#[derive(Debug, Error)]
pub enum LeaseError {
    /// Token 拆分失败、base64url 解码失败、JSON 解析失败。
    #[error("Lease 格式错误：{0}")]
    InvalidFormat(String),
    /// Ed25519 签名不匹配。
    #[error("Lease 签名校验失败")]
    InvalidSignature,
    /// payload `kind` 字段不是 `license_lease`。
    #[error("Lease kind 非法，期望 {LEASE_KIND_LICENSE}")]
    InvalidKind,
    /// payload 绑定的 device_id 与期望不一致。
    #[error("Lease 设备不匹配")]
    DeviceMismatch,
    /// payload `exp` 小于等于当前时间。
    #[error("Lease 已过期")]
    Expired,
    /// 公钥本身不合法（通常是常量配置错误）。
    #[error("Lease 公钥非法：{0}")]
    InvalidPublicKey(String),
}

/// Lease 验签器。
///
/// 典型用法：
/// ```ignore
/// let verifier = LeaseVerifier::from_public_key_b64(LICENSE_PUBLIC_KEY_B64)?;
/// let payload = verifier.verify(&token, Some(&device_id), now_epoch, false)?;
/// ```
#[derive(Debug)]
pub struct LeaseVerifier {
    public_key: VerifyingKey,
}

impl LeaseVerifier {
    /// 用默认公钥（`LICENSE_PUBLIC_KEY_B64`）构造。
    pub fn new() -> Result<Self, LeaseError> {
        Self::from_public_key_b64(LICENSE_PUBLIC_KEY_B64)
    }

    /// 用 base64url 编码的公钥构造。方便测试注入自定义密钥。
    pub fn from_public_key_b64(public_key_b64url: &str) -> Result<Self, LeaseError> {
        let bytes = URL_SAFE_NO_PAD
            .decode(public_key_b64url.as_bytes())
            .map_err(|e| LeaseError::InvalidPublicKey(format!("base64url 解码失败：{e}")))?;
        let key_bytes: [u8; 32] = bytes
            .try_into()
            .map_err(|_| LeaseError::InvalidPublicKey("公钥长度必须为 32 字节".into()))?;
        let public_key = VerifyingKey::from_bytes(&key_bytes)
            .map_err(|e| LeaseError::InvalidPublicKey(e.to_string()))?;
        Ok(Self { public_key })
    }

    /// 校验 Token 并返回 payload。
    ///
    /// - `expected_device_id`: 若为 `Some`，校验 payload 的 `device_id` 字段一致
    /// - `now_epoch`: 当前 Unix 秒，用于 `exp` 检查
    /// - `allow_expired`: `true` 时允许 `exp` 已过的 Lease 通过（供「展示已过期授权信息」场景）
    pub fn verify(
        &self,
        token: &str,
        expected_device_id: Option<&str>,
        now_epoch: i64,
        allow_expired: bool,
    ) -> Result<LeasePayload, LeaseError> {
        let (payload_b64, sig_b64) = split_token(token)?;

        verify_signature(&self.public_key, payload_b64, sig_b64)?;

        let payload = decode_payload(payload_b64)?;

        if !payload.has_valid_kind() {
            return Err(LeaseError::InvalidKind);
        }

        if let Some(expected) = expected_device_id.filter(|v| !v.is_empty()) {
            if payload.device_id != expected {
                return Err(LeaseError::DeviceMismatch);
            }
        }

        if !allow_expired && !payload.is_still_valid_at(now_epoch) {
            return Err(LeaseError::Expired);
        }

        Ok(payload)
    }
}

fn split_token(token: &str) -> Result<(&str, &str), LeaseError> {
    let (payload_b64, sig_b64) = token
        .split_once('.')
        .ok_or_else(|| LeaseError::InvalidFormat("缺少 `.` 分隔符".into()))?;
    if payload_b64.is_empty() || sig_b64.is_empty() {
        return Err(LeaseError::InvalidFormat("payload 或 signature 为空".into()));
    }
    Ok((payload_b64, sig_b64))
}

fn verify_signature(
    public_key: &VerifyingKey,
    payload_b64: &str,
    signature_b64: &str,
) -> Result<(), LeaseError> {
    let signature_bytes = URL_SAFE_NO_PAD
        .decode(signature_b64.as_bytes())
        .map_err(|e| LeaseError::InvalidFormat(format!("signature base64url 解码失败：{e}")))?;
    let signature = Signature::from_slice(&signature_bytes)
        .map_err(|e| LeaseError::InvalidFormat(format!("signature 长度非法：{e}")))?;
    public_key
        .verify(payload_b64.as_bytes(), &signature)
        .map_err(|_| LeaseError::InvalidSignature)?;
    Ok(())
}

fn decode_payload(payload_b64: &str) -> Result<LeasePayload, LeaseError> {
    let bytes = URL_SAFE_NO_PAD
        .decode(payload_b64.as_bytes())
        .map_err(|e| LeaseError::InvalidFormat(format!("payload base64url 解码失败：{e}")))?;
    serde_json::from_slice(&bytes)
        .map_err(|e| LeaseError::InvalidFormat(format!("payload JSON 非法：{e}")))
}

#[cfg(test)]
mod tests {
    use super::*;
    use ed25519_dalek::{Signer, SigningKey};
    use rand::rngs::OsRng;

    fn keypair() -> (SigningKey, String) {
        let sk = SigningKey::generate(&mut OsRng);
        let vk_b64 = URL_SAFE_NO_PAD.encode(sk.verifying_key().as_bytes());
        (sk, vk_b64)
    }

    fn sign_payload_json(sk: &SigningKey, payload: &serde_json::Value) -> String {
        let payload_bytes = serde_json::to_vec(payload).unwrap();
        let payload_b64 = URL_SAFE_NO_PAD.encode(&payload_bytes);
        let sig = sk.sign(payload_b64.as_bytes());
        let sig_b64 = URL_SAFE_NO_PAD.encode(sig.to_bytes());
        format!("{payload_b64}.{sig_b64}")
    }

    fn default_payload(device_id: &str, exp: i64) -> serde_json::Value {
        serde_json::json!({
            "kind": "license_lease",
            "license_key": "ABCD-EFGH",
            "device_id": device_id,
            "issued_at": 1_730_000_000i64,
            "exp": exp,
            "renew_after": 1_730_086_400i64,
            "task_policy": ["review_find", "batch_delivery"],
            "risk_level": "low",
        })
    }

    #[test]
    fn from_public_key_rejects_invalid_base64url() {
        let err = LeaseVerifier::from_public_key_b64("!!not-base64!!").unwrap_err();
        assert!(matches!(err, LeaseError::InvalidPublicKey(_)));
    }

    #[test]
    fn from_public_key_rejects_wrong_length() {
        let short = URL_SAFE_NO_PAD.encode(b"too-short");
        let err = LeaseVerifier::from_public_key_b64(&short).unwrap_err();
        assert!(matches!(err, LeaseError::InvalidPublicKey(_)));
    }

    #[test]
    fn default_constructor_loads_prd_public_key() {
        let verifier = LeaseVerifier::new().expect("PRD 常量应当能解析出合法公钥");
        // 无法直接读取，但可以断言 VerifyingKey 长度恒为 32 字节。
        let _ = verifier.public_key;
    }

    #[test]
    fn verify_returns_payload_on_happy_path() {
        let (sk, vk) = keypair();
        let verifier = LeaseVerifier::from_public_key_b64(&vk).unwrap();
        let token = sign_payload_json(&sk, &default_payload("dev-1", i64::MAX));

        let payload = verifier
            .verify(&token, Some("dev-1"), 1_730_000_100, false)
            .unwrap();
        assert_eq!(payload.license_key, "ABCD-EFGH");
        assert_eq!(payload.device_id, "dev-1");
        assert!(payload.has_valid_kind());
    }

    #[test]
    fn tampered_payload_byte_fails_signature() {
        let (sk, vk) = keypair();
        let verifier = LeaseVerifier::from_public_key_b64(&vk).unwrap();
        let token = sign_payload_json(&sk, &default_payload("dev-1", i64::MAX));

        // 修改 payload 的最后一个字符
        let (payload, signature) = token.split_once('.').unwrap();
        let mut chars: Vec<char> = payload.chars().collect();
        let last = chars.last_mut().unwrap();
        *last = if *last == 'A' { 'B' } else { 'A' };
        let tampered: String = chars.into_iter().collect();
        let bad_token = format!("{tampered}.{signature}");

        let err = verifier
            .verify(&bad_token, Some("dev-1"), 1_730_000_100, false)
            .unwrap_err();
        assert!(matches!(err, LeaseError::InvalidSignature));
    }

    #[test]
    fn tampered_signature_fails_signature() {
        let (sk, vk) = keypair();
        let verifier = LeaseVerifier::from_public_key_b64(&vk).unwrap();
        let token = sign_payload_json(&sk, &default_payload("dev-1", i64::MAX));

        let bad_token = format!("{}X", &token[..token.len() - 1]);
        let err = verifier
            .verify(&bad_token, Some("dev-1"), 1_730_000_100, false)
            .unwrap_err();
        assert!(matches!(
            err,
            LeaseError::InvalidSignature | LeaseError::InvalidFormat(_)
        ));
    }

    #[test]
    fn device_mismatch_returns_device_mismatch_error() {
        let (sk, vk) = keypair();
        let verifier = LeaseVerifier::from_public_key_b64(&vk).unwrap();
        let token = sign_payload_json(&sk, &default_payload("dev-A", i64::MAX));

        let err = verifier
            .verify(&token, Some("dev-B"), 1_730_000_100, false)
            .unwrap_err();
        assert!(matches!(err, LeaseError::DeviceMismatch));
    }

    #[test]
    fn empty_expected_device_id_skips_device_check() {
        let (sk, vk) = keypair();
        let verifier = LeaseVerifier::from_public_key_b64(&vk).unwrap();
        let token = sign_payload_json(&sk, &default_payload("dev-A", i64::MAX));

        // 空字符串 = 跳过校验
        let payload = verifier
            .verify(&token, Some(""), 1_730_000_100, false)
            .unwrap();
        assert_eq!(payload.device_id, "dev-A");

        // None 也跳过
        let payload2 = verifier
            .verify(&token, None, 1_730_000_100, false)
            .unwrap();
        assert_eq!(payload2, payload);
    }

    #[test]
    fn expired_token_rejected_when_not_allowed() {
        let (sk, vk) = keypair();
        let verifier = LeaseVerifier::from_public_key_b64(&vk).unwrap();
        let token = sign_payload_json(&sk, &default_payload("dev-1", 1_500));

        let err = verifier.verify(&token, None, 2_000, false).unwrap_err();
        assert!(matches!(err, LeaseError::Expired));
    }

    #[test]
    fn expired_token_accepted_when_allow_expired_true() {
        let (sk, vk) = keypair();
        let verifier = LeaseVerifier::from_public_key_b64(&vk).unwrap();
        let token = sign_payload_json(&sk, &default_payload("dev-1", 1_500));

        let payload = verifier.verify(&token, None, 2_000, true).unwrap();
        assert_eq!(payload.device_id, "dev-1");
    }

    #[test]
    fn wrong_kind_returns_invalid_kind() {
        let (sk, vk) = keypair();
        let verifier = LeaseVerifier::from_public_key_b64(&vk).unwrap();
        let payload = serde_json::json!({
            "kind": "task_grant",
            "license_key": "ABCD",
            "device_id": "dev-1",
            "issued_at": 0,
            "exp": i64::MAX,
            "renew_after": 0,
            "task_policy": [],
            "risk_level": "low",
        });
        let token = sign_payload_json(&sk, &payload);

        let err = verifier.verify(&token, None, 1_000, false).unwrap_err();
        assert!(matches!(err, LeaseError::InvalidKind));
    }

    #[test]
    fn token_without_dot_returns_invalid_format() {
        let verifier = LeaseVerifier::from_public_key_b64(
            &URL_SAFE_NO_PAD.encode(&[1u8; 32]),
        )
        .unwrap();
        let err = verifier.verify("nodot", None, 0, false).unwrap_err();
        match err {
            LeaseError::InvalidFormat(msg) => assert!(msg.contains("分隔符")),
            other => panic!("预期 InvalidFormat，实际 {other:?}"),
        }
    }

    #[test]
    fn empty_token_segments_return_invalid_format() {
        let verifier = LeaseVerifier::from_public_key_b64(
            &URL_SAFE_NO_PAD.encode(&[1u8; 32]),
        )
        .unwrap();
        let err = verifier.verify(".sig", None, 0, false).unwrap_err();
        assert!(matches!(err, LeaseError::InvalidFormat(_)));

        let err = verifier.verify("payload.", None, 0, false).unwrap_err();
        assert!(matches!(err, LeaseError::InvalidFormat(_)));
    }

    #[test]
    fn malformed_payload_json_returns_invalid_format() {
        let (sk, vk) = keypair();
        let verifier = LeaseVerifier::from_public_key_b64(&vk).unwrap();

        // 手工伪造一个"签名正确但 payload 不是 JSON"的 token
        let payload_b64 = URL_SAFE_NO_PAD.encode(b"not-json");
        let sig = sk.sign(payload_b64.as_bytes());
        let token = format!("{}.{}", payload_b64, URL_SAFE_NO_PAD.encode(sig.to_bytes()));

        let err = verifier.verify(&token, None, 0, false).unwrap_err();
        assert!(matches!(err, LeaseError::InvalidFormat(_)));
    }

    #[test]
    fn wrong_public_key_fails_signature() {
        let (sk_a, _) = keypair();
        let (_, vk_b) = keypair();
        let verifier = LeaseVerifier::from_public_key_b64(&vk_b).unwrap();
        let token = sign_payload_json(&sk_a, &default_payload("dev-1", i64::MAX));

        let err = verifier.verify(&token, None, 1_000, false).unwrap_err();
        assert!(matches!(err, LeaseError::InvalidSignature));
    }
}
