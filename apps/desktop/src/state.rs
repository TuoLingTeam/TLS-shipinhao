use std::path::{Path, PathBuf};
use std::sync::atomic::AtomicBool;
use std::sync::Arc;

use base64::engine::general_purpose::URL_SAFE_NO_PAD;
use base64::Engine;

use crate::adapters::secure_storage::{init_default_store, SecretStore, StorageError};
use api_contracts::{LicenseState, RuntimeState};
use desktop_services::{parse_cookie_profile, CookieProfile};
use license_service::{verify_stored_lease_local, LeaseVerifier, TaskGrantCache};
use serde::{Deserialize, Serialize};
use tokio::sync::Mutex;

/// 当 `LeaseVerifier::new()` 解析编译期公钥失败（极端场景：二进制被局部
/// 篡改 / strip 出错把常量打坏）时，我们不再让应用直接 panic 退出，而是
/// 用一个**已知不可能匹配真实签名的哨兵公钥**构造 verifier，并在启动时
/// 立刻把 RuntimeState 置为 `Compromised`，让 UI 显示「完整性损坏」并禁
/// 用业务功能。哨兵公钥用 `[1u8; 32]` —— 这是 lease.rs 既有测试已验证
/// 能 ed25519 from_bytes 解码的字节序列；使用它的好处是「编译期可计算
/// base64」、「真实签名永远校验失败」。
fn init_lease_verifier_or_sentinel() -> (LeaseVerifier, bool) {
    match LeaseVerifier::new() {
        Ok(verifier) => (verifier, false),
        Err(err) => {
            tracing::error!(
                target: "state.lease_verifier",
                "授权公钥加载失败，启用 Compromised 哨兵：{err}"
            );
            let sentinel_b64 = URL_SAFE_NO_PAD.encode([1u8; 32]);
            let sentinel = LeaseVerifier::from_public_key_b64(&sentinel_b64)
                .expect("哨兵公钥 [1u8;32] 必须能解析（ed25519 已知有效点）");
            (sentinel, true)
        }
    }
}

const APP_HOME_DIR_NAME: &str = ".tls-shipinhao";
const COOKIE_FILE_NAME: &str = "cookie.txt";
const COOKIE_DIR_POINTER_FILE: &str = "selected_config_dir.txt";
const LICENSE_FILE_NAME: &str = "license.json";
const LOGIN_WEBVIEW_DIR_NAME: &str = "login-webview";
const STORE_REGISTRY_FILE_NAME: &str = "store-registry.json";
const STORES_DIR_NAME: &str = "stores";
const STORE_META_FILE_NAME: &str = "meta.json";

use api_contracts::blank_debug_release;
blank_debug_release!(Slp);

#[cfg_attr(debug_assertions, derive(Debug))]
#[derive(Clone, Serialize, Deserialize, PartialEq, Eq, Default)]
#[serde(rename_all = "snake_case")]
pub struct Slp {
    pub license_key: String,
    pub license_state: String,
    pub license_expires_at: Option<String>,
    pub last_verified_at: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Default)]
#[serde(rename_all = "snake_case")]
pub struct StoreMeta {
    pub store_id: String,
    pub store_name: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Default)]
#[serde(rename_all = "snake_case")]
pub struct StoreRegistry {
    pub active_store_id: Option<String>,
    #[serde(default)]
    pub stores: Vec<StoreMeta>,
}

impl StoreRegistry {
    pub fn active_store(&self) -> Option<StoreMeta> {
        let active_store_id = self.active_store_id.as_deref()?;
        self.find_store(active_store_id)
    }

    pub fn find_store(&self, store_id: &str) -> Option<StoreMeta> {
        self.stores
            .iter()
            .find(|store| store.store_id == store_id)
            .cloned()
    }

    pub fn upsert_store(&mut self, store: StoreMeta) -> bool {
        if let Some(existing) = self
            .stores
            .iter_mut()
            .find(|existing| existing.store_id == store.store_id)
        {
            existing.store_name = store.store_name;
            false
        } else {
            self.stores.push(store);
            true
        }
    }

