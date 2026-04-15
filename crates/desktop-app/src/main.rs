slint::include_modules!();

fn main() -> Result<(), slint::PlatformError> {
    let app = AppWindow::new()?;
    app.on_start_review_find(|| {
        let _ = "review_find";
    });
    app.on_start_batch_delivery(|| {
        let _ = "batch_delivery";
    });
    app.run()
}
