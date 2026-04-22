use crate::adapters::common::{build_client, build_weixin_shop_headers};
use desktop_services::order_fetcher::{
    backoff_seconds, is_api_rate_limited, is_http_rate_limited, RATE_LIMIT_RETRY_COUNT,
};
use desktop_services::review_batch_match::EvaluationRecord;
use desktop_services::review_match_flow::{is_evaluation_replyable, reply_deadline};
use desktop_services::ReviewQuery;
use desktop_services::ReviewSource;
use domain_core::{MatchSource, MatchStrategy, OrderMatchResult};
use serde_json::Value;
use std::future::Future;
use std::time::Duration;

/// 评价接口 URL / Referer：obfstr 编译期加密
fn evaluation_search_url() -> String {
    obfstr::obfstr!("https://store.weixin.qq.com/shop-faas/mmchannelstradeevaluation/cgi/search")
        .to_string()
}
fn evaluation_referer() -> String {
    obfstr::obfstr!("https://store.weixin.qq.com/shop/evaluate/home").to_string()
}
const EVALUATION_PAGE_SIZE: usize = 20;
const EVALUATION_MAX_PAGES: usize = 50;

pub struct HttpReviewSource {
    cookie_header: String,
    biz_magic: String,
    grant_id: Option<String>,
    client: reqwest::Client,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ReviewRetryReason {
    RateLimited { api_level: bool },
    TemporaryFailure,
}

#[derive(Debug, PartialEq)]
enum ReviewRequestOutcome<T> {
    Ready(T),
    Retry(ReviewRetryReason),
}

fn is_temporary_review_status(status_code: u16) -> bool {
    matches!(status_code, 408 | 425 | 500 | 502 | 503 | 504)
}

fn is_temporary_review_error(error: &reqwest::Error) -> bool {
    error.is_timeout() || error.is_connect() || error.is_request() || error.is_body()
}

fn review_retry_error_message(reason: ReviewRetryReason) -> &'static str {
    match reason {
        ReviewRetryReason::RateLimited { .. } => "评价接口持续触发频率限制，请稍后再试",
        ReviewRetryReason::TemporaryFailure => "评价接口临时失败，多次重试后仍未恢复，请稍后再试",
    }
}

async fn retry_review_request_with_sleep<T, F, Fut, S, SleepFut>(
    mut operation: F,
    mut sleep_fn: S,
) -> anyhow::Result<T>
where
    F: FnMut() -> Fut,
    Fut: Future<Output = anyhow::Result<ReviewRequestOutcome<T>>>,
    S: FnMut(u64, ReviewRetryReason) -> SleepFut,
    SleepFut: Future<Output = ()>,
{
    let mut attempt = 0u32;
    loop {
        match operation().await? {
            ReviewRequestOutcome::Ready(value) => return Ok(value),
            ReviewRequestOutcome::Retry(reason) => {
                if attempt >= RATE_LIMIT_RETRY_COUNT {
                    anyhow::bail!(review_retry_error_message(reason));
                }
                let wait_secs = backoff_seconds(attempt);
                sleep_fn(wait_secs, reason).await;
                attempt += 1;
            }
        }
    }
}

async fn retry_review_request<T, F, Fut>(mut operation: F) -> anyhow::Result<T>
where
    F: FnMut() -> Fut,
    Fut: Future<Output = anyhow::Result<ReviewRequestOutcome<T>>>,
{
    retry_review_request_with_sleep(&mut operation, |wait_secs, reason| async move {
        match reason {
            ReviewRetryReason::RateLimited { api_level } => {
                let suffix = if api_level { "(API)" } else { "" };
                tracing::warn!(
                    target: "review.fetch.retry",
                    "评价接口触发频率限制{suffix}，等待 {wait_secs} 秒后重试"
                );
            }
            ReviewRetryReason::TemporaryFailure => {
                tracing::warn!(
                    target: "review.fetch.retry",
                    "评价接口临时失败，等待 {wait_secs} 秒后重试"
                );
            }
        }
        tokio::time::sleep(Duration::from_secs(wait_secs)).await;
    })
    .await
}

