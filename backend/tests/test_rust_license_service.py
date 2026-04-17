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
        cargo_toml = ROOT / "apps" / "license-worker" / "Cargo.toml"
        lib_rs = ROOT / "apps" / "license-worker" / "src" / "lib.rs"
        wrangler = ROOT / "apps" / "license-worker" / "wrangler.toml"
        self.assertTrue(cargo_toml.exists(), "缺少 apps/license-worker/Cargo.toml")
        self.assertTrue(lib_rs.exists(), "缺少 apps/license-worker/src/lib.rs")
        self.assertTrue(wrangler.exists(), "缺少 apps/license-worker/wrangler.toml")
        cargo_text = cargo_toml.read_text(encoding="utf-8")
        self.assertIn('license-service = { path = "../../backend/crates/license-service" }', cargo_text)
        self.assertIn('crate-type = ["cdylib", "rlib"]', cargo_text)
        lib_text = lib_rs.read_text(encoding="utf-8")
        for symbol in (
            "pub fn handle_activate",
            "pub fn handle_verify",
            "pub fn route_request",
            "pub fn handle_json_request",
            "pub enum WorkerRoute",
            '#[event(fetch)]',
            'rust_worker_repository_pending',
        ):
            self.assertIn(symbol, lib_text)
        wrangler_text = wrangler.read_text(encoding="utf-8")
        for symbol in (
            'main = "build/worker/shim.mjs"',
            'worker-build --release',
            'binding = "DB"',
        ):
            self.assertIn(symbol, wrangler_text)

    def test_legacy_js_worker_is_only_compatibility_shell(self):
        worker_js = ROOT / "backend" / "src" / "worker" / "index.js"
        self.assertTrue(worker_js.exists(), "缺少 backend/src/worker/index.js")
        text = worker_js.read_text(encoding="utf-8")
        self.assertIn('legacy_js_worker_retired_use_apps_license_worker', text)
        for removed_symbol in (
            'handleActivate',
            'handleVerify',
            'verifyClaimsToken',
            'issueSessionToken',
            'buildLeasePayload',
        ):
            self.assertNotIn(removed_symbol, text)

    def test_desktop_app_manifest_exists_for_future_slint_migration(self):
        cargo_toml = ROOT / "apps" / "desktop" / "Cargo.toml"
        self.assertTrue(cargo_toml.exists(), "缺少 apps/desktop/Cargo.toml")


if __name__ == "__main__":
    unittest.main()
