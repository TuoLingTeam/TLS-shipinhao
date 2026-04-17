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
//! 未实现（留给 M2-07）：Keychain 不可用（CI、无 seahorse 的 Linux）时
//! 回退到加密文件后备；接口层面 `SecretStore` 已经可以让业务层无感切换。

use std::sync::Mutex;

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
}

impl From<keyring::Error> for StorageError {
    fn from(err: keyring::Error) -> Self {
        StorageError::Backend(err.to_string())
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
        let store = KeychainSecretStore::new_with(
            "com.tuoling.tls-shipinhao.test-runtime",
            "ci-test-slot",
        )
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
}
