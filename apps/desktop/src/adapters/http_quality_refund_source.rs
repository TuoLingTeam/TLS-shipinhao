use domain_core::{MatchSource, MatchStrategy, OrderMatchResult, TimeWindow};
use reqwest::header::{
    HeaderMap, HeaderName, HeaderValue, CONTENT_TYPE, COOKIE, ORIGIN, REFERER, USER_AGENT,
};
use serde_json::{json, Value};

const QUALITY_REFUND_ORDER_URL: &str =
    "https://store.weixin.qq.com/shop-faas/statistic/dsr/product/refund/order";
const QUALITY_REFUND_REFERER: &str = "https://store.weixin.qq.com/shop/setting/ratedetail?type=product&key=productQualityRatio_30d&detail=order";
const REQUEST_TIMEOUT_SECS: u64 = 30;

pub struct HttpQualityRefundSource {
    cookie_header: String,
    biz_magic: String,
    client: reqwest::Client,
}

impl HttpQualityRefundSource {
    pub fn new(cookie_header: String, biz_magic: String) -> Self {
        let client = reqwest::Client::builder()
            .timeout(std::time::Duration::from_secs(REQUEST_TIMEOUT_SECS))
            .build()
            .unwrap_or_default();
        Self {
            cookie_header,
            biz_magic,
            client,
        }
    }

