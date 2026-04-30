pub mod common;
pub mod delivery;
pub mod order;
pub mod review;

pub use common::day_window;
pub use common::http_client;
pub use common::matching;
pub use common::update_service;
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

use crate::domain::{
    DeliveryUpdateRequest, DeliveryUpdateResult, OrderCacheEntry, OrderMatchResult, TimeWindow,
};
use api_contracts::Rg;
use async_trait::async_trait;
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
    pub runtime_grant: Option<Rg>,
}

/// 评价数据源。L4-2 第三期改为 async_trait：上游 `commands::find_reviews` 与
/// `run_review_match_flow` 已是 async fn，HTTP 拉取链路一并 await 化，删除
/// 历史上 `Handle::block_on` 桥接的 sync trait 薄壳。
#[async_trait]
pub trait ReviewSource: Send + Sync {
    async fn fetch_reviews(&self, query: &ReviewQuery) -> anyhow::Result<Vec<OrderMatchResult>>;
}

pub trait OrderCacheStore {
    fn load_recent_orders(&self, window: &TimeWindow) -> anyhow::Result<Vec<OrderCacheEntry>>;
    fn save_orders(&self, orders: &[OrderCacheEntry]) -> anyhow::Result<()>;
}

/// 物流更新单条入口。L4-2 第三期改为 async_trait：命令层 `update_delivery`
/// 与 batch 调度循环统一走 async，删除 `Handle::block_on` 桥接。
#[async_trait]
pub trait DeliveryGateway: Send + Sync {
    async fn update_delivery(
        &self,
        request: &DeliveryUpdateRequest,
    ) -> anyhow::Result<DeliveryUpdateResult>;
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
