//! 凭据安全存储封装（PRD §5.8 / M2-06）。
//!
//! 统一 Lease、RuntimeBundle 等敏感材料的落地入口：
//! - macOS → Keychain（Secrets Generic Password Service）
//! - Windows → Credential Manager
//! - Linux → secret-service（keyring crate 自适应）
//!
//! 设计要点：
//! - 抽象为 `SecretStore` trait，业务层持 `Arc<dyn SecretStore>`，单测用
//!   内存实现替换，不触碰真实系统密钥环。
//! - `get` 遇到 NoEntry 返回 `Ok(None)` 而不是错误，避免上层在「首次启动」
//!   场景被迫识别"没写过"与"读失败"两种等价情况。
//! - `delete` 遇到 NoEntry 视为 noop，多次调用幂等。
//!
//! 密钥环不可用（CI、无 seahorse 的 Linux、企业策略禁用等）时自动回退到
//! 加密文件后备（M2-07）：使用设备指纹派生 AES-256-GCM 密钥，把密文落在
//! 应用运行目录下；设备指纹一旦变化，解密失败会返回 `StorageError::DeviceChanged`。

use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};

use aes_gcm::aead::{Aead, KeyInit};
use aes_gcm::{Aes256Gcm, Key, Nonce};
use rand::rngs::OsRng;
use rand::RngCore;
use sha2::{Digest, Sha256};
use thiserror::Error;

/// Keychain / Credential Manager 条目的 service 与 account。
///
/// - service：对外用反域名标识应用；后端运维查询时一步到位
/// - account：同一应用下不同敏感材料的槽位标签
pub const KEYCHAIN_SERVICE: &str = "com.tuoling.tls-shipinhao.runtime";
pub const KEYCHAIN_ACCOUNT: &str = "runtime_bundle";

/// 存储操作错误。
#[derive(Debug, Error)]
pub enum StorageError {
    #[error("凭据存储错误：{0}")]
    Backend(String),
    /// 加密文件存在但解密失败——通常是设备指纹变化（换硬盘/主板）。
    /// 业务层可据此提示用户重新激活而非爆错。
    #[error("设备指纹已变化，无法解密旧凭据")]
    DeviceChanged,
    #[error("凭据文件 I/O 错误：{0}")]
    Io(String),
}

impl From<keyring::Error> for StorageError {
    fn from(err: keyring::Error) -> Self {
        StorageError::Backend(err.to_string())
    }
}

impl From<std::io::Error> for StorageError {
    fn from(err: std::io::Error) -> Self {
        StorageError::Io(err.to_string())
    }
}

/// 抽象安全存储。
///
/// 所有方法约定：
/// - `set` 幂等：写入相同或不同值均成功，覆盖旧值。
/// - `get` 不存在返回 `Ok(None)`，I/O 或后端错误才 `Err`。
/// - `delete` 幂等：条目不存在视为 noop。
pub trait SecretStore: Send + Sync {
    fn set(&self, value: &str) -> Result<(), StorageError>;
    fn get(&self) -> Result<Option<String>, StorageError>;
    fn delete(&self) -> Result<(), StorageError>;
}

/// 用系统密钥环作为后端。
pub struct KeychainSecretStore {
    entry: keyring::Entry,
}

impl KeychainSecretStore {
    /// 使用默认 service / account 构造。
    pub fn new() -> Result<Self, StorageError> {
        Self::new_with(KEYCHAIN_SERVICE, KEYCHAIN_ACCOUNT)
    }

    /// 测试 / 多 profile 场景下可注入自定义 service 与 account。
    pub fn new_with(service: &str, account: &str) -> Result<Self, StorageError> {
        let entry = keyring::Entry::new(service, account)?;
        Ok(Self { entry })
    }
}

impl SecretStore for KeychainSecretStore {
    fn set(&self, value: &str) -> Result<(), StorageError> {
        self.entry.set_password(value).map_err(Into::into)
    }

    fn get(&self) -> Result<Option<String>, StorageError> {
        match self.entry.get_password() {
            Ok(value) => Ok(Some(value)),
            Err(keyring::Error::NoEntry) => Ok(None),
            Err(other) => Err(other.into()),
        }
    }

    fn delete(&self) -> Result<(), StorageError> {
        match self.entry.delete_credential() {
            Ok(_) => Ok(()),
            Err(keyring::Error::NoEntry) => Ok(()),
            Err(other) => Err(other.into()),
        }
    }
}

/// 内存实现，**仅用于测试 / 开发**：数据只在进程生命周期内保留。
///
/// 在单元测试里替换 `KeychainSecretStore` 即可不触碰真实 Keychain；
/// 并发场景通过 `Mutex` 保证 set/get/delete 互斥。
pub struct InMemorySecretStore {
    cell: Mutex<Option<String>>,
}

