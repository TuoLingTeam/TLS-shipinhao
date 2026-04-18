pub mod shared;
pub mod review;
pub mod order;
pub mod delivery;

pub use delivery::batch_runner as delivery_batch_runner;
pub use delivery::update as delivery_update;
pub use order::cache_repository as order_cache_repository;
pub use order::cache_storage as order_cache_storage;
pub use order::fetcher as order_fetcher;
pub use order::fetcher_risk as order_fetcher_risk;
pub use order::gap_planner as order_gap_planner;
pub use order::match_scoring as order_match_scoring;
pub use order::sync_planner as order_sync_planner;
pub use order::sync_service as order_sync_service;
pub use order::utils as order_utils;
pub use review::batch_match as review_batch_match;
pub use review::candidate_scoring as review_candidate_scoring;
pub use review::index as review_index;
pub use review::match_flow as review_match_flow;
pub use review::matcher_helpers as review_matcher_helpers;
pub use shared::day_window;
pub use shared::http_client;
pub use shared::matching;
pub use shared::update_service;

use crate::delivery_batch_runner::{
    run_batch_delivery, BatchDeliveryGateway as DeliveryBatchGateway, BatchDeliveryItem,
    BatchDeliveryReport, BatchDeliveryRuntimeGuard,
};
use api_contracts::RuntimeGrant;
use domain_core::{
    DeliveryUpdateRequest, DeliveryUpdateResult, OrderCacheEntry, OrderMatchResult, TimeWindow,
};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Default)]
#[serde(rename_all = "snake_case")]
pub struct CookieProfile {
    pub cookie_header: String,
    pub biz_magic: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Default)]
#[serde(rename_all = "snake_case")]
pub struct ReviewQuery {
    pub days: u32,
    pub time_window: TimeWindow,
    pub runtime_grant: Option<RuntimeGrant>,
}

pub trait ReviewSource {
    fn fetch_reviews(&self, query: &ReviewQuery) -> anyhow::Result<Vec<OrderMatchResult>>;
}

pub trait OrderCacheStore {
    fn load_recent_orders(&self, window: &TimeWindow) -> anyhow::Result<Vec<OrderCacheEntry>>;
    fn save_orders(&self, orders: &[OrderCacheEntry]) -> anyhow::Result<()>;
}

pub trait DeliveryGateway {
    fn update_delivery(
        &self,
        request: &DeliveryUpdateRequest,
    ) -> anyhow::Result<DeliveryUpdateResult>;
}

pub struct DesktopServices<R, C, D> {
    review_source: R,
    cache_store: C,
    delivery_gateway: D,
}

impl<R, C, D> DesktopServices<R, C, D>
where
    R: ReviewSource,
    C: OrderCacheStore,
    D: DeliveryGateway,
{
    pub fn new(review_source: R, cache_store: C, delivery_gateway: D) -> Self {
        Self {
            review_source,
            cache_store,
            delivery_gateway,
        }
    }

    pub fn find_reviews(&self, query: &ReviewQuery) -> anyhow::Result<Vec<OrderMatchResult>> {
        self.review_source.fetch_reviews(query)
    }

    pub fn refresh_cache(
        &self,
        window: &TimeWindow,
        orders: &[OrderCacheEntry],
    ) -> anyhow::Result<Vec<OrderCacheEntry>> {
        self.cache_store.save_orders(orders)?;
        self.cache_store.load_recent_orders(window)
    }

    pub fn update_delivery(
        &self,
        request: &DeliveryUpdateRequest,
    ) -> anyhow::Result<DeliveryUpdateResult> {
        self.delivery_gateway.update_delivery(request)
    }
}

pub fn run_batch_delivery_flow<G, RG>(
    items: &[BatchDeliveryItem],
    gateway: &mut G,
    runtime_guard: &mut RG,
) -> anyhow::Result<BatchDeliveryReport>
where
    G: DeliveryBatchGateway,
    RG: BatchDeliveryRuntimeGuard,
{
    Ok(run_batch_delivery(items, gateway, runtime_guard))
}

pub fn parse_cookie_profile(cookie_header: &str) -> CookieProfile {
    let biz_magic = cookie_header
        .split(';')
        .map(str::trim)
        .find_map(|segment| segment.strip_prefix("biz_magic="))
        .map(str::to_string);
    CookieProfile {
        cookie_header: cookie_header.to_string(),
        biz_magic,
    }
}
