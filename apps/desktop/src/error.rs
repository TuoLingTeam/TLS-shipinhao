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
        serializer.serialize_str(&self.to_string())
    }
}

impl From<String> for AppError {
    fn from(msg: String) -> Self {
        Self::Message(msg)
    }
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
    fn internal_variant_serializes_as_anyhow_message() {
        let error: AppError = anyhow::anyhow!("更新物流信息失败").into();
        assert!(matches!(error, AppError::Internal(_)));
        let json = serde_json::to_string(&error).unwrap();
        assert_eq!(json, "\"更新物流信息失败\"");
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
    fn display_is_stable_across_variants_and_matches_serialization() {
        // Serialize 实现固定走 `self.to_string()`；断言 Display 与 JSON 字面量一致，
        // 防止有人日后把 Serialize 改成 serde 派生，破坏前端字符串契约。
        let cases: Vec<AppError> = vec![
            AppError::Message("M".into()),
            anyhow::anyhow!("anyhow").into(),
            domain_core::DomainError::UnsupportedTaskKind.into(),
        ];
        for error in cases {
            let display = error.to_string();
            let json = serde_json::to_string(&error).unwrap();
            assert_eq!(json, format!("\"{display}\""));
        }
    }
}
