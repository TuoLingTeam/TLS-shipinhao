use std::sync::Arc;
use tokio::sync::Mutex;

use desktop_services::{
    CookieProfile, DeliveryGateway, DesktopServices, OrderCacheStore, ReviewSource,
};

pub struct StubReviewSource;
pub struct StubOrderCache;
pub struct StubDeliveryGateway;

impl ReviewSource for StubReviewSource {
    fn fetch_reviews(
        &self,
        _query: &desktop_services::ReviewQuery,
    ) -> anyhow::Result<Vec<domain_core::OrderMatchResult>> {
        Ok(vec![domain_core::OrderMatchResult {
            evaluation_id: "eval-stub".into(),
            order_id: "stub-order-001".into(),
            matched: true,
            source: domain_core::MatchSource::ReceiverAndTimeWindow,
            confidence_score: 95,
        }])
    }
}

impl OrderCacheStore for StubOrderCache {
    fn load_recent_orders(
        &self,
        _window: &domain_core::TimeWindow,
    ) -> anyhow::Result<Vec<domain_core::OrderCacheEntry>> {
        Ok(vec![])
    }

    fn save_orders(&self, _orders: &[domain_core::OrderCacheEntry]) -> anyhow::Result<()> {
        Ok(())
    }
}

impl DeliveryGateway for StubDeliveryGateway {
    fn update_delivery(
        &self,
        request: &domain_core::DeliveryUpdateRequest,
    ) -> anyhow::Result<domain_core::DeliveryUpdateResult> {
        Ok(domain_core::DeliveryUpdateResult {
            order_id: request.order_id.clone(),
            success: true,
            previous_waybill: None,
            error_message: None,
        })
    }
}

pub type Services = DesktopServices<StubReviewSource, StubOrderCache, StubDeliveryGateway>;

pub struct AppState {
    pub services: Arc<Services>,
    pub cookie_profile: Mutex<CookieProfile>,
}

impl AppState {
    pub fn new() -> Self {
        Self {
            services: Arc::new(DesktopServices::new(
                StubReviewSource,
                StubOrderCache,
                StubDeliveryGateway,
            )),
            cookie_profile: Mutex::new(CookieProfile::default()),
        }
    }
}
