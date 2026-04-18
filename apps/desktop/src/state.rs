use std::path::{Path, PathBuf};
use std::sync::atomic::AtomicBool;
use std::sync::Arc;

use crate::adapters::secure_storage::{init_default_store, SecretStore, StorageError};
use api_contracts::{LicenseState, RuntimeState};
use desktop_services::{parse_cookie_profile, CookieProfile};
use license_service::{verify_stored_lease_local, LeaseVerifier, TaskGrantCache};
use serde::{Deserialize, Serialize};
use tokio::sync::Mutex;

const APP_HOME_DIR_NAME: &str = ".tls-shipinhao";
const COOKIE_FILE_NAME: &str = "cookie.txt";
const COOKIE_DIR_POINTER_FILE: &str = "selected_config_dir.txt";
const LICENSE_FILE_NAME: &str = "license.json";
const LOGIN_WEBVIEW_DIR_NAME: &str = "login-webview";

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Default)]
#[serde(rename_all = "snake_case")]
pub struct StoredLicenseProfile {
    pub license_key: String,
    pub license_state: String,
    pub license_expires_at: Option<String>,
    pub last_verified_at: Option<String>,
}

/// Tauri 全局状态：Cookie / 授权信息在内存 + 磁盘双写。
pub struct AppState {
    pub cookie_profile: Mutex<CookieProfile>,
    pub cookie_path: Mutex<PathBuf>,
    pub app_home_dir: PathBuf,
    pub integrity_manifest_path: Option<PathBuf>,
    pub device_id: String,
    pub lease_store: Arc<dyn SecretStore>,
    pub lease_verifier: LeaseVerifier,
    pub task_grant_cache: TaskGrantCache,
    pub runtime_license_state: Mutex<RuntimeState>,
    pub license_profile: Mutex<StoredLicenseProfile>,
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
        let lease_store = init_default_store(&app_home_dir, &device_id);
        let lease_verifier =
            LeaseVerifier::new().unwrap_or_else(|err| panic!("授权公钥加载失败：{err}"));
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
        let cookie_path = resolve_cookie_path(&app_home_dir);
        let profile = load_cookie_from_file(&cookie_path);
        let license_profile = load_license_profile(&app_home_dir).unwrap_or_default();
        let cookie_health = CookieHealthSnapshot {
            healthy: false,
            configured: !profile.cookie_header.is_empty(),
            has_biz_magic: profile.biz_magic.is_some(),
            last_checked_at: None,
            hint: if profile.cookie_header.is_empty() {
                Some("尚未配置 Cookie".to_string())
            } else {
                Some("尚未探测".to_string())
            },
        };
        Self {
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

fn cookie_dir_pointer_path(app_home_dir: &Path) -> PathBuf {
    app_home_dir.join(COOKIE_DIR_POINTER_FILE)
}

fn license_profile_path(app_home_dir: &Path) -> PathBuf {
    app_home_dir.join(LICENSE_FILE_NAME)
}

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

fn parse_biz_magic(cookie_header: &str) -> Option<String> {
    let profile = parse_cookie_profile(cookie_header);
    profile.biz_magic
}

fn load_cookie_from_file(path: &Path) -> CookieProfile {
    match std::fs::read_to_string(path) {
        Ok(content) => {
            let cookie_header = content.trim().to_string();
            if cookie_header.is_empty() {
                return CookieProfile::default();
            }
            let biz_magic = parse_biz_magic(&cookie_header);
            eprintln!(
                "[state] 从 {} 加载 Cookie（biz_magic={}）",
                path.display(),
                if biz_magic.is_some() {
                    "已提取"
                } else {
                    "缺失"
                }
            );
            CookieProfile {
                cookie_header,
                biz_magic,
            }
        }
        Err(_) => {
            eprintln!("[state] Cookie 文件不存在：{}", path.display());
            CookieProfile::default()
        }
    }
}

pub fn save_cookie_to_file(path: &Path, cookie_header: &str) -> std::io::Result<()> {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    std::fs::write(path, cookie_header)
}

pub fn load_license_profile(app_home_dir: &Path) -> Option<StoredLicenseProfile> {
    let text = std::fs::read_to_string(license_profile_path(app_home_dir)).ok()?;
    serde_json::from_str(&text).ok()
}

pub fn save_license_profile(
    app_home_dir: &Path,
    profile: &StoredLicenseProfile,
) -> std::io::Result<()> {
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
        let profile = StoredLicenseProfile {
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
    fn find_integrity_manifest_path_picks_existing_file() {
        let home = unique_temp_dir("integrity_home");
        let manifest = home.join(security_core::INTEGRITY_MANIFEST_FILE_NAME);
        std::fs::write(&manifest, "{}").unwrap();
        assert_eq!(find_integrity_manifest_path(&home), Some(manifest));
    }
}
