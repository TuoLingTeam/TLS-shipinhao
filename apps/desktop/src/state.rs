use std::path::{Path, PathBuf};

use desktop_services::{parse_cookie_profile, CookieProfile};
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
    pub license_profile: Mutex<StoredLicenseProfile>,
}

impl AppState {
    pub fn new() -> Self {
        let app_home_dir = app_home_dir();
        let cookie_path = resolve_cookie_path(&app_home_dir);
        let profile = load_cookie_from_file(&cookie_path);
        let license_profile = load_license_profile(&app_home_dir).unwrap_or_default();
        Self {
            cookie_profile: Mutex::new(profile),
            cookie_path: Mutex::new(cookie_path),
            app_home_dir,
            license_profile: Mutex::new(license_profile),
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
}