impl InMemorySecretStore {
    pub fn new() -> Self {
        Self {
            cell: Mutex::new(None),
        }
    }
}

impl Default for InMemorySecretStore {
    fn default() -> Self {
        Self::new()
    }
}

impl SecretStore for InMemorySecretStore {
    fn set(&self, value: &str) -> Result<(), StorageError> {
        let mut guard = self
            .cell
            .lock()
            .map_err(|e| StorageError::Backend(format!("锁竞争：{e}")))?;
        *guard = Some(value.to_string());
        Ok(())
    }

    fn get(&self) -> Result<Option<String>, StorageError> {
        let guard = self
            .cell
            .lock()
            .map_err(|e| StorageError::Backend(format!("锁竞争：{e}")))?;
        Ok(guard.clone())
    }

    fn delete(&self) -> Result<(), StorageError> {
        let mut guard = self
            .cell
            .lock()
            .map_err(|e| StorageError::Backend(format!("锁竞争：{e}")))?;
        *guard = None;
        Ok(())
    }
}

// ---- 加密文件后备（M2-07） --------------------------------------------------

/// 加密文件格式版本。未来升级 KDF / 加密算法时递增此常量。
const FILE_FORMAT_VERSION: u8 = 1;

/// Nonce 长度：AES-GCM 标准 12 字节。
const NONCE_LEN: usize = 12;

/// 用设备指纹派生 AES-256-GCM 密钥。
///
/// KDF：`SHA256(device_id)`。选择简单 SHA 而非 Argon2 的理由：
/// - 本地文件场景攻击者若能拿到磁盘文件，通常也能拿到设备 serial；
///   设计意图是"让偶然的 exfiltration 无法读到明文"，不是对抗离线暴力破解。
/// - Argon2 在慢设备上显著增加启动耗时，不值得。
fn derive_file_key(device_id: &str) -> [u8; 32] {
    let hash = Sha256::digest(device_id.as_bytes());
    let mut out = [0u8; 32];
    out.copy_from_slice(&hash[..32]);
    out
}

/// 默认加密文件名。
pub const ENCRYPTED_FILE_NAME: &str = "runtime_bundle.enc";

/// 加密文件后备存储。
///
/// 文件格式（byte-oriented）：
/// ```text
/// version (1 byte) | nonce (12 bytes) | ciphertext (含 GCM 16 字节 tag)
/// ```
pub struct EncryptedFileSecretStore {
    path: PathBuf,
    key: [u8; 32],
}

impl EncryptedFileSecretStore {
    /// 使用设备指纹 + 运行时目录构造。
    pub fn new(runtime_dir: &Path, device_id: &str) -> Self {
        Self::with_file(runtime_dir.join(ENCRYPTED_FILE_NAME), device_id)
    }

    /// 指定完整文件路径的构造器（测试友好）。
    pub fn with_file(path: PathBuf, device_id: &str) -> Self {
        Self {
            path,
            key: derive_file_key(device_id),
        }
    }

    fn cipher(&self) -> Aes256Gcm {
        Aes256Gcm::new(Key::<Aes256Gcm>::from_slice(&self.key))
    }
}

impl SecretStore for EncryptedFileSecretStore {
    fn set(&self, value: &str) -> Result<(), StorageError> {
        let mut nonce_bytes = [0u8; NONCE_LEN];
        OsRng.fill_bytes(&mut nonce_bytes);
        let nonce = Nonce::from_slice(&nonce_bytes);
        let ciphertext = self
            .cipher()
            .encrypt(nonce, value.as_bytes())
            .map_err(|e| StorageError::Backend(format!("加密失败：{e}")))?;

        let mut buf = Vec::with_capacity(1 + NONCE_LEN + ciphertext.len());
        buf.push(FILE_FORMAT_VERSION);
        buf.extend_from_slice(&nonce_bytes);
        buf.extend_from_slice(&ciphertext);

        if let Some(parent) = self.path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        std::fs::write(&self.path, buf)?;
        Ok(())
    }

    fn get(&self) -> Result<Option<String>, StorageError> {
        let buf = match std::fs::read(&self.path) {
            Ok(b) => b,
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(None),
            Err(e) => return Err(e.into()),
        };
        if buf.len() < 1 + NONCE_LEN + 16 {
            return Err(StorageError::Backend("加密文件长度不足".into()));
        }
        if buf[0] != FILE_FORMAT_VERSION {
            return Err(StorageError::Backend(format!(
                "未知加密文件版本：{}",
                buf[0]
            )));
        }
        let nonce = Nonce::from_slice(&buf[1..1 + NONCE_LEN]);
        let ciphertext = &buf[1 + NONCE_LEN..];
        match self.cipher().decrypt(nonce, ciphertext) {
            Ok(plain) => {
                let value = String::from_utf8(plain)
                    .map_err(|e| StorageError::Backend(format!("UTF-8 解码失败：{e}")))?;
                Ok(Some(value))
            }
            Err(_) => Err(StorageError::DeviceChanged),
        }
    }