    pub fn set_active_store(&mut self, store_id: impl Into<String>) {
        self.active_store_id = Some(store_id.into());
    }
}

/// Tauri 全局状态：Cookie / 授权信息在内存 + 磁盘双写。
///
/// ## 锁顺序协议（避免潜在死锁，新 handler 必须遵守）
///
/// 当一个函数需要**同时**持有多把下列 `Mutex`/`RwLock` 时，**必须按以下顺序获取**：
///
/// 1. `store_registry`
/// 2. `cookie_profile`
/// 3. `cookie_path`
/// 4. `runtime_license_state`
/// 5. `license_profile`
/// 6. `cookie_health`
///
/// 单锁持有或用 `{}` 显式作用域**串行**获取多把锁，无需顺序约束。
/// 违反顺序短期可能靠 `{}` 立即释放规避，但一旦某处 await 点跨越
/// guard 生命周期就会出现真实死锁——保险起见始终按此协议写。
pub struct AppState {
    pub store_registry: Mutex<StoreRegistry>,
    pub cookie_profile: Mutex<CookieProfile>,
    pub cookie_path: Mutex<PathBuf>,
    pub app_home_dir: PathBuf,
    pub integrity_manifest_path: Option<PathBuf>,
    pub device_id: String,
    pub lease_store: Arc<dyn SecretStore>,
    pub lease_verifier: LeaseVerifier,
    pub task_grant_cache: TaskGrantCache,
    pub runtime_license_state: Mutex<RuntimeState>,
    pub license_profile: Mutex<Slp>,
    /// 批量发货取消标志：由 `cancel_batch_delivery` 置 true，
    /// `batch_delivery` 进入循环前重置为 false。
    pub batch_delivery_cancel: Arc<AtomicBool>,
    /// Cookie 健康状态快照（启动 / 定时探测后刷新）。
    pub cookie_health: Mutex<CookieHealthSnapshot>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Default)]
#[serde(rename_all = "snake_case")]
pub struct CookieHealthSnapshot {
    pub healthy: bool,
    pub configured: bool,
    pub has_biz_magic: bool,
    pub last_checked_at: Option<String>,
    pub hint: Option<String>,
}

impl AppState {
    pub fn new() -> Self {
        let app_home_dir = app_home_dir();
        let device_id = security_core::get_device_id();
        let store_registry = load_store_registry(&app_home_dir).unwrap_or_default();
        let lease_store = init_default_store(&app_home_dir, &device_id);
        let (lease_verifier, verifier_compromised) = init_lease_verifier_or_sentinel();
        let integrity_manifest_path = find_integrity_manifest_path(&app_home_dir);
        let runtime_license_state = load_runtime_state_from_store(
            lease_store.as_ref(),
            &device_id,
            now_epoch(),
            &lease_verifier,
        )
        .unwrap_or_else(|err| {
            tracing::warn!("启动时恢复本地 Lease 失败，降级为 invalid：{err}");
            RuntimeState::reason_only(LicenseState::Invalid)
        });
        let runtime_license_state = if verifier_compromised {
            RuntimeState {
                reason: LicenseState::Compromised,
                status_hint: LicenseState::Compromised,
                compromised: true,
                runtime_backend: "rust".to_string(),
                ..runtime_license_state
            }
        } else {
            runtime_license_state
        };
        let runtime_license_state =
            if let Err(err) = validate_integrity_if_present(integrity_manifest_path.as_deref()) {
                tracing::error!("启动时完整性校验失败：{err}");
                RuntimeState {
                    reason: LicenseState::Compromised,
                    status_hint: LicenseState::Compromised,
                    compromised: true,
                    runtime_backend: "rust".to_string(),
                    ..runtime_license_state
                }
            } else {
                runtime_license_state
            };
        let cookie_path = resolve_store_cookie_path(&app_home_dir, &store_registry)
            .unwrap_or_else(|| resolve_cookie_path(&app_home_dir));
        let profile = load_cookie_from_file(&cookie_path);
        let license_profile = load_license_profile(&app_home_dir).unwrap_or_default();
        let cookie_health = cookie_health_from_profile(&profile);
        Self {
            store_registry: Mutex::new(store_registry),
            cookie_profile: Mutex::new(profile),
            cookie_path: Mutex::new(cookie_path),
            app_home_dir,
            integrity_manifest_path,
            device_id,
            lease_store,
            lease_verifier,
            task_grant_cache: TaskGrantCache::new(),
            runtime_license_state: Mutex::new(runtime_license_state),
            license_profile: Mutex::new(license_profile),
            batch_delivery_cancel: Arc::new(AtomicBool::new(false)),
            cookie_health: Mutex::new(cookie_health),
        }
    }
}

