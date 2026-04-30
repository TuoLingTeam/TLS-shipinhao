use serde::Serialize;

#[derive(Debug, thiserror::Error)]
pub enum AppError {
    #[error("{0}")]
    Domain(#[from] desktop::domain::DomainError),

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
/// - 替换以 SQL 关键字开头的整条语句残片为 `<sql>`（防 rusqlite 等
///   把 `INSERT INTO orders (...) VALUES (...)` 顺带包进 anyhow 错误）
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
    let with_loc = strip_source_locations(&text);
    let with_sql = strip_sql_statements(&with_loc);
    let mut sanitized = with_sql;
    if sanitized.chars().count() > 280 {
        let truncated: String = sanitized.chars().take(280).collect();
        sanitized = format!("{truncated}…");
    }
    sanitized
}

/// 把 SQL 语句残片整体替换为 `<sql>`。
///
/// 仅在以下任一开头作为「语句起点」识别（不区分大小写）：
/// `SELECT ` / `INSERT INTO ` / `UPDATE ` / `DELETE FROM ` / `CREATE TABLE `
/// / `ALTER TABLE ` / `DROP TABLE `
///
/// 起点要求：要么是字符串首位，要么前面是空白 / `(` / `:`，避免误伤业务名词
/// 中包含「Update」等子串的描述（这些不会以独立 token 形式紧跟空格）。
/// 终点：到下一个 `;` 或换行或全文结束为止；这是保守做法，对单语句错误最适用。
fn strip_sql_statements(input: &str) -> String {
    const KEYWORDS: &[&str] = &[
        "SELECT ",
        "INSERT INTO ",
        "UPDATE ",
        "DELETE FROM ",
        "CREATE TABLE ",
        "ALTER TABLE ",
        "DROP TABLE ",
    ];
    let lower = input.to_ascii_lowercase();
    let mut out = String::with_capacity(input.len());
    // 用字节索引推进，但写出按 char 切片避免破坏 UTF-8。
    let mut i = 0;
    while i < input.len() {
        let mut matched_kw_len: Option<usize> = None;
        for kw in KEYWORDS {
            let kw_lower = kw.to_ascii_lowercase();
            if lower[i..].starts_with(&kw_lower) {
                let start_ok = i == 0
                    || matches!(
                        input.as_bytes()[i - 1],
                        b' ' | b'\t' | b'(' | b':' | b'\n' | b'\r'
                    );
                if start_ok {
                    matched_kw_len = Some(kw.len());
                    break;
                }
            }
        }
        if matched_kw_len.is_some() {
            out.push_str("<sql>");
            let mut j = i;
            while j < input.len() {
                let b = input.as_bytes()[j];
                if b == b';' || b == b'\n' {
                    break;
                }
                j += 1;
            }
            i = j;
            continue;
        }
        // 取从 i 开始的下一个完整 char（按 UTF-8 边界推进）
        let next_char = input[i..].chars().next();
        match next_char {
            Some(ch) => {
                out.push(ch);
                i += ch.len_utf8();
            }
            None => break,
        }
    }
    out
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
    fn internal_variant_strips_sql_statement_residue() {
        let error: AppError = anyhow::anyhow!(
            "rusqlite execute failed: INSERT INTO orders (id, payload) VALUES (1, 'top secret')"
        )
        .into();
        let json = serde_json::to_string(&error).unwrap();
        assert!(json.contains("<sql>"), "应替换 SQL 残片为 <sql>: {json}");
        assert!(!json.contains("INSERT INTO"), "原 SQL 不应出现：{json}");
        assert!(!json.contains("orders (id"), "SQL 字段不应出现：{json}");
    }

    #[test]
    fn internal_variant_does_not_strip_select_in_business_text() {
        // "Selectable" 出现在 sentence 中且不是 SQL 起点（不带空格 token 边界）
        // 不应被误判为 SQL。
        let error: AppError = anyhow::anyhow!("Selectable rows are zero, please retry").into();
        let json = serde_json::to_string(&error).unwrap();
        assert!(
            !json.contains("<sql>"),
            "Selectable 不应触发 SQL 脱敏：{json}"
        );
        assert!(json.contains("Selectable"));
    }

    #[test]
    fn domain_variant_serializes_as_domain_error_display() {
        let error: AppError = desktop::domain::DomainError::UnsupportedTaskKind.into();
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
            desktop::domain::DomainError::UnsupportedTaskKind.into(),
        ] {
            let display = error.to_string();
            let json = serde_json::to_string(&error).unwrap();
            assert_eq!(json, format!("\"{display}\""));
        }
    }
}
