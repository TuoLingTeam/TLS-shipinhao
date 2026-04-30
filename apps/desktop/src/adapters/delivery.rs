use crate::adapters::common::{build_client, build_weixin_shop_headers};
use async_trait::async_trait;
use desktop::domain::{DeliveryUpdateRequest, DeliveryUpdateResult};
use desktop::services::delivery_batch_runner::BatchDeliveryGateway;
use desktop::services::delivery_update::{
    build_raw_update_delivery_payload, determine_delivery_override_from_raw_info,
    is_delivery_mismatch_error,
};
use desktop::services::DeliveryGateway;
use serde_json::Value;

/// 发货链路相关 URL：obfstr 编译期加密
fn order_detail_url() -> String {
    obfstr::obfstr!(
        "https://store.weixin.qq.com/shop-faas/mmchannelstradeorder/detail/cgi/orderDetail"
    )
    .to_string()
}
fn init_ship_data_url() -> String {
    obfstr::obfstr!(
        "https://store.weixin.qq.com/shop-faas/mmchannelstradeorder/ship/cgi/initShipData"
    )
    .to_string()
}
fn delivery_update_url() -> String {
    obfstr::obfstr!(
        "https://store.weixin.qq.com/shop-faas/mmchannelstradeorder/ship/cgi/updateDeliveryInfo"
    )
    .to_string()
}
fn order_list_referer() -> String {
    obfstr::obfstr!("https://store.weixin.qq.com/shop/order/list").to_string()
}

pub struct HttpDeliveryGateway {
    cookie_header: String,
    biz_magic: String,
    grant_id: Option<String>,
    client: reqwest::Client,
}

impl HttpDeliveryGateway {
    pub fn new_with_grant(
        cookie_header: String,
        biz_magic: String,
        grant_id: Option<String>,
    ) -> Self {
        Self {
            cookie_header,
            biz_magic,
            grant_id,
            client: build_client(),
        }
    }

    fn build_headers(&self) -> reqwest::header::HeaderMap {
        build_weixin_shop_headers(
            &order_list_referer(),
            &self.cookie_header,
            &self.biz_magic,
            self.grant_id.as_deref(),
        )
    }

    async fn post_json(&self, url: &str, body: &Value) -> anyhow::Result<Value> {
        let url = format!("{}?token=&lang=zh_CN", url);
        let resp = self
            .client
            .post(&url)
            .headers(self.build_headers())
            .json(body)
            .send()
            .await?
            .json::<Value>()
            .await?;
        Ok(resp)
    }

    async fn fetch_init_ship_data(&self, order_id: &str) -> anyhow::Result<Value> {
        let body = serde_json::json!({"id": order_id});
        let resp = self.post_json(&init_ship_data_url(), &body).await?;
        ensure_payload_success(&resp, "发货初始化接口返回失败")?;
        Ok(resp)
    }

    async fn fetch_order_detail(&self, order_id: &str) -> anyhow::Result<Value> {
        let body = serde_json::json!({"id": order_id});
        let resp = self.post_json(&order_detail_url(), &body).await?;
        ensure_payload_success(&resp, "订单详情接口返回失败")?;
        Ok(resp)
    }

    async fn extract_delivery_info(&self, order_id: &str) -> anyhow::Result<(Value, String)> {
        let init_result = match self.fetch_init_ship_data(order_id).await {
            Ok(payload) => extract_raw_delivery_product_info_from_init_ship_data(&payload),
            Err(err) => Err(err),
        };
        match init_result {
            Ok(info) => {
                let old_waybill = old_waybill_from_raw_delivery_info(&info);
                Ok((info, old_waybill))
            }
            Err(init_err) if is_missing_snapshot_error(&init_err) => {
                let detail_result = match self.fetch_order_detail(order_id).await {
                    Ok(payload) => extract_raw_delivery_product_info_from_order_detail(&payload),
                    Err(err) => Err(err),
                };
                match detail_result {
                    Ok(info) => {
                        let old_waybill = old_waybill_from_raw_delivery_info(&info);
                        Ok((info, old_waybill))
                    }
                    Err(detail_err) if is_missing_snapshot_error(&detail_err) => {
                        anyhow::bail!("订单详情中没有可更新的物流信息")
                    }
                    Err(detail_err) => Err(detail_err),
                }
            }
            Err(init_err) => Err(init_err),
        }
    }

