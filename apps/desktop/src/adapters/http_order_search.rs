//! 微信小店订单列表 `orderSearch`，请求体与 `review_matcher._build_order_search_payload` 对齐。

use domain_core::OrderCacheEntry;
use reqwest::header::{HeaderMap, HeaderName, HeaderValue, CONTENT_TYPE, COOKIE, ORIGIN, REFERER, USER_AGENT};
use serde_json::{json, Value};
use std::collections::HashMap;
use std::time::Duration;

const ORDER_SEARCH_URL: &str =
    "https://store.weixin.qq.com/shop-faas/mmchannelstradeorder/list/cgi/orderSearch";
const ORDER_LIST_REFERER: &str = "https://store.weixin.qq.com/shop/order/list";
const ORDER_PAGE_SIZE: i64 = 100;
const REQUEST_TIMEOUT_SECS: u64 = 30;
/// 与 `FETCH_PAGE_INTERVAL_SECONDS` 对齐（秒）。
const FETCH_PAGE_INTERVAL: Duration = Duration::from_millis(300);

pub struct HttpOrderSearchClient {
    cookie_header: String,
    biz_magic: String,
    client: reqwest::Client,
}

impl HttpOrderSearchClient {
    pub fn new(cookie_header: String, biz_magic: String) -> Self {
        let client = reqwest::Client::builder()
            .timeout(Duration::from_secs(REQUEST_TIMEOUT_SECS))
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
        headers.insert(ORIGIN, HeaderValue::from_static("https://store.weixin.qq.com"));
        headers.insert(REFERER, HeaderValue::from_static(ORDER_LIST_REFERER));
        headers.insert(
            USER_AGENT,
            HeaderValue::from_static(
                "Mozilla/5.0 (Macintosh; Intel Mac OS X 10_15_7) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/144.0.0.0 Safari/537.36",
            ),
        );
        if let Ok(v) = HeaderValue::from_str(&self.cookie_header) {
            headers.insert(COOKIE, v);
        }
        if let Ok(v) = HeaderValue::from_str(&self.biz_magic) {
            headers.insert(HeaderName::from_static("biz_magic"), v);
        }
        headers.insert(HeaderName::from_static("potter-scene"), HeaderValue::from_static("weixinShop"));
        headers.insert(
            HeaderName::from_static("sec-ch-ua-platform"),
            HeaderValue::from_static("\"macOS\""),
        );
        headers
    }

    /// 按页顺序拉取，客户端按 `createTime` 过滤到 `[start_unix, end_unix]`（与 Python 侧「API 忽略时间参数 + 客户端过滤」一致）。
    pub async fn fetch_orders_in_window(
        &self,
        start_unix: i64,
        end_unix: i64,
    ) -> anyhow::Result<Vec<OrderCacheEntry>> {
        let mut by_id: HashMap<String, OrderCacheEntry> = HashMap::new();
        let now_rfc = chrono::Utc::now().to_rfc3339();
        let url = format!("{ORDER_SEARCH_URL}?token=&lang=zh_CN");
        let mut page: i64 = 1;

        loop {
            let body = json!({
                "pageSize": ORDER_PAGE_SIZE,
                "nextKey": "",
                "orderStatus": "",
                "searchType": 0,
                "page": page,
            });

            let payload = self.post_order_search_with_retry(&url, &body).await?;
            let Some(list) = order_list_or_stop(&payload)? else {
                break;
            };

            let latest_on_page = merge_order_page(list, start_unix, end_unix, &now_rfc, &mut by_id);

            if start_unix > 0 && latest_on_page > 0 && latest_on_page < start_unix {
                break;
            }

            page += 1;
            tokio::time::sleep(FETCH_PAGE_INTERVAL).await;
        }

        Ok(by_id.into_values().collect())
    }

    async fn post_order_search_with_retry(&self, url: &str, body: &Value) -> anyhow::Result<Value> {
        let headers = self.build_headers();
        for attempt in 0u32..5 {
            let response = self
                .client
                .post(url)
                .headers(headers.clone())
                .json(body)
                .send()
                .await?;

            if response.status().as_u16() == 429 {
                tokio::time::sleep(Duration::from_secs(2u64.pow(attempt.min(4)))).await;
                continue;
            }

            let val: Value = response.json().await?;
            if val.get("code").and_then(|c| c.as_i64()) == Some(429) {
                tokio::time::sleep(Duration::from_secs(2u64.pow(attempt.min(4)))).await;
                continue;
            }
            return Ok(val);
        }
        anyhow::bail!("订单搜索持续触发频率限制，请稍后再试")
    }
}

/// `Some(rows)` 继续处理；`None` 表示已到末尾或空页，停止翻页。
fn order_list_or_stop(payload: &Value) -> anyhow::Result<Option<&[Value]>> {
    let code = payload.get("code").and_then(|c| c.as_i64()).unwrap_or(-1);
    if code == 10003 {
        return Ok(None);
    }
    if code == 429 {
        anyhow::bail!("订单接口频率限制，请稍后再试");
    }
    if code != 0 {
        let msg = payload
            .get("msg")
            .or_else(|| payload.get("message"))
            .and_then(|m| m.as_str())
            .unwrap_or("未知错误");
        anyhow::bail!("订单搜索 API 错误（code={code}）：{msg}");
    }
    let list = payload
        .get("orderList")
        .and_then(|v| v.as_array())
        .map(|a| a.as_slice())
        .unwrap_or(&[]);
    if list.is_empty() {
        return Ok(None);
    }
    Ok(Some(list))
}

