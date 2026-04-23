use crate::adapters::common::{build_client, build_weixin_shop_headers};
use anyhow::Context;
use reqwest::header::{HeaderMap, HeaderValue, ACCEPT};
use serde_json::Value;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StoreIdentity {
    pub store_id: String,
    pub store_name: String,
}

fn store_profile_url() -> String {
    obfstr::obfstr!("https://store.weixin.qq.com/shop/setting/profile").to_string()
}

pub struct HttpStoreProfileClient {
    cookie_header: String,
    biz_magic: String,
    client: reqwest::Client,
}

impl HttpStoreProfileClient {
    pub fn new(cookie_header: String, biz_magic: String) -> Self {
        Self {
            cookie_header,
            biz_magic,
            client: build_client(),
        }
    }

    fn build_headers(&self) -> HeaderMap {
        let mut headers = build_weixin_shop_headers(
            &store_profile_url(),
            &self.cookie_header,
            &self.biz_magic,
            None,
        );
        headers.insert(
            ACCEPT,
            HeaderValue::from_static(
                "text/html,application/xhtml+xml,application/xml;q=0.9,*/*;q=0.8",
            ),
        );
        headers
    }

    pub async fn fetch_store_identity(&self) -> anyhow::Result<StoreIdentity> {
        let response = self
            .client
            .get(store_profile_url())
            .headers(self.build_headers())
            .send()
            .await
            .context("request profile page")?
            .error_for_status()
            .context("profile page returned error status")?
            .text()
            .await
            .context("read profile page body")?;
        extract_store_identity_from_html(&response)
    }
}

pub fn extract_store_identity_from_html(html: &str) -> anyhow::Result<StoreIdentity> {
    let json_text = extract_assigned_json_object(html, "window.__INITIAL_PINIA_DATA__=")
        .context("missing window.__INITIAL_PINIA_DATA__ assignment")?;
    let payload: Value = serde_json::from_str(json_text).context("parse pinia bootstrap json")?;
    let user_info = payload
        .pointer("/baseStore/initialState/userInfo")
        .context("missing userInfo in pinia bootstrap json")?;
    let store_id = user_info
        .get("appid")
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .context("missing userInfo.appid")?;
    let store_name = user_info
        .get("nickName")
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .context("missing userInfo.nickName")?;
    Ok(StoreIdentity {
        store_id: store_id.to_string(),
        store_name: store_name.to_string(),
    })
}

fn extract_assigned_json_object<'a>(source: &'a str, marker: &str) -> Option<&'a str> {
    let marker_index = source.find(marker)?;
    let mut start = marker_index + marker.len();
    while let Some(ch) = source[start..].chars().next() {
        if ch == '{' {
            break;
        }
        start += ch.len_utf8();
    }
    if source[start..].chars().next()? != '{' {
        return None;
    }

    let mut depth = 0usize;
    let mut in_string = false;
    let mut escaped = false;
    for (offset, ch) in source[start..].char_indices() {
        if in_string {
            if escaped {
                escaped = false;
                continue;
            }
            match ch {
                '\\' => escaped = true,
                '"' => in_string = false,
                _ => {}
            }
            continue;
        }

        match ch {
            '"' => in_string = true,
            '{' => depth += 1,
            '}' => {
                depth = depth.saturating_sub(1);
                if depth == 0 {
                    return Some(&source[start..start + offset + ch.len_utf8()]);
                }
            }
            _ => {}
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn extracts_store_identity_from_ssr_bootstrap() {
        let html = r#"
            <script>
              window.__INITIAL_PINIA_DATA__={"baseStore":{"initialState":{"userInfo":{"appid":"wx61f28d69d9174ddf","nickName":"精选内衣店"}}}};
              window.__USE_VITE__=false;
            </script>
        "#;

        let identity = extract_store_identity_from_html(html).unwrap();
        assert_eq!(identity.store_id, "wx61f28d69d9174ddf");
        assert_eq!(identity.store_name, "精选内衣店");
    }

    #[test]
    fn extracts_json_object_even_with_nested_quotes() {
        let html = r#"window.__INITIAL_PINIA_DATA__={"baseStore":{"initialState":{"userInfo":{"appid":"wx1","nickName":"名\"字"}}}};window.__USE_VITE__=false;"#;
        let raw = extract_assigned_json_object(html, "window.__INITIAL_PINIA_DATA__=").unwrap();
        let payload: Value = serde_json::from_str(raw).unwrap();
        assert_eq!(
            payload.pointer("/baseStore/initialState/userInfo/appid"),
            Some(&Value::String("wx1".into()))
        );
    }
}
