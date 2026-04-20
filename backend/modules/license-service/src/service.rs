//! 授权协议常量与纯函数。
//!
//! 历史上本文件还持有一套同步 `LicenseService<R> + LicenseRepository` 的业务
//! 流水线（activate / verify 的 D1 兼容路径），但生产侧（Cloudflare Worker）
//! 早已切换到 `license-worker::runtime_*` 异步路径，桌面端也只依赖本模块的
//! Lease/Grant/常量，同步服务从未被任何生产代码调用。自 T-01 起彻底下线同步
//! 路径，只保留下列**协议层单一事实源**：
//!
//! - 常量：Lease 过期窗口、协议版本、签发者标识、默认任务白名单、公钥 base64url
//! - 纯函数：`issue_license_lease` —— 给定关键字段构造 `LicenseLease`
//!
//! 若日后需要在非 Worker 环境（例如管理端 Rust SDK）复用激活/校验流程，请优先
//! 把 `license-worker::runtime_*` 抽象成独立的 trait/helper 而不是重建同步分支。

use api_contracts::{LicenseLease, LicenseState, SUPPORTED_TASKS};

pub const LEASE_RENEWAL_HOURS: i64 = 24;
pub const LEASE_HARD_EXPIRY_HOURS: i64 = 72;
pub const LICENSE_PROTOCOL_VERSION: u32 = 3;
pub const ISSUER: &str = "tls-license-backend";

/// 任务级授权 Grant 的有效期（分钟）。与 Python 4.3.0 `LICENSE_RUNTIME_GRANT_MINUTES` 对齐。
pub const LICENSE_RUNTIME_GRANT_MINUTES: i64 = 30;

/// Worker 签发 Lease 使用的 Ed25519 公钥（base64url）。客户端用来验签。
///
/// 轮换这把密钥时必须同步更新此处与 Worker secret；旧客户端会因验签失败落入
/// `LicenseState::Invalid`，从而触发「请重新激活」流程。
pub const LICENSE_PUBLIC_KEY_B64: &str = "1IS6t6PdHin8DEX9fy3s5oUfXs__QqGfN_T1o4PyQSo";

/// 默认签发给 Lease 的任务白名单。与 `api_contracts::SUPPORTED_TASKS` 同值（单一事实源），
/// 新增任务类型时只需在 `api-contracts` 一处扩展 `SUPPORTED_TASKS`。
pub const DEFAULT_TASK_POLICY: &[&str] = SUPPORTED_TASKS;

/// 按给定字段构造一个 `LicenseLease` 结构（未签名的 UI 展示层）。
///
/// Worker 端会拿这个结构转成 `LeasePayload` 再做 Ed25519 签名；客户端验签后
/// 会以相同字段回填。字段命名和默认任务策略都锁死在本模块里，保证协议 v3
/// 前后端完全一致。
pub fn issue_license_lease(
    license_key: &str,
    device_id: &str,
    state: LicenseState,
    license_expires_at: &str,
    lease_expires_at: &str,
    renew_after: &str,
    issued_at: &str,
) -> LicenseLease {
    LicenseLease {
        license_key: license_key.to_string(),
        device_id: device_id.to_string(),
        license_status: state,
        license_expires_at: license_expires_at.to_string(),
        lease_expires_at: lease_expires_at.to_string(),
        renew_after: renew_after.to_string(),
        task_policy: DEFAULT_TASK_POLICY
            .iter()
            .map(|item| (*item).to_string())
            .collect(),
        keyset_version: 1,
        binding_version: LICENSE_PROTOCOL_VERSION,
        issued_at: issued_at.to_string(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn issue_license_lease_fills_all_protocol_fields() {
        let lease = issue_license_lease(
            "TLS-TEST",
            "device-1",
            LicenseState::Active,
            "2026-05-01T00:00:00Z",
            "2026-04-19T00:00:00Z",
            "2026-04-17T00:00:00Z",
            "2026-04-16T00:00:00Z",
        );
        assert_eq!(lease.license_key, "TLS-TEST");
        assert_eq!(lease.device_id, "device-1");
        assert_eq!(lease.license_status, LicenseState::Active);
        assert_eq!(lease.license_expires_at, "2026-05-01T00:00:00Z");
        assert_eq!(lease.lease_expires_at, "2026-04-19T00:00:00Z");
        assert_eq!(lease.renew_after, "2026-04-17T00:00:00Z");
        assert_eq!(lease.issued_at, "2026-04-16T00:00:00Z");
        assert_eq!(lease.keyset_version, 1);
        assert_eq!(lease.binding_version, LICENSE_PROTOCOL_VERSION);
        // task_policy 与 SUPPORTED_TASKS 同构，顺序锁定以保 Lease 字节稳定
        let expected: Vec<String> = SUPPORTED_TASKS.iter().map(|s| s.to_string()).collect();
        assert_eq!(lease.task_policy, expected);
    }

    #[test]
    fn default_task_policy_mirrors_api_contracts_supported_tasks() {
        assert_eq!(DEFAULT_TASK_POLICY.len(), SUPPORTED_TASKS.len());
        for (a, b) in DEFAULT_TASK_POLICY.iter().zip(SUPPORTED_TASKS.iter()) {
            assert_eq!(a, b);
        }
    }

    #[test]
    fn license_public_key_is_32_byte_base64url() {
        use base64::engine::general_purpose::URL_SAFE_NO_PAD;
        use base64::Engine;
        let bytes = URL_SAFE_NO_PAD
            .decode(LICENSE_PUBLIC_KEY_B64.as_bytes())
            .expect("LICENSE_PUBLIC_KEY_B64 必须是合法 base64url");
        assert_eq!(bytes.len(), 32, "Ed25519 公钥必须是 32 字节");
    }
}
