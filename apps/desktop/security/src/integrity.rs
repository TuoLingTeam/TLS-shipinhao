//! 完整性 Manifest 校验（PRD §5.9 / M2-09）。
//!
//! 启动 / 续约前 / 任务授权前，按签名 Manifest 逐一核对关键文件的 SHA256。
//! 签名由打包侧用 Ed25519 签发；客户端只带公钥验签，不可伪造。
//!
//! 与 `security_core_verify_integrity_manifest`（FFI）的关系：
//! - FFI 版返回 `Value`（为 Python 桥接保留），语义扁平化为 "ok" / "compromised"
//! - 本模块是纯 Rust API：`Result<(), IntegrityError>`，让业务层可以精确
//!   区分"缺失 manifest" vs "签名错" vs "哪个文件被改"，便于日志定位与 UI 引导

use std::path::{Path, PathBuf};

use base64::engine::general_purpose::URL_SAFE_NO_PAD;
use base64::Engine;
use ed25519_dalek::{Signature, Verifier, VerifyingKey};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use thiserror::Error;

use backend::blank_debug_release;

/// 默认 Manifest 文件名。与打包侧 `xtask generate-manifest` 输出保持一致。
pub const INTEGRITY_MANIFEST_FILE_NAME: &str = "integrity_manifest.json";

/// 单条文件记录。
#[cfg_attr(debug_assertions, derive(Debug))]
#[derive(Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct Mf {
    pub path: String,
    pub sha256: String,
}

/// Manifest payload 主体（去掉 `signature` 之外的所有字段）。
///
/// 字段命名与 Worker 签名侧保持一致；新增字段要同时更新 canonical 序列化。
/// `version` 与 `backend::contracts::IntegrityManifest.version` 对齐为 `u32`，
/// 便于两侧在同一份 manifest 上比对 canonical 字节串完全一致。
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ManifestPayload {
    /// 格式版本号（打包侧当前恒为 1；日后升级再迭代）。
    pub version: u32,
    pub generated_at: String,
    pub files: Vec<Mf>,
}

/// 完整 Manifest = payload + 签名。
#[cfg_attr(debug_assertions, derive(Debug))]
#[derive(Clone, Serialize, Deserialize)]
pub struct Sm {
    #[serde(flatten)]
    pub payload: ManifestPayload,
    pub signature: String,
}
blank_debug_release!(Mf);
blank_debug_release!(Sm);

/// 完整性校验失败分类。
#[derive(Debug, Error)]
pub enum IntegrityError {
    /// Manifest 文件缺失。通常是打包流程出问题或用户误删。
    #[error("Manifest 文件缺失：{0}")]
    MissingManifest(String),
    /// Manifest 内容非法（JSON 解析失败、字段缺失等）。
    #[error("Manifest 非法：{0}")]
    InvalidManifest(String),
    /// 签名校验失败（被篡改或公钥漂移）。
    #[error("Manifest 签名校验失败")]
    InvalidSignature,
    /// 某个文件的实际 SHA256 与 Manifest 记录不一致。
    #[error("文件被篡改：{0}")]
    FileModified(String),
    /// 某个文件缺失。
    #[error("关键文件缺失：{0}")]
    FileMissing(String),
    /// 读取 Manifest 或关键文件失败（IO 错）。
    #[error("完整性校验 I/O 失败：{0}")]
    Io(String),
    /// 公钥参数本身非法（通常是常量漂移）。
    #[error("完整性公钥非法：{0}")]
    InvalidPublicKey(String),
}

impl From<std::io::Error> for IntegrityError {
    fn from(err: std::io::Error) -> Self {
        IntegrityError::Io(err.to_string())
    }
}

