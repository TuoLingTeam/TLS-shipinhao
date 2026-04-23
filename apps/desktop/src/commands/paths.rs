//! 桌面端命令统一使用的本地磁盘路径。
//!
//! 原本在 `order.rs` / `review.rs` 内各自复制了一份 `cache_data_dir` / `rich_order_cache_path`，
//! 一旦业务目录调整就会漏改一处。抽到这里由 `pub(crate)` 统一暴露，保证两侧指向同一位置。

use crate::state;
use std::path::{Path, PathBuf};

/// 应用用户数据目录（`~/Library/Application Support/TLS-shipinhao` 等平台对应位置）。
/// 平台 API 不可用时回退到当前工作目录，保证 CLI/CI 下也能跑。
///
/// 多店铺上线前的订单缓存一直存放在这套全局目录里；当当前未激活任何店铺时，
/// 仍回退到这里兼容旧数据读取。
pub(crate) fn default_cache_data_dir() -> PathBuf {
    dirs::data_local_dir()
        .unwrap_or_else(|| PathBuf::from("."))
        .join("TLS-shipinhao")
}

/// 解析当前店铺的轻缓存目录。
///
/// - 已激活店铺：落到 `~/.tls-shipinhao/stores/<store_id>/`
/// - 尚未迁移旧数据：继续沿用历史全局目录
pub(crate) fn cache_data_dir_for_store(app_home_dir: &Path, store_id: Option<&str>) -> PathBuf {
    match store_id
        .map(str::trim)
        .filter(|store_id| !store_id.is_empty())
    {
        Some(store_id) => state::store_dir(app_home_dir, store_id),
        None => default_cache_data_dir(),
    }
}

/// 当前店铺的富订单缓存 SQLite 文件位置。
pub(crate) fn rich_order_cache_path_for_store(
    app_home_dir: &Path,
    store_id: Option<&str>,
) -> PathBuf {
    cache_data_dir_for_store(app_home_dir, store_id).join("order_cache.sqlite3")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn cache_data_dir_for_store_uses_store_directory_when_store_is_active() {
        let app_home = PathBuf::from("/tmp/tls-shipinhao-home");

        let path = cache_data_dir_for_store(&app_home, Some("wx61f28d69d9174ddf"));

        assert_eq!(path, app_home.join("stores").join("wx61f28d69d9174ddf"));
    }

    #[test]
    fn rich_order_cache_path_for_store_falls_back_to_legacy_global_cache() {
        let app_home = PathBuf::from("/tmp/tls-shipinhao-home");

        let path = rich_order_cache_path_for_store(&app_home, None);

        assert_eq!(path, default_cache_data_dir().join("order_cache.sqlite3"));
    }
}
