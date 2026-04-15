import unittest
from pathlib import Path


ROOT = Path(__file__).resolve().parents[1]


class RustDesktopAppTests(unittest.TestCase):
    def test_desktop_app_uses_slint_and_declares_main_window(self):
        cargo_toml = ROOT / "crates" / "desktop-app" / "Cargo.toml"
        main_rs = ROOT / "crates" / "desktop-app" / "src" / "main.rs"
        slint_file = ROOT / "crates" / "desktop-app" / "ui" / "app-window.slint"
        self.assertTrue(cargo_toml.exists())
        self.assertTrue(main_rs.exists())
        self.assertTrue(slint_file.exists())

        cargo_text = cargo_toml.read_text(encoding="utf-8")
        self.assertIn('slint =', cargo_text)
        self.assertIn('desktop-services = { path = "../desktop-services" }', cargo_text)

        slint_text = slint_file.read_text(encoding="utf-8")
        for symbol in ("export component AppWindow", "in-out property <string> license_status", "callback start_review_find", "callback start_batch_delivery"):
            self.assertIn(symbol, slint_text)

    def test_desktop_app_wires_ui_actions_to_rust_service_shell(self):
        main_rs = ROOT / "crates" / "desktop-app" / "src" / "main.rs"
        text = main_rs.read_text(encoding="utf-8")
        for symbol in ("fn main()", "AppWindow::new", "on_start_review_find", "on_start_batch_delivery"):
            self.assertIn(symbol, text)


if __name__ == "__main__":
    unittest.main()
