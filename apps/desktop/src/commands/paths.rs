//! 桌面端命令统一使用的本地磁盘路径。
//!
//! 原本在 `order.rs` / `review.rs` 内各自复制了一份 `cache_data_dir` / `rich_order_cache_path`，
//! 一旦业务目录调整就会漏改一处。抽到这里由 `pub(crate)` 统一暴露，保证两侧指向同一位置。

use std::path::PathBuf;

/// 应用用户数据目录（`~/Library/Application Support/TLS-shipinhao` 等平台对应位置）。
/// 平台 API 不可用时回退到当前工作目录，保证 CLI/CI 下也能跑。
pub(crate) fn cache_data_dir() -> PathBuf {
    dirs::data_local_dir()
        .unwrap_or_else(|| PathBuf::from("."))
        .join("TLS-shipinhao")
}

/// 富订单缓存 SQLite 文件位置。
pub(crate) fn rich_order_cache_path() -> PathBuf {
    cache_data_dir().join("order_cache.sqlite3")
}
