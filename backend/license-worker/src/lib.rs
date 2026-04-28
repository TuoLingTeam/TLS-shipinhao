//! `license-worker` 的 crate 入口。
//!
//! 这个 crate 的职责是把 `license-service` / `api-contracts` 暴露的授权协议
//! 落成一个可部署到 Cloudflare Workers 的异步服务。整体结构尽量扁平：
//!
//! - [`messages`]：HTTP DTO 与路由枚举（`parse_route` / `WorkerRoute` /
//!   `SignedLicenseApiResponse` 等），与外部协议一对一。
//! - [`runtime`]：异步运行时业务层 + Cloudflare D1 仓储实现。所有
//!   `runtime_activate / verify / refresh_lease / revoke / task_authorize /
//!   handle_async_runtime_json` 都在这里。
//! - `admin`（仅 `wasm32`）：`/admin` 页面与 `/api/admin/*` 路由。
//! - `cloudflare_entry`（仅 `wasm32`）：`#[event(fetch)]` 入口 + 路由 +
//!   secret 读取 + D1 绑定获取。
//! - [`LeaseTokenSigner`]（**本文件唯一的业务类型**）：把 `LicenseLease`
//!   用 `LICENSE_SIGNING_PRIVATE_KEY_B64` 这把 Ed25519 私钥签名成客户端可验证
//!   的 Lease Token。放在 lib.rs 主要是因为 `cloudflare_entry` 需要它，且它
//!   的职责与"runtime 业务流程"解耦。
//!
//! 外部调用（测试 / 其它 crate）通过 `pub use messages::*;` + `pub use
//! runtime::*;` 继续以 `license_worker::<symbol>` 的形式访问所有公开 API，
//! 完成 runtime 业务从 lib.rs 抽出的搬家时不会破坏任何上游 import。

use api_contracts::LicenseLease;
use base64::engine::general_purpose::{STANDARD, URL_SAFE_NO_PAD};
use base64::Engine;
use ed25519_dalek::pkcs8::DecodePrivateKey;
use ed25519_dalek::{Signer, SigningKey};

// 把 `async_trait::async_trait` 重新导出到 crate root，方便 `runtime` 模块里
// 的 trait 定义以及 `tests` 模块里的内存实现都能通过 `use super::*;` 共享同
// 一份 macro 展开。不重新导出会让 tests.rs 的 `#[async_trait(?Send)]` 找不到
// macro 或得到不同版本，引发 "lifetimes do not match method in trait" 的
// 编译错误。
#[doc(hidden)]
pub use async_trait::async_trait;

pub mod messages;
pub mod runtime;

#[cfg(target_arch = "wasm32")]
mod admin;

pub use messages::{
    parse_route, route_request, route_requires_signer, AdminRevokeRequest, LeaseRefreshRequest,
    LeaseRevokeRequest, Lrr, SignedLicenseApiResponse, TaskAuthorizeRequest, WorkerRoute,
};
pub use runtime::{
    admin_auth_error_contract, handle_admin_revoke_json, handle_async_runtime_json,
    revoke_error_contract, revoke_generated_key_update_sql, revoke_response_status,
    runtime_activate, runtime_refresh_lease, runtime_revoke, runtime_task_authorize,
    runtime_verify, AsyncRuntimeRepository, REVOKE_GENERATED_KEY_SQL_FALLBACK,
    REVOKE_GENERATED_KEY_SQL_WITH_METADATA, WORKER_RUNTIME_ERROR_MESSAGE,
};

/// Ed25519 Lease Token 签发器。
///
/// 从 Cloudflare Secret `LICENSE_SIGNING_PRIVATE_KEY_B64` 里加载 PKCS8 DER
/// 或 PEM 文本；`sign_license_lease` 把 `LicenseLease` 折叠成 `Lp`
/// canonical JSON 后做 `base64url(payload).base64url(ed25519_sig)` 格式的
/// Token 输出，客户端 `security_core` / `license-service::lease::LeaseVerifier`
/// 用同一份公钥（`LICENSE_PUBLIC_KEY_B64`）验签。
#[derive(Debug, Clone)]
pub struct LeaseTokenSigner {
    signing_key: SigningKey,
}

impl LeaseTokenSigner {
    pub fn from_private_key_b64(private_key_b64: &str) -> anyhow::Result<Self> {
        let raw = STANDARD.decode(private_key_b64.trim())?;
        if let Ok(text) = String::from_utf8(raw.clone()) {
            if text.contains("BEGIN PRIVATE KEY") {
                let body = text
                    .lines()
                    .filter(|line| !line.starts_with("-----"))
                    .collect::<String>();
                let der = STANDARD.decode(body.as_bytes())?;
                return Ok(Self {
                    signing_key: SigningKey::from_pkcs8_der(&der)?,
                });
            }
        }
        Ok(Self {
            signing_key: SigningKey::from_pkcs8_der(&raw)?,
        })
    }

    pub fn public_key_b64(&self) -> String {
        URL_SAFE_NO_PAD.encode(self.signing_key.verifying_key().as_bytes())
    }

    pub fn sign_license_lease(&self, lease: &LicenseLease) -> anyhow::Result<String> {
        let payload = crate::runtime::lease_to_payload(lease)?;
        let payload_bytes = serde_json::to_vec(&payload)?;
        let payload_b64 = URL_SAFE_NO_PAD.encode(payload_bytes);
        let signature = self.signing_key.sign(payload_b64.as_bytes());
        let signature_b64 = URL_SAFE_NO_PAD.encode(signature.to_bytes());
        Ok(format!("{payload_b64}.{signature_b64}"))
    }
}

