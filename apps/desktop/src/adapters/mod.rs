//! 桌面进程侧 HTTP / 存储适配。
//!
//! 出站 HTTP 客户端构造以 `common::build_client` →
//! `desktop_services::http_client::build_desktop_http_client` 为唯一事实源。
//! **L4-2（async trait 化）** 的分期任务与锁顺序说明见
//! `crates/desktop-services/src/common/http_client.rs` 模块级注释。

pub(crate) mod common;
pub mod delivery;
pub mod license;
pub mod order;
pub mod order_cache;
pub mod quality_refund;
pub mod review;
pub mod secure_storage;
pub mod store;