/// 校验 Manifest + 关键文件。成功则 `Ok(())`。
///
/// 步骤：
/// 1. 加载公钥（base64url）→ 若常量漂移立即 `InvalidPublicKey`
/// 2. 读 Manifest JSON → 缺失 `MissingManifest`，解析失败 `InvalidManifest`
/// 3. 规范化 payload（按字段名排序）→ Ed25519 verify → `InvalidSignature`
/// 4. 逐个核对 files：缺失 `FileMissing`、hash 不匹配 `FileModified`
///
/// `manifest_path` 所在目录作为相对路径基准。
pub fn validate_runtime_continuity(
    manifest_path: &Path,
    public_key_b64url: &str,
) -> Result<(), IntegrityError> {
    let public_key = load_public_key(public_key_b64url)?;
    let raw = read_manifest_file(manifest_path)?;
    let signed: Sm = serde_json::from_str(&raw)
        .map_err(|e| IntegrityError::InvalidManifest(format!("JSON 解析失败：{e}")))?;

    verify_manifest_signature(&public_key, &signed)?;

    let base_dir = manifest_path.parent().unwrap_or_else(|| Path::new("."));
    verify_manifest_files(base_dir, &signed.payload)?;
    Ok(())
}

fn load_public_key(b64url: &str) -> Result<VerifyingKey, IntegrityError> {
    let bytes = URL_SAFE_NO_PAD
        .decode(b64url.as_bytes())
        .map_err(|e| IntegrityError::InvalidPublicKey(format!("base64url 解码失败：{e}")))?;
    let key_bytes: [u8; 32] = bytes
        .try_into()
        .map_err(|_| IntegrityError::InvalidPublicKey("公钥长度必须为 32 字节".into()))?;
    VerifyingKey::from_bytes(&key_bytes)
        .map_err(|e| IntegrityError::InvalidPublicKey(e.to_string()))
}

fn read_manifest_file(path: &Path) -> Result<String, IntegrityError> {
    match std::fs::read_to_string(path) {
        Ok(raw) => Ok(raw),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
            Err(IntegrityError::MissingManifest(path.display().to_string()))
        }
        Err(e) => Err(IntegrityError::Io(e.to_string())),
    }
}

fn verify_manifest_signature(public_key: &VerifyingKey, signed: &Sm) -> Result<(), IntegrityError> {
    if signed.signature.trim().is_empty() {
        return Err(IntegrityError::InvalidManifest("signature 为空".into()));
    }
    let signature_bytes = URL_SAFE_NO_PAD
        .decode(signed.signature.as_bytes())
        .map_err(|e| IntegrityError::InvalidManifest(format!("signature 解码失败：{e}")))?;
    let signature = Signature::from_slice(&signature_bytes)
        .map_err(|e| IntegrityError::InvalidManifest(format!("signature 长度非法：{e}")))?;

    let canonical = canonicalize_manifest(&signed.payload)?;
    public_key
        .verify(&canonical, &signature)
        .map_err(|_| IntegrityError::InvalidSignature)?;
    Ok(())
}

fn verify_manifest_files(base_dir: &Path, payload: &ManifestPayload) -> Result<(), IntegrityError> {
    for file in &payload.files {
        if file.path.is_empty() {
            return Err(IntegrityError::InvalidManifest("path 为空".into()));
        }
        if file.sha256.is_empty() {
            return Err(IntegrityError::InvalidManifest("sha256 为空".into()));
        }
        let absolute = base_dir.join(&file.path);
        let actual = match sha256_hex_of_file(&absolute) {
            Ok(hex) => hex,
            Err(IntegrityError::Io(msg))
                if msg.contains("No such file") || msg.contains("不到") =>
            {
                return Err(IntegrityError::FileMissing(file.path.clone()));
            }
            Err(e) => return Err(e),
        };
        if actual != file.sha256 {
            return Err(IntegrityError::FileModified(file.path.clone()));
        }
    }
    Ok(())
}

fn sha256_hex_of_file(path: &PathBuf) -> Result<String, IntegrityError> {
    let bytes = match std::fs::read(path) {
        Ok(b) => b,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
            return Err(IntegrityError::FileMissing(path.display().to_string()))
        }
        Err(e) => return Err(e.into()),
    };
    Ok(format!("{:x}", Sha256::digest(&bytes)))
}

