//! 桌面端命令的公共前置校验 helper。
//!
//! 原本 `order` / `review` / `delivery` 命令各自复制了「取锁 → 判 cookie 非空 → 克隆 cookie/biz_magic」
//! 6 次，任何一处改文案或加埋点都要漏改一份。这里抽到一处，保证所有命令走同一套检查。

use crate::commands::paths::{cache_data_dir_for_store, rich_order_cache_path_for_store};
use crate::error::AppError;
use crate::state::{AppState, CookieHealthSnapshot, StoreRegistry};
use desktop_services::CookieProfile;
use std::path::{Path, PathBuf};
use tauri::State;

/// 业务命令运行所需的 Cookie 凭证，字段已脱出 `Mutex` 锁。
pub(crate) struct CookieCredentials {
    pub(crate) cookie: String,
    pub(crate) magic: String,
}

/// 当前店铺对应的本地路径快照。
pub(crate) struct CurrentStorePaths {
    pub(crate) data_dir: PathBuf,
    pub(crate) rich_order_cache_path: PathBuf,
}

/// 需要同时绑定当前店铺路径与 Cookie 的命令上下文。
pub(crate) struct StoreRuntimeContext {
    pub(crate) cookie: String,
    pub(crate) magic: String,
    pub(crate) data_dir: PathBuf,
    pub(crate) rich_order_cache_path: PathBuf,
}

fn store_paths_for(app_home_dir: &Path, store_id: Option<&str>) -> CurrentStorePaths {
    CurrentStorePaths {
        data_dir: cache_data_dir_for_store(app_home_dir, store_id),
        rich_order_cache_path: rich_order_cache_path_for_store(app_home_dir, store_id),
    }
}

/// 校验应用状态中是否已保存合法 Cookie，并把 `cookie_header` / `biz_magic` 克隆返回。
///
/// 行为与各命令中的原重复分支一致：
/// - 为空时返回 `AppError::Message("请先在设置中配置 Cookie")`
/// - 非空时克隆两个字段后立即释放锁
pub(crate) async fn require_cookie_credentials(
    state: &State<'_, AppState>,
) -> Result<CookieCredentials, AppError> {
    let cookie_profile = state.cookie_profile.lock().await;
    if cookie_profile.cookie_header.is_empty() {
        return Err(AppError::Message("请先在设置中配置 Cookie".to_string()));
    }
    Ok(CookieCredentials {
        cookie: cookie_profile.cookie_header.clone(),
        magic: cookie_profile.biz_magic.clone().unwrap_or_default(),
    })
}

/// 读取当前激活店铺对应的磁盘路径；未激活任何店铺时，回退到旧版全局缓存目录。
pub(crate) async fn current_store_paths(state: &State<'_, AppState>) -> CurrentStorePaths {
    let store_registry = state.store_registry.lock().await;
    store_paths_for(
        &state.app_home_dir,
        store_registry.active_store_id.as_deref(),
    )
}

/// 同时快照当前店铺路径与 Cookie，避免同步命令把 A 店 Cookie 写进 B 店缓存。
///
/// 这里按 `store_registry -> cookie_profile` 的锁顺序一次性取快照，和 `AppState` 约定一致。
pub(crate) async fn require_store_runtime_context(
    state: &State<'_, AppState>,
) -> Result<StoreRuntimeContext, AppError> {
    let store_registry = state.store_registry.lock().await;
    let paths = store_paths_for(
        &state.app_home_dir,
        store_registry.active_store_id.as_deref(),
    );
    let cookie_profile = state.cookie_profile.lock().await;
    if cookie_profile.cookie_header.is_empty() {
        return Err(AppError::Message("请先在设置中配置 Cookie".to_string()));
    }
    Ok(StoreRuntimeContext {
        cookie: cookie_profile.cookie_header.clone(),
        magic: cookie_profile.biz_magic.clone().unwrap_or_default(),
        data_dir: paths.data_dir,
        rich_order_cache_path: paths.rich_order_cache_path,
    })
}

/// 仅用于 `get_cookie_status` 命令的「Cookie 状态四锁快照」。
///
/// 字段已脱出对应 `Mutex`：
/// - registry：当前店铺注册表 + 活跃店铺
/// - profile：cookie_header + biz_magic
/// - cookie_path：当前 cookie 文件磁盘路径
/// - health：cookie 健康度快照
///
/// 历史实现把这四把锁分别按需克隆在命令体内，时间序列上多个 lock().await
/// 之间可能被别的命令插入修改。重构后仍**不**承诺事务一致（事务一致需要
/// 长持锁，会显著加大死锁风险），但把锁顺序从 system::get_cookie_status
/// 体内移到 helper 内部，与 AppState 顶部的「锁顺序协议」对齐为单点事实源。
pub(crate) struct CookieStatusSnapshot {
    pub(crate) registry: StoreRegistry,
    pub(crate) profile: CookieProfile,
    pub(crate) cookie_path: PathBuf,
    pub(crate) health: CookieHealthSnapshot,
}

/// 一次按 `store_registry -> cookie_profile -> cookie_path -> cookie_health`
/// 顺序拿四把锁、克隆 + 立即释放。
///
/// 不同字段间仍是「尽力而为」一致性（与重构前等价）；如果业务需要事务级
/// 快照，应另外评估是否值得引入复合锁。
pub(crate) async fn cookie_status_snapshot(state: &State<'_, AppState>) -> CookieStatusSnapshot {
    let registry = state.store_registry.lock().await.clone();
    let profile = state.cookie_profile.lock().await.clone();
    let cookie_path = state.cookie_path.lock().await.clone();
    let health = state.cookie_health.lock().await.clone();
    CookieStatusSnapshot {
        registry,
        profile,
        cookie_path,
        health,
    }
}