fn merge_order_page(
    list: &[Value],
    start_unix: i64,
    end_unix: i64,
    now_rfc: &str,
    by_id: &mut HashMap<String, OrderCacheEntry>,
) -> i64 {
    let mut latest_on_page = 0i64;
    for item in list {
        let Some(order) = order_json_to_entry(item, now_rfc) else {
            continue;
        };
        let ct = parse_create_time(item);
        if ct > latest_on_page {
            latest_on_page = ct;
        }
        if ct >= start_unix && ct <= end_unix {
            by_id.insert(order.order_id.clone(), order);
        }
    }
    latest_on_page
}

fn parse_create_time(raw: &Value) -> i64 {
    raw.get("commonInfo")
        .and_then(|c| c.get("createTime"))
        .and_then(|v| v.as_i64().or_else(|| v.as_u64().map(|u| u as i64)))
        .unwrap_or(0)
}

fn parse_i64_like(value: &Value) -> Option<i64> {
    value
        .as_i64()
        .or_else(|| value.as_u64().map(|u| u as i64))
        .or_else(|| value.as_str().and_then(|s| s.trim().parse::<i64>().ok()))
}

fn pick_amount_cent(v: &Value) -> i64 {
    if let Some(n) = v.pointer("/priceInfo/orderPrice").and_then(parse_i64_like) {
        return n;
    }
    if let Some(n) = v.pointer("/payInfo/payAmount").and_then(parse_i64_like) {
        return n;
    }
    if let Some(n) = v.pointer("/paymentInfo/shouldPayAmount").and_then(parse_i64_like) {
        return n;
    }
    if let Some(y) = v.pointer("/orderAmountInfo/orderAmount").and_then(|x| x.as_f64()) {
        return (y * 100.0).round() as i64;
    }
    if let Some(y) = v
        .pointer("/orderAmountInfo/orderAmount")
        .and_then(|x| x.as_str())
        .and_then(|s| s.trim().parse::<f64>().ok())
    {
        return (y * 100.0).round() as i64;
    }
    0
}

fn order_json_to_entry(raw: &Value, now_rfc: &str) -> Option<OrderCacheEntry> {
    let common = raw.get("commonInfo")?.as_object()?;
    let order_id = common.get("orderId")?.as_str()?.trim();
    if order_id.is_empty() {
        return None;
    }
    let create_ts = common
        .get("createTime")
        .and_then(|v| v.as_i64().or_else(|| v.as_u64().map(|u| u as i64)))
        .unwrap_or(0);

    let buyer = raw
        .get("buyerInfo")
        .and_then(|b| b.get("nickName"))
        .and_then(|v| v.as_str())
        .unwrap_or("")
        .to_string();

    let receiver = raw
        .pointer("/acceptInfo/receiverName")
        .or_else(|| raw.pointer("/acceptInfo/addressInfo/userName"))
        .or_else(|| raw.pointer("/deliveryInfo/receiverName"))
        .or_else(|| raw.pointer("/addressInfo/userName"))
        .and_then(|v| v.as_str())
        .unwrap_or("")
        .to_string();

    let amount_cent = pick_amount_cent(raw);
    let created_at = chrono::DateTime::from_timestamp(create_ts, 0)
        .map(|d| d.to_rfc3339())
        .unwrap_or_default();

    Some(OrderCacheEntry {
        order_id: order_id.to_string(),
        buyer_name: buyer,
        receiver_name: receiver,
        amount_cent,
        created_at,
        updated_at: now_rfc.to_string(),
    })
}

/// 将前端传入的 ISO 时间窗解析为 Unix 秒（与本地缓存查询使用同一语义）。
pub fn parse_iso_window(start_at: &str, end_at: &str) -> anyhow::Result<(i64, i64)> {
    let start = chrono::DateTime::parse_from_rfc3339(start_at.trim())
        .map(|d| d.timestamp())
        .or_else(|_| {
            chrono::NaiveDateTime::parse_from_str(start_at.trim(), "%Y-%m-%dT%H:%M:%SZ")
                .map(|n| n.and_utc().timestamp())
        })
        .map_err(|_| anyhow::anyhow!("无效的开始时间：{start_at}"))?;

    let end = chrono::DateTime::parse_from_rfc3339(end_at.trim())
        .map(|d| d.timestamp())
        .or_else(|_| {
            chrono::NaiveDateTime::parse_from_str(end_at.trim(), "%Y-%m-%dT%H:%M:%SZ")
                .map(|n| n.and_utc().timestamp())
        })
        .map_err(|_| anyhow::anyhow!("无效的结束时间：{end_at}"))?;

    Ok((start, end))
}


#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn order_json_to_entry_maps_nested_receiver_and_string_amount() {
        let raw = json!({
            "commonInfo": {
                "orderId": "3735739244192085760",
                "createTime": 1776324243
            },
            "buyerInfo": {
                "nickName": "琼花🌸若现"
            },
            "acceptInfo": {
                "addressInfo": {
                    "userName": "李**"
                }
            },
            "priceInfo": {
                "orderPrice": "5990"
            }
        });

        let entry = order_json_to_entry(&raw, "2026-04-16T07:30:00Z").expect("order entry");
        assert_eq!(entry.order_id, "3735739244192085760");
        assert_eq!(entry.buyer_name, "琼花🌸若现");
        assert_eq!(entry.receiver_name, "李**");
        assert_eq!(entry.amount_cent, 5990);
        assert_eq!(entry.created_at, "2026-04-16T07:24:03+00:00");
    }
}
