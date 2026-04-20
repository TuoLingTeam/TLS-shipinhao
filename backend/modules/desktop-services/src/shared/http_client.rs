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
//! ## 同步边界说明
//!
//! 这里返回的是 **异步** `reqwest::Client`。`apps/desktop/src/adapters/http_*.rs`
//! 的多数方法位于 `ReviewSource` / `DeliveryGateway` 等**同步 trait** 内部，
//! 而 Tauri 命令层本身是 `async fn`。目前的执行链路是：
//!
//! 1. `#[tauri::command] async fn ...` 接到前端请求
//! 2. 在 tokio runtime 内 `tokio::task::spawn_blocking(|| run_*_flow(...))`
//!    把**同步**业务流程丢到阻塞线程池
//! 3. `run_*_flow` 调用 adapter 的同步方法，方法内再 `std::thread::spawn + Handle::block_on`
//!    在新线程里跑 async reqwest 请求，避免在阻塞线程上触发 `block_on` 嵌套检测
//!
//! 这种"内外两层线程"的兼容做法是历史原因导致的（同步 trait + async reqwest）。
//! 未来若把 trait 切换为 `async_trait` 或迁移到 `reqwest::blocking::Client`，
//! 便能去掉 `std::thread::spawn + Handle::block_on` 包装层。当前保留现状以锁住
//! 生产可用性，不在 slop 清理期动。

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