/// 规范化 Manifest payload 用于签名/验签。
///
/// 按 `version`、`generated_at`、`files` 固定字段顺序序列化（serde 的 struct
/// 默认就是按字段声明顺序），避免空白字符和字段顺序差异导致签名不一致。
pub fn canonicalize_manifest(payload: &ManifestPayload) -> Result<Vec<u8>, IntegrityError> {
    serde_json::to_vec(payload)
        .map_err(|e| IntegrityError::InvalidManifest(format!("canonical 序列化失败：{e}")))
}

#[cfg(test)]
mod tests {
    use super::*;
    use ed25519_dalek::{Signer, SigningKey};
    use rand::rngs::OsRng;
    use tempfile::TempDir;

    fn keypair() -> (SigningKey, String) {
        let sk = SigningKey::generate(&mut OsRng);
        let vk_b64 = URL_SAFE_NO_PAD.encode(sk.verifying_key().as_bytes());
        (sk, vk_b64)
    }

    fn build_manifest(payload: &ManifestPayload, sk: &SigningKey) -> Sm {
        let canonical = canonicalize_manifest(payload).unwrap();
        let signature = sk.sign(&canonical);
        Sm {
            payload: payload.clone(),
            signature: URL_SAFE_NO_PAD.encode(signature.to_bytes()),
        }
    }

    fn write_file(dir: &Path, rel: &str, content: &[u8]) -> String {
        let absolute = dir.join(rel);
        if let Some(parent) = absolute.parent() {
            std::fs::create_dir_all(parent).unwrap();
        }
        std::fs::write(&absolute, content).unwrap();
        format!("{:x}", Sha256::digest(content))
    }

    fn fresh_setup() -> (TempDir, PathBuf, ManifestPayload, SigningKey, String) {
        let dir = tempfile::tempdir().unwrap();
        let (sk, vk_b64) = keypair();
        let hash_a = write_file(dir.path(), "bin/app", b"hello world");
        let hash_b = write_file(dir.path(), "apps/ui/dist/index.html", b"<html></html>");
        let payload = ManifestPayload {
            version: 1,
            generated_at: "2026-04-16T00:00:00Z".into(),
            files: vec![
                Mf {
                    path: "bin/app".into(),
                    sha256: hash_a,
                },
                Mf {
                    path: "apps/ui/dist/index.html".into(),
                    sha256: hash_b,
                },
            ],
        };
        let manifest_path = dir.path().join(INTEGRITY_MANIFEST_FILE_NAME);
        std::fs::write(
            &manifest_path,
            serde_json::to_vec_pretty(&build_manifest(&payload, &sk)).unwrap(),
        )
        .unwrap();
        (dir, manifest_path, payload, sk, vk_b64)
    }

    #[test]
    fn happy_path_accepts_valid_manifest_and_files() {
        let (_dir, manifest_path, _, _sk, vk) = fresh_setup();
        validate_runtime_continuity(&manifest_path, &vk).unwrap();
    }

    #[test]
    fn missing_manifest_file_reports_missing_manifest() {
        let dir = tempfile::tempdir().unwrap();
        let (_, vk) = keypair();
        let err = validate_runtime_continuity(&dir.path().join(INTEGRITY_MANIFEST_FILE_NAME), &vk)
            .unwrap_err();
        assert!(matches!(err, IntegrityError::MissingManifest(_)));
    }

