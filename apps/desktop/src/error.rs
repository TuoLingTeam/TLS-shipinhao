use serde::Serialize;

#[derive(Debug, thiserror::Error)]
pub enum AppError {
    #[error("{0}")]
    Domain(#[from] domain_core::DomainError),

    #[error("{0}")]
    Internal(#[from] anyhow::Error),

    #[error("{0}")]
    Message(String),
}

impl Serialize for AppError {
    fn serialize<S: serde::Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        // Internal 走 anyhow 的 ? 链，常常会把底层错误（含绝对路径、SQL、堆栈帧）
        // 顺手吞下；直接 to_string 给前端会泄露内部信息。其他变体都是明确手写的
        // 用户态文案，原样输出。
        let text = match self {
            AppError::Internal(err) => sanitize_user_message(err),
            AppError::Domain(_) | AppError::Message(_) => self.to_string(),
        };
        serializer.serialize_str(&text)
    }
}

impl From<String> for AppError {
    fn from(msg: String) -> Self {
        Self::Message(msg)
    }
}

/// 给前端渲染前对 Internal 错误做最小脱敏：
/// - 取 root cause 文本（避免暴露完整 anyhow context chain）
/// - 替换绝对路径前缀（`/Users/...` / `/home/...` / `C:\...`）为 `<path>`
/// - 替换 Rust 源码行号引用（`xxx.rs:NN`）为 `<loc>`
/// - 截断到 280 字符（足够说明，不再放整段堆栈）
///
/// 完整原文仍由 tracing 在调用方记录到日志，运维侧定位不受影响。
fn sanitize_user_message(err: &anyhow::Error) -> String {
    let raw = err.root_cause().to_string();
    let trimmed = if raw.is_empty() { err.to_string() } else { raw };
    let mut text = String::with_capacity(trimmed.len());
    let mut chars = trimmed.chars().peekable();
    while let Some(ch) = chars.next() {
        if (ch == '/' || ch == '\\') && looks_like_path_start(&text) {
            text.push_str("<path>");
            while let Some(&peek) = chars.peek() {
                if peek == ' ' || peek == ')' || peek == '"' {
                    break;
                }
                chars.next();
            }
            continue;
        }
        text.push(ch);
    }
    let mut sanitized = strip_source_locations(&text);
    if sanitized.chars().count() > 280 {
        let truncated: String = sanitized.chars().take(280).collect();
        sanitized = format!("{truncated}…");
    }
    sanitized
}

fn looks_like_path_start(prefix: &str) -> bool {
    if prefix.ends_with("/Users")
        || prefix.ends_with("/home")
        || prefix.ends_with("/var")
        || prefix.ends_with("/tmp")
        || prefix.ends_with("/private")
    {
        return true;
    }
    if prefix
        .chars()
        .last()
        .map(|c| c == ':' || c == '"' || c.is_whitespace())
        .unwrap_or(false)
        && (prefix.contains("C:") || prefix.contains("D:") || prefix.contains("E:"))
    {
        return true;
    }
    false
}

fn strip_source_locations(input: &str) -> String {
    // 把 `xxx.rs:123` / `xxx.rs:123:45` 形式的源码行号替换成 `<loc>`，
    // 避免逆向者从前端错误就能定位到源文件。
    let mut out = String::with_capacity(input.len());
    let bytes = input.as_bytes();
    let mut i = 0;
    while i < bytes.len() {
        let rest = &input[i..];
        if let Some(rs_idx) = rest.find(".rs:") {
            // 找 .rs: 前面回溯到非 ident 字符
            let abs_pos = i + rs_idx;
            let mut start = abs_pos;
            while start > 0 {
                let prev = bytes[start - 1];
                if !(prev.is_ascii_alphanumeric() || prev == b'_' || prev == b'/' || prev == b'\\')
                {
                    break;
                }
                start -= 1;
            }
            // 复制 i..start 原样
            out.push_str(&input[i..start]);
            // 跳过 file_name.rs:NN[:NN]
            let mut end = abs_pos + ".rs:".len();
            while end < bytes.len() && bytes[end].is_ascii_digit() {
                end += 1;
            }
            if end < bytes.len() && bytes[end] == b':' {
                end += 1;
                while end < bytes.len() && bytes[end].is_ascii_digit() {
                    end += 1;
                }
            }
            out.push_str("<loc>");
            i = end;
        } else {
            out.push_str(rest);
            break;
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn message_variant_serializes_as_plain_string() {
        let error = AppError::Message("请先在设置中配置 Cookie".to_string());
        let json = serde_json::to_string(&error).unwrap();
        assert_eq!(json, "\"请先在设置中配置 Cookie\"");
    }

    #[test]
    fn internal_variant_serializes_as_sanitized_root_cause() {
        // 无路径 / 无源码行号的简单 Internal 错误，脱敏后语义保持等价。
        let error: AppError = anyhow::anyhow!("更新物流信息失败").into();
        assert!(matches!(error, AppError::Internal(_)));
        let json = serde_json::to_string(&error).unwrap();
        assert_eq!(json, "\"更新物流信息失败\"");
    }

    #[test]
    fn internal_variant_strips_absolute_paths_from_serialized_message() {
        let error: AppError = anyhow::anyhow!("无法读取 /Users/alice/secret/license.json").into();
        let json = serde_json::to_string(&error).unwrap();
        assert!(json.contains("<path>"), "应替换绝对路径为 <path>: {json}");
        assert!(!json.contains("/Users/alice"), "原绝对路径不应出现：{json}");
    }

    #[test]
    fn internal_variant_strips_source_file_locations() {
        let error: AppError = anyhow::anyhow!("解析失败 src/cache_storage.rs:512:18").into();
        let json = serde_json::to_string(&error).unwrap();
        assert!(json.contains("<loc>"), "应替换源码行号为 <loc>: {json}");
        assert!(!json.contains(".rs:512"), "原行号不应出现：{json}");
    }

    #[test]
    fn internal_variant_truncates_excessively_long_message() {
        let long_msg = "X".repeat(500);
        let error: AppError = anyhow::anyhow!(long_msg).into();
        let json = serde_json::to_string(&error).unwrap();
        // JSON 字符串里包含两个 "（首尾），还有可能的省略号
        assert!(
            json.chars().count() <= 290,
            "应截断到 ~280 字符: {}",
            json.chars().count()
        );
    }

    #[test]
    fn domain_variant_serializes_as_domain_error_display() {
        let error: AppError = domain_core::DomainError::UnsupportedTaskKind.into();
        assert!(matches!(error, AppError::Domain(_)));
        let json = serde_json::to_string(&error).unwrap();
        assert_eq!(json, format!("\"{}\"", error));
        assert!(!json.is_empty() && !json.contains("null"));
    }

    #[test]
    fn from_string_maps_to_message_variant() {
        let error: AppError = "业务失败".to_string().into();
        match error {
            AppError::Message(msg) => assert_eq!(msg, "业务失败"),
            other => panic!("预期 Message，实际 {other:?}"),
        }
    }

    #[test]
    fn display_remains_full_for_logging_even_when_serialized_is_sanitized() {
        // Display 仍输出原 anyhow chain，让 tracing/日志层拿到全文用于运维排障；
        // Serialize 走 sanitize_user_message 把对前端的输出收紧。两者职责分离。
        let internal: AppError =
            anyhow::anyhow!("无法读取 /tmp/license.json：Permission denied").into();
        let display = internal.to_string();
        let json = serde_json::to_string(&internal).unwrap();

        assert!(
            display.contains("/tmp/license.json"),
            "Display 用于日志，应保留路径：{display}"
        );
        assert!(
            !json.contains("/tmp/license.json"),
            "Serialize 应已脱敏：{json}"
        );

        // Message 与 Domain 变体显示与序列化保持一致（无脱敏需要）。
        for error in [
            AppError::Message("M".into()),
            domain_core::DomainError::UnsupportedTaskKind.into(),
        ] {
            let display = error.to_string();
            let json = serde_json::to_string(&error).unwrap();
            assert_eq!(json, format!("\"{display}\""));
        }
    }
}
