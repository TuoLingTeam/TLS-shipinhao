import unittest
from pathlib import Path


ROOT = Path(__file__).resolve().parents[2]


class RustLicenseServiceTests(unittest.TestCase):
    def test_license_service_exposes_activation_and_verify_flow(self):
        lib_rs = ROOT / "backend" / "crates" / "license-service" / "src" / "lib.rs"
        self.assertTrue(lib_rs.exists(), "缺少 backend/crates/license-service/src/lib.rs")
        text = lib_rs.read_text(encoding="utf-8")
        for symbol in (
            "pub trait LicenseRepository",
            "pub struct LicenseService",
            "pub struct LicenseRecord",
            "pub struct GeneratedKeyRecord",
            "pub struct DeviceRegistration",
            "pub struct LicenseServiceResponse",
            "pub struct ActivationInput",
            "pub struct VerifyInput",
            "pub fn issue_license_lease",
            "pub fn activate",
            "pub fn verify",
            "pub struct AuditEvent",
            "pub const LEASE_RENEWAL_HOURS",
            "pub const LEASE_HARD_EXPIRY_HOURS",
        ):
            self.assertIn(symbol, text)

    def test_license_worker_adapter_exists_and_depends_on_rust_service(self):
        cargo_toml = ROOT / "backend" / "license-worker" / "Cargo.toml"
        lib_rs = ROOT / "backend" / "license-worker" / "src" / "lib.rs"
        wrangler = ROOT / "backend" / "wrangler.toml"
        self.assertTrue(cargo_toml.exists(), "缺少 backend/license-worker/Cargo.toml")
        self.assertTrue(lib_rs.exists(), "缺少 backend/license-worker/src/lib.rs")
        self.assertTrue(wrangler.exists(), "缺少 backend/wrangler.toml")
        cargo_text = cargo_toml.read_text(encoding="utf-8")
        self.assertIn('license-service = { path = "../../backend/crates/license-service" }', cargo_text)
        self.assertIn('crate-type = ["cdylib", "rlib"]', cargo_text)
        lib_text = lib_rs.read_text(encoding="utf-8")
        for symbol in (
            "pub fn route_request",
            "pub async fn handle_async_runtime_json",
            "pub enum WorkerRoute",
            "pub async fn runtime_activate",
            "pub async fn runtime_verify",
            "pub async fn runtime_refresh_lease",
            "pub async fn runtime_task_authorize",
        ):
            self.assertIn(symbol, lib_text)
        wrangler_text = wrangler.read_text(encoding="utf-8")
        for symbol in (
            'main = "./license-worker/build/worker/shim.mjs"',
            'worker-build --release',
            'binding = "DB"',
        ):
            self.assertIn(symbol, wrangler_text)

    def test_legacy_js_worker_is_only_compatibility_shell(self):
        worker_js = ROOT / "backend" / "legacy" / "js-worker" / "index.js"
        self.assertFalse(worker_js.exists(), "旧版 JS worker 应已被移除")

    def test_desktop_app_manifest_exists_for_future_slint_migration(self):
        cargo_toml = ROOT / "apps" / "desktop" / "Cargo.toml"
        self.assertTrue(cargo_toml.exists(), "缺少 apps/desktop/Cargo.toml")


if __name__ == "__main__":
    unittest.main()
