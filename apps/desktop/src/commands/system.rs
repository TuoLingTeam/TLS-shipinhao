use desktop_services::parse_cookie_profile;
use desktop_services::update_service::{fetch_latest_version_info, UpdateInfo};
use domain_core::brand::{get_window_title, APP_NAME, APP_NAME_EN, AUTHOR_WECHAT};
use reqwest::Url;
use tauri::{Manager, State, WebviewUrl, WebviewWindowBuilder};

use crate::error::AppError;
use crate::migration::{LegacyPythonMigrator, MigrationPaths, MigrationReport};
use crate::state::{self, AppState};

const STORE_LOGIN_URL: &str = "https://store.weixin.qq.com/";
const COOKIE_LOGIN_WINDOW_LABEL: &str = "cookie-login";

const DEFAULT_UI_SCALE: f64 = 1.0;
const MIN_UI_SCALE: f64 = 0.82;
const MAX_UI_SCALE: f64 = 1.0;

fn clamp_ui_scale(scale: f64) -> f64 {
    scale.clamp(MIN_UI_SCALE, MAX_UI_SCALE)
}


#[tauri::command]
pub async fn check_for_update() -> Result<UpdateInfo, AppError> {
    fetch_latest_version_info(None)
        .await
        .map_err(|e| AppError::Message(format!("检查更新失败：{e}")))
}

#[tauri::command]
pub async fn get_app_info() -> Result<serde_json::Value, AppError> {
    let version = env!("CARGO_PKG_VERSION");
    Ok(serde_json::json!({
        "name": APP_NAME,
        "name_en": APP_NAME_EN,
        "version": version,
        "author_wechat": AUTHOR_WECHAT,
        "window_title": get_window_title(version),
        "runtime": "tauri-2.0",
    }))
}

#[tauri::command]
pub async fn get_ui_scale() -> Result<f64, AppError> {
    Ok(DEFAULT_UI_SCALE)
}

#[tauri::command(rename_all = "snake_case")]
pub async fn set_ui_scale(scale: f64) -> Result<f64, AppError> {
    Ok(clamp_ui_scale(scale))
}

#[tauri::command(rename_all = "snake_case")]
pub async fn set_cookie(
    state: State<'_, AppState>,
    cookie_header: String,
) -> Result<serde_json::Value, AppError> {
    let profile = parse_cookie_profile(cookie_header.trim());
    let cookie_path = { state.cookie_path.lock().await.clone() };

    {
        let mut current = state.cookie_profile.lock().await;
        *current = profile.clone();
    }

    state::save_cookie_to_file(&cookie_path, &profile.cookie_header)
        .map_err(|e| AppError::Message(format!("保存 Cookie 失败：{e}")))?;

    Ok(serde_json::json!({
        "success": true,
        "biz_magic": profile.biz_magic,
        "cookie_path": cookie_path.display().to_string(),
    }))
}

#[tauri::command(rename_all = "snake_case")]
pub async fn get_cookie_status(state: State<'_, AppState>) -> Result<serde_json::Value, AppError> {
    let profile = state.cookie_profile.lock().await;
    let cookie_path = state.cookie_path.lock().await.clone();
    Ok(serde_json::json!({
        "configured": !profile.cookie_header.is_empty(),
        "has_biz_magic": profile.biz_magic.is_some(),
        "cookie_path": cookie_path.display().to_string(),
    }))
}

#[tauri::command(rename_all = "snake_case")]
pub async fn pick_cookie_save_dir(
    state: State<'_, AppState>,
) -> Result<serde_json::Value, AppError> {
    let default_dir = {
        let current_path = state.cookie_path.lock().await.clone();
        current_path
            .parent()
            .map(|path| path.to_path_buf())
            .unwrap_or_else(|| state.app_home_dir.clone())
    };

    let selected_dir = tokio::task::spawn_blocking(move || {
        rfd::FileDialog::new()
            .set_directory(default_dir)
            .pick_folder()
    })
    .await
    .map_err(|e| AppError::Message(format!("打开目录选择器失败：{e}")))?;

    let Some(selected_dir) = selected_dir else {
        let cookie_path = state.cookie_path.lock().await.clone();
        return Ok(serde_json::json!({
            "selected": false,
            "cookie_path": cookie_path.display().to_string(),
        }));
    };

    state::save_user_cookie_dir(&state.app_home_dir, &selected_dir)
        .map_err(|e| AppError::Message(format!("记录 Cookie 保存目录失败：{e}")))?;

    let new_cookie_path = state::cookie_path_in_dir(&selected_dir);
    let current_profile = state.cookie_profile.lock().await.clone();
    if !current_profile.cookie_header.is_empty() {
        state::save_cookie_to_file(&new_cookie_path, &current_profile.cookie_header)
            .map_err(|e| AppError::Message(format!("同步 Cookie 到新目录失败：{e}")))?;
    }

    {
        let mut current_path = state.cookie_path.lock().await;
        *current_path = new_cookie_path.clone();
    }

    Ok(serde_json::json!({
        "selected": true,
        "cookie_path": new_cookie_path.display().to_string(),
    }))
}

