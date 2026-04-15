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
            "pub mod order_utils",
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


class RustDesktopOrderUtilsTests(unittest.TestCase):
    def test_order_utils_module_exists(self):
        module_rs = ROOT / "crates" / "desktop-services" / "src" / "order_utils.rs"
        self.assertTrue(module_rs.exists(), "缺少 crates/desktop-services/src/order_utils.rs")
        text = module_rs.read_text(encoding="utf-8")
        for symbol in (
            "pub fn first_non_empty",
            "pub fn normalize_sale_param",
            "pub fn parse_confirm_receipt_timestamp",
            "pub fn parse_timestamp",
            "pub fn normalize_product_text",
            "pub fn split_sku_tokens",
        ):
            self.assertIn(symbol, text)


class RustDesktopOrderMatchScoringTests(unittest.TestCase):
    def test_order_match_scoring_module_exists(self):
        module_rs = ROOT / "crates" / "desktop-services" / "src" / "order_match_scoring.rs"
        self.assertTrue(module_rs.exists(), "缺少 crates/desktop-services/src/order_match_scoring.rs")
        text = module_rs.read_text(encoding="utf-8")
        for symbol in (
            "pub struct ProductSimilarityResult",
            "pub struct MatchScoreResult",
            "pub fn similarity_percent",
            "pub fn title_similarity_percent",
            "pub fn compute_product_similarity",
            "pub fn compute_match_score",
        ):
            self.assertIn(symbol, text)
