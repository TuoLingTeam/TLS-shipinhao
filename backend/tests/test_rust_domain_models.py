import re
import unittest
from pathlib import Path


ROOT = Path(__file__).resolve().parents[2]


class RustApiContractsTests(unittest.TestCase):
    def test_api_contracts_define_runtime_and_lease_models(self):
        lib_rs = ROOT / "backend" / "crates" / "api-contracts" / "src" / "lib.rs"
        self.assertTrue(lib_rs.exists(), "缺少 api-contracts/src/lib.rs")
        text = lib_rs.read_text(encoding="utf-8")
        for symbol in (
            "pub struct LicenseLease",
            "pub struct RuntimeGrant",
            "pub struct IntegrityManifest",
            "pub struct IntegrityManifestFile",
            "pub struct RiskReport",
            "pub enum LicenseState",
            "pub enum RiskLevel",
        ):
            self.assertIn(symbol, text)

    def test_api_contracts_use_stable_serde_shapes(self):
        lib_rs = ROOT / "backend" / "crates" / "api-contracts" / "src" / "lib.rs"
        text = lib_rs.read_text(encoding="utf-8")
        self.assertRegex(text, re.compile(r'#\[serde\(rename_all = "snake_case"\)\]'))
        self.assertIn("#[serde(default)]", text)


class RustDomainCoreTests(unittest.TestCase):
    def test_domain_core_defines_order_and_matching_models(self):
        lib_rs = ROOT / "backend" / "crates" / "domain-core" / "src" / "lib.rs"
        self.assertTrue(lib_rs.exists(), "缺少 domain-core/src/lib.rs")
        text = lib_rs.read_text(encoding="utf-8")
        for symbol in (
            "pub struct OrderCacheEntry",
            "pub struct OrderMatchResult",
            "pub struct DeliveryUpdateRequest",
            "pub struct DeliveryUpdateResult",
            "pub enum TaskKind",
            "pub enum MatchSource",
        ):
            self.assertIn(symbol, text)

    def test_domain_core_defines_shared_error_and_time_window_types(self):
        lib_rs = ROOT / "backend" / "crates" / "domain-core" / "src" / "lib.rs"
        text = lib_rs.read_text(encoding="utf-8")
        self.assertIn("pub struct TimeWindow", text)
        self.assertIn("pub enum DomainError", text)
        self.assertIn("#[derive(thiserror::Error", text)


if __name__ == "__main__":
    unittest.main()
