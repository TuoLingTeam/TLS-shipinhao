//! 桌面端后端统一 HTTP 客户端构造器。
//!
//! 所有走外网的后端 HTTP 调用都必须经过本模块，原因：
//! - 授权服务（`sphapi.199908.top` 等域名）上游启用了 Cloudflare 反机器人 JS
//!   挑战（Managed Challenge）。`reqwest` 的默认 User-Agent 形如
//!   `reqwest/0.12.x`，会被 Cloudflare 直接拦下，返回 HTTP 403 + HTML
//!   挑战页而非业务 JSON。历史上这让前端看到"激活失败"却拿不到任何业务
//!   原因——本地 HTTP 层把 HTML 当 JSON 解析失败，异常经 `useTauriInvoke`
//!   被吞，UI 彻底沉默。
//! - 统一一处配 UA 便于审计、抓包，未来升级 TLS / Proxy 策略也只改这一处。
//!
//! 约定：
//! - UA 固定带产品名 + 版本号 + 平台，不模仿浏览器。Cloudflare 需要的只是
//!   "非默认 reqwest UA"，使用真实标识更利于上游运维溯源。
//! - 调用方传入 `timeout`，不再各自 `timeout()` + `Client::new()`。
//!
//! ## 同步 / 异步边界
//!
//! 这里返回的是 **异步** `reqwest::Client`。`apps/desktop/src/adapters/` 下的
//! HTTP 适配器（`store` / `quality_refund` / `delivery` / `review` / `order` /
//! `license`）**核心方法均为 `async fn`**，已不再存在「外层 `std::thread::spawn`
//! + 内层 `Handle::block_on`」的双层线程包装。
//!
//! 命令层调用约定按场景分两类：
//!
//! 1. **纯 HTTP 命令**（`#[tauri::command] async fn`，如 `update_delivery`、
//!    `find_quality_refund_orders`、`set_cookie`）：直接 `.await` 适配器 async
//!    方法，无 `spawn_blocking`。
//! 2. **HTTP + SQLite / license-guard 的混合流程命令**（如 `find_reviews`、
//!    `sync_recent_order_cache`、`batch_delivery`）：命令层用
//!    `tokio::task::spawn_blocking` 把同步业务流程丢到阻塞线程池；流程内调用
//!    剩余 3 个同步 trait（`BatchDeliveryGateway` / `OrderCacheStore` /
//!    `CacheOrderFinder`），trait 实现内部以
//!    `tokio::runtime::Handle::current().block_on(self.<async_method>())` 桥接到
//!    async 实现——blocking 线程不属于异步执行上下文，是 tokio 文档允许的用法。
//!
//! **硬约束**：外网请求必须继续走本模块的 [`build_desktop_http_client`]，
//! 禁止新增绕过 UA 策略的裸 `Client::new()`；新增 HTTP 适配器请遵循「内部纯
//! `async fn` + 同步 trait 仅作 `Handle::block_on` 薄壳」的当前模式。
//!
//! ## 剩余 sync trait 的边界决策
//!
//! `BatchDeliveryGateway` / `OrderCacheStore` / `CacheOrderFinder` 三个 trait
//! **决议保留 sync**，不再追加 `#[async_trait]`。理由：HTTP 已经在适配器内
//! 全 async，SQLite 仍是单连接同步串行；改 async trait 仅消除「内部 block_on
//! 桥接 60 行」却引入 `±500 行` 重构与全套 `#[tokio::test]` 改造，运行时无
//! 任何并发收益。后续如要继续 async 化，应先确认 SQLite 与 license-guard 的
//! 并发模型收益足以覆盖重构成本。

use std::time::Duration;

/// 统一桌面端 HTTP UA。显式暴露出去便于回归测试锁死。
pub const DESKTOP_HTTP_USER_AGENT: &str = concat!(
    "TLS-shipinhao/",
    env!("CARGO_PKG_VERSION"),
    " (Tauri desktop)"
);

/// 构造一个供桌面端后端调用外网 API 使用的 `reqwest::Client`。
///
/// 失败时回退到 `Client::default()`（极端场景下至少保证返回可用实例），
/// 但默认路径一定会带上 [`DESKTOP_HTTP_USER_AGENT`] 与指定超时。
pub fn build_desktop_http_client(timeout: Duration) -> reqwest::Client {
    reqwest::Client::builder()
        .timeout(timeout)
        .user_agent(DESKTOP_HTTP_USER_AGENT)
        .build()
        .unwrap_or_default()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn desktop_http_user_agent_is_not_default_reqwest() {
        // 若未来有人把 UA 改回默认（空串或以 "reqwest/" 开头），Cloudflare 会再次
        // 返回 HTML 挑战页，导致"激活永远失败"类 P0 故障。这条断言是最后防线。
        assert!(
            !DESKTOP_HTTP_USER_AGENT.is_empty(),
            "UA 不能为空，否则 Cloudflare 会拦下所有 /api/* 请求"
        );
        assert!(
            !DESKTOP_HTTP_USER_AGENT.starts_with("reqwest/"),
            "UA 不能是 reqwest 默认值：{DESKTOP_HTTP_USER_AGENT}"
        );
        assert!(
            DESKTOP_HTTP_USER_AGENT.contains("TLS-shipinhao/"),
            "UA 必须带产品标识方便上游运维溯源：{DESKTOP_HTTP_USER_AGENT}"
        );
    }

    #[test]
    fn build_client_succeeds_with_normal_timeout() {
        let _client = build_desktop_http_client(Duration::from_secs(8));
    }
}
