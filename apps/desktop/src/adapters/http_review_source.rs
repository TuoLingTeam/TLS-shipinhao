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

fn parse_review_record(eval: &Value) -> Option<OrderMatchResult> {
    let op_info = eval.get("operationInfo")?;
    let attitude = op_info.get("attitudeName").and_then(Value::as_str).unwrap_or("");
    if attitude != "不够好" {
        return None;
    }

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

    let order_id = eval
        .get("orderId")
        .and_then(Value::as_str)
        .unwrap_or("")
        .trim()
        .to_string();
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
    let product_id = first_non_empty_string(&product_info, &["productId", "product_id", "spuId", "spu_id"]);
    let sku_id = first_non_empty_string(&product_info, &["skuId", "sku_id"]);
    let sku_name = first_non_empty_string(&product_info, &["skuName", "saleParam", "sale_param", "specName", "spec"]);
    let product_name =
        first_non_empty_string(&product_info, &["spuName", "title", "productName", "name"]);

    Some(OrderMatchResult {
        evaluation_id,
        order_id,
        buyer_nickname,
        evaluation_content,
        product_id,
        sku_id,
        sku_name,
        product_name,
        matched: true,
        source: MatchSource::ExactOrderId,
        confidence_score: 100,
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
                let total_pages = (total + EVALUATION_PAGE_SIZE - 1) / EVALUATION_PAGE_SIZE;
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

            if page <= max_pages {
                std::thread::sleep(std::time::Duration::from_millis(500));
            }
        }

        Ok(all_results)
    }
}


#[cfg(test)]
mod tests {
    use super::*;

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
                "attitudeName": "不够好"
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
}
