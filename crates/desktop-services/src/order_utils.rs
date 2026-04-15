use regex::Regex;
use std::sync::OnceLock;

pub fn first_non_empty<'a>(data: &'a serde_json::Map<String, serde_json::Value>, keys: &[&str]) -> serde_json::Value {
    for key in keys {
        if let Some(value) = data.get(*key) {
            match value {
                serde_json::Value::String(text) => {
                    let trimmed = text.trim();
                    if !trimmed.is_empty() {
                        return serde_json::Value::String(trimmed.to_string());
                    }
                }
                serde_json::Value::Null => {}
                serde_json::Value::Array(items) if items.is_empty() => {}
                serde_json::Value::Object(map) if map.is_empty() => {}
                other => return other.clone(),
            }
        }
    }
    serde_json::Value::String(String::new())
}

pub fn normalize_sale_param(raw_value: &serde_json::Value) -> String {
    match raw_value {
        serde_json::Value::Array(items) => items
            .iter()
            .filter_map(|item| {
                let text = item.as_str().map(str::to_string).unwrap_or_else(|| item.to_string());
                let trimmed = text.trim().to_string();
                (!trimmed.is_empty()).then_some(trimmed)
            })
            .collect::<Vec<_>>()
            .join("|"),
        serde_json::Value::Null => String::new(),
        other => stringify_value(other).trim().to_string(),
    }
}

pub fn parse_confirm_receipt_timestamp(value: &serde_json::Value) -> i64 {
    match value {
        serde_json::Value::Null => 0,
        _ => {
            let text = stringify_value(value);
            if text.chars().all(|ch| ch.is_ascii_digit()) {
                text.parse::<i64>().unwrap_or(0)
            } else {
                0
            }
        }
    }
}

pub fn parse_timestamp(value: &serde_json::Value) -> i64 {
    match value {
        serde_json::Value::Null => 0,
        serde_json::Value::String(text) if text.trim().is_empty() => 0,
        _ => {
            let text = stringify_value(value);
            let trimmed = text.trim();
            if !trimmed.chars().all(|ch| ch.is_ascii_digit()) {
                return 0;
            }
            let mut parsed = trimmed.parse::<i64>().unwrap_or(0);
            if parsed > 10_i64.pow(12) {
                parsed /= 1000;
            }
            parsed
        }
    }
}

pub fn normalize_product_text(value: &str) -> String {
    static RE: OnceLock<Regex> = OnceLock::new();
    let re = RE.get_or_init(|| Regex::new(r"[\s\-_/|,，、]+").expect("valid regex"));
    re.replace_all(&value.trim().to_lowercase(), "").into_owned()
}

pub fn split_sku_tokens(value: &str) -> Vec<String> {
    static RE: OnceLock<Regex> = OnceLock::new();
    let re = RE.get_or_init(|| Regex::new(r"[|/,，、]+").expect("valid regex"));
    re.split(value)
        .map(str::trim)
        .filter(|token| !token.is_empty())
        .map(str::to_string)
        .collect()
}

fn stringify_value(value: &serde_json::Value) -> String {
    match value {
        serde_json::Value::String(text) => text.trim().to_string(),
        other => other.to_string(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn returns_first_non_empty_string_or_value() {
        let data = json!({"buyer": "", "fallback": " 张三 ", "count": 1});
        let map = data.as_object().unwrap();
        assert_eq!(first_non_empty(map, &["buyer", "fallback"]), json!("张三"));
        assert_eq!(first_non_empty(map, &["missing", "count"]), json!(1));
    }

    #[test]
    fn normalizes_sale_params_and_timestamps() {
        assert_eq!(normalize_sale_param(&json!(["红色", "  XL  ", ""])), "红色|XL");
        assert_eq!(parse_confirm_receipt_timestamp(&json!("1712910000")), 1712910000);
        assert_eq!(parse_timestamp(&json!(1712910000123i64)), 1712910000);
        assert_eq!(parse_timestamp(&json!("abc")), 0);
    }

    #[test]
    fn normalizes_product_text_and_sku_tokens() {
        assert_eq!(normalize_product_text("  洗发水 / 清爽款-大瓶 "), "洗发水清爽款大瓶");
        assert_eq!(split_sku_tokens("红色|XL,经典款"), vec!["红色", "XL", "经典款"]);
    }
}
