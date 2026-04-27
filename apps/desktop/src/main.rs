#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod adapters;
mod app_settings;
mod commands;
mod error;
mod migration;
mod state;

use desktop_services::update_service::{fetch_latest_version_info, UPDATE_CHECK_DELAY_MS};
use security_core::get_device_id;
use state::AppState;
use tauri::{Emitter, Manager};

/// 初始化全局 tracing 订阅器：
/// - 默认级别 `info`，尊重 `RUST_LOG` 环境变量（例如
///   `RUST_LOG=review.match.diagnostic=warn,desktop_services=info`）。
/// - 原来整个 app 没装订阅器，所有 `tracing::warn!/info!/error!` 都是哑巴
///   ——包括 HttpLicenseClient 的网络层警告与评价匹配诊断。补齐后这些
///   宝贵的运营日志才会出现在 `cargo tauri dev` 的终端输出里。
fn init_tracing() {
    use tracing_subscriber::{fmt, EnvFilter};
    let filter = EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new("info"));
    let _ = fmt()
        .with_env_filter(filter)
        .with_target(true)
        .with_level(true)
        .try_init();
}

#[cfg(windows)]
fn configure_portable_webview2_runtime() {
    use std::os::windows::process::CommandExt;

    const CREATE_NO_WINDOW: u32 = 0x08000000;

    if std::env::var_os("WEBVIEW2_BROWSER_EXECUTABLE_FOLDER").is_some() {
        return;
    }

    let Some(exe_dir) = std::env::current_exe()
        .ok()
        .and_then(|path| path.parent().map(std::path::Path::to_path_buf))
    else {
        return;
    };

    let runtime_root = exe_dir.join("WebView2Runtime");
    let mut candidates = vec![
        runtime_root.clone(),
        runtime_root.join("FixedVersionRuntime"),
    ];
    if let Ok(entries) = std::fs::read_dir(&runtime_root) {
        candidates.extend(
            entries
                .flatten()
                .map(|entry| entry.path())
                .filter(|path| path.is_dir()),
        );
    }

    if let Some(runtime_dir) = candidates
        .iter()
        .find(|dir| dir.join("msedgewebview2.exe").is_file())
    {
        for sid in ["*S-1-15-2-2:(OI)(CI)(RX)", "*S-1-15-2-1:(OI)(CI)(RX)"] {
            let _ = std::process::Command::new("icacls")
                .arg(runtime_dir)
                .arg("/grant")
                .arg(sid)
                .creation_flags(CREATE_NO_WINDOW)
                .status();
        }

        // WebView2Loader reads this before creating the first WebView2 environment.
        std::env::set_var("WEBVIEW2_BROWSER_EXECUTABLE_FOLDER", runtime_dir);
    }
}

#[cfg(not(windows))]
fn configure_portable_webview2_runtime() {}

fn main() {
    configure_portable_webview2_runtime();
    init_tracing();
    tauri::Builder::default()
        .manage(AppState::new())
        .setup(|app| {
            // 主窗口标题运行时派生版本号：tauri.conf.json 里只保留产品名，
            // 这里拼 " {version}"，升级时只改 version 字段即可同步标题栏。
            let version = app.package_info().version.to_string();
            if let Some(win) = app.get_webview_window("main") {
                let _ = win.set_title(&format!("驼铃·视频小店差评处理 {version}"));
            }

            let app_handle = app.handle().clone();
            tauri::async_runtime::spawn(async move {
                tokio::time::sleep(std::time::Duration::from_millis(UPDATE_CHECK_DELAY_MS)).await;
                let device_id = get_device_id();
                match fetch_latest_version_info(None, Some(&device_id)).await {
                    Ok(info) if info.has_update => {
                        let _ = app_handle.emit("update-available", info);
                    }
                    Ok(_) => {}
                    Err(err) => {
                        tracing::warn!(target: "update.check", %err, "跳过本次自动更新检查");
                    }
                }
            });
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            commands::review::find_reviews,
            commands::review::find_quality_refund_orders,
            commands::order::load_order_cache,
            commands::order::get_order_cache_status,
            commands::order::sync_recent_order_cache,
            commands::delivery::update_delivery,
            commands::delivery::batch_delivery,
            commands::delivery::cancel_batch_delivery,
            commands::license::activate_license,
            commands::license::verify_license,
            commands::license::get_license_status,
            commands::system::check_for_update,
            commands::system::set_cookie,
            commands::system::get_cookie_status,
            commands::system::get_store_registry,
            commands::system::select_store,
            commands::system::check_cookie_health,
            commands::system::get_cookie_health,
            commands::system::open_external_url,
            commands::system::pick_cookie_save_dir,
            commands::system::open_cookie_login,
            commands::system::close_cookie_login_window,
            commands::system::extract_cookie_from_login,
            commands::system::start_legacy_migration,
        ])
        .run(tauri::generate_context!())
        .unwrap_or_else(|_| std::process::exit(1));
}
