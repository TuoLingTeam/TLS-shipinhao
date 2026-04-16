//! 微信小店订单列表 `orderSearch`，请求体与 `review_matcher._build_order_search_payload` 对齐。

use desktop_services::order_sync_service::{CacheFetchResult, CacheOrderFinder, SyncWindowOrders};
use desktop_services::order_cache_storage::{CacheOrderProduct, CacheOrderRecord};
use domain_core::OrderCacheEntry;
use reqwest::header::{
    HeaderMap, HeaderName, HeaderValue, CONTENT_TYPE, COOKIE, ORIGIN, REFERER, USER_AGENT,
};
use serde_json::{json, Value};
use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use std::sync::atomic::{AtomicBool, AtomicI64, Ordering};
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

pub struct HttpOrderCacheFinder {
    cookie_header: String,
    biz_magic: String,
    stopped: bool,
}

#[derive(Debug, Default)]
pub struct SyncedOrderSnapshot {
    pub ui_entries: Vec<OrderCacheEntry>,
    pub cache_records: Vec<CacheOrderRecord>,
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
        headers.insert(
            ORIGIN,
            HeaderValue::from_static("https://store.weixin.qq.com"),
        );
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
        headers.insert(
            HeaderName::from_static("potter-scene"),
            HeaderValue::from_static("weixinShop"),
        );
        headers.insert(
            HeaderName::from_static("sec-ch-ua-platform"),
            HeaderValue::from_static("\"macOS\""),
        );
        headers
    }

    /// 按页顺序拉取，客户端按 `createTime` 过滤到 `[start_unix, end_unix]`（与 Python 侧「API 忽略时间参数 + 客户端过滤」一致）。
    #[allow(dead_code)]
    pub async fn fetch_orders_in_window(
        &self,
        start_unix: i64,
        end_unix: i64,
    ) -> anyhow::Result<Vec<OrderCacheEntry>> {
        Ok(self
            .fetch_order_snapshots_in_window(start_unix, end_unix)
            .await?
            .ui_entries)
    }

    pub async fn fetch_order_snapshots_in_window(
        &self,
        start_unix: i64,
        end_unix: i64,
    ) -> anyhow::Result<SyncedOrderSnapshot> {
        self.fetch_order_snapshots_in_window_parallel(start_unix, end_unix, 1).await
    }

    pub async fn fetch_order_snapshots_in_window_parallel(
        &self,
        start_unix: i64,
        end_unix: i64,
        workers: usize,
    ) -> anyhow::Result<SyncedOrderSnapshot> {
        let worker_count = workers.max(1);
        let now_rfc = Arc::new(chrono::Utc::now().to_rfc3339());
        let now_epoch = chrono::Utc::now().timestamp();
        let url = Arc::new(format!("{ORDER_SEARCH_URL}?token=&lang=zh_CN"));
        let next_page = Arc::new(AtomicI64::new(1));
        let should_stop = Arc::new(AtomicBool::new(false));
        let ui_by_id = Arc::new(Mutex::new(HashMap::<String, OrderCacheEntry>::new()));
        let cache_by_id = Arc::new(Mutex::new(HashMap::<String, CacheOrderRecord>::new()));
        let mut tasks = tokio::task::JoinSet::new();

        for _ in 0..worker_count {
            let client = self.client.clone();
            let headers = self.build_headers();
            let url = Arc::clone(&url);
            let next_page = Arc::clone(&next_page);
            let should_stop = Arc::clone(&should_stop);
            let ui_by_id = Arc::clone(&ui_by_id);
            let cache_by_id = Arc::clone(&cache_by_id);
            let now_rfc = Arc::clone(&now_rfc);

            tasks.spawn(async move {
                loop {
                    if should_stop.load(Ordering::Relaxed) {
                        break Ok::<(), anyhow::Error>(());
                    }

                    let page = next_page.fetch_add(1, Ordering::Relaxed);
                    let body = json!({
                        "pageSize": ORDER_PAGE_SIZE,
                        "nextKey": "",
                        "orderStatus": "",
                        "searchType": 0,
                        "page": page,
                    });

                    let payload = post_order_search_with_retry_inner(&client, headers.clone(), &url, &body).await?;
                    let Some(list) = order_list_or_stop(&payload)? else {
                        should_stop.store(true, Ordering::Relaxed);
                        break Ok(());
                    };

                    let mut page_ui = HashMap::new();
                    let mut page_cache = HashMap::new();
                    let latest_on_page = merge_order_page(
                        list,
                        start_unix,
                        end_unix,
                        &now_rfc,
                        now_epoch,
                        &mut page_ui,
                        &mut page_cache,
                    );

                    if !page_ui.is_empty() || !page_cache.is_empty() {
                        let mut ui_guard = ui_by_id.lock().expect("ui cache lock");
                        ui_guard.extend(page_ui);
                        drop(ui_guard);
                        let mut cache_guard = cache_by_id.lock().expect("rich cache lock");
                        cache_guard.extend(page_cache);
                    }

                    if start_unix > 0 && latest_on_page > 0 && latest_on_page < start_unix {
                        should_stop.store(true, Ordering::Relaxed);
                        break Ok(());
                    }

                    tokio::time::sleep(FETCH_PAGE_INTERVAL).await;
                }
            });
        }

        while let Some(result) = tasks.join_next().await {
            result.map_err(|join_err| anyhow::anyhow!(join_err.to_string()))??;
        }

        let ui_entries = Arc::try_unwrap(ui_by_id)
            .map_err(|_| anyhow::anyhow!("ui cache still shared"))?
            .into_inner()
            .map_err(|_| anyhow::anyhow!("ui cache poisoned"))?
            .into_values()
            .collect();
        let cache_records = Arc::try_unwrap(cache_by_id)
            .map_err(|_| anyhow::anyhow!("rich cache still shared"))?
            .into_inner()
            .map_err(|_| anyhow::anyhow!("rich cache poisoned"))?
            .into_values()
            .collect();

        Ok(SyncedOrderSnapshot { ui_entries, cache_records })
    }

}