#[tauri::command(rename_all = "snake_case")]
pub async fn open_cookie_login(
    app: tauri::AppHandle,
    state: State<'_, AppState>,
) -> Result<serde_json::Value, AppError> {
    if let Some(existing) = app.get_webview_window(COOKIE_LOGIN_WINDOW_LABEL) {
        let _ = existing.show();
        let _ = existing.set_focus();
        return Ok(serde_json::json!({
            "success": true,
            "already_open": true,
        }));
    }

    let login_url =
        Url::parse(STORE_LOGIN_URL).map_err(|e| AppError::Message(format!("登录地址无效：{e}")))?;
    let data_dir = state::login_webview_data_dir(&state.app_home_dir);

    WebviewWindowBuilder::new(
        &app,
        COOKIE_LOGIN_WINDOW_LABEL,
        WebviewUrl::External(login_url),
    )
    .title("视频号小店登录")
    .inner_size(1280.0, 860.0)
    .resizable(true)
    .data_directory(data_dir)
    .build()
    .map_err(|e| AppError::Message(format!("打开登录窗口失败：{e}")))?;

    Ok(serde_json::json!({
        "success": true,
        "already_open": false,
    }))
}

#[tauri::command(rename_all = "snake_case")]
pub async fn extract_cookie_from_login(
    app: tauri::AppHandle,
    state: State<'_, AppState>,
) -> Result<serde_json::Value, AppError> {
    let window = app
        .get_webview_window(COOKIE_LOGIN_WINDOW_LABEL)
        .ok_or_else(|| AppError::Message("请先点击“打开登录页”并完成登录".to_string()))?;

    let cookies = window
        .cookies_for_url(
            Url::parse(STORE_LOGIN_URL)
                .map_err(|e| AppError::Message(format!("读取 Cookie 失败：{e}")))?,
        )
        .map_err(|e| AppError::Message(format!("读取登录窗口 Cookie 失败：{e}")))?;

    let cookie_header = serialize_store_cookie_header(&cookies);
    if cookie_header.is_empty() || !looks_like_logged_in_store_session(&cookies) {
        return Err(AppError::Message(
            "暂未检测到视频号小店登录态，请在弹出的登录页完成登录后再提取".to_string(),
        ));
    }

    let profile = parse_cookie_profile(&cookie_header);
    let cookie_path = { state.cookie_path.lock().await.clone() };

    {
        let mut current = state.cookie_profile.lock().await;
        *current = profile.clone();
    }

    state::save_cookie_to_file(&cookie_path, &cookie_header)
        .map_err(|e| AppError::Message(format!("写入 Cookie 文件失败：{e}")))?;

    Ok(serde_json::json!({
        "success": true,
        "biz_magic": profile.biz_magic,
        "cookie_header": cookie_header,
        "cookie_path": cookie_path.display().to_string(),
    }))
}

fn serialize_store_cookie_header(cookies: &[tauri::webview::Cookie<'static>]) -> String {
    let mut pairs = cookies
        .iter()
        .filter(|cookie| {
            cookie
                .domain()
                .map(|domain| domain.contains("weixin"))
                .unwrap_or(true)
        })
        .filter_map(|cookie| {
            let name = cookie.name().trim();
            let value = cookie.value_trimmed().trim();
            if name.is_empty() || value.is_empty() {
                None
            } else {
                Some((name.to_string(), value.to_string()))
            }
        })
        .collect::<Vec<_>>();

    pairs.sort_by(|a, b| a.0.cmp(&b.0));
    pairs.dedup_by(|left, right| left.0 == right.0);

    pairs
        .into_iter()
        .map(|(name, value)| format!("{name}={value}"))
        .collect::<Vec<_>>()
        .join("; ")
}

/// 触发从 Python 4.3.0 旧目录的一次性数据迁移。
///
/// 幂等：新目录已有同名文件时直接跳过，不覆盖。所有错误聚合在 `MigrationReport.errors`。
#[tauri::command(rename_all = "snake_case")]
pub async fn start_legacy_migration() -> Result<MigrationReport, AppError> {
    tokio::task::spawn_blocking(|| {
        let paths = MigrationPaths::default_platform()
            .map_err(|e| AppError::Message(format!("解析迁移路径失败：{e}")))?;
        Ok::<MigrationReport, AppError>(LegacyPythonMigrator::new(paths).run())
    })
    .await
    .map_err(|e| AppError::Message(e.to_string()))?
}

fn looks_like_logged_in_store_session(cookies: &[tauri::webview::Cookie<'static>]) -> bool {
    cookies.iter().any(|cookie| {
        let name = cookie.name().to_ascii_lowercase();
        let value = cookie.value_trimmed();
        let domain_ok = cookie
            .domain()
            .map(|domain| domain.contains("store.weixin"))
            .unwrap_or(true);
        domain_ok
            && value.len() >= 8
            && [
                "sid", "sess", "token", "ticket", "biz", "auth", "key", "uin", "pass",
            ]
            .iter()
            .any(|keyword| name.contains(keyword))
    })
}


#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn get_app_info_returns_brand_fields() {
        let payload = get_app_info().await.expect("app info");
        assert_eq!(payload["name"], APP_NAME);
        assert_eq!(payload["name_en"], APP_NAME_EN);
        assert_eq!(payload["author_wechat"], AUTHOR_WECHAT);
        assert!(payload["window_title"].as_str().unwrap_or("").contains(APP_NAME));
    }

    #[tokio::test]
    async fn set_ui_scale_clamps_to_supported_range() {
        assert_eq!(get_ui_scale().await.expect("default scale"), 1.0);
        assert_eq!(set_ui_scale(0.6).await.expect("low scale"), 0.82);
        assert_eq!(set_ui_scale(1.2).await.expect("high scale"), 1.0);
        assert_eq!(set_ui_scale(0.9).await.expect("normal scale"), 0.9);
    }
}
