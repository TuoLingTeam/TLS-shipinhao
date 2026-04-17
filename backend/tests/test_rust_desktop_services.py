import unittest
from pathlib import Path


ROOT = Path(__file__).resolve().parents[2]


class RustDesktopServicesTests(unittest.TestCase):
    def test_desktop_services_expose_review_cache_delivery_and_cookie_flows(self):
        lib_rs = ROOT / "backend" / "crates" / "desktop-services" / "src" / "lib.rs"
        self.assertTrue(lib_rs.exists(), "缺少 backend/crates/desktop-services/src/lib.rs")
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
            "pub fn run_batch_delivery_flow",
            "pub mod order_utils;",
        ):
            self.assertIn(symbol, text)

    def test_desktop_services_depend_on_domain_and_contract_crates(self):
        cargo_toml = ROOT / "backend" / "crates" / "desktop-services" / "Cargo.toml"
        self.assertTrue(cargo_toml.exists(), "缺少 backend/crates/desktop-services/Cargo.toml")
        text = cargo_toml.read_text(encoding="utf-8")
        self.assertIn('api-contracts = { path = "../api-contracts" }', text)
        self.assertIn('domain-core = { path = "../domain-core" }', text)


if __name__ == "__main__":
    unittest.main()


class RustDesktopOrderUtilsTests(unittest.TestCase):
    def test_order_utils_module_exists(self):
        module_rs = ROOT / "backend" / "crates" / "desktop-services" / "src" / "order_utils.rs"
        self.assertTrue(module_rs.exists(), "缺少 backend/crates/desktop-services/src/order_utils.rs")
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
        module_rs = ROOT / "backend" / "crates" / "desktop-services" / "src" / "order_match_scoring.rs"
        self.assertTrue(module_rs.exists(), "缺少 backend/crates/desktop-services/src/order_match_scoring.rs")
        text = module_rs.read_text(encoding="utf-8")
        for symbol in (
            "pub struct ProductSimilarityResult",
            "pub struct MatchScoreResult",
            "similarity_percent",
            "pub fn title_similarity_percent",
            "pub fn compute_product_similarity",
            "pub fn compute_match_score",
        ):
            self.assertIn(symbol, text)


class RustDesktopReviewMatcherHelperTests(unittest.TestCase):
    def test_review_matcher_helper_module_exists(self):
        module_rs = ROOT / "backend" / "crates" / "desktop-services" / "src" / "review_matcher_helpers.rs"
        self.assertTrue(module_rs.exists(), "缺少 backend/crates/desktop-services/src/review_matcher_helpers.rs")
        text = module_rs.read_text(encoding="utf-8")
        for symbol in (
            "pub struct CandidateMatch",
            "pub fn build_product_reason",
            "pub fn build_nickname_reason",
            "pub fn pick_best_match",
        ):
            self.assertIn(symbol, text)


class RustDesktopReviewCandidateScoringTests(unittest.TestCase):
    def test_review_candidate_scoring_module_exists(self):
        module_rs = ROOT / "backend" / "crates" / "desktop-services" / "src" / "review_candidate_scoring.rs"
        self.assertTrue(module_rs.exists(), "缺少 backend/crates/desktop-services/src/review_candidate_scoring.rs")
        text = module_rs.read_text(encoding="utf-8")
        for symbol in (
            "pub const MATCH_MIN_SCORE",
            "pub struct EvaluationMatchContext",
            "pub struct CandidateOrder",
            "pub struct ScoredCandidateOrder",
            "pub fn resolve_reference_time",
            "pub fn score_candidate_order",
        ):
            self.assertIn(symbol, text)


class RustDesktopReviewMatchFlowTests(unittest.TestCase):
    def test_review_match_flow_module_exists(self):
        module_rs = ROOT / "backend" / "crates" / "desktop-services" / "src" / "review_match_flow.rs"
        self.assertTrue(module_rs.exists(), "缺少 backend/crates/desktop-services/src/review_match_flow.rs")
        text = module_rs.read_text(encoding="utf-8")
        for symbol in (
            "pub const AUTO_FILL_SCORE_THRESHOLD",
            "pub enum MatchStrategy",
            "pub struct SingleEvaluationMatch",
            "pub fn match_strategy_by_score",
            "pub fn match_single_evaluation",
        ):
            self.assertIn(symbol, text)


class RustDesktopReviewIndexTests(unittest.TestCase):
    def test_review_index_module_exists(self):
        module_rs = ROOT / "backend" / "crates" / "desktop-services" / "src" / "review_index.rs"
        self.assertTrue(module_rs.exists(), "缺少 backend/crates/desktop-services/src/review_index.rs")
        text = module_rs.read_text(encoding="utf-8")
        for symbol in (
            "pub type ProductIndex",
            "pub fn build_product_id_key",
            "pub fn build_product_value_key",
            "pub fn build_candidate_index_keys",
            "pub fn build_product_sku_index",
            "pub fn collect_candidate_orders",
        ):
            self.assertIn(symbol, text)


