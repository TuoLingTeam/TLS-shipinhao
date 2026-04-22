//! 微信小店订单列表 `orderSearch`，请求体与 `review_matcher._build_order_search_payload` 对齐。

use crate::adapters::common::{build_client, build_weixin_shop_headers};
use desktop_services::order_cache_repository::{CacheOrderProduct, CacheOrderRecord};
use desktop_services::order_fetcher::{backoff_seconds, is_api_rate_limited, is_http_rate_limited};
use desktop_services::order_sync_service::{CacheFetchResult, CacheOrderFinder, SyncWindowOrders};
use domain_core::OrderCacheEntry;
use reqwest::header::HeaderMap;
use serde_json::{json, Value};
use std::collections::HashMap;
use std::future::Future;
use std::sync::atomic::{AtomicBool, AtomicI64, AtomicU32, AtomicU64, Ordering};
use std::sync::{Arc, Mutex};
use std::time::Duration;

/// 订单接口 URL / Referer：obfstr 编译期加密，二进制里 `strings` 扫不到原文
fn order_search_url() -> String {
    obfstr::obfstr!("https://store.weixin.qq.com/shop-faas/mmchannelstradeorder/list/cgi/orderSearch")
        .to_string()
}
fn order_list_referer() -> String {
    obfstr::obfstr!("https://store.weixin.qq.com/shop/order/list").to_string()
}
const ORDER_PAGE_SIZE: i64 = 100;
/// 订单缓存拉取并发数：过高会加速触发平台频率限制，2 是经验上较稳的折中值。
const ORDER_CACHE_FETCH_WORKERS: usize = 2;
/// 订单搜索的限流退避次数（全窗口共享）。
///
/// 与评价接口不同：订单接口采用多 worker 并行抓取，一旦触发限流需要更长的
/// 恢复时间。因此在此处覆盖全局 `RATE_LIMIT_RETRY_COUNT`，扩展到 5 次：
/// 2/4/8/16/32 秒，最多累计 62 秒退避。任一 worker 请求成功后立即归零。
const ORDER_RATE_LIMIT_RETRY_COUNT: u32 = 5;
/// 单次拉取窗口允许的累计退避秒数上限（兵底）。
///
/// 达到该阈值仍未恢复时，终止拉取并向上层汇报，避免长时间死等。
const ORDER_RATE_LIMIT_MAX_TOTAL_WAIT_SECS: u64 = 120;
/// 与 `FETCH_PAGE_INTERVAL_SECONDS` 对齐（秒）。
const FETCH_PAGE_INTERVAL: Duration = Duration::from_millis(300);

#[derive(Debug, PartialEq)]
enum OrderSearchRequestOutcome<T> {
    Ready(T),
    RetryRateLimited,
}

fn cache_fetch_worker_count() -> usize {
    ORDER_CACHE_FETCH_WORKERS
}

/// 限流退避调度的结果。
#[derive(Debug, PartialEq, Eq)]
enum BackoffSchedule {
    /// 安排了新一轮退避，调用方应 sleep 这么多秒后重试。
    Scheduled(u64),
    /// 当前仍处于其他 worker 已安排的退避窗口内，调用方应 sleep 剩余秒数后重试，
    /// 但**不**消耗本次重试配额。
    Waiting(u64),
    /// 已耗尽退避预算（次数或累计时长到顶），调用方应放弃。
    Exhausted,
}

async fn retry_order_search_with_gate<T, F, Fut>(
    mut operation: F,
    rate_limit_gate: Arc<OrderRateLimitGate>,
) -> anyhow::Result<T>
where
    F: FnMut() -> Fut,
    Fut: Future<Output = anyhow::Result<OrderSearchRequestOutcome<T>>>,
{
    loop {
        match operation().await? {
            OrderSearchRequestOutcome::Ready(value) => {
                rate_limit_gate.record_success();
                return Ok(value);
            }
            OrderSearchRequestOutcome::RetryRateLimited => {
                match rate_limit_gate.try_schedule_backoff() {
                    BackoffSchedule::Scheduled(wait_secs) => {
                        tracing::warn!(
                            target: "order.fetch.retry",
                            "订单接口触发频率限制，等待 {wait_secs} 秒后重试"
                        );
                        tokio::time::sleep(Duration::from_secs(wait_secs)).await;
                    }
                    BackoffSchedule::Waiting(wait_secs) => {
                        tracing::debug!(
                            target: "order.fetch.retry",
                            "订单接口仍在已安排的退避窗口内，继续等待 {wait_secs} 秒"
                        );
                        tokio::time::sleep(Duration::from_secs(wait_secs)).await;
                    }
                    BackoffSchedule::Exhausted => {
                        anyhow::bail!("订单搜索持续触发频率限制，请稍后再试");
                    }
                }
            }
        }
    }
}

