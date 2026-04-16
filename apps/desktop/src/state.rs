use tokio::sync::Mutex;

use desktop_services::CookieProfile;

/// Tauri 全局状态：当前仅持久化内存中的 Cookie（与 Python 侧「设置里填 Cookie」一致）。
pub struct AppState {
    pub cookie_profile: Mutex<CookieProfile>,
}

impl AppState {
    pub fn new() -> Self {
        Self {
            cookie_profile: Mutex::new(CookieProfile::default()),
        }
    }
}
