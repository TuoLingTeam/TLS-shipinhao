use api_contracts::RuntimeGrant;
use desktop_services::delivery_batch_runner::{
    BatchDeliveryGateway, BatchDeliveryItem, BatchDeliveryRuntimeGuard,
};
use desktop_services::{
    run_batch_delivery_flow, DeliveryGateway, DesktopServices, OrderCacheStore, ReviewQuery, ReviewSource,
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
    runtime_guard: StubRuntimeGuard,
}

impl AppShell {
    pub fn new() -> Self {
        Self {
            services: DesktopServices::new(
                StubReviewSource::default(),
                StubOrderCacheStore::default(),
                StubDeliveryGateway::default(),
            ),
            runtime_guard: StubRuntimeGuard,
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
        let items = vec![
            BatchDeliveryItem {
                order_id: "3735560095122745088".into(),
                tracking_number: "JT00000001".into(),
            },
            BatchDeliveryItem {
                order_id: "3735560095122745089".into(),
                tracking_number: "JT00000002".into(),
            },
        ];
        let mut gateway = StubDeliveryGateway::default();
        let mut runtime_guard = self.runtime_guard;
        match run_batch_delivery_flow(&items, &mut gateway, &mut runtime_guard) {
            Ok(report) => ShellCommandResult {
                title: "批量发货".into(),
                log: format!(
                    "已调用 Rust delivery_batch_runner\n批量总数：{}\n成功：{}\n失败：{}\n首单状态：{:?}",
                    report.total_count,
                    report.success_count,
                    report.failure_count,
                    report.steps.first().map(|step| step.status.clone())
                ),
                error: report.fatal_error,
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
            buyer_nickname: "测试买家".into(),
            evaluation_content: "默认差评内容".into(),
            product_id: "product-1".into(),
            sku_id: "sku-1".into(),
            sku_name: "默认规格".into(),
            product_name: "测试商品".into(),
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

impl BatchDeliveryGateway for StubDeliveryGateway {
    fn update_single_order(&mut self, order_id: &str, tracking_number: &str) -> anyhow::Result<Option<String>> {
        let request = DeliveryUpdateRequest {
            order_id: order_id.to_string(),
            tracking_number: tracking_number.to_string(),
            carrier_code: "JT".into(),
        };
        let result = self.update_delivery(&request)?;
        Ok(result.previous_waybill)
    }
}

#[derive(Default, Clone, Copy)]
struct StubRuntimeGuard;

impl BatchDeliveryRuntimeGuard for StubRuntimeGuard {
    fn authorize(&mut self, task_type: &str) -> anyhow::Result<()> {
        let _grant = RuntimeGrant {
            task_type: task_type.to_string(),
            granted: true,
            grant_id: "grant-1".into(),
            valid_until: "2026-04-16T23:59:59Z".into(),
            risk_level: Some(api_contracts::RiskLevel::Low),
            degraded_reason: None,
        };
        Ok(())
    }

    fn validate_continuity(&mut self, _task_type: &str, _index: usize) -> anyhow::Result<()> {
        Ok(())
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
        assert!(result.log.contains("delivery_batch_runner"));
        assert!(result.log.contains("成功：2"));
    }
}
