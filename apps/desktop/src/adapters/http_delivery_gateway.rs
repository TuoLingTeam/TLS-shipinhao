use desktop_services::delivery_batch_runner::BatchDeliveryGateway;
use desktop_services::delivery_update::{
    build_raw_update_delivery_payload, determine_delivery_override_from_raw_info,
    is_delivery_mismatch_error,
};
use desktop_services::DeliveryGateway;
use domain_core::{DeliveryUpdateRequest, DeliveryUpdateResult};
use serde_json::Value;

const ORDER_DETAIL_URL: &str =
    "https://store.weixin.qq.com/shop-faas/mmchannelstradeorder/detail/cgi/orderDetail";
const INIT_SHIP_DATA_URL: &str =
    "https://store.weixin.qq.com/shop-faas/mmchannelstradeorder/ship/cgi/initShipData";
const DELIVERY_UPDATE_URL: &str =
    "https://store.weixin.qq.com/shop-faas/mmchannelstradeorder/ship/cgi/updateDeliveryInfo";
const ORDER_LIST_REFERER: &str = "https://store.weixin.qq.com/shop/order/list";
const REQUEST_TIMEOUT_SECS: u64 = 30;

pub struct HttpDeliveryGateway {
    cookie_header: String,
    biz_magic: String,
    grant_id: Option<String>,
    client: reqwest::Client,
}

impl HttpDeliveryGateway {
    /// 无任务授权 grant 时的便捷构造；当前业务统一走 `new_with_grant`，保留该入口供未来或脚本直接调用。
    #[allow(dead_code)]
    pub fn new(cookie_header: String, biz_magic: String) -> Self {
        Self::new_with_grant(cookie_header, biz_magic, None)
    }

    pub fn new_with_grant(
        cookie_header: String,
        biz_magic: String,
        grant_id: Option<String>,
    ) -> Self {
        let client = desktop_services::http_client::build_desktop_http_client(
            std::time::Duration::from_secs(REQUEST_TIMEOUT_SECS),
        );
        Self {
            cookie_header,
            biz_magic,
            grant_id,
            client,
        }
    }

    fn build_headers(&self) -> reqwest::header::HeaderMap {
        use reqwest::header::{
            HeaderMap, HeaderName, HeaderValue, CONTENT_TYPE, COOKIE, ORIGIN, REFERER, USER_AGENT,
        };
        let mut headers = HeaderMap::new();
        headers.insert(CONTENT_TYPE, HeaderValue::from_static("application/json"));
        headers.insert(
            ORIGIN,
            HeaderValue::from_static("https://store.weixin.qq.com"),
        );
        headers.insert(REFERER, HeaderValue::from_static(ORDER_LIST_REFERER));
        headers.insert(
            USER_AGENT,
            HeaderValue::from_static(security_core::http_headers::get_user_agent()),
        );
        if let Ok(v) = HeaderValue::from_str(&self.cookie_header) {
            headers.insert(COOKIE, v);
        }
        if let Ok(v) = HeaderValue::from_str(&self.biz_magic) {
            headers.insert(HeaderName::from_static("biz_magic"), v);
        }
        if let Some(grant_id) = self.grant_id.as_deref() {
            if let Ok(v) = HeaderValue::from_str(grant_id) {
                headers.insert(HeaderName::from_static("x-grant-id"), v);
            }
        }
        headers.insert(
            HeaderName::from_static("potter-scene"),
            HeaderValue::from_static("weixinShop"),
        );
        headers.insert(
            HeaderName::from_static("sec-ch-ua-platform"),
            HeaderValue::from_static(security_core::http_headers::get_sec_ch_ua_platform()),
        );
        headers
    }

    fn post_json_sync(&self, url: &str, body: &Value) -> anyhow::Result<Value> {
        let rt = tokio::runtime::Handle::current();
        let headers = self.build_headers();
        let client = self.client.clone();
        let url = format!("{}?token=&lang=zh_CN", url);
        let body = body.clone();

        let resp = std::thread::spawn(move || {
            rt.block_on(async {
                client
                    .post(&url)
                    .headers(headers)
                    .json(&body)
                    .send()
                    .await?
                    .json::<Value>()
                    .await
            })
        })
        .join()
        .map_err(|_| anyhow::anyhow!("请求线程崩溃"))??;

        Ok(resp)
    }