class RustDesktopReviewBatchMatchTests(unittest.TestCase):
    def test_review_batch_match_module_exists(self):
        module_rs = ROOT / "backend" / "crates" / "desktop-services" / "src" / "review_batch_match.rs"
        self.assertTrue(module_rs.exists(), "缺少 backend/crates/desktop-services/src/review_batch_match.rs")
        text = module_rs.read_text(encoding="utf-8")
        for symbol in (
            "pub struct EvaluationRecord",
            "pub struct MatchedEvaluationResult",
            "pub fn match_orders_with_evaluations",
            "pub fn build_match_result",
        ):
            self.assertIn(symbol, text)


class RustDesktopOrderCacheStorageTests(unittest.TestCase):
    def test_order_cache_storage_module_exists(self):
        module_rs = ROOT / "backend" / "crates" / "desktop-services" / "src" / "order_cache_storage.rs"
        self.assertTrue(module_rs.exists(), "缺少 backend/crates/desktop-services/src/order_cache_storage.rs")
        text = module_rs.read_text(encoding="utf-8")
        for symbol in (
            "CacheOrderRecord",
            "CacheOrderProduct",
            "SyncStateRecord",
            "OrderCacheRepository",
            "pub fn now_epoch_seconds",
        ):
            self.assertIn(symbol, text)


class RustDesktopOrderSyncPlannerTests(unittest.TestCase):
    def test_order_sync_planner_module_exists(self):
        module_rs = ROOT / "backend" / "crates" / "desktop-services" / "src" / "order_sync_planner.rs"
        self.assertTrue(module_rs.exists(), "缺少 backend/crates/desktop-services/src/order_sync_planner.rs")
        text = module_rs.read_text(encoding="utf-8")
        for symbol in (
            "pub const ORDER_CACHE_COVERAGE_DAYS",
            "pub const ORDER_CACHE_INCREMENTAL_DAYS",
            "pub const ORDER_CACHE_INCREMENTAL_OVERLAP_DAYS",
            "pub struct SyncPlannerState",
            "pub fn retention_start",
            "pub fn sync_now",
            "pub fn incremental_refresh_start",
        ):
            self.assertIn(symbol, text)


class RustDesktopDayWindowTests(unittest.TestCase):
    def test_day_window_module_exists(self):
        module_rs = ROOT / "backend" / "crates" / "desktop-services" / "src" / "day_window.rs"
        self.assertTrue(module_rs.exists(), "缺少 backend/crates/desktop-services/src/day_window.rs")
        text = module_rs.read_text(encoding="utf-8")
        for symbol in (
            "pub fn start_of_day_timestamp",
            "pub fn end_of_day_timestamp",
            "pub fn recent_day_range_timestamps",
        ):
            self.assertIn(symbol, text)


class RustDesktopOrderSyncServiceTests(unittest.TestCase):
    def test_order_sync_service_module_exists(self):
        module_rs = ROOT / "backend" / "crates" / "desktop-services" / "src" / "order_sync_service.rs"
        self.assertTrue(module_rs.exists(), "缺少 backend/crates/desktop-services/src/order_sync_service.rs")
        text = module_rs.read_text(encoding="utf-8")
        for symbol in (
            "pub const ORDER_CACHE_SCOPE",
            "pub struct SyncWindowOrders",
            "pub struct CacheFetchResult",
            "pub trait CacheOrderFinder",
            "pub struct OrderSyncService",
            "pub fn deduplicate_orders_by_id",
        ):
            self.assertIn(symbol, text)


class RustDesktopDeliveryUpdateTests(unittest.TestCase):
    def test_delivery_update_module_exists(self):
        module_rs = ROOT / "backend" / "crates" / "desktop-services" / "src" / "delivery_update.rs"
        self.assertTrue(module_rs.exists(), "缺少 backend/crates/desktop-services/src/delivery_update.rs")
        text = module_rs.read_text(encoding="utf-8")
        for symbol in (
            "pub const DELIVERY_MISMATCH_MESSAGE",
            "pub struct DeliveryProductInfo",
            "pub struct DeliverySnapshot",
            "pub struct DeliveryOverride",
            "pub fn build_delivery_candidates",
            "pub fn build_update_delivery_payload",
            "pub fn is_delivery_mismatch_error",
            "pub fn determine_delivery_override_on_mismatch",
            "pub fn delivery_update_succeeded",
        ):
            self.assertIn(symbol, text)


class RustDesktopDeliveryBatchRunnerTests(unittest.TestCase):
    def test_delivery_batch_runner_module_exists(self):
        module_rs = ROOT / "backend" / "crates" / "desktop-services" / "src" / "delivery_batch_runner.rs"
        self.assertTrue(module_rs.exists(), "缺少 backend/crates/desktop-services/src/delivery_batch_runner.rs")
        text = module_rs.read_text(encoding="utf-8")
        for symbol in (
            "pub const BATCH_DELIVERY_TASK_TYPE",
            "pub struct BatchDeliveryItem",
            "pub enum BatchDeliveryStepStatus",
            "pub struct BatchDeliveryStepResult",
            "pub struct BatchDeliveryReport",
            "pub trait BatchDeliveryGateway",
            "pub trait BatchDeliveryRuntimeGuard",
            "pub fn run_batch_delivery",
        ):
            self.assertIn(symbol, text)