async fn post_order_search_with_retry_inner(
    client: &reqwest::Client,
    headers: HeaderMap,
    url: &str,
    body: &Value,
) -> anyhow::Result<Value> {
    for attempt in 0u32..5 {
        let response = client
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

impl HttpOrderCacheFinder {
    pub fn new(cookie_header: String, biz_magic: String) -> Self {
        Self {
            cookie_header,
            biz_magic,
            stopped: false,
        }
    }
}

impl CacheOrderFinder for HttpOrderCacheFinder {
    fn stop(&mut self) {
        self.stopped = true;
    }

    fn get_orders_for_cache(
        &mut self,
        earliest_time: i64,
        create_time_start: i64,
        create_time_end: i64,
    ) -> anyhow::Result<CacheFetchResult> {
        if self.stopped {
            return Ok(CacheFetchResult::default());
        }

        let start = earliest_time.max(create_time_start);
        let end = create_time_end;
        let rt = tokio::runtime::Runtime::new()?;
        let client = HttpOrderSearchClient::new(self.cookie_header.clone(), self.biz_magic.clone());
        let snapshot = rt.block_on(client.fetch_order_snapshots_in_window_parallel(start, end, 3))?;

        Ok(CacheFetchResult {
            windows: vec![SyncWindowOrders {
                window_id: format!("{start}-{end}"),
                start_ts: start,
                end_ts: end,
                orders: snapshot.cache_records,
            }],
            warnings: Vec::new(),
        })
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
    now_epoch: i64,
    ui_by_id: &mut HashMap<String, OrderCacheEntry>,
    cache_by_id: &mut HashMap<String, CacheOrderRecord>,
) -> i64 {
    let mut latest_on_page = 0i64;
    for item in list {
        let ct = parse_create_time(item);
        if ct > latest_on_page {
            latest_on_page = ct;
        }
        if ct >= start_unix && ct <= end_unix {
            if let Some(order) = order_json_to_entry(item, now_rfc) {
                ui_by_id.insert(order.order_id.clone(), order);
            }
            if let Some(record) = order_json_to_cache_record(item, now_epoch) {
                cache_by_id.insert(record.order_id.clone(), record);
            }
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
    if let Some(n) = v
        .pointer("/paymentInfo/shouldPayAmount")
        .and_then(parse_i64_like)
    {
        return n;
    }
    if let Some(y) = v
        .pointer("/orderAmountInfo/orderAmount")
        .and_then(|x| x.as_f64())
    {
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

fn first_non_empty_value<'a>(value: &'a Value, keys: &[&str]) -> Option<&'a Value> {
    keys.iter().find_map(|key| {
        let candidate = value.get(*key)?;
        match candidate {
            Value::Null => None,
            Value::String(text) if text.trim().is_empty() => None,
            Value::Array(items) if items.is_empty() => None,
            Value::Object(map) if map.is_empty() => None,
            _ => Some(candidate),
        }
    })
}

fn normalize_sale_param(raw_value: Option<&Value>) -> String {
    match raw_value {
        Some(Value::Array(items)) => items
            .iter()
            .filter_map(Value::as_str)
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .collect::<Vec<_>>()
            .join("|"),
        Some(Value::String(text)) => text.trim().to_string(),
        Some(other) => other.to_string(),
        None => String::new(),
    }
}

fn order_json_to_cache_record(raw: &Value, now_epoch: i64) -> Option<CacheOrderRecord> {
    let common = raw.get("commonInfo")?;
    let order_id = common.get("orderId")?.as_str()?.trim();
    if order_id.is_empty() {
        return None;
    }

    let buyer_nickname = raw
        .pointer("/buyerInfo/nickName")
        .and_then(Value::as_str)
        .map(str::trim)
        .unwrap_or("")
        .to_string();
    let receiver_name = raw
        .pointer("/acceptInfo/receiverName")
        .or_else(|| raw.pointer("/acceptInfo/addressInfo/userName"))
        .or_else(|| raw.pointer("/deliveryInfo/receiverName"))
        .or_else(|| raw.pointer("/addressInfo/userName"))
        .and_then(Value::as_str)
        .map(str::trim)
        .unwrap_or("")
        .to_string();
    let confirm_receipt_time = raw
        .pointer("/acceptInfo/confirmReceiptTime")
        .and_then(parse_i64_like)
        .unwrap_or(0);
    let is_waybill_received = raw
        .pointer("/orderStatus/autoConfirmInfo/isWaybillReceived")
        .and_then(Value::as_bool)
        .unwrap_or(false);
    let waybill_received_time = raw
        .pointer("/orderStatus/autoConfirmInfo/waybillReceivedTime")
        .and_then(parse_i64_like)
        .unwrap_or(0);
    let products = raw
        .get("orderProductInfo")
        .and_then(Value::as_array)
        .or_else(|| raw.get("productInfos").and_then(Value::as_array))
        .cloned()
        .unwrap_or_default()
        .into_iter()
        .map(|product| CacheOrderProduct {
            product_id: first_non_empty_value(
                &product,
                &["productId", "product_id", "spuId", "spu_id"],
            )
            .and_then(Value::as_str)
            .map(str::trim)
            .unwrap_or("")
            .to_string(),
            sku_id: first_non_empty_value(&product, &["skuId", "sku_id"])
                .and_then(Value::as_str)
                .map(str::trim)
                .unwrap_or("")
                .to_string(),
            sale_param: normalize_sale_param(first_non_empty_value(
                &product,
                &["saleParam", "sale_param", "skuName", "specName", "spec"],
            )),
            product_name: first_non_empty_value(
                &product,
                &["title", "spuName", "productName", "name"],
            )
            .and_then(Value::as_str)
            .map(str::trim)
            .unwrap_or("")
            .to_string(),
            thumb_img: first_non_empty_value(
                &product,
                &["thumbImg", "imgUrl", "image", "imageUrl"],
            )
            .and_then(Value::as_str)
            .map(str::trim)
            .unwrap_or("")
            .to_string(),
        })
        .collect::<Vec<_>>();

    Some(CacheOrderRecord {
        order_id: order_id.to_string(),
        buyer_nickname: buyer_nickname.clone(),
        normalized_nickname: buyer_nickname,
        receiver_name,
        amount_cent: pick_amount_cent(raw),
        create_time: common
            .get("createTime")
            .and_then(parse_i64_like)
            .unwrap_or(0),
        confirm_receipt_time,
        is_waybill_received,
        waybill_received_time,
        is_education_order: common
            .get("isEducationOrder")
            .and_then(Value::as_bool)
            .unwrap_or(false),
        order_status: common.get("status").and_then(parse_i64_like).unwrap_or(0),
        openid: common
            .get("openid")
            .and_then(Value::as_str)
            .map(str::trim)
            .unwrap_or("")
            .to_string(),
        raw_source: "order_api".to_string(),
        updated_at: now_epoch,
        products,
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

    #[test]
    fn order_json_to_cache_record_maps_products_and_receipt_fields() {
        let raw = json!({
            "commonInfo": {
                "orderId": "3735739244192085760",
                "createTime": 1776324243,
                "status": 20,
                "openid": "openid-1",
                "isEducationOrder": false
            },
            "buyerInfo": {
                "nickName": "琼花🌸若现"
            },
            "acceptInfo": {
                "confirmReceiptTime": "1776400000"
            },
            "orderStatus": {
                "autoConfirmInfo": {
                    "isWaybillReceived": true,
                    "waybillReceivedTime": 1776380000
                }
            },
            "orderProductInfo": [
                {
                    "productId": "10000496403296",
                    "skuId": "400-1",
                    "saleParam": ["单瓶", "400ml"],
                    "title": "仁和二硫化硒去屑洗发水",
                    "thumbImg": "https://img.example.com/1.png"
                }
            ]
        });

        let record = order_json_to_cache_record(&raw, 1776329999).expect("cache record");
        assert_eq!(record.order_id, "3735739244192085760");
        assert_eq!(record.buyer_nickname, "琼花🌸若现");
        assert_eq!(record.receiver_name, "");
        assert_eq!(record.amount_cent, 0);
        assert_eq!(record.create_time, 1776324243);
        assert_eq!(record.confirm_receipt_time, 1776400000);
        assert!(record.is_waybill_received);
        assert_eq!(record.waybill_received_time, 1776380000);
        assert_eq!(record.order_status, 20);
        assert_eq!(record.openid, "openid-1");
        assert_eq!(record.products.len(), 1);
        assert_eq!(record.products[0].product_id, "10000496403296");
        assert_eq!(record.products[0].sku_id, "400-1");
        assert_eq!(record.products[0].sale_param, "单瓶|400ml");
        assert_eq!(record.products[0].product_name, "仁和二硫化硒去屑洗发水");
    }
}
