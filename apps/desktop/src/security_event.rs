//! 客户端安全事件结构化日志。
//!
//! 用 `tracing::warn!(target: "security", event = ..., ...)` 通道输出。
//! `init_tracing()` 配置 subscriber 的 jsonl layer，把 target = "security"
//! 的事件单独写到 `<config_dir>/TLS-shipinhao/security-events.jsonl`，
//! 与控制台 RUST_LOG 路径互不干扰。
//!
//! ## 设计取舍：仅本地落盘，不主动上报
//!
//! 客户端**不会**把事件主动 POST 到 Worker，原因：
//! - 增加客户端→服务端的写依赖会扩大攻击面
//! - Worker `AuditEvent` 表已记录服务端动作，客户端事件如果走 D1 会爆量
//! - 隐私可控：用户可通过"导出诊断包"在排查时主动提交给客服
//!
//! 未来若要走主动上报，新增 `POST /api/security/event` Worker 端点 + 客户端
//! 后台队列即可，本模块的事件类型与字段约定可直接复用。
//!
//! ## 7 类必须采集的事件
//!
//! 来源：`docs/security/授权链路安全审查与整改建议.md` § 4.8。
//!
//! | Kind | 触发位点 |
//! |---|---|
//! | `MaterialCorrupt` | lease store 解码失败、文件长度异常等 |
//! | `LeaseVerifyFailed` | Lease 验签失败（签名/kind/时间） |
//! | `DeviceMismatch` | Lease 绑定的 device_id 与本机不符 |
//! | `LeaseRefreshFailed` | 续约调 Worker 失败（网络 / 服务端拒绝） |
//! | `GrantFailed` | 任务级 Grant 本地与远端都失败 |
//! | `IntegrityFailed` | 完整性 manifest 校验失败 |
//! | `AuthBurstAnomaly` | 短时间多次激活 / 校验失败 |

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SecurityEventKind {
    MaterialCorrupt,
    LeaseVerifyFailed,
    DeviceMismatch,
    LeaseRefreshFailed,
    GrantFailed,
    IntegrityFailed,
    AuthBurstAnomaly,
}

impl SecurityEventKind {
    /// 序列化字符串，作为 jsonl 中 `event` 字段的稳定值。
    /// **绝不可改名**，下游运营会基于该字段做事件分类与告警。
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::MaterialCorrupt => "material_corrupt",
            Self::LeaseVerifyFailed => "lease_verify_failed",
            Self::DeviceMismatch => "device_mismatch",
            Self::LeaseRefreshFailed => "lease_refresh_failed",
            Self::GrantFailed => "grant_failed",
            Self::IntegrityFailed => "integrity_failed",
            Self::AuthBurstAnomaly => "auth_burst_anomaly",
        }
    }
}

/// emit 一条安全事件，仅含 reason。
///
/// `reason` 是简短字符串，用于事件聚合分类（不要塞 PII 或大段 stacktrace）。
pub fn emit(kind: SecurityEventKind, reason: &str) {
    tracing::warn!(
        target: "security",
        event = kind.as_str(),
        reason = reason,
    );
}

/// emit 一条带 detail 字段的安全事件。
///
/// `detail` 用于附加排查所需的上下文，例如 `lease_kind=other / exp=1234`，
/// 仍要避免敏感信息（卡密原文、Lease Token 完整字符串等）。
pub fn emit_with_detail(kind: SecurityEventKind, reason: &str, detail: &str) {
    tracing::warn!(
        target: "security",
        event = kind.as_str(),
        reason = reason,
        detail = detail,
    );
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashSet;

    #[test]
    fn kind_str_values_are_stable() {
        assert_eq!(
            SecurityEventKind::MaterialCorrupt.as_str(),
            "material_corrupt"
        );
        assert_eq!(
            SecurityEventKind::LeaseVerifyFailed.as_str(),
            "lease_verify_failed"
        );
        assert_eq!(
            SecurityEventKind::DeviceMismatch.as_str(),
            "device_mismatch"
        );
        assert_eq!(
            SecurityEventKind::LeaseRefreshFailed.as_str(),
            "lease_refresh_failed"
        );
        assert_eq!(SecurityEventKind::GrantFailed.as_str(), "grant_failed");
        assert_eq!(
            SecurityEventKind::IntegrityFailed.as_str(),
            "integrity_failed"
        );
        assert_eq!(
            SecurityEventKind::AuthBurstAnomaly.as_str(),
            "auth_burst_anomaly"
        );
    }

    #[test]
    fn kind_str_values_are_distinct_and_complete() {
        let all = [
            SecurityEventKind::MaterialCorrupt,
            SecurityEventKind::LeaseVerifyFailed,
            SecurityEventKind::DeviceMismatch,
            SecurityEventKind::LeaseRefreshFailed,
            SecurityEventKind::GrantFailed,
            SecurityEventKind::IntegrityFailed,
            SecurityEventKind::AuthBurstAnomaly,
        ];
        let strs: HashSet<&'static str> = all.iter().map(|k| k.as_str()).collect();
        assert_eq!(strs.len(), 7, "字符串值必须互异且共 7 个");
    }
}
