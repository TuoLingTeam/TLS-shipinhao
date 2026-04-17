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
use tauri::Emitter;

fn main() {
    tauri::Builder::default()
        .manage(AppState::new())
        .setup(|app| {
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
                        eprintln!("update check skipped: {err}");
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
            commands::order::sync_orders,
            commands::order::sync_recent_order_cache,
            commands::delivery::update_delivery,
            commands::delivery::batch_delivery,
            commands::license::activate_license,
            commands::license::verify_license,
            commands::license::get_license_status,
            commands::system::check_for_update,
            commands::system::get_app_info,
            commands::system::get_ui_scale,
            commands::system::set_ui_scale,
            commands::system::set_cookie,
            commands::system::get_cookie_status,
            commands::system::pick_cookie_save_dir,
            commands::system::open_cookie_login,
            commands::system::extract_cookie_from_login,
            commands::system::start_legacy_migration,
        ])
        .run(tauri::generate_context!())
        .expect("启动 Tauri 应用失败");
}
