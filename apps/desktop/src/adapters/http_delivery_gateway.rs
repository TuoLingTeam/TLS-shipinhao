use desktop_services::DeliveryGateway;
use desktop_services::delivery_batch_runner::BatchDeliveryGateway;
use domain_core::{DeliveryUpdateRequest, DeliveryUpdateResult};

pub struct HttpDeliveryGateway {
    pub base_url: String,
    pub cookie_header: String,
}

impl HttpDeliveryGateway {
    pub fn new(base_url: String, cookie_header: String) -> Self {
        Self { base_url, cookie_header }
    }
}

impl DeliveryGateway for HttpDeliveryGateway {
    fn update_delivery(
        &self,
        _request: &DeliveryUpdateRequest,
    ) -> anyhow::Result<DeliveryUpdateResult> {
        // TODO: 调用视频号发货 API
        anyhow::bail!("HTTP 发货网关尚未实现")
    }
}

impl BatchDeliveryGateway for HttpDeliveryGateway {
    fn update_single_order(
        &mut self,
        _order_id: &str,
        _tracking_number: &str,
    ) -> anyhow::Result<Option<String>> {
        // TODO: 调用视频号单笔发货 API
        anyhow::bail!("HTTP 批量发货网关尚未实现")
    }
}
