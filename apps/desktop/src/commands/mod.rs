//! Tauri `#[tauri::command]` 业务入口聚合模块。
//!
//! 每个子模块对应一条产品线的命令集，供前端通过 `invoke("命令名")` 调用：
//!
//! | 子模块     | 负责职责                                                     | 典型命令 |
//! | ---------- | ------------------------------------------------------------ | -------- |
//! | `delivery` | 单条 / 批量发货、取消批量发货                                | `update_delivery` / `batch_delivery` / `cancel_batch_delivery` |
//! | `license`  | 授权激活、校验、Lease 续约、任务级 `RuntimeGrant` 授权       | `activate_license` / `verify_license` / `authorize_runtime_task` |
//! | `order`    | 订单缓存加载、状态查询、近 30 天同步、时间窗同步             | `load_order_cache` / `sync_recent_order_cache` / `sync_orders` |
//! | `review`   | 差评评分匹配、品退直连订单                                   | `find_reviews` / `find_quality_refund_orders` |
//! | `system`   | 应用元信息、外链、Cookie 登录窗口、旧版 Python 数据迁移等     | `get_app_info` / `open_cookie_login` / `start_legacy_migration` |
//!
//! 以下两个模块对业务命令共享使用，不对外暴露为 Tauri 命令：
//!
//! - `paths`：统一 `cache_data_dir` / `rich_order_cache_path` 等本地磁盘路径，避免各命令复制常量
//! - `shared`：命令前置校验的公共 helper（`require_cookie_credentials` 等）
//!
//! 新增命令时需：
//! 1. 在对应子模块写 `#[tauri::command]` 函数，返回 `Result<T, AppError>`
//! 2. 到 `apps/desktop/src/main.rs` 的 `tauri::generate_handler![...]` 注册
//! 3. 若要做统一 Cookie / license 前置校验，从 `shared` 引用 helper，不要重复代码
//!
//! ## 多锁交互注意事项
//!
//! 命令实现里若**同时**持有多把 `AppState` 上的 `Mutex` / `RwLock`，必须按
//! `crate::state::AppState` 文档顶部声明的「锁顺序协议」获取，否则会引入
//! 跨命令的潜在死锁（例如 `cookie_profile` → `store_registry` 的逆序）。
//!
//! 参考实现：
//! - 取一组 store + cookie 快照：`shared::require_store_runtime_context`
//! - 读授权 runtime / profile：`license::get_license_status`
//!
//! Code review 时请把"是否按锁顺序协议获取多锁"作为必查项，不要在新 handler
//! 里复制其他模块的局部加锁顺序——以 `state.rs` 顶部的协议为唯一事实源。

pub mod delivery;
pub mod license;
pub mod order;
pub(crate) mod paths;
pub mod review;
pub(crate) mod shared;
pub mod system;
