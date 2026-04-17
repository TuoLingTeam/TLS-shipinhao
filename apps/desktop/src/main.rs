#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod adapters;
mod app_settings;
mod commands;
mod error;
mod migration;
mod state;

use state::AppState;

fn main() {
    tauri::Builder::default()
        .manage(AppState::new())
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
            commands::system::get_app_info,
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
