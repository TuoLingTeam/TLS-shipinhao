use desktop_services::{
    DeliveryGateway, DesktopServices, OrderCacheStore, ReviewQuery, ReviewSource,
};
use domain_core::{
    DeliveryUpdateRequest, DeliveryUpdateResult, MatchSource, OrderCacheEntry, OrderMatchResult, TimeWindow,
};

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct ShellCommandResult {
    pub title: String,
    pub log: String,
    pub error: Option<String>,
}

pub struct AppShell {
    services: DesktopServices<StubReviewSource, StubOrderCacheStore, StubDeliveryGateway>,
}

impl AppShell {
    pub fn new() -> Self {
        Self {
            services: DesktopServices::new(
                StubReviewSource::default(),
                StubOrderCacheStore::default(),
                StubDeliveryGateway::default(),
            ),
        }
    }

    pub fn license_status(&self) -> &'static str {
        "授权状态：本地 Rust 壳已接线"
    }

    pub fn start_review_find(&self) -> ShellCommandResult {
        let query = ReviewQuery {
            days: 30,
            time_window: TimeWindow {
                start_at: "2026-03-17T00:00:00Z".into(),
                end_at: "2026-04-16T23:59:59Z".into(),
            },
            runtime_grant: None,
        };
        match self.services.find_reviews(&query) {
            Ok(results) => ShellCommandResult {
                title: "中差评查找".into(),
                log: format!(
                    "已调用 Rust desktop-services.find_reviews\n时间窗口：{} ~ {}\n匹配结果数：{}\n示例订单：{}",
                    query.time_window.start_at,
                    query.time_window.end_at,
                    results.len(),
                    results.first().map(|item| item.order_id.clone()).unwrap_or_else(|| "无".into())
                ),
                error: None,
            },
            Err(error) => ShellCommandResult {
                title: "中差评查找".into(),
                log: String::new(),
                error: Some(error.to_string()),
            },
        }
    }

    pub fn start_batch_delivery(&self) -> ShellCommandResult {
        let request = DeliveryUpdateRequest {
            order_id: "3735560095122745088".into(),
            tracking_number: "JT00000001".into(),
            carrier_code: "JT".into(),
        };
        match self.services.update_delivery(&request) {
            Ok(result) => ShellCommandResult {
                title: "批量发货".into(),
                log: format!(
                    "已调用 Rust desktop-services.update_delivery\n订单号：{}\n更新结果：{}\n原单号：{}",
                    result.order_id,
                    if result.success { "success" } else { "failed" },
                    result.previous_waybill.unwrap_or_else(|| "无原物流单号".into())
                ),
                error: result.error_message,
            },
            Err(error) => ShellCommandResult {
                title: "批量发货".into(),
                log: String::new(),
                error: Some(error.to_string()),
            },
        }
    }
}

#[derive(Default)]
struct StubReviewSource;

impl ReviewSource for StubReviewSource {
    fn fetch_reviews(&self, _query: &ReviewQuery) -> anyhow::Result<Vec<OrderMatchResult>> {
        Ok(vec![OrderMatchResult {
            evaluation_id: "eval-1".into(),
            order_id: "3735563912835389952".into(),
            matched: true,
            source: MatchSource::ReceiverAndTimeWindow,
            confidence_score: 100,
        }])
    }
}

#[derive(Default)]
struct StubOrderCacheStore;

impl OrderCacheStore for StubOrderCacheStore {
    fn load_recent_orders(&self, window: &TimeWindow) -> anyhow::Result<Vec<OrderCacheEntry>> {
        Ok(vec![OrderCacheEntry {
            order_id: "cache-1".into(),
            buyer_name: "buyer".into(),
            receiver_name: "buyer".into(),
            amount_cent: 1999,
            created_at: window.start_at.clone(),
            updated_at: window.end_at.clone(),
        }])
    }

    fn save_orders(&self, _orders: &[OrderCacheEntry]) -> anyhow::Result<()> {
        Ok(())
    }
}

#[derive(Default)]
struct StubDeliveryGateway;

impl DeliveryGateway for StubDeliveryGateway {
    fn update_delivery(&self, request: &DeliveryUpdateRequest) -> anyhow::Result<DeliveryUpdateResult> {
        Ok(DeliveryUpdateResult {
            order_id: request.order_id.clone(),
            success: true,
            previous_waybill: Some("73666162791371".into()),
            error_message: None,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn shell_returns_review_find_log() {
        let shell = AppShell::new();
        let result = shell.start_review_find();
        assert!(result.error.is_none());
        assert!(result.log.contains("desktop-services.find_reviews"));
        assert!(result.log.contains("3735563912835389952"));
    }

    #[test]
    fn shell_returns_batch_delivery_log() {
        let shell = AppShell::new();
        let result = shell.start_batch_delivery();
        assert!(result.error.is_none());
        assert!(result.log.contains("desktop-services.update_delivery"));
        assert!(result.log.contains("73666162791371"));
    }
}