#[derive(Debug)]
struct OrderRateLimitGate {
    /// 当前退避窗口的结束时刻（全局共享）。
    pause_until: Mutex<std::time::Instant>,
    /// 已消耗的退避次数，成功时归零。
    attempt: AtomicU32,
    /// 本窗口累计退避秒数，成功时归零。
    total_wait_secs: AtomicU64,
}

impl Default for OrderRateLimitGate {
    fn default() -> Self {
        Self {
            pause_until: Mutex::new(std::time::Instant::now()),
            attempt: AtomicU32::new(0),
            total_wait_secs: AtomicU64::new(0),
        }
    }
}

impl OrderRateLimitGate {
    /// 尝试安排一次限流退避：
    ///
    /// - 如果当前仍在其他 worker 已经安排的 pause 窗口里，返回 `Waiting(remaining)`
    ///   并**不**消耗次数配额——避免并发请求同时把 `attempt` 一次性耗尽。
    /// - 否则检查次数和累计时长：
    ///   - 若已达 `ORDER_RATE_LIMIT_RETRY_COUNT` 或超出 `ORDER_RATE_LIMIT_MAX_TOTAL_WAIT_SECS`
    ///     返回 `Exhausted`。
    ///   - 否则按 `backoff_seconds(attempt)` 计算 wait 秒数，更新 pause 与累计，
    ///     返回 `Scheduled(wait_secs)`。
    fn try_schedule_backoff(&self) -> BackoffSchedule {
        let mut guard = self.pause_until.lock().expect("rate limit gate lock");
        let now = std::time::Instant::now();

        if *guard > now {
            let remaining = guard.saturating_duration_since(now).as_secs();
            return BackoffSchedule::Waiting(remaining.max(1));
        }

        let current_attempt = self.attempt.load(Ordering::Relaxed);
        if current_attempt >= ORDER_RATE_LIMIT_RETRY_COUNT {
            return BackoffSchedule::Exhausted;
        }
        let wait_secs = backoff_seconds(current_attempt);
        let total = self.total_wait_secs.load(Ordering::Relaxed);
        if total.saturating_add(wait_secs) > ORDER_RATE_LIMIT_MAX_TOTAL_WAIT_SECS {
            return BackoffSchedule::Exhausted;
        }

        *guard = now + Duration::from_secs(wait_secs);
        self.attempt.store(current_attempt + 1, Ordering::Relaxed);
        self.total_wait_secs.store(total + wait_secs, Ordering::Relaxed);
        BackoffSchedule::Scheduled(wait_secs)
    }

    /// 任一 worker 请求成功后调用，归零全局退避状态。
    fn record_success(&self) {
        self.attempt.store(0, Ordering::Relaxed);
        self.total_wait_secs.store(0, Ordering::Relaxed);
    }

    async fn wait_if_needed(&self) {
        loop {
            let until = *self.pause_until.lock().expect("rate limit gate lock");
            let now = std::time::Instant::now();
            if now >= until {
                return;
            }
            tokio::time::sleep(
                until
                    .saturating_duration_since(now)
                    .min(Duration::from_millis(50)),
            )
            .await;
        }
    }
}

pub struct HttpOrderSearchClient {
    cookie_header: String,
    biz_magic: String,
    grant_id: Option<String>,
    client: reqwest::Client,
}

pub struct HttpOrderCacheFinder {
    cookie_header: String,
    biz_magic: String,
    grant_id: Option<String>,
    stopped: bool,
}

#[derive(Debug, Default)]
pub struct SyncedOrderSnapshot {
    pub cache_records: Vec<CacheOrderRecord>,
}

impl HttpOrderSearchClient {
    pub fn new(cookie_header: String, biz_magic: String) -> Self {
        Self::new_with_grant(cookie_header, biz_magic, None)
    }

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

    fn build_headers(&self) -> HeaderMap {
        build_weixin_shop_headers(
            &order_list_referer(),
            &self.cookie_header,
            &self.biz_magic,
            self.grant_id.as_deref(),
        )
    }