    #[test]
    fn invalid_json_reports_invalid_manifest() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join(INTEGRITY_MANIFEST_FILE_NAME);
        std::fs::write(&path, b"{not json").unwrap();
        let (_, vk) = keypair();
        let err = validate_runtime_continuity(&path, &vk).unwrap_err();
        assert!(matches!(err, IntegrityError::InvalidManifest(_)));
    }

    #[test]
    fn tampered_file_byte_reports_file_modified() {
        let (dir, manifest_path, _, _sk, vk) = fresh_setup();
        // 改 apps/ui/dist/index.html 一个字节
        std::fs::write(
            dir.path().join("apps/ui/dist/index.html"),
            b"<html>X</html>",
        )
        .unwrap();
        let err = validate_runtime_continuity(&manifest_path, &vk).unwrap_err();
        match err {
            IntegrityError::FileModified(path) => assert_eq!(path, "apps/ui/dist/index.html"),
            other => panic!("预期 FileModified，实际 {other:?}"),
        }
    }

    #[test]
    fn missing_listed_file_reports_file_missing() {
        let (dir, manifest_path, _, _sk, vk) = fresh_setup();
        std::fs::remove_file(dir.path().join("bin/app")).unwrap();
        let err = validate_runtime_continuity(&manifest_path, &vk).unwrap_err();
        assert!(matches!(err, IntegrityError::FileMissing(_)));
    }

    #[test]
    fn signature_tamper_reports_invalid_signature() {
        let (_dir, manifest_path, _, _sk, vk) = fresh_setup();
        let raw = std::fs::read_to_string(&manifest_path).unwrap();
        let mut signed: Sm = serde_json::from_str(&raw).unwrap();
        // 确定性篡改签名首字符：保持 base64url 字符合法，但签名内容一定不同。
        let first = signed.signature.chars().next().unwrap();
        let replacement = if first == 'A' { 'B' } else { 'A' };
        signed
            .signature
            .replace_range(0..first.len_utf8(), &replacement.to_string());
        std::fs::write(&manifest_path, serde_json::to_vec_pretty(&signed).unwrap()).unwrap();

        let err = validate_runtime_continuity(&manifest_path, &vk).unwrap_err();
        assert!(matches!(
            err,
            IntegrityError::InvalidSignature | IntegrityError::InvalidManifest(_)
        ));
    }

    #[test]
    fn wrong_public_key_reports_invalid_signature() {
        let (_dir, manifest_path, _, _sk, _) = fresh_setup();
        let (_, wrong_vk) = keypair();
        let err = validate_runtime_continuity(&manifest_path, &wrong_vk).unwrap_err();
        assert!(matches!(err, IntegrityError::InvalidSignature));
    }

    #[test]
    fn bad_public_key_reports_invalid_public_key() {
        let (_dir, manifest_path, _, _sk, _) = fresh_setup();
        let err = validate_runtime_continuity(&manifest_path, "!!not-valid!!").unwrap_err();
        assert!(matches!(err, IntegrityError::InvalidPublicKey(_)));
    }

    #[test]
    fn empty_signature_is_rejected() {
        let (_dir, manifest_path, _, _sk, vk) = fresh_setup();
        let raw = std::fs::read_to_string(&manifest_path).unwrap();
        let mut signed: Sm = serde_json::from_str(&raw).unwrap();
        signed.signature = "".into();
        std::fs::write(&manifest_path, serde_json::to_vec_pretty(&signed).unwrap()).unwrap();

        let err = validate_runtime_continuity(&manifest_path, &vk).unwrap_err();
        assert!(matches!(err, IntegrityError::InvalidManifest(_)));
    }

    #[test]
    fn canonical_serialization_is_stable_for_signature() {
        let payload = ManifestPayload {
            version: 1,
            generated_at: "2026-04-16T00:00:00Z".into(),
            files: vec![
                Mf {
                    path: "a".into(),
                    sha256: "aa".into(),
                },
                Mf {
                    path: "b".into(),
                    sha256: "bb".into(),
                },
            ],
        };
        let bytes1 = canonicalize_manifest(&payload).unwrap();
        let bytes2 = canonicalize_manifest(&payload).unwrap();
        assert_eq!(bytes1, bytes2);
        let parsed: serde_json::Value = serde_json::from_slice(&bytes1).unwrap();
        assert_eq!(parsed["version"], 1);
        assert_eq!(parsed["files"][0]["path"], "a");
    }

    #[test]
    fn empty_files_list_still_verifies_signature_only() {
        let dir = tempfile::tempdir().unwrap();
        let (sk, vk) = keypair();
        let payload = ManifestPayload {
            version: 1,
            generated_at: "2026-04-16T00:00:00Z".into(),
            files: vec![],
        };
        let manifest_path = dir.path().join(INTEGRITY_MANIFEST_FILE_NAME);
        std::fs::write(
            &manifest_path,
            serde_json::to_vec_pretty(&build_manifest(&payload, &sk)).unwrap(),
        )
        .unwrap();

        validate_runtime_continuity(&manifest_path, &vk).unwrap();
    }
}
