#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod adapters;
mod app_settings;
mod commands;
mod error;
mod state;

use state::AppState;

fn main() {
    tauri::Builder::default()
        .manage(AppState::new())
        .invoke_handler(tauri::generate_handler![
            commands::review::find_reviews,
            commands::order::load_order_cache,
            commands::order::sync_orders,
            commands::delivery::update_delivery,
            commands::delivery::batch_delivery,
            commands::license::activate_license,
            commands::license::verify_license,
            commands::system::get_app_info,
            commands::system::set_cookie,
            commands::system::get_cookie_status,
        ])
        .run(tauri::generate_context!())
        .expect("启动 Tauri 应用失败");
}