    pub async fn fetch_order_snapshots_in_window(
        &self,
        start_unix: i64,
        end_unix: i64,
    ) -> anyhow::Result<SyncedOrderSnapshot> {
        self.fetch_order_snapshots_in_window_parallel(start_unix, end_unix, 1)
            .await
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
        let url = Arc::new(format!("{}?token=&lang=zh_CN", order_search_url()));
        let next_page = Arc::new(AtomicI64::new(1));
        let should_stop = Arc::new(AtomicBool::new(false));
        let rate_limit_gate = Arc::new(OrderRateLimitGate::default());
        let ui_by_id = Arc::new(Mutex::new(HashMap::<String, OrderCacheEntry>::new()));
        let cache_by_id = Arc::new(Mutex::new(HashMap::<String, CacheOrderRecord>::new()));
        let mut tasks = tokio::task::JoinSet::new();

        for _ in 0..worker_count {
            tasks.spawn(run_order_fetch_worker(
                self.client.clone(),
                self.build_headers(),
                Arc::clone(&url),
                Arc::clone(&next_page),
                Arc::clone(&should_stop),
                Arc::clone(&rate_limit_gate),
                Arc::clone(&ui_by_id),
                Arc::clone(&cache_by_id),
                Arc::clone(&now_rfc),
                now_epoch,
                start_unix,
                end_unix,
            ));
        }

        while let Some(result) = tasks.join_next().await {
            result.map_err(|join_err| anyhow::anyhow!(join_err.to_string()))??;
        }

        let cache_records = Arc::try_unwrap(cache_by_id)
            .map_err(|_| anyhow::anyhow!("rich cache still shared"))?
            .into_inner()
            .map_err(|_| anyhow::anyhow!("rich cache poisoned"))?
            .into_values()
            .collect();

        Ok(SyncedOrderSnapshot { cache_records })
    }
}