impl HttpReviewSource {
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
            &evaluation_referer(),
            &self.cookie_header,
            &self.biz_magic,
            self.grant_id.as_deref(),
        )
    }

    fn post_json_sync(&self, body: &Value) -> anyhow::Result<Value> {
        let rt = tokio::runtime::Handle::current();
        let headers = self.build_headers();
        let client = self.client.clone();
        let url = format!("{}?token=&lang=zh_CN", evaluation_search_url());
        let body = body.clone();

        let resp = std::thread::spawn(move || {
            rt.block_on(async {
                retry_review_request(|| {
                    let client = client.clone();
                    let headers = headers.clone();
                    let url = url.clone();
                    let body = body.clone();
                    async move {
                        let response =
                            match client.post(&url).headers(headers).json(&body).send().await {
                                Ok(response) => response,
                                Err(error) if is_temporary_review_error(&error) => {
                                    return Ok(ReviewRequestOutcome::Retry(
                                        ReviewRetryReason::TemporaryFailure,
                                    ));
                                }
                                Err(error) => return Err(error.into()),
                            };

                        let status_code = response.status().as_u16();
                        if is_http_rate_limited(status_code) {
                            return Ok(ReviewRequestOutcome::Retry(
                                ReviewRetryReason::RateLimited { api_level: false },
                            ));
                        }
                        if is_temporary_review_status(status_code) {
                            return Ok(ReviewRequestOutcome::Retry(
                                ReviewRetryReason::TemporaryFailure,
                            ));
                        }

                        let payload = response.json::<Value>().await?;
                        if is_api_rate_limited(&payload) {
                            return Ok(ReviewRequestOutcome::Retry(
                                ReviewRetryReason::RateLimited { api_level: true },
                            ));
                        }

                        Ok(ReviewRequestOutcome::Ready(payload))
                    }
                })
                .await
            })
        })
        .join()
        .map_err(|_| anyhow::anyhow!("请求线程崩溃"))??;

        Ok(resp)
    }

    fn parse_timestamp(ts_str: &str) -> i64 {
        let trimmed = ts_str.trim();
        chrono::DateTime::parse_from_rfc3339(trimmed)
            .map(|dt| dt.timestamp())
            .or_else(|_| {
                chrono::NaiveDateTime::parse_from_str(trimmed, "%Y-%m-%dT%H:%M:%SZ")
                    .map(|dt| dt.and_utc().timestamp())
            })
            .or_else(|_| {
                chrono::NaiveDate::parse_from_str(trimmed.get(..10).unwrap_or(trimmed), "%Y-%m-%d")
                    .map(|d| {
                        d.and_hms_opt(0, 0, 0)
                            .map(|dt| dt.and_utc().timestamp())
                            .unwrap_or(0)
                    })
            })
            .unwrap_or(0)
    }
}

fn parse_review_record(eval: &Value) -> Option<OrderMatchResult> {
    let evaluation = parse_evaluation_record(eval)?;
    let replyable = is_evaluation_replyable(
        evaluation.can_reply_expire_time,
        chrono::Utc::now().timestamp(),
    );
    let reply_deadline = reply_deadline(evaluation.can_reply_expire_time).map(|dt| dt.to_rfc3339());

    Some(OrderMatchResult {
        evaluation_id: evaluation.evaluation_id,
        order_id: eval
            .get("orderId")
            .and_then(Value::as_str)
            .unwrap_or("")
            .trim()
            .to_string(),
        buyer_nickname: evaluation.buyer_nickname,
        evaluation_content: if evaluation.evaluation_content.is_empty() {
            evaluation.default_content
        } else {
            evaluation.evaluation_content
        },
        product_id: evaluation.product_id,
        sku_id: evaluation.sku_id,
        sku_name: evaluation.sku_name,
        product_name: evaluation.product_name,
        matched: false,
        source: MatchSource::ManualFallback,
        strategy: MatchStrategy::Fallback,
        replyable,
        reply_deadline,
        confidence_score: 0,
        quality_refund_info: None,
        match_reasons: Vec::new(),
        candidate_count: 0,
        top_score: 0,
    })
}