pub fn app_home_dir() -> PathBuf {
    dirs::home_dir()
        .unwrap_or_else(|| std::env::current_dir().unwrap_or_else(|_| PathBuf::from(".")))
        .join(APP_HOME_DIR_NAME)
}

pub fn login_webview_data_dir(app_home_dir: &Path) -> PathBuf {
    app_home_dir.join(LOGIN_WEBVIEW_DIR_NAME)
}

pub fn store_login_webview_data_dir(app_home_dir: &Path, store_id: &str) -> PathBuf {
    store_dir(app_home_dir, store_id).join(LOGIN_WEBVIEW_DIR_NAME)
}

pub fn find_integrity_manifest_path(app_home_dir: &Path) -> Option<PathBuf> {
    let mut candidates = Vec::new();
    candidates.push(app_home_dir.to_path_buf());
    if let Ok(mut dir) = std::env::current_dir() {
        candidates.push(dir.clone());
        for _ in 0..5 {
            if !dir.pop() {
                break;
            }
            candidates.push(dir.clone());
        }
    }

    candidates
        .into_iter()
        .map(|dir| dir.join(security_core::INTEGRITY_MANIFEST_FILE_NAME))
        .find(|path| path.exists())
}

pub fn cookie_path_in_dir(dir: &Path) -> PathBuf {
    dir.join(COOKIE_FILE_NAME)
}

pub fn store_registry_path(app_home_dir: &Path) -> PathBuf {
    app_home_dir.join(STORE_REGISTRY_FILE_NAME)
}

pub fn stores_root(app_home_dir: &Path) -> PathBuf {
    app_home_dir.join(STORES_DIR_NAME)
}

pub fn store_dir(app_home_dir: &Path, store_id: &str) -> PathBuf {
    stores_root(app_home_dir).join(store_id.trim())
}

pub fn store_cookie_path(app_home_dir: &Path, store_id: &str) -> PathBuf {
    store_dir(app_home_dir, store_id).join(COOKIE_FILE_NAME)
}

pub fn store_meta_path(app_home_dir: &Path, store_id: &str) -> PathBuf {
    store_dir(app_home_dir, store_id).join(STORE_META_FILE_NAME)
}

fn cookie_dir_pointer_path(app_home_dir: &Path) -> PathBuf {
    app_home_dir.join(COOKIE_DIR_POINTER_FILE)
}

fn license_profile_path(app_home_dir: &Path) -> PathBuf {
    app_home_dir.join(LICENSE_FILE_NAME)
}