#[allow(clippy::too_many_arguments)]
async fn run_order_fetch_worker(
    client: reqwest::Client,
    headers: HeaderMap,
    url: Arc<String>,
    next_page: Arc<AtomicI64>,
    should_stop: Arc<AtomicBool>,
    rate_limit_gate: Arc<OrderRateLimitGate>,
    ui_by_id: Arc<Mutex<HashMap<String, OrderCacheEntry>>>,
    cache_by_id: Arc<Mutex<HashMap<String, CacheOrderRecord>>>,
    now_rfc: Arc<String>,
    now_epoch: i64,
    start_unix: i64,
    end_unix: i64,
) -> anyhow::Result<()> {
    loop {
        if should_stop.load(Ordering::Relaxed) {
            break Ok(());
        }

        let page = next_page.fetch_add(1, Ordering::Relaxed);
        let body = json!({
            "pageSize": ORDER_PAGE_SIZE,
            "nextKey": "",
            "orderStatus": "",
            "searchType": 0,
            "page": page,
        });

        let payload = post_order_search_with_retry_inner(
            &client,
            headers.clone(),
            &url,
            &body,
            Arc::clone(&rate_limit_gate),
        )
        .await?;
        let Some(list) = order_list_or_stop(&payload)? else {
            should_stop.store(true, Ordering::Relaxed);
            break Ok(());
        };

        let mut page_ui = HashMap::new();
        let mut page_cache = HashMap::new();
        let latest_on_page = merge_order_page(
            list, start_unix, end_unix, &now_rfc, now_epoch,
            &mut page_ui, &mut page_cache,
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
}

async fn post_order_search_with_retry_inner(
    client: &reqwest::Client,
    headers: HeaderMap,
    url: &str,
    body: &Value,
    rate_limit_gate: Arc<OrderRateLimitGate>,
) -> anyhow::Result<Value> {
    let request_gate = Arc::clone(&rate_limit_gate);
    retry_order_search_with_gate(
        || {
            let headers = headers.clone();
            let rate_limit_gate = Arc::clone(&request_gate);
            async move {
                rate_limit_gate.wait_if_needed().await;

                let response = client.post(url).headers(headers).json(body).send().await?;

                if is_http_rate_limited(response.status().as_u16()) {
                    return Ok(OrderSearchRequestOutcome::RetryRateLimited);
                }

                let val: Value = response.json().await?;
                if is_api_rate_limited(&val) {
                    return Ok(OrderSearchRequestOutcome::RetryRateLimited);
                }
                Ok(OrderSearchRequestOutcome::Ready(val))
            }
        },
        rate_limit_gate,
    )
    .await
}

impl HttpOrderCacheFinder {
    pub fn new(cookie_header: String, biz_magic: String) -> Self {
        Self::new_with_grant(cookie_header, biz_magic, None)
    }

    pub fn new_with_grant(
        cookie_header: String,
        biz_magic: String,
        grant_id: Option<String>,
    ) -> Self {
        Self {
            cookie_header,
            biz_magic,
            grant_id,
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
        let client = HttpOrderSearchClient::new_with_grant(
            self.cookie_header.clone(),
            self.biz_magic.clone(),
            self.grant_id.clone(),
        );
        let snapshot = rt.block_on(client.fetch_order_snapshots_in_window_parallel(
            start,
            end,
            cache_fetch_worker_count(),
        ))?;

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

    let amount_cent = pick_amount_cent(raw);
    let created_at = chrono::DateTime::from_timestamp(create_ts, 0)
        .map(|d| d.to_rfc3339())
        .unwrap_or_default();

    Some(OrderCacheEntry {
        order_id: order_id.to_string(),
        buyer_name: buyer,
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
impl OrderRateLimitGate {
    /// 测试辅助：立即把 pause 窗口过期，以便模拟时间推进。
    fn force_expire_pause(&self) {
        *self.pause_until.lock().expect("rate limit gate lock") = std::time::Instant::now();
    }

    fn attempt_count(&self) -> u32 {
        self.attempt.load(Ordering::Relaxed)
    }

    fn total_wait_secs_snapshot(&self) -> u64 {
        self.total_wait_secs.load(Ordering::Relaxed)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;
    use std::sync::atomic::{AtomicU32, Ordering};
    use std::sync::Arc;

    #[test]
    fn recent_cache_fetch_uses_two_workers() {
        assert_eq!(cache_fetch_worker_count(), 2);
    }

    #[test]
    fn gate_schedules_expanded_2_4_8_16_32_sequence_then_exhausts() {
        let gate = OrderRateLimitGate::default();
        let mut scheduled = Vec::<u64>::new();
        for _ in 0..ORDER_RATE_LIMIT_RETRY_COUNT {
            match gate.try_schedule_backoff() {
                BackoffSchedule::Scheduled(secs) => scheduled.push(secs),
                other => panic!("期望 Scheduled，实际 {other:?}"),
            }
            gate.force_expire_pause();
        }
        assert_eq!(scheduled, vec![2, 4, 8, 16, 32]);
        assert_eq!(gate.attempt_count(), ORDER_RATE_LIMIT_RETRY_COUNT);

        match gate.try_schedule_backoff() {
            BackoffSchedule::Exhausted => (),
            other => panic!("超过上限应 Exhausted，实际 {other:?}"),
        }
    }

    #[test]
    fn concurrent_gate_calls_share_backoff_budget() {
        let gate = OrderRateLimitGate::default();
        match gate.try_schedule_backoff() {
            BackoffSchedule::Scheduled(secs) => assert_eq!(secs, 2),
            other => panic!("首次期望 Scheduled(2)，实际 {other:?}"),
        }
        match gate.try_schedule_backoff() {
            BackoffSchedule::Waiting(_) => (),
            other => panic!("同窗口内第二次应 Waiting，实际 {other:?}"),
        }
        assert_eq!(
            gate.attempt_count(),
            1,
            "并发调用不应重复消耗 attempt 配额"
        );
    }

    #[test]
    fn gate_record_success_resets_state() {
        let gate = OrderRateLimitGate::default();
        let _ = gate.try_schedule_backoff();
        gate.force_expire_pause();
        let _ = gate.try_schedule_backoff();
        assert_eq!(gate.attempt_count(), 2);
        assert!(gate.total_wait_secs_snapshot() > 0);

        gate.record_success();
        assert_eq!(gate.attempt_count(), 0);
        assert_eq!(gate.total_wait_secs_snapshot(), 0);
    }

    #[tokio::test(start_paused = true)]
    async fn order_search_retry_succeeds_after_three_rate_limits() {
        let call_count = Arc::new(AtomicU32::new(0));
        let cc = call_count.clone();
        let gate = Arc::new(OrderRateLimitGate::default());

        let result = retry_order_search_with_gate(
            move || {
                let cc = cc.clone();
                async move {
                    let n = cc.fetch_add(1, Ordering::SeqCst);
                    if n < 3 {
                        Ok(OrderSearchRequestOutcome::<serde_json::Value>::RetryRateLimited)
                    } else {
                        Ok(OrderSearchRequestOutcome::Ready(
                            serde_json::json!({"code": 0}),
                        ))
                    }
                }
            },
            Arc::clone(&gate),
        )
        .await
        .expect("should eventually succeed");

        assert_eq!(
            result.get("code").and_then(serde_json::Value::as_i64),
            Some(0)
        );
        assert_eq!(call_count.load(Ordering::SeqCst), 4);
        assert_eq!(gate.attempt_count(), 0, "成功后应归零 attempt");
        assert_eq!(gate.total_wait_secs_snapshot(), 0, "成功后应归零累计等待");
    }

    #[test]
    fn order_json_to_entry_maps_buyer_and_string_amount() {
        let raw = json!({
            "commonInfo": {
                "orderId": "3735739244192085760",
                "createTime": 1776324243
            },
            "buyerInfo": {
                "nickName": "琼花🌸若现"
            },
            "priceInfo": {
                "orderPrice": "5990"
            }
        });

        let entry = order_json_to_entry(&raw, "2026-04-16T07:30:00Z").expect("order entry");
        assert_eq!(entry.order_id, "3735739244192085760");
        assert_eq!(entry.buyer_name, "琼花🌸若现");
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