fn parse_evaluation_record(eval: &Value) -> Option<EvaluationRecord> {
    let op_info = eval.get("operationInfo")?;
    let attitude = op_info
        .get("attitudeName")
        .and_then(Value::as_str)
        .unwrap_or("");
    if attitude != "不够好" {
        return None;
    }
    let can_reply_expire_time = op_info
        .get("canReplyExpireTime")
        .and_then(|v| v.as_i64().or_else(|| v.as_u64().map(|u| u as i64)))
        .unwrap_or(0);
    let evaluation_info = eval.get("evaluationInfo").cloned().unwrap_or_default();
    let product_info = eval.get("productInfo").cloned().unwrap_or_default();

    let evaluation_id = eval
        .get("productEvaluationId")
        .and_then(Value::as_str)
        .unwrap_or("")
        .trim()
        .to_string();
    if evaluation_id.is_empty() {
        return None;
    }

    let eval_time = evaluation_info
        .get("firstEvaluationInfo")
        .and_then(|v| v.get("buyerEvaluationInfo"))
        .and_then(|v| v.get("createTime"))
        .and_then(|v| v.as_i64().or_else(|| v.as_u64().map(|u| u as i64)))
        .unwrap_or(0);
    let buyer_nickname = evaluation_info
        .get("buyer")
        .and_then(|v| v.get("identity"))
        .and_then(|v| v.get("nickname"))
        .and_then(Value::as_str)
        .unwrap_or("")
        .trim()
        .to_string();
    let evaluation_content = evaluation_info
        .get("firstEvaluationInfo")
        .and_then(|v| v.get("buyerEvaluationInfo"))
        .and_then(|v| v.get("content"))
        .and_then(Value::as_str)
        .unwrap_or("")
        .trim()
        .to_string();
    let default_content = evaluation_info
        .get("firstEvaluationInfo")
        .and_then(|v| v.get("buyerEvaluationInfo"))
        .and_then(|v| v.get("defaultContent"))
        .and_then(Value::as_str)
        .unwrap_or("")
        .trim()
        .to_string();
    let product_id = first_non_empty_string(
        &product_info,
        &["productId", "product_id", "spuId", "spu_id"],
    );
    let sku_id = first_non_empty_string(&product_info, &["skuId", "sku_id"]);
    let sku_name = first_non_empty_string(
        &product_info,
        &["skuName", "saleParam", "sale_param", "specName", "spec"],
    );
    let product_name =
        first_non_empty_string(&product_info, &["spuName", "title", "productName", "name"]);
    let evaluation_star = evaluation_info
        .get("evaluationStar")
        .and_then(|v| v.as_i64().or_else(|| v.as_u64().map(|u| u as i64)))
        .unwrap_or(0) as i32;

    Some(EvaluationRecord {
        evaluation_id,
        buyer_nickname,
        product_id,
        sku_id,
        sku_name,
        product_name,
        eval_time,
        attitude_name: attitude.to_string(),
        evaluation_content,
        default_content,
        evaluation_star,
        can_reply_expire_time,
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

impl ReviewSource for HttpReviewSource {
    fn fetch_reviews(&self, query: &ReviewQuery) -> anyhow::Result<Vec<OrderMatchResult>> {
        let start_ts = Self::parse_timestamp(&query.time_window.start_at);
        let end_ts = Self::parse_timestamp(&query.time_window.end_at);

        let mut all_results = Vec::new();
        let mut page = 1;
        let mut max_pages = EVALUATION_MAX_PAGES;

        while page <= max_pages {
            let body = serde_json::json!({
                "orderId": "",
                "productId": "",
                "productEvaluationId": "",
                "buyerEvaluationTimeStart": start_ts,
                "buyerEvaluationTimeEnd": end_ts,
                "page": page,
                "status": 2,
                "visibleType": 0,
            });

            let resp = self.post_json_sync(&body)?;

            if resp.get("code").and_then(Value::as_i64) != Some(0) {
                anyhow::bail!("评价 API 错误：{}", resp);
            }

            let evaluations = resp
                .get("finderProductEvaluationInfoList")
                .and_then(Value::as_array)
                .cloned()
                .unwrap_or_default();

            if let Some(total) = resp.get("totalCnt").and_then(Value::as_i64) {
                let total = total.max(0) as usize;
                let total_pages = total.div_ceil(EVALUATION_PAGE_SIZE);
                max_pages = EVALUATION_MAX_PAGES.min(total_pages.max(1));
            }

            for eval in &evaluations {
                if let Some(record) = parse_review_record(eval) {
                    all_results.push(record);
                }
            }

            if evaluations.is_empty() {
                break;
            }

            page += 1;
        }

        Ok(all_results)
    }
}

impl HttpReviewSource {
    pub fn fetch_evaluation_records(
        &self,
        query: &ReviewQuery,
    ) -> anyhow::Result<Vec<EvaluationRecord>> {
        let start_ts = Self::parse_timestamp(&query.time_window.start_at);
        let end_ts = Self::parse_timestamp(&query.time_window.end_at);

        let mut all_results = Vec::new();
        let mut page = 1;
        let mut max_pages = EVALUATION_MAX_PAGES;

        while page <= max_pages {
            let body = serde_json::json!({
                "orderId": "",
                "productId": "",
                "productEvaluationId": "",
                "buyerEvaluationTimeStart": start_ts,
                "buyerEvaluationTimeEnd": end_ts,
                "page": page,
                "status": 2,
                "visibleType": 0,
            });

            let resp = self.post_json_sync(&body)?;

            if resp.get("code").and_then(Value::as_i64) != Some(0) {
                anyhow::bail!("评价 API 错误：{}", resp);
            }

            let evaluations = resp
                .get("finderProductEvaluationInfoList")
                .and_then(Value::as_array)
                .cloned()
                .unwrap_or_default();

            if let Some(total) = resp.get("totalCnt").and_then(Value::as_i64) {
                let total = total.max(0) as usize;
                let total_pages = total.div_ceil(EVALUATION_PAGE_SIZE);
                max_pages = EVALUATION_MAX_PAGES.min(total_pages.max(1));
            }

            for eval in &evaluations {
                if let Some(record) = parse_evaluation_record(eval) {
                    all_results.push(record);
                }
            }

            if evaluations.is_empty() {
                break;
            }

            page += 1;
        }

        Ok(all_results)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::Utc;
    use std::sync::atomic::{AtomicU32, Ordering};
    use std::sync::{Arc, Mutex};

    #[test]
    fn temporary_review_statuses_are_retryable() {
        assert!(is_temporary_review_status(408));
        assert!(is_temporary_review_status(500));
        assert!(is_temporary_review_status(502));
        assert!(is_temporary_review_status(503));
        assert!(is_temporary_review_status(504));
        assert!(!is_temporary_review_status(200));
        assert!(!is_temporary_review_status(429));
        assert!(!is_temporary_review_status(430));
    }

    #[tokio::test]
    async fn review_retry_request_succeeds_after_rate_limit_and_temporary_failure() {
        let call_count = Arc::new(AtomicU32::new(0));
        let wait_log = Arc::new(Mutex::new(Vec::<(u64, ReviewRetryReason)>::new()));
        let cc = call_count.clone();
        let wait_log_inner = wait_log.clone();

        let result = retry_review_request_with_sleep(
            move || {
                let cc = cc.clone();
                async move {
                    let n = cc.fetch_add(1, Ordering::SeqCst);
                    match n {
                        0 => Ok(ReviewRequestOutcome::Retry(
                            ReviewRetryReason::RateLimited { api_level: true },
                        )),
                        1 => Ok(ReviewRequestOutcome::Retry(
                            ReviewRetryReason::TemporaryFailure,
                        )),
                        _ => Ok(ReviewRequestOutcome::Ready(serde_json::json!({"code": 0}))),
                    }
                }
            },
            move |wait_secs, reason| {
                let wait_log = wait_log_inner.clone();
                async move {
                    wait_log.lock().unwrap().push((wait_secs, reason));
                }
            },
        )
        .await
        .expect("should eventually succeed");

        assert_eq!(
            result.get("code").and_then(serde_json::Value::as_i64),
            Some(0)
        );
        assert_eq!(call_count.load(Ordering::SeqCst), 3);
        assert_eq!(
            *wait_log.lock().unwrap(),
            vec![
                (2, ReviewRetryReason::RateLimited { api_level: true }),
                (4, ReviewRetryReason::TemporaryFailure),
            ]
        );
    }

    #[tokio::test]
    async fn review_retry_request_exhausts_after_repeated_rate_limits() {
        let wait_log = Arc::new(Mutex::new(Vec::<(u64, ReviewRetryReason)>::new()));
        let wait_log_inner = wait_log.clone();
        let result = retry_review_request_with_sleep(
            || async {
                Ok::<_, anyhow::Error>(ReviewRequestOutcome::<serde_json::Value>::Retry(
                    ReviewRetryReason::RateLimited { api_level: false },
                ))
            },
            move |wait_secs, reason| {
                let wait_log = wait_log_inner.clone();
                async move {
                    wait_log.lock().unwrap().push((wait_secs, reason));
                }
            },
        )
        .await;

        let err = result.expect_err("should exhaust retry budget");
        assert!(
            err.to_string().contains("频率限制"),
            "unexpected error: {err}"
        );
        assert_eq!(
            *wait_log.lock().unwrap(),
            vec![
                (2, ReviewRetryReason::RateLimited { api_level: false }),
                (4, ReviewRetryReason::RateLimited { api_level: false }),
                (8, ReviewRetryReason::RateLimited { api_level: false }),
            ]
        );
    }

    #[test]
    fn parses_review_details_from_api_payload() {
        let item = serde_json::json!({
            "productEvaluationId": "eval-1",
            "orderId": "order-1",
            "productInfo": {
                "productId": "p-100",
                "skuId": "sku-9",
                "skuName": "红色 / XL",
                "spuName": "春装外套"
            },
            "evaluationInfo": {
                "buyer": { "identity": { "nickname": "买家小王" } },
                "firstEvaluationInfo": {
                    "buyerEvaluationInfo": {
                        "content": "尺码偏小，物流慢"
                    }
                }
            },
            "operationInfo": {
                "attitudeName": "不够好",
                "canReplyExpireTime": chrono::Utc::now().timestamp() + 86_400
            }
        });

        let parsed = parse_review_record(&item).expect("should parse");
        assert_eq!(parsed.evaluation_id, "eval-1");
        assert_eq!(parsed.order_id, "order-1");
        assert_eq!(parsed.buyer_nickname, "买家小王");
        assert_eq!(parsed.evaluation_content, "尺码偏小，物流慢");
        assert_eq!(parsed.product_id, "p-100");
        assert_eq!(parsed.sku_id, "sku-9");
        assert_eq!(parsed.sku_name, "红色 / XL");
        assert_eq!(parsed.product_name, "春装外套");
    }

    #[test]
    fn parses_evaluation_record_for_scored_matching() {
        let item = serde_json::json!({
            "productEvaluationId": "eval-2",
            "productInfo": {
                "productId": "p-100",
                "skuId": "sku-9",
                "skuName": "红色 / XL",
                "spuName": "春装外套"
            },
            "evaluationInfo": {
                "evaluationStar": 1,
                "buyer": { "identity": { "nickname": "买家小王" } },
                "firstEvaluationInfo": {
                    "buyerEvaluationInfo": {
                        "content": "尺码偏小，物流慢",
                        "defaultContent": "系统默认评价",
                        "createTime": 1776324243
                    }
                }
            },
            "operationInfo": {
                "attitudeName": "不够好",
                "canReplyExpireTime": 1776924243
            }
        });

        let parsed = parse_evaluation_record(&item).expect("evaluation record");
        assert_eq!(parsed.evaluation_id, "eval-2");
        assert_eq!(parsed.buyer_nickname, "买家小王");
        assert_eq!(parsed.product_id, "p-100");
        assert_eq!(parsed.sku_id, "sku-9");
        assert_eq!(parsed.sku_name, "红色 / XL");
        assert_eq!(parsed.product_name, "春装外套");
        assert_eq!(parsed.eval_time, 1776324243);
        assert_eq!(parsed.attitude_name, "不够好");
        assert_eq!(parsed.evaluation_content, "尺码偏小，物流慢");
        assert_eq!(parsed.default_content, "系统默认评价");
        assert_eq!(parsed.can_reply_expire_time, 1776924243);
    }

    #[test]
    fn keeps_reviews_even_when_reply_window_is_missing_or_expired() {
        let now = Utc::now().timestamp();
        let expired = serde_json::json!({
            "productEvaluationId": "eval-expired",
            "productInfo": {
                "productId": "p-100",
                "skuId": "sku-9",
                "skuName": "红色 / XL",
                "spuName": "春装外套"
            },
            "evaluationInfo": {
                "evaluationStar": 1,
                "buyer": { "identity": { "nickname": "买家小王" } },
                "firstEvaluationInfo": {
                    "buyerEvaluationInfo": {
                        "content": "尺码偏小，物流慢",
                        "defaultContent": "系统默认评价",
                        "createTime": 1776324243
                    }
                }
            },
            "operationInfo": {
                "attitudeName": "不够好",
                "canReplyExpireTime": now - 45 * 86_400
            }
        });
        let missing = serde_json::json!({
            "productEvaluationId": "eval-missing",
            "productInfo": {
                "productId": "p-100",
                "skuId": "sku-9",
                "skuName": "红色 / XL",
                "spuName": "春装外套"
            },
            "evaluationInfo": {
                "evaluationStar": 1,
                "buyer": { "identity": { "nickname": "买家小王" } },
                "firstEvaluationInfo": {
                    "buyerEvaluationInfo": {
                        "content": "尺码偏小，物流慢",
                        "defaultContent": "系统默认评价",
                        "createTime": 1776324243
                    }
                }
            },
            "operationInfo": {
                "attitudeName": "不够好"
            }
        });

        assert!(parse_evaluation_record(&expired).is_some());
        assert!(parse_evaluation_record(&missing).is_some());
    }
}