    async fn do_update(
        &self,
        order_id: &str,
        tracking_number: &str,
        old_info: &Value,
        delivery_override: Option<(&str, &str)>,
    ) -> anyhow::Result<()> {
        let delivery_override = delivery_override.map(|(delivery_id, delivery_name)| {
            desktop::services::delivery_update::DeliveryOverride {
                delivery_id: delivery_id.to_string(),
                delivery_name: delivery_name.to_string(),
            }
        });
        let body = build_raw_update_delivery_payload(
            order_id,
            tracking_number,
            old_info,
            delivery_override.as_ref(),
        )?;
        let resp = self.post_json(&delivery_update_url(), &body).await?;
        check_update_response(&resp)
    }

    /// 真正执行单条物流更新的 async 入口。命令层 `update_delivery` 直接 `await` 它；
    /// `DeliveryGateway` / `BatchDeliveryGateway` 同步 trait 实现走 `Handle::block_on`
    /// 复用同一份业务逻辑，避免重复维护。
    pub async fn update_delivery_async(
        &self,
        request: &DeliveryUpdateRequest,
    ) -> anyhow::Result<DeliveryUpdateResult> {
        let (old_info, old_waybill) = self.extract_delivery_info(&request.order_id).await?;

        match self
            .do_update(&request.order_id, &request.tracking_number, &old_info, None)
            .await
        {
            Ok(()) => Ok(DeliveryUpdateResult {
                order_id: request.order_id.clone(),
                success: true,
                previous_waybill: empty_to_none(old_waybill),
                error_message: None,
            }),
            Err(e) if is_delivery_mismatch_error(&e.to_string()) => {
                if let Some(override_info) =
                    determine_delivery_override_from_raw_info(&request.tracking_number, &old_info)
                {
                    self.do_update(
                        &request.order_id,
                        &request.tracking_number,
                        &old_info,
                        Some((&override_info.delivery_id, &override_info.delivery_name)),
                    )
                    .await?;
                    return Ok(DeliveryUpdateResult {
                        order_id: request.order_id.clone(),
                        success: true,
                        previous_waybill: empty_to_none(old_waybill),
                        error_message: None,
                    });
                }
                Ok(DeliveryUpdateResult {
                    order_id: request.order_id.clone(),
                    success: false,
                    previous_waybill: None,
                    error_message: Some("快递单号与物流商不匹配，且无法自动映射".to_string()),
                })
            }
            Err(e) => Ok(DeliveryUpdateResult {
                order_id: request.order_id.clone(),
                success: false,
                previous_waybill: None,
                error_message: Some(e.to_string()),
            }),
        }
    }
}

fn empty_to_none(value: String) -> Option<String> {
    if value.is_empty() {
        None
    } else {
        Some(value)
    }
}

fn ensure_payload_success(payload: &Value, default_msg: &str) -> anyhow::Result<()> {
    if payload.get("success") == Some(&Value::Bool(false)) {
        let msg = extract_error_message(payload).unwrap_or_else(|| default_msg.to_string());
        anyhow::bail!("{}", msg);
    }
    if let Some(code) = payload.get("code").and_then(Value::as_i64) {
        if code != 0 {
            let msg = extract_error_message(payload)
                .unwrap_or_else(|| format!("{}（错误码 {}）", default_msg, code));
            anyhow::bail!("{}", msg);
        }
    }
    Ok(())
}

fn extract_error_message(payload: &Value) -> Option<String> {
    for key in &["errmsg", "message", "msg"] {
        if let Some(v) = payload.get(key).and_then(Value::as_str) {
            if !v.is_empty() {
                return Some(v.to_string());
            }
        }
    }
    None
}