    fn fetch_init_ship_data(&self, order_id: &str) -> anyhow::Result<Value> {
        let body = serde_json::json!({"id": order_id});
        let resp = self.post_json_sync(INIT_SHIP_DATA_URL, &body)?;
        ensure_payload_success(&resp, "发货初始化接口返回失败")?;
        Ok(resp)
    }

    fn fetch_order_detail(&self, order_id: &str) -> anyhow::Result<Value> {
        let body = serde_json::json!({"id": order_id});
        let resp = self.post_json_sync(ORDER_DETAIL_URL, &body)?;
        ensure_payload_success(&resp, "订单详情接口返回失败")?;
        Ok(resp)
    }

    fn extract_delivery_info(&self, order_id: &str) -> anyhow::Result<(Value, String)> {
        let init_result = self
            .fetch_init_ship_data(order_id)
            .and_then(|payload| extract_raw_delivery_product_info_from_init_ship_data(&payload));
        match init_result {
            Ok(info) => {
                let old_waybill = old_waybill_from_raw_delivery_info(&info);
                Ok((info, old_waybill))
            }
            Err(init_err) if is_missing_snapshot_error(&init_err) => {
                let detail_result = self.fetch_order_detail(order_id).and_then(|payload| {
                    extract_raw_delivery_product_info_from_order_detail(&payload)
                });
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

    fn do_update(
        &self,
        order_id: &str,
        tracking_number: &str,
        old_info: &Value,
        delivery_override: Option<(&str, &str)>,
    ) -> anyhow::Result<()> {
        let delivery_override = delivery_override.map(|(delivery_id, delivery_name)| {
            desktop_services::delivery_update::DeliveryOverride {
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
        let resp = self.post_json_sync(DELIVERY_UPDATE_URL, &body)?;
        check_update_response(&resp)
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

impl DeliveryGateway for HttpDeliveryGateway {
    fn update_delivery(
        &self,
        request: &DeliveryUpdateRequest,
    ) -> anyhow::Result<DeliveryUpdateResult> {
        let (old_info, old_waybill) = self.extract_delivery_info(&request.order_id)?;

        match self.do_update(&request.order_id, &request.tracking_number, &old_info, None) {
            Ok(()) => {
                return Ok(DeliveryUpdateResult {
                    order_id: request.order_id.clone(),
                    success: true,
                    previous_waybill: if old_waybill.is_empty() {
                        None
                    } else {
                        Some(old_waybill)
                    },
                    error_message: None,
                });
            }
            Err(e) if is_delivery_mismatch_error(&e.to_string()) => {
                if let Some(override_info) =
                    determine_delivery_override_from_raw_info(&request.tracking_number, &old_info)
                {
                    self.do_update(
                        &request.order_id,
                        &request.tracking_number,
                        &old_info,
                        Some((&override_info.delivery_id, &override_info.delivery_name)),
                    )?;
                    return Ok(DeliveryUpdateResult {
                        order_id: request.order_id.clone(),
                        success: true,
                        previous_waybill: if old_waybill.is_empty() {
                            None
                        } else {
                            Some(old_waybill)
                        },
                        error_message: None,
                    });
                }
                return Ok(DeliveryUpdateResult {
                    order_id: request.order_id.clone(),
                    success: false,
                    previous_waybill: None,
                    error_message: Some("快递单号与物流商不匹配，且无法自动映射".to_string()),
                });
            }
            Err(e) => {
                return Ok(DeliveryUpdateResult {
                    order_id: request.order_id.clone(),
                    success: false,
                    previous_waybill: None,
                    error_message: Some(e.to_string()),
                });
            }
        }
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
        let result = self.update_delivery(&request)?;
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