// 历史遗留：用户自定义 cookie 目录的保存路径。
//
// 现状（保留理由）：
// - 生产路径已被多店铺 store registry 取代，运行时不再调用本函数。
// - 仍保留 `pub fn` 形态是因为 `load_user_cookie_dir` + `resolve_cookie_path`
//   还会读旧 pointer 文件做 cookie 路径回退（迁移期老用户机器上残留），写入侧
//   只在 `#[cfg(test)]` 下被回归测试触发，证明读写 pair 行为一致。
// - 完全删除会丢掉读写对称的回归保护；改为 `#[cfg(test)]` 又会让外部
//   `pub fn` 在 release 下消失，扰动 cargo doc 与潜在的迁移脚本调用。
//
// 因此保留 `#[allow(dead_code)]` 抑制 release 警告即可；新代码请勿在生产路径
// 调用本函数，应通过 `StoreRegistry` API 维护 cookie 目录。
#[allow(dead_code)]
pub fn save_user_cookie_dir(app_home_dir: &Path, selected_dir: &Path) -> std::io::Result<()> {
    let selected_dir = selected_dir.expand_home()?.canonicalize()?;
    std::fs::create_dir_all(&selected_dir)?;
    std::fs::create_dir_all(app_home_dir)?;
    std::fs::write(
        cookie_dir_pointer_path(app_home_dir),
        selected_dir.to_string_lossy().as_ref(),
    )
}

pub fn load_user_cookie_dir(app_home_dir: &Path) -> Option<PathBuf> {
    let raw = std::fs::read_to_string(cookie_dir_pointer_path(app_home_dir)).ok()?;
    let trimmed = raw.trim();
    if trimmed.is_empty() {
        None
    } else {
        Some(PathBuf::from(trimmed))
    }
}

pub fn resolve_cookie_path(app_home_dir: &Path) -> PathBuf {
    let store_registry = load_store_registry(app_home_dir).unwrap_or_default();
    if let Some(path) = resolve_store_cookie_path(app_home_dir, &store_registry) {
        return path;
    }

    if let Some(saved_dir) = load_user_cookie_dir(app_home_dir) {
        return cookie_path_in_dir(&saved_dir);
    }

    let mut dir = std::env::current_dir().unwrap_or_else(|_| PathBuf::from("."));
    for _ in 0..5 {
        let candidate = dir.join(COOKIE_FILE_NAME);
        if candidate.exists() {
            return candidate;
        }

        let cargo = dir.join("Cargo.toml");
        if cargo.exists()
            && std::fs::read_to_string(&cargo)
                .map(|content| content.contains("[workspace]"))
                .unwrap_or(false)
        {
            return dir.join(COOKIE_FILE_NAME);
        }

        if !dir.pop() {
            break;
        }
    }

    cookie_path_in_dir(app_home_dir)
}

pub fn resolve_store_cookie_path(
    app_home_dir: &Path,
    store_registry: &StoreRegistry,
) -> Option<PathBuf> {
    let active_store = store_registry.active_store()?;
    Some(store_cookie_path(app_home_dir, &active_store.store_id))
}

fn parse_biz_magic(cookie_header: &str) -> Option<String> {
    let profile = parse_cookie_profile(cookie_header);
    profile.biz_magic
}

pub fn load_cookie_from_file(path: &Path) -> CookieProfile {
    match std::fs::read_to_string(path) {
        Ok(content) => {
            let cookie_header = content.trim().to_string();
            if cookie_header.is_empty() {
                return CookieProfile::default();
            }
            let biz_magic = parse_biz_magic(&cookie_header);
            tracing::debug!(
                target: "state.cookie.load",
                file = %path.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default(),
                biz_magic = if biz_magic.is_some() { "extracted" } else { "missing" },
                "从磁盘加载 Cookie",
            );
            CookieProfile {
                cookie_header,
                biz_magic,
            }
        }
        Err(_) => {
            tracing::debug!(
                target: "state.cookie.load",
                file = %path.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default(),
                "Cookie 文件不存在，返回空 CookieProfile",
            );
            CookieProfile::default()
        }
    }
}

pub fn cookie_health_from_profile(profile: &CookieProfile) -> CookieHealthSnapshot {
    CookieHealthSnapshot {
        healthy: false,
        configured: !profile.cookie_header.is_empty(),
        has_biz_magic: profile.biz_magic.is_some(),
        last_checked_at: None,
        hint: if profile.cookie_header.is_empty() {
            Some("尚未配置 Cookie".to_string())
        } else {
            Some("尚未探测".to_string())
        },
    }
}

