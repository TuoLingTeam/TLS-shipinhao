import unittest
from pathlib import Path


ROOT = Path(__file__).resolve().parents[1]


class RustLicenseServiceTests(unittest.TestCase):
    def test_license_service_exposes_activation_and_verify_flow(self):
        lib_rs = ROOT / "crates" / "license-service" / "src" / "lib.rs"
        self.assertTrue(lib_rs.exists(), "缺少 crates/license-service/src/lib.rs")
        text = lib_rs.read_text(encoding="utf-8")
        for symbol in (
            "pub trait LicenseRepository",
            "pub struct LicenseService",
            "pub struct LicenseRecord",
            "pub struct ActivationInput",
            "pub struct VerifyInput",
            "pub fn issue_license_lease",
            "pub fn activate",
            "pub fn verify",
            "pub struct AuditEvent",
        ):
            self.assertIn(symbol, text)

    def test_license_worker_adapter_exists_and_depends_on_rust_service(self):
        cargo_toml = ROOT / "apps" / "license-worker" / "Cargo.toml"
        lib_rs = ROOT / "apps" / "license-worker" / "src" / "lib.rs"
        self.assertTrue(cargo_toml.exists(), "缺少 apps/license-worker/Cargo.toml")
        self.assertTrue(lib_rs.exists(), "缺少 apps/license-worker/src/lib.rs")
        cargo_text = cargo_toml.read_text(encoding="utf-8")
        self.assertIn('license-service = { path = "../../crates/license-service" }', cargo_text)
        lib_text = lib_rs.read_text(encoding="utf-8")
        for symbol in ("pub fn handle_activate", "pub fn handle_verify", "pub fn route_request"):
            self.assertIn(symbol, lib_text)

    def test_desktop_app_manifest_exists_for_future_slint_migration(self):
        cargo_toml = ROOT / "apps" / "desktop" / "Cargo.toml"
        self.assertTrue(cargo_toml.exists(), "缺少 apps/desktop/Cargo.toml")


if __name__ == "__main__":
    unittest.main()