fn check_update_response(resp: &Value) -> anyhow::Result<()> {
    if resp.get("success") == Some(&Value::Bool(true)) {
        return Ok(());
    }
    if resp.get("code") == Some(&Value::Number(0.into()))
        && resp.get("errcode") == Some(&Value::Number(0.into()))
    {
        return Ok(());
    }
    let msg = extract_error_message(resp).unwrap_or_else(|| format!("物流更新失败：{}", resp));
    anyhow::bail!("更新物流信息失败：{}", msg);
}

fn extract_raw_delivery_product_info_from_init_ship_data(payload: &Value) -> anyhow::Result<Value> {
    payload
        .pointer("/orderDetail/expressInfo/deliveryProductInfo")
        .and_then(Value::as_array)
        .and_then(|arr| arr.first())
        .cloned()
        .ok_or_else(|| anyhow::anyhow!("initShipData 中没有可更新的物流信息"))
}

fn extract_raw_delivery_product_info_from_order_detail(payload: &Value) -> anyhow::Result<Value> {
    payload
        .pointer("/expressInfo/deliveryProductInfo")
        .and_then(Value::as_array)
        .and_then(|arr| arr.first())
        .cloned()
        .ok_or_else(|| anyhow::anyhow!("orderDetail 中没有可更新的物流信息"))
}

fn is_missing_snapshot_error(err: &anyhow::Error) -> bool {
    let message = err.to_string();
    message.contains("没有可更新的物流信息")
}

fn old_waybill_from_raw_delivery_info(info: &Value) -> String {
    info.get("waybillId")
        .and_then(Value::as_str)
        .unwrap_or("")
        .to_string()
}

#[async_trait]
impl DeliveryGateway for HttpDeliveryGateway {
    async fn update_delivery(
        &self,
        request: &DeliveryUpdateRequest,
    ) -> anyhow::Result<DeliveryUpdateResult> {
        // L4-2 第三期：trait 已 async 化，直接复用 inherent async 实现，
        // 不再需要 `Handle::block_on` 桥接。
        self.update_delivery_async(request).await
    }
}

impl BatchDeliveryGateway for HttpDeliveryGateway {
    fn update_single_order(
        &mut self,
        order_id: &str,
        tracking_number: &str,
    ) -> anyhow::Result<Option<String>> {
        let request = DeliveryUpdateRequest {
            order_id: order_id.to_string(),
            tracking_number: tracking_number.to_string(),
            carrier_code: String::new(),
        };
        let result =
            tokio::runtime::Handle::current().block_on(self.update_delivery_async(&request))?;
        if result.success {
            Ok(result.previous_waybill)
        } else {
            anyhow::bail!(
                "{}",
                result
                    .error_message
                    .unwrap_or_else(|| "更新失败".to_string())
            )
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn init_ship_data_snapshot_is_preferred() {
        let payload = json!({
            "orderDetail": {"expressInfo": {"deliveryProductInfo": [{"waybillId": "WB-1", "deliveryId": "ZTO"}]}}
        });
        let info = extract_raw_delivery_product_info_from_init_ship_data(&payload).expect("info");
        assert_eq!(info["waybillId"], "WB-1");
    }

    #[test]
    fn falls_back_to_order_detail_snapshot() {
        let payload = json!({
            "expressInfo": {"deliveryProductInfo": [{"waybillId": "WB-2", "deliveryId": "SF"}]}
        });
        let info = extract_raw_delivery_product_info_from_order_detail(&payload).expect("info");
        assert_eq!(info["waybillId"], "WB-2");
    }

    #[test]
    fn missing_snapshot_errors_are_detected_and_merged() {
        let init_err =
            extract_raw_delivery_product_info_from_init_ship_data(&json!({})).unwrap_err();
        let detail_err =
            extract_raw_delivery_product_info_from_order_detail(&json!({})).unwrap_err();
        assert!(is_missing_snapshot_error(&init_err));
        assert!(is_missing_snapshot_error(&detail_err));
        assert!(init_err.to_string().contains("initShipData"));
        assert!(detail_err.to_string().contains("orderDetail"));
    }

    #[test]
    fn old_waybill_is_extracted_from_raw_snapshot() {
        let info = json!({"waybillId": "73666162791371"});
        assert_eq!(old_waybill_from_raw_delivery_info(&info), "73666162791371");
    }
}