pub fn save_cookie_to_file(path: &Path, cookie_header: &str) -> std::io::Result<()> {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    std::fs::write(path, cookie_header)
}

pub fn load_store_registry(app_home_dir: &Path) -> Option<StoreRegistry> {
    let text = std::fs::read_to_string(store_registry_path(app_home_dir)).ok()?;
    serde_json::from_str(&text).ok()
}

pub fn save_store_registry(app_home_dir: &Path, registry: &StoreRegistry) -> anyhow::Result<()> {
    std::fs::create_dir_all(app_home_dir)?;
    let text = serde_json::to_string_pretty(registry)?;
    std::fs::write(store_registry_path(app_home_dir), text)?;
    Ok(())
}

pub fn save_store_meta(app_home_dir: &Path, store: &StoreMeta) -> anyhow::Result<()> {
    let path = store_meta_path(app_home_dir, &store.store_id);
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    let text = serde_json::to_string_pretty(store)?;
    std::fs::write(path, text)?;
    Ok(())
}

pub fn load_license_profile(app_home_dir: &Path) -> Option<Slp> {
    let text = std::fs::read_to_string(license_profile_path(app_home_dir)).ok()?;
    serde_json::from_str(&text).ok()
}

pub fn save_license_profile(app_home_dir: &Path, profile: &Slp) -> std::io::Result<()> {
    std::fs::create_dir_all(app_home_dir)?;
    let text = serde_json::to_string_pretty(profile)?;
    std::fs::write(license_profile_path(app_home_dir), text)
}

pub fn runtime_state_to_license_state(runtime: &RuntimeState) -> String {
    match runtime.reason {
        LicenseState::Active if runtime.status_hint == LicenseState::RenewalDue => {
            "renewal_due".to_string()
        }
        LicenseState::Active => "active".to_string(),
        LicenseState::NotFound => "not_found".to_string(),
        LicenseState::Invalid => "invalid".to_string(),
        LicenseState::Expired => "expired".to_string(),
        LicenseState::DeviceMismatch => "device_mismatch".to_string(),
        LicenseState::ReactivationRequired => "reactivation_required".to_string(),
        LicenseState::Revoked => "revoked".to_string(),
        LicenseState::OnlineRefreshRequired => "online_refresh_required".to_string(),
        LicenseState::RenewalDue => "renewal_due".to_string(),
        LicenseState::Compromised => "compromised".to_string(),
    }
}

pub fn load_runtime_state_from_store(
    store: &dyn SecretStore,
    device_id: &str,
    now_epoch: i64,
    verifier: &LeaseVerifier,
) -> Result<RuntimeState, String> {
    let token = match store.get() {
        Ok(token) => token,
        Err(StorageError::DeviceChanged) => {
            return Ok(RuntimeState::reason_only(
                LicenseState::ReactivationRequired,
            ));
        }
        Err(err) => return Err(err.to_string()),
    };

    Ok(verify_stored_lease_local(
        token.as_deref(),
        device_id,
        now_epoch,
        verifier,
    ))
}

pub fn verify_and_store_license_token(
    store: &dyn SecretStore,
    token: &str,
    device_id: &str,
    now_epoch: i64,
    verifier: &LeaseVerifier,
) -> Result<RuntimeState, String> {
    verifier
        .verify(token, Some(device_id), now_epoch, false)
        .map_err(|err| err.to_string())?;
    store.set(token).map_err(|err| err.to_string())?;
    load_runtime_state_from_store(store, device_id, now_epoch, verifier)
}

fn now_epoch() -> i64 {
    chrono::Utc::now().timestamp()
}

pub fn validate_integrity_if_present(manifest_path: Option<&Path>) -> Result<(), String> {
    let Some(manifest_path) = manifest_path else {
        return Ok(());
    };
    security_core::validate_runtime_continuity(
        manifest_path,
        license_service::LICENSE_PUBLIC_KEY_B64,
    )
    .map_err(|err| err.to_string())
}

