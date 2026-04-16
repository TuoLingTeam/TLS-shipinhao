mod app_shell;

use std::cell::RefCell;
use std::rc::Rc;

use app_shell::AppShell;

slint::include_modules!();

fn main() -> Result<(), slint::PlatformError> {
    let app = AppWindow::new()?;
    let shell = Rc::new(RefCell::new(AppShell::new()));

    app.set_license_status(shell.borrow().license_status().into());
    app.set_last_action("等待操作".into());
    app.set_event_log("Rust desktop-app 已接入命令壳。".into());
    app.set_last_error("".into());

    {
        let app_handle = app.as_weak();
        let shell = shell.clone();
        app.on_start_review_find(move || {
            let result = shell.borrow().start_review_find();
            if let Some(app) = app_handle.upgrade() {
                app.set_last_action(result.title.into());
                app.set_event_log(result.log.into());
                app.set_last_error(result.error.unwrap_or_default().into());
            }
        });
    }

    {
        let app_handle = app.as_weak();
        let shell = shell.clone();
        app.on_start_batch_delivery(move || {
            let result = shell.borrow().start_batch_delivery();
            if let Some(app) = app_handle.upgrade() {
                app.set_last_action(result.title.into());
                app.set_event_log(result.log.into());
                app.set_last_error(result.error.unwrap_or_default().into());
            }
        });
    }

    app.run()
}
