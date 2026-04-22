//! 桌面端命令的公共前置校验 helper。
//!
//! 原本 `order` / `review` / `delivery` 命令各自复制了「取锁 → 判 cookie 非空 → 克隆 cookie/biz_magic」
//! 6 次，任何一处改文案或加埋点都要漏改一份。这里抽到一处，保证所有命令走同一套检查。

use crate::error::AppError;
use crate::state::AppState;
use tauri::State;

/// 业务命令运行所需的 Cookie 凭证，字段已脱出 `Mutex` 锁。
pub(crate) struct CookieCredentials {
    pub(crate) cookie: String,
    pub(crate) magic: String,
}

/// 校验应用状态中是否已保存合法 Cookie，并把 `cookie_header` / `biz_magic` 克隆返回。
///
/// 行为与各命令中的原重复分支一致：
/// - 为空时返回 `AppError::Message("请先在设置中配置 Cookie")`
/// - 非空时克隆两个字段后立即释放锁
pub(crate) async fn require_cookie_credentials(
    state: &State<'_, AppState>,
) -> Result<CookieCredentials, AppError> {
    let cookie_profile = state.cookie_profile.lock().await;
    if cookie_profile.cookie_header.is_empty() {
        return Err(AppError::Message("请先在设置中配置 Cookie".to_string()));
    }
    Ok(CookieCredentials {
        cookie: cookie_profile.cookie_header.clone(),
        magic: cookie_profile.biz_magic.clone().unwrap_or_default(),
    })
}
