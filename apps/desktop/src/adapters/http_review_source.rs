use desktop_services::ReviewQuery;
use desktop_services::ReviewSource;
use domain_core::{MatchSource, OrderMatchResult};
use serde_json::Value;

const EVALUATION_SEARCH_URL: &str =
    "https://store.weixin.qq.com/shop-faas/mmchannelstradeevaluation/cgi/search";
const EVALUATION_REFERER: &str = "https://store.weixin.qq.com/shop/evaluate/home";
const EVALUATION_PAGE_SIZE: usize = 20;
const EVALUATION_MAX_PAGES: usize = 50;
const REQUEST_TIMEOUT_SECS: u64 = 30;

pub struct HttpReviewSource {
    cookie_header: String,
    biz_magic: String,
    client: reqwest::Client,
}

impl HttpReviewSource {
    pub fn new(cookie_header: String, biz_magic: String) -> Self {
        let client = reqwest::Client::builder()
            .timeout(std::time::Duration::from_secs(REQUEST_TIMEOUT_SECS))
            .build()
            .unwrap_or_default();
        Self { cookie_header, biz_magic, client }
    }

    fn build_headers(&self) -> reqwest::header::HeaderMap {
        use reqwest::header::{HeaderMap, HeaderName, HeaderValue, CONTENT_TYPE, COOKIE, ORIGIN, REFERER, USER_AGENT};
        let mut headers = HeaderMap::new();
        headers.insert(CONTENT_TYPE, HeaderValue::from_static("application/json"));
        headers.insert(ORIGIN, HeaderValue::from_static("https://store.weixin.qq.com"));
        headers.insert(REFERER, HeaderValue::from_static(EVALUATION_REFERER));
        headers.insert(USER_AGENT, HeaderValue::from_static(
            "Mozilla/5.0 (Macintosh; Intel Mac OS X 10_15_7) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/144.0.0.0 Safari/537.36"
        ));
        if let Ok(v) = HeaderValue::from_str(&self.cookie_header) {
            headers.insert(COOKIE, v);
        }
        if let Ok(v) = HeaderValue::from_str(&self.biz_magic) {
            headers.insert(HeaderName::from_static("biz_magic"), v);
        }
        headers.insert(HeaderName::from_static("potter-scene"), HeaderValue::from_static("weixinShop"));
        headers.insert(HeaderName::from_static("sec-ch-ua-platform"), HeaderValue::from_static("\"macOS\""));
        headers
    }

    fn post_json_sync(&self, body: &Value) -> anyhow::Result<Value> {
        let rt = tokio::runtime::Handle::current();
        let headers = self.build_headers();
        let client = self.client.clone();
        let url = format!("{}?token=&lang=zh_CN", EVALUATION_SEARCH_URL);
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

    fn parse_timestamp(ts_str: &str) -> i64 {
        let d = chrono::NaiveDate::parse_from_str(ts_str.get(..10).unwrap_or(ts_str), "%Y-%m-%d")
            .unwrap_or_default();
        d.and_hms_opt(0, 0, 0)
            .map(|dt| dt.and_utc().timestamp())
            .unwrap_or(0)
    }
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
                let total_pages = (total + EVALUATION_PAGE_SIZE - 1) / EVALUATION_PAGE_SIZE;
                max_pages = EVALUATION_MAX_PAGES.min(total_pages.max(1));
            }

            for eval in &evaluations {
                let op_info = eval.get("operationInfo").cloned().unwrap_or_default();
                let attitude = op_info.get("attitudeName").and_then(Value::as_str).unwrap_or("");
                if attitude != "不够好" {
                    continue;
                }

                let eval_id = eval
                    .get("productEvaluationId")
                    .and_then(Value::as_str)
                    .unwrap_or("")
                    .to_string();
                let order_id = eval
                    .get("orderId")
                    .and_then(Value::as_str)
                    .unwrap_or("")
                    .to_string();

                all_results.push(OrderMatchResult {
                    evaluation_id: eval_id,
                    order_id,
                    matched: true,
                    source: MatchSource::ExactOrderId,
                    confidence_score: 100,
                });
            }

            if evaluations.is_empty() {
                break;
            }

            page += 1;

            if page <= max_pages {
                std::thread::sleep(std::time::Duration::from_millis(500));
            }
        }

        Ok(all_results)
    }
}