    fn build_headers(&self) -> HeaderMap {
        let mut headers = HeaderMap::new();
        headers.insert(CONTENT_TYPE, HeaderValue::from_static("application/json"));
        headers.insert(
            ORIGIN,
            HeaderValue::from_static("https://store.weixin.qq.com"),
        );
        headers.insert(REFERER, HeaderValue::from_static(QUALITY_REFUND_REFERER));
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

    fn request_sync(&self, method: reqwest::Method) -> anyhow::Result<Value> {
        let rt = tokio::runtime::Handle::current();
        let headers = self.build_headers();
        let client = self.client.clone();
        let url = format!("{QUALITY_REFUND_ORDER_URL}?token=&lang=zh_CN");

        let resp = std::thread::spawn(move || {
            rt.block_on(async move {
                let builder = client.request(method.clone(), &url).headers(headers);
                let response = if method == reqwest::Method::POST {
                    builder.json(&json!({})).send().await?
                } else {
                    builder.send().await?
                };
                response.json::<Value>().await
            })
        })
        .join()
        .map_err(|_| anyhow::anyhow!("品退请求线程崩溃"))??;

        Ok(resp)
    }

    pub fn fetch_quality_refund_orders(
        &self,
        window: &TimeWindow,
    ) -> anyhow::Result<Vec<OrderMatchResult>> {
        let start_ts = parse_window_boundary(&window.start_at)?;
        let end_ts = parse_window_boundary(&window.end_at)?;
        let methods = [reqwest::Method::GET, reqwest::Method::POST];
        let mut errors = Vec::new();

        for method in methods {
            match self.request_sync(method.clone()) {
                Ok(payload) => {
                    if payload.get("code").and_then(Value::as_i64) != Some(0) {
                        errors.push(format!("{method} API错误: {payload}"));
                        continue;
                    }
                    let items = payload
                        .get("data")
                        .and_then(Value::as_array)
                        .cloned()
                        .unwrap_or_default();
                    let mut results = Vec::new();
                    for (index, item) in items.iter().enumerate() {
                        if let Some(record) =
                            parse_quality_refund_record(item, index, start_ts, end_ts)
                        {
                            results.push(record);
                        }
                    }
                    return Ok(results);
                }
                Err(err) => errors.push(format!("{method} 请求异常: {err}")),
            }
        }

        anyhow::bail!(errors.join("；"))
    }
}

fn parse_quality_refund_record(
    item: &Value,
    _index: usize,
    start_ts: i64,
    end_ts: i64,
) -> Option<OrderMatchResult> {
    let order_info = item.get("orderInfo")?;
    let order_id = first_non_empty_string(order_info, &["orderId", "order_id"]);
    if order_id.is_empty() {
        return None;
    }

    let create_time = first_non_empty_timestamp(
        order_info,
        &[
            "createTime",
            "create_time",
            "createTs",
            "orderCreateTime",
            "refundTime",
        ],
    )
    .or_else(|| {
        first_non_empty_timestamp(
            item,
            &[
                "createTime",
                "create_time",
                "createTs",
                "orderCreateTime",
                "refundTime",
            ],
        )
    })
    .unwrap_or(0);

    if create_time > 0 && (create_time < start_ts || create_time > end_ts) {
        return None;
    }

    let product_id =
        first_non_empty_string(order_info, &["spuId", "spu_id", "productId", "product_id"]);
    let sku_id = first_non_empty_string(order_info, &["skuCode", "skuId", "sku_id"]);
    let sku_name = first_non_empty_string(
        order_info,
        &["skuName", "saleParam", "sale_param", "specName", "spec"],
    );
    let product_name =
        first_non_empty_string(order_info, &["name", "title", "spuName", "productName"]);
    let reason = first_non_empty_string(item, &["reason", "refundReason", "reasonDesc"]);

    Some(OrderMatchResult {
        evaluation_id: order_id.clone(),
        order_id,
        buyer_nickname: String::new(),
        evaluation_content: if reason.is_empty() {
            "品退订单".to_string()
        } else {
            format!("品退原因：{reason}")
        },
        product_id,
        sku_id,
        sku_name,
        product_name,
        matched: true,
        source: MatchSource::ExactOrderId,
        strategy: MatchStrategy::ExactMatch,
        replyable: true,
        reply_deadline: None,
        confidence_score: 100,
        match_reasons: Vec::new(),
        candidate_count: 0,
        top_score: 0,
    })
}

fn first_non_empty_string(value: &Value, keys: &[&str]) -> String {
    keys.iter()
        .find_map(|key| {
            value
                .get(*key)
                .and_then(Value::as_str)
                .map(str::trim)
                .filter(|v| !v.is_empty())
                .map(str::to_string)
        })
        .unwrap_or_default()
}

fn first_non_empty_timestamp(value: &Value, keys: &[&str]) -> Option<i64> {
    keys.iter()
        .find_map(|key| value.get(*key).and_then(parse_timestamp_like))
}

fn parse_timestamp_like(value: &Value) -> Option<i64> {
    let ts = value
        .as_i64()
        .or_else(|| value.as_u64().map(|v| v as i64))
        .or_else(|| value.as_str().and_then(|s| s.trim().parse::<i64>().ok()))?;
    Some(if ts > 9_999_999_999 { ts / 1000 } else { ts })
}

fn parse_window_boundary(value: &str) -> anyhow::Result<i64> {
    chrono::DateTime::parse_from_rfc3339(value.trim())
        .map(|d| d.timestamp())
        .or_else(|_| {
            chrono::NaiveDateTime::parse_from_str(value.trim(), "%Y-%m-%dT%H:%M:%SZ")
                .map(|n| n.and_utc().timestamp())
        })
        .map_err(|_| anyhow::anyhow!("无效的时间：{value}"))
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn parses_quality_refund_payload_into_match_result() {
        let item = json!({
            "reason": "商品质量问题",
            "orderInfo": {
                "orderId": "3735739244192085760",
                "createTime": 1776324243,
                "spuId": "10000496403296",
                "skuCode": "400-1",
                "skuName": "单瓶（体验装） 400*1瓶",
                "name": "仁和二硫化硒去屑洗发水"
            }
        });

        let parsed = parse_quality_refund_record(&item, 0, 1776320000, 1776330000).expect("record");
        assert_eq!(parsed.evaluation_id, "3735739244192085760");
        assert_eq!(parsed.order_id, "3735739244192085760");
        assert_eq!(parsed.product_id, "10000496403296");
        assert_eq!(parsed.sku_id, "400-1");
        assert_eq!(parsed.sku_name, "单瓶（体验装） 400*1瓶");
        assert_eq!(parsed.product_name, "仁和二硫化硒去屑洗发水");
        assert_eq!(parsed.evaluation_content, "品退原因：商品质量问题");
        assert_eq!(parsed.strategy, domain_core::MatchStrategy::ExactMatch);
    }
}
