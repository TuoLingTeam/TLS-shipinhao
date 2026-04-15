import unittest
from pathlib import Path


ROOT = Path(__file__).resolve().parents[1]


class RustDesktopServicesTests(unittest.TestCase):
    def test_desktop_services_expose_review_cache_delivery_and_cookie_flows(self):
        lib_rs = ROOT / "crates" / "desktop-services" / "src" / "lib.rs"
        self.assertTrue(lib_rs.exists(), "缺少 crates/desktop-services/src/lib.rs")
        text = lib_rs.read_text(encoding="utf-8")
        for symbol in (
            "pub struct CookieProfile",
            "pub struct ReviewQuery",
            "pub trait ReviewSource",
            "pub trait OrderCacheStore",
            "pub trait DeliveryGateway",
            "pub struct DesktopServices",
            "pub fn find_reviews",
            "pub fn refresh_cache",
            "pub fn update_delivery",
            "pub fn parse_cookie_profile",
        ):
            self.assertIn(symbol, text)

    def test_desktop_services_depend_on_domain_and_contract_crates(self):
        cargo_toml = ROOT / "crates" / "desktop-services" / "Cargo.toml"
        self.assertTrue(cargo_toml.exists(), "缺少 crates/desktop-services/Cargo.toml")
        text = cargo_toml.read_text(encoding="utf-8")
        self.assertIn('api-contracts = { path = "../api-contracts" }', text)
        self.assertIn('domain-core = { path = "../domain-core" }', text)


if __name__ == "__main__":
    unittest.main()
