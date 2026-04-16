use desktop_services::ReviewSource;
use desktop_services::ReviewQuery;
use domain_core::OrderMatchResult;

pub struct HttpReviewSource {
    pub base_url: String,
    pub cookie_header: String,
}

impl HttpReviewSource {
    pub fn new(base_url: String, cookie_header: String) -> Self {
        Self { base_url, cookie_header }
    }
}

impl ReviewSource for HttpReviewSource {
    fn fetch_reviews(&self, _query: &ReviewQuery) -> anyhow::Result<Vec<OrderMatchResult>> {
        // TODO: 接入视频号评价 API
        // 使用 self.cookie_header 作为认证，通过 self.base_url 发送请求
        Ok(vec![])
    }
}