#[cfg(target_arch = "wasm32")]
mod cloudflare_entry {
    use super::*;
    use crate::runtime::D1RuntimeRepo;
    use chrono::Utc;
    use serde_json::Value;
    use worker::{event, Env, Method, Request, Response, Result};

    fn missing_secret(name: &str) -> Result<Response> {
        Response::from_json(&serde_json::json!({
            "success": false,
            "message": format!("{name} 未配置"),
        }))
        .map(|resp| resp.with_status(503))
    }

    fn response_from_revoke_payload(payload: &str) -> Result<Response> {
        let value: SignedLicenseApiResponse =
            serde_json::from_str(payload).map_err(worker_error)?;
        Response::from_json(&value).map(|resp| resp.with_status(revoke_response_status(&value)))
    }

    fn load_signer(env: &Env) -> anyhow::Result<LeaseTokenSigner> {
        let value = env.secret("LICENSE_SIGNING_PRIVATE_KEY_B64")?;
        LeaseTokenSigner::from_private_key_b64(&value.to_string())
    }

    fn compatibility_payload(path: &str) -> String {
        serde_json::json!({
            "success": false,
            "message": WORKER_RUNTIME_ERROR_MESSAGE,
            "path": path,
        })
        .to_string()
    }

    fn worker_error(err: impl ToString) -> worker::Error {
        worker::Error::RustError(err.to_string())
    }

    fn response_from_json_string(payload: String) -> Result<Response> {
        let value: Value = serde_json::from_str(&payload).map_err(worker_error)?;
        Response::from_json(&value)
    }

    async fn route_fetch(mut req: Request, env: Env) -> Result<Response> {
        let path = req.path();
        let method = req.method();

        if method == Method::Get && path == "/admin" {
            return crate::admin::serve_admin_html().await;
        }

        if path.starts_with("/api/admin/") {
            return crate::admin::handle_admin_request(req, &env).await;
        }

        if method != Method::Post {
            return Response::error("Method Not Allowed", 405);
        }

        let route = parse_route(&path);
        if route == WorkerRoute::NotFound {
            return Response::error("not_found", 404);
        }

        if route == WorkerRoute::LeaseRevoke {
            if let Some(resp) = crate::admin::check_admin(req.headers(), &env)? {
                return Ok(resp);
            }
        }

        let body = req.text().await.unwrap_or_default();
        match route {
            WorkerRoute::Activate
            | WorkerRoute::Verify
            | WorkerRoute::LeaseRefresh
            | WorkerRoute::TaskAuthorize
            | WorkerRoute::LeaseRevoke => {
                let signer = if route_requires_signer(route) {
                    match load_signer(&env) {
                        Ok(signer) => Some(signer),
                        Err(_) => return missing_secret("LICENSE_SIGNING_PRIVATE_KEY_B64"),
                    }
                } else {
                    None
                };
                let db = env.d1("DB")?;
                let repo = D1RuntimeRepo::new(&db);
                let payload = match handle_async_runtime_json(
                    &repo,
                    &path,
                    &body,
                    signer.as_ref(),
                    Utc::now(),
                )
                .await
                {
                    Ok(payload) => payload,
                    Err(err) if route == WorkerRoute::LeaseRevoke => {
                        let err_text = err.to_string();
                        let (status, message) = revoke_error_contract(&err_text);
                        // Cloudflare console 分级：吊销路径已经把对外文案脱敏在
                        // revoke_error_contract，这里再补一条 warn 给运维定位
                        // （只进 wrangler tail，不改 HTTP 响应字段）
                        worker::console_warn!(
                            "[lease/revoke] runtime error → status={status}, root={err_text}"
                        );
                        return Response::from_json(&serde_json::json!({
                            "success": false,
                            "message": message,
                        }))
                        .map(|resp| resp.with_status(status));
                    }
                    Err(err) => {
                        // 业务路径失败（activate/verify/refresh/task_authorize）：
                        // 维持「以 worker::Error 形式继续上抛」的对外契约，但补 warn
                        // 让运维能从 wrangler tail 看到 root cause 而不仅是兜底 200。
                        worker::console_warn!("[runtime] route={route:?} error={err}");
                        return Err(worker_error(err));
                    }
                };
                if route == WorkerRoute::LeaseRevoke {
                    response_from_revoke_payload(&payload)
                } else {
                    response_from_json_string(payload)
                }
            }
            WorkerRoute::NotFound => Response::error("not_found", 404),
        }
    }

    #[event(fetch)]
    pub async fn fetch(req: Request, env: Env, _ctx: worker::Context) -> Result<Response> {
        route_fetch(req, env).await.or_else(|err| {
            // 顶层兜底：HTTP 响应仍走 200 + compatibility_payload 不变，避免破坏
            // 客户端的兼容握手；同时把错误以 console_error 形式记录到 Cloudflare
            // 日志（wrangler tail / Logpush），用于事后分级排障。
            worker::console_error!("[fetch] route_fetch failed before HTTP shaping: {err}");
            Response::ok(compatibility_payload("/error"))
        })
    }
}

#[cfg(test)]
mod tests;
