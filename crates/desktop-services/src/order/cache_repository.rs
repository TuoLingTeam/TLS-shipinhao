//! 订单富缓存仓储 trait 与数据结构。
//!
//! 这里定义所有业务层使用的数据抽象（`OrderCacheRepository`），让
//! `OrderSyncService` 以及 Tauri 命令无需直接依赖 sqlite。具体的 sqlite
//! 实现（`SqliteOrderCacheRepository`）位于 `apps/desktop/src/adapters/`。

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Default)]
#[serde(rename_all = "camelCase")]
pub struct CacheOrderProduct {
    pub product_id: String,
    pub sku_id: String,
    pub sale_param: String,
    pub product_name: String,
    pub thumb_img: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Default)]
#[serde(rename_all = "camelCase")]
pub struct CacheOrderRecord {
    pub order_id: String,
    pub buyer_nickname: String,
    pub normalized_nickname: String,
    pub amount_cent: i64,
    pub create_time: i64,
    pub confirm_receipt_time: i64,
    pub is_waybill_received: bool,
    pub waybill_received_time: i64,
    pub is_education_order: bool,
    pub order_status: i64,
    pub openid: String,
    pub raw_source: String,
    pub updated_at: i64,
    pub products: Vec<CacheOrderProduct>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Default)]
#[serde(rename_all = "camelCase")]
pub struct SyncStateRecord {
    pub scope: String,
    pub coverage_start: i64,
    pub coverage_end: i64,
    pub last_incremental_start: i64,
    pub last_incremental_end: i64,
    pub last_success_at: i64,
    pub last_mode: String,
    pub last_error: String,
}

/// 业务层使用的订单富缓存仓储抽象。
///
/// 所有方法取 `&self`，实现方必须自行处理内部可变性（通常通过 `Mutex<Connection>`），
/// 以便 `Arc<dyn OrderCacheRepository>` 能被多个异步任务共享。
pub trait OrderCacheRepository: Send + Sync {
    /// 确保 schema 就位（幂等）。
    fn initialize(&self) -> anyhow::Result<()>;

    /// 清空所有缓存表（orders / order_products / sync_state / cache_segments）。
    fn clear_all(&self) -> anyhow::Result<()>;

    /// 批量 upsert 订单及其商品行；返回写入订单数。
    fn upsert_orders(&self, orders: &[CacheOrderRecord]) -> anyhow::Result<usize>;

    /// 保存同步状态（按 scope 唯一）。
    fn save_state(&self, state: &SyncStateRecord) -> anyhow::Result<()>;

    /// 查询指定 scope 的同步状态。
    fn get_state(&self, scope: &str) -> anyhow::Result<Option<SyncStateRecord>>;

    /// 取单个订单（含商品行）。
    fn fetch_order(&self, order_id: &str) -> anyhow::Result<Option<CacheOrderRecord>>;

    /// 标记一个已完成的时间段。
    fn mark_segment_complete(
        &self,
        scope: &str,
        start_timestamp: i64,
        end_timestamp: i64,
    ) -> anyhow::Result<()>;

    /// 取 [start, end] 内所有已完成段（按开始时间升序）。
    fn get_complete_segments(
        &self,
        scope: &str,
        start_timestamp: i64,
        end_timestamp: i64,
    ) -> anyhow::Result<Vec<(i64, i64)>>;

    /// 基于已完成段计算出尚未覆盖的缺口（merge_tolerance 合并，min_gap_width 过滤）。
    fn get_missing_segments(
        &self,
        scope: &str,
        start_timestamp: i64,
        end_timestamp: i64,
        merge_tolerance: i64,
        min_gap_width: i64,
    ) -> anyhow::Result<Vec<(i64, i64)>>;

    /// 检测是否存在以 `[` 开头的脏 sale_param（Python 早期版本误写 JSON 数组字面量）。
    fn has_dirty_sale_param(&self) -> anyhow::Result<bool>;

    /// 删除早于 cutoff 的订单，同时裁剪 cache_segments 里超出保留窗口的段。
    fn delete_older_than(&self, scope: &str, cutoff_timestamp: i64) -> anyhow::Result<usize>;

    /// 按时间窗口读取订单（含商品行），按 create_time 降序。
    fn fetch_orders_in_range(
        &self,
        start_timestamp: i64,
        end_timestamp: i64,
    ) -> anyhow::Result<Vec<CacheOrderRecord>>;

    /// 当前订单总数（用于 UI 展示）。
    fn count_orders(&self) -> anyhow::Result<usize>;

    /// 指定时间窗口内的订单数（用于仪表盘轻量统计）。
    fn count_orders_in_range(
        &self,
        start_timestamp: i64,
        end_timestamp: i64,
    ) -> anyhow::Result<usize>;
}
