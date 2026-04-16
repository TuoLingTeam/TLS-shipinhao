use desktop_services::OrderCacheStore;
use domain_core::{OrderCacheEntry, TimeWindow};

pub struct SqliteOrderCache {
    pub db_path: String,
}

impl SqliteOrderCache {
    pub fn new(db_path: String) -> Self {
        Self { db_path }
    }
}

impl OrderCacheStore for SqliteOrderCache {
    fn load_recent_orders(&self, _window: &TimeWindow) -> anyhow::Result<Vec<OrderCacheEntry>> {
        // TODO: 使用 rusqlite 查询本地 SQLite 数据库
        Ok(vec![])
    }

    fn save_orders(&self, _orders: &[OrderCacheEntry]) -> anyhow::Result<()> {
        // TODO: 批量 INSERT OR REPLACE 到本地 SQLite
        Ok(())
    }
}