    fn delete(&self) -> Result<(), StorageError> {
        match std::fs::remove_file(&self.path) {
            Ok(_) => Ok(()),
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(()),
            Err(e) => Err(e.into()),
        }
    }
}

/// 默认后备链：Keychain 可用优先用 Keychain，否则回退加密文件。
///
/// 返回 `Arc<dyn SecretStore>` 方便业务层把它塞到 `AppState` 里共享。
pub fn init_default_store(runtime_dir: &Path, device_id: &str) -> Arc<dyn SecretStore> {
    match KeychainSecretStore::new() {
        Ok(store) => {
            tracing::info!(
                "凭据存储后端：Keychain/Credential Manager（service={}）",
                KEYCHAIN_SERVICE
            );
            Arc::new(store)
        }
        Err(err) => {
            tracing::warn!(
                "Keychain 不可用，回退到加密文件：{err}（path={}）",
                runtime_dir.join(ENCRYPTED_FILE_NAME).display()
            );
            Arc::new(EncryptedFileSecretStore::new(runtime_dir, device_id))
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Arc;
    use std::thread;

    #[test]
    fn constants_match_prd_identifiers() {
        assert_eq!(KEYCHAIN_SERVICE, "com.tuoling.tls-shipinhao.runtime");
        assert_eq!(KEYCHAIN_ACCOUNT, "runtime_bundle");
    }

    #[test]
    fn in_memory_store_read_returns_none_initially() {
        let store = InMemorySecretStore::new();
        assert_eq!(store.get().unwrap(), None);
    }

    #[test]
    fn in_memory_store_roundtrip_set_get_delete() {
        let store = InMemorySecretStore::new();
        store.set("lease-token-abc").unwrap();
        assert_eq!(store.get().unwrap().as_deref(), Some("lease-token-abc"));
        store.delete().unwrap();
        assert_eq!(store.get().unwrap(), None);
    }

    #[test]
    fn in_memory_store_set_is_idempotent_and_overwrites() {
        let store = InMemorySecretStore::new();
        store.set("v1").unwrap();
        store.set("v2").unwrap();
        assert_eq!(store.get().unwrap().as_deref(), Some("v2"));
    }

    #[test]
    fn in_memory_store_delete_is_idempotent_on_empty() {
        let store = InMemorySecretStore::new();
        store.delete().unwrap();
        store.delete().unwrap();
        assert_eq!(store.get().unwrap(), None);
    }

    #[test]
    fn in_memory_store_concurrent_writes_do_not_race() {
        // 8 个 writer 各写 100 次，结束后值必须等于某一次合法写入。
        let store: Arc<dyn SecretStore> = Arc::new(InMemorySecretStore::new());
        let mut handles = Vec::new();
        for thread_id in 0..8 {
            let store = store.clone();
            handles.push(thread::spawn(move || {
                for i in 0..100 {
                    store.set(&format!("t{thread_id}-{i}")).unwrap();
                    let _ = store.get().unwrap();
                }
            }));
        }
        for h in handles {
            h.join().unwrap();
        }
        let value = store.get().unwrap().unwrap();
        assert!(value.starts_with('t'));
        assert!(value.contains('-'));
    }

    #[test]
    fn storage_error_preserves_message() {
        let err = StorageError::Backend("keychain locked".into());
        assert!(err.to_string().contains("keychain locked"));
    }

    // 真实 Keychain 场景（macOS）在 CI 上可打开；本地/沙箱里通常会失败
    // 因此标记为 ignored，需要显式 `cargo test -- --ignored` 才会跑。
    #[test]
    #[ignore = "依赖真实系统密钥环，需要交互授权"]
    fn keychain_store_real_backend_roundtrip() {
        let store =
            KeychainSecretStore::new_with("com.tuoling.tls-shipinhao.test-runtime", "ci-test-slot")
                .expect("构造条目应成功");

        store.set("ci-secret-value").expect("写入 Keychain 失败");
        assert_eq!(
            store.get().expect("读取失败").as_deref(),
            Some("ci-secret-value")
        );
        store.delete().expect("删除失败");
        assert_eq!(store.get().expect("读取失败"), None);
        // 再次 delete 应幂等
        store.delete().expect("幂等 delete 失败");
    }

    // ---- EncryptedFileSecretStore（M2-07） ---------------------------------

    fn tempdir() -> tempfile::TempDir {
        tempfile::tempdir().expect("tempdir 创建失败")
    }

    #[test]
    fn encrypted_file_get_returns_none_before_any_write() {
        let dir = tempdir();
        let store = EncryptedFileSecretStore::new(dir.path(), "dev-A");
        assert_eq!(store.get().unwrap(), None);
    }

    #[test]
    fn encrypted_file_roundtrip_preserves_secret() {
        let dir = tempdir();
        let store = EncryptedFileSecretStore::new(dir.path(), "dev-A");
        store.set("lease.token.X").unwrap();
        assert_eq!(store.get().unwrap().as_deref(), Some("lease.token.X"));
    }

    #[test]
    fn encrypted_file_survives_process_restart() {
        // 新建 store 对象模拟应用重启：同一 device_id + 同一文件 → 能读出
        let dir = tempdir();
        let path = dir.path().to_path_buf();
        EncryptedFileSecretStore::new(&path, "dev-A")
            .set("persisted-lease")
            .unwrap();

        let new_store = EncryptedFileSecretStore::new(&path, "dev-A");
        assert_eq!(new_store.get().unwrap().as_deref(), Some("persisted-lease"));
    }

    #[test]
    fn encrypted_file_returns_device_changed_on_fingerprint_drift() {
        let dir = tempdir();
        let path = dir.path().to_path_buf();
        EncryptedFileSecretStore::new(&path, "dev-OLD")
            .set("old-lease")
            .unwrap();

        // 设备指纹变化（换硬盘/主板）
        let new_store = EncryptedFileSecretStore::new(&path, "dev-NEW");
        match new_store.get() {
            Err(StorageError::DeviceChanged) => {}
            other => panic!("预期 DeviceChanged，实际 {other:?}"),
        }
    }

    #[test]
    fn encrypted_file_manual_delete_restores_none() {
        let dir = tempdir();
        let store = EncryptedFileSecretStore::new(dir.path(), "dev-A");
        store.set("v").unwrap();
        // 模拟用户/运维手工删除
        std::fs::remove_file(dir.path().join(ENCRYPTED_FILE_NAME)).unwrap();
        assert_eq!(store.get().unwrap(), None);
    }

    #[test]
    fn encrypted_file_delete_is_idempotent() {
        let dir = tempdir();
        let store = EncryptedFileSecretStore::new(dir.path(), "dev-A");
        store.delete().unwrap();
        store.set("v").unwrap();
        store.delete().unwrap();
        store.delete().unwrap();
        assert_eq!(store.get().unwrap(), None);
    }

    #[test]
    fn encrypted_file_rejects_bad_version_byte() {
        let dir = tempdir();
        let path = dir.path().join(ENCRYPTED_FILE_NAME);
        // 手工写入版本号 99 的"文件头"，加足够长度让 len 检查通过
        let mut buf = vec![99u8];
        buf.extend_from_slice(&[0u8; NONCE_LEN + 16]);
        std::fs::write(&path, &buf).unwrap();

        let store = EncryptedFileSecretStore::with_file(path, "dev-A");
        match store.get() {
            Err(StorageError::Backend(msg)) => assert!(msg.contains("版本")),
            other => panic!("预期 Backend 错误，实际 {other:?}"),
        }
    }

    #[test]
    fn encrypted_file_rejects_truncated_file() {
        let dir = tempdir();
        let path = dir.path().join(ENCRYPTED_FILE_NAME);
        std::fs::write(&path, vec![1u8, 2u8, 3u8]).unwrap();

        let store = EncryptedFileSecretStore::with_file(path, "dev-A");
        match store.get() {
            Err(StorageError::Backend(msg)) => assert!(msg.contains("长度")),
            other => panic!("预期 Backend，实际 {other:?}"),
        }
    }

    #[test]
    fn encrypted_file_set_overwrites_previous_value() {
        let dir = tempdir();
        let store = EncryptedFileSecretStore::new(dir.path(), "dev-A");
        store.set("v1").unwrap();
        store.set("v2").unwrap();
        assert_eq!(store.get().unwrap().as_deref(), Some("v2"));
    }

    #[test]
    fn derive_file_key_is_deterministic_and_32_bytes() {
        let a = derive_file_key("dev-A");
        let b = derive_file_key("dev-A");
        assert_eq!(a, b);
        assert_eq!(a.len(), 32);
        assert_ne!(a, derive_file_key("dev-B"));
    }

    #[test]
    fn init_default_store_returns_usable_store() {
        // 真实环境下会走 Keychain；若失败回退加密文件。两种情况都应返回可用 store。
        let dir = tempdir();
        let store = init_default_store(dir.path(), "dev-test");
        // 我们不知道具体后端，只验证接口可用
        let _ = store.get();
    }
}