// 仅被 save_user_cookie_dir 内部使用；后者本身已标 allow(dead_code)，此 trait 同步 allow
#[allow(dead_code)]
trait ExpandHome {
    fn expand_home(&self) -> std::io::Result<PathBuf>;
}

impl ExpandHome for Path {
    fn expand_home(&self) -> std::io::Result<PathBuf> {
        let raw = self.to_string_lossy();
        if raw == "~" || raw.starts_with("~/") {
            let home = dirs::home_dir().ok_or_else(|| {
                std::io::Error::new(std::io::ErrorKind::NotFound, "无法定位用户主目录")
            })?;
            if raw == "~" {
                Ok(home)
            } else {
                Ok(home.join(raw.trim_start_matches("~/")))
            }
        } else {
            Ok(self.to_path_buf())
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::adapters::secure_storage::{InMemorySecretStore, SecretStore};
    use base64::engine::general_purpose::URL_SAFE_NO_PAD;
    use base64::Engine;
    use ed25519_dalek::{Signer, SigningKey};
    use license_service::LeaseVerifier;
    use rand::rngs::OsRng;
    use serde_json::json;
    use std::sync::Arc;

    fn unique_temp_dir(name: &str) -> PathBuf {
        let nanos = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let dir = std::env::temp_dir().join(format!(
            "tls_shipinhao_{name}_{}_{}",
            std::process::id(),
            nanos
        ));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        dir
    }

    #[test]
    fn remembers_selected_cookie_dir_and_uses_cookie_txt() {
        let home = unique_temp_dir("cookie_home");
        let selected = unique_temp_dir("cookie_selected");

        save_user_cookie_dir(&home, &selected).unwrap();
        let loaded = load_user_cookie_dir(&home).expect("saved dir");
        let canonical_selected = selected.canonicalize().unwrap();
        assert_eq!(loaded, canonical_selected);
        assert_eq!(
            cookie_path_in_dir(&loaded),
            canonical_selected.join("cookie.txt")
        );
    }

    #[test]
    fn persists_license_profile_roundtrip() {
        let home = unique_temp_dir("license_home");
        let profile = Slp {
            license_key: "TLS-TEST".into(),
            license_state: "active".into(),
            license_expires_at: Some("2026-05-01T00:00:00Z".into()),
            last_verified_at: Some("2026-04-16T08:00:00Z".into()),
        };

        save_license_profile(&home, &profile).unwrap();
        let loaded = load_license_profile(&home).expect("saved profile");
        assert_eq!(loaded, profile);
    }

    fn signed_lease_token(device_id: &str, renew_after: i64, exp: i64) -> (String, String) {
        let signing_key = SigningKey::generate(&mut OsRng);
        let verifying_key_b64 = URL_SAFE_NO_PAD.encode(signing_key.verifying_key().as_bytes());
        let payload = json!({
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

    #[test]
    fn verified_lease_token_roundtrip_stores_runtime_bundle() {
        let store: Arc<dyn SecretStore> = Arc::new(InMemorySecretStore::new());
        let (token, public_key_b64) = signed_lease_token("dev-1", 1_800_000_000, 1_900_000_000);

        let runtime = verify_and_store_license_token(
            store.as_ref(),
            &token,
            "dev-1",
            1_750_000_000,
            &LeaseVerifier::from_public_key_b64(&public_key_b64).unwrap(),
        )
        .expect("token should be accepted");

        assert_eq!(store.get().unwrap().as_deref(), Some(token.as_str()));
        assert_eq!(runtime.license_key, "TLS-TEST");
        assert_eq!(runtime.device_id, "dev-1");
    }

    #[test]
    fn runtime_state_uses_renewal_due_for_soft_refresh_window() {
        let store: Arc<dyn SecretStore> = Arc::new(InMemorySecretStore::new());
        let (token, public_key_b64) = signed_lease_token("dev-2", 1_750_000_000, 1_900_000_000);
        store.set(&token).unwrap();

        let runtime = load_runtime_state_from_store(
            store.as_ref(),
            "dev-2",
            1_800_000_000,
            &LeaseVerifier::from_public_key_b64(&public_key_b64).unwrap(),
        )
        .expect("runtime state should load");

        assert_eq!(runtime_state_to_license_state(&runtime), "renewal_due");
    }

    #[test]
    fn storage_device_change_maps_to_reactivation_required() {
        let old_store_root = unique_temp_dir("lease_store_old");
        let new_store_root = old_store_root.clone();
        let old_store = crate::adapters::secure_storage::EncryptedFileSecretStore::new(
            &old_store_root,
            "dev-old",
        );
        old_store.set("sensitive-lease").unwrap();
        let new_store = crate::adapters::secure_storage::EncryptedFileSecretStore::new(
            &new_store_root,
            "dev-new",
        );
        let verifier = LeaseVerifier::new().unwrap();

        let runtime =
            load_runtime_state_from_store(&new_store, "dev-new", 1_800_000_000, &verifier)
                .expect("device drift should map to runtime state");

        assert_eq!(
            runtime_state_to_license_state(&runtime),
            "reactivation_required"
        );
    }

    #[test]
    fn validate_integrity_if_present_allows_missing_manifest() {
        assert!(validate_integrity_if_present(None).is_ok());
    }

    #[test]
    fn init_lease_verifier_succeeds_with_real_constant() {
        // 正常路径：编译期常量正确，verifier 构造成功，第二个返回值 false
        let (_verifier, compromised) = init_lease_verifier_or_sentinel();
        assert!(
            !compromised,
            "LICENSE_PUBLIC_KEY_B64 正确时不应进入哨兵分支"
        );
    }

    #[test]
    fn lease_verifier_sentinel_fingerprint_rejects_real_signatures() {
        // 哨兵公钥（[1u8;32]）与生产签名永远不匹配；任意 Token 都应校验失败而非 panic。
        let sentinel_b64 = base64::engine::general_purpose::URL_SAFE_NO_PAD.encode([1u8; 32]);
        let sentinel = LeaseVerifier::from_public_key_b64(&sentinel_b64)
            .expect("[1u8;32] 必须能解析为合法 ed25519 公钥");
        let probe_token = "AAAA.AAAA";
        let result = sentinel.verify(probe_token, None, 0, true);
        assert!(
            result.is_err(),
            "哨兵 verifier 不能让任意 token 通过签名校验"
        );
    }

    #[test]
    fn find_integrity_manifest_path_picks_existing_file() {
        let home = unique_temp_dir("integrity_home");
        let manifest = home.join(security_core::INTEGRITY_MANIFEST_FILE_NAME);
        std::fs::write(&manifest, "{}").unwrap();
        assert_eq!(find_integrity_manifest_path(&home), Some(manifest));
    }

    #[test]
    fn store_registry_roundtrip_persists_active_store() {
        let home = unique_temp_dir("store_registry_home");
        let registry = StoreRegistry {
            active_store_id: Some("wx61f28d69d9174ddf".into()),
            stores: vec![StoreMeta {
                store_id: "wx61f28d69d9174ddf".into(),
                store_name: "精选内衣店".into(),
            }],
        };

        save_store_registry(&home, &registry).unwrap();
        let loaded = load_store_registry(&home).expect("saved registry");

        assert_eq!(loaded, registry);
        assert_eq!(
            loaded.active_store().expect("active store").store_name,
            "精选内衣店"
        );
    }

    #[test]
    fn resolve_cookie_path_prefers_active_store_cookie_file() {
        let home = unique_temp_dir("store_cookie_home");
        let registry = StoreRegistry {
            active_store_id: Some("wx61f28d69d9174ddf".into()),
            stores: vec![StoreMeta {
                store_id: "wx61f28d69d9174ddf".into(),
                store_name: "精选内衣店".into(),
            }],
        };
        save_store_registry(&home, &registry).unwrap();

        assert_eq!(
            resolve_cookie_path(&home),
            store_cookie_path(&home, "wx61f28d69d9174ddf")
        );
    }
}
