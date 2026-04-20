//! 桌面端 HTTP adapter 的公共组件。
//!
//! 原本 `http_review_source` / `http_quality_refund_source` / `http_delivery_gateway`
//! / `http_order_search` 各自复制了一份 `build_headers` + `REQUEST_TIMEOUT_SECS`
//! + `build_desktop_http_client(...)`，唯一差异只是 `Referer`。平台一改头字段，
//! 就要同步改 4 份且容易漏。这里统一出来：
//!
//! - [`REQUEST_TIMEOUT_SECS`]：桌面端出站 HTTP 的统一超时
//! - [`build_client`]：封装 `desktop_services::http_client::build_desktop_http_client`
//! - [`build_weixin_shop_headers`]：微信小店业务出站请求的公共 HeaderMap，`Referer` 由调用方传入

use reqwest::header::{
    HeaderMap, HeaderName, HeaderValue, CONTENT_TYPE, COOKIE, ORIGIN, REFERER, USER_AGENT,
};
use std::time::Duration;

/// 所有桌面端业务出站 HTTP 请求的统一超时（秒）。
pub(crate) const REQUEST_TIMEOUT_SECS: u64 = 30;

/// 构造默认的出站 `reqwest::Client`（含 UA 策略、连接复用等）。
pub(crate) fn build_client() -> reqwest::Client {
    desktop_services::http_client::build_desktop_http_client(Duration::from_secs(
        REQUEST_TIMEOUT_SECS,
    ))
}

/// 构造微信小店业务出站请求的公共 `HeaderMap`。
///
/// - 固定头：`Content-Type` / `Origin` / `User-Agent` / `potter-scene` / `sec-ch-ua-platform`
/// - 可变头：`Referer`（来自调用方的 URL 上下文）、`Cookie` / `biz_magic` / `x-grant-id`
pub(crate) fn build_weixin_shop_headers(
    referer: &'static str,
    cookie_header: &str,
    biz_magic: &str,
    grant_id: Option<&str>,
) -> HeaderMap {
    let mut headers = HeaderMap::new();
    headers.insert(CONTENT_TYPE, HeaderValue::from_static("application/json"));
    headers.insert(
        ORIGIN,
        HeaderValue::from_static("https://store.weixin.qq.com"),
    );
    headers.insert(REFERER, HeaderValue::from_static(referer));
    headers.insert(
        USER_AGENT,
        HeaderValue::from_static(security_core::http_headers::get_user_agent()),
    );
    if let Ok(v) = HeaderValue::from_str(cookie_header) {
        headers.insert(COOKIE, v);
    }
    if let Ok(v) = HeaderValue::from_str(biz_magic) {
        headers.insert(HeaderName::from_static("biz_magic"), v);
    }
    if let Some(grant_id) = grant_id {
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
