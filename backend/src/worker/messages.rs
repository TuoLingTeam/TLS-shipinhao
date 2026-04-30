//! Worker 对外请求/响应数据结构与路由枚举。
//!
//! 这里只放**纯 DTO + 路由 parse helper**，不包含业务逻辑。拆出来的动机：
//!
//! - `lib.rs` 原本近 2000 行，导入 / DTO / handler / dispatch 全堆在一起
//! - DTO 与路由枚举**无副作用、无外部依赖**（除了 serde 与 contracts），
//!   迁移风险最小；对外公共 API 通过 `pub use messages::*` 保持不变
//! - 后续若要继续拆 handler / dispatch，可以在本模块稳定后以相同思路进行

use crate::blank_debug_release;
use crate::contracts::LicenseState;
use serde::{Deserialize, Serialize};

/// Worker 支持的路由枚举。
///
/// 新增路由必须同时更新 [`parse_route`] / [`route_request`] /
/// `handle_async_runtime_json` 与对应的请求/响应结构（[`LeaseRefreshRequest`] 等）。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WorkerRoute {
    Activate,
    Verify,
    /// M2-04 续约端点：input = [`LeaseRefreshRequest`]，output = [`Lrr`]
    LeaseRefresh,
    /// 管理员吊销端点：由后台管理 UI 触发
    LeaseRevoke,
    /// M2-08 任务级授权：input = [`TaskAuthorizeRequest`]，output =
    /// `crate::contracts::RuntimeGrant`
    TaskAuthorize,
    NotFound,
}

/// `/api/lease/refresh` 入参。
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct LeaseRefreshRequest {
    pub license_key: String,
    pub device_id: String,
    /// 原 Lease 的 issued_at，Worker 用它做乐观并发控制。
    pub current_issued_at: i64,
}

/// `/api/lease/refresh` 响应。
#[cfg_attr(debug_assertions, derive(Debug))]
#[derive(Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct Lrr {
    pub success: bool,
    #[serde(default)]
    pub message: String,
    /// 新签发的 Lease Token（`base64url(payload).base64url(signature)`），
    /// 客户端需本地验签后才能写回 Keychain。
    pub new_token: String,
}
blank_debug_release!(Lrr);

/// `/api/lease/revoke` 入参（管理员使用）。
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct LeaseRevokeRequest {
    pub license_key: String,
    pub device_id: String,
    #[serde(default)]
    pub reason: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct AdminRevokeRequest {
    pub key: String,
}

/// `/api/task/authorize` 入参。
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct TaskAuthorizeRequest {
    pub license_key: String,
    pub device_id: String,
    pub task_type: String,
    #[serde(default)]
    pub client_version: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct SignedLicenseApiResponse {
    pub success: bool,
    #[serde(default)]
    pub message: String,
    pub license_state: LicenseState,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub license_lease: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub license_expires_at: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub activated_at: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub device_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub license_key: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub lease_expires_at: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub renew_after: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub issued_at: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub license_status: Option<LicenseState>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub task_policy: Option<Vec<String>>,
}

/// 把 HTTP path 解析为 [`WorkerRoute`]。未命中时返回 [`WorkerRoute::NotFound`]。
pub fn parse_route(path: &str) -> WorkerRoute {
    match path {
        "/api/activate" => WorkerRoute::Activate,
        "/api/verify" => WorkerRoute::Verify,
        "/api/lease/refresh" => WorkerRoute::LeaseRefresh,
        "/api/lease/revoke" => WorkerRoute::LeaseRevoke,
        "/api/task/authorize" => WorkerRoute::TaskAuthorize,
        _ => WorkerRoute::NotFound,
    }
}

/// 把 [`WorkerRoute`] 展开为审计日志用的路由名。
pub fn route_request(path: &str) -> &'static str {
    match parse_route(path) {
        WorkerRoute::Activate => "activate",
        WorkerRoute::Verify => "verify",
        WorkerRoute::LeaseRefresh => "lease_refresh",
        WorkerRoute::LeaseRevoke => "lease_revoke",
        WorkerRoute::TaskAuthorize => "task_authorize",
        WorkerRoute::NotFound => "not_found",
    }
}

/// 某些路由需要 `LeaseTokenSigner`（签发新 Lease），dispatch 层据此决定是否 500。
pub fn route_requires_signer(route: WorkerRoute) -> bool {
    matches!(
        route,
        WorkerRoute::Activate | WorkerRoute::Verify | WorkerRoute::LeaseRefresh
    )
}
