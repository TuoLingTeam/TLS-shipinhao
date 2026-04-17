pub mod day_window;
pub mod delivery_batch_runner;
pub mod delivery_update;
pub mod order_cache_repository;
pub mod order_cache_storage;
pub mod order_fetcher;
pub mod order_gap_planner;
pub mod order_fetcher_risk;
pub mod order_match_scoring;
pub mod order_sync_planner;
pub mod order_sync_service;
pub mod order_utils;
pub mod review_batch_match;
pub mod review_candidate_scoring;
pub mod review_index;
pub mod review_match_flow;
pub mod review_matcher_helpers;

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
