use desktop_services::OrderCacheStore;
use domain_core::{OrderCacheEntry, TimeWindow};
use rusqlite::{params, Connection};
use std::path::PathBuf;

const DB_FILE_NAME: &str = "order_cache.db";

pub struct SqliteOrderCache {
    db_path: PathBuf,
}

impl SqliteOrderCache {
    pub fn new(data_dir: PathBuf) -> Self {
        std::fs::create_dir_all(&data_dir).ok();
        let db_path = data_dir.join(DB_FILE_NAME);
        if let Ok(conn) = Connection::open(&db_path) {
            conn.execute_batch(
                "PRAGMA journal_mode = WAL;
                 CREATE TABLE IF NOT EXISTS orders (
                     order_id TEXT PRIMARY KEY,
                     buyer_name TEXT NOT NULL DEFAULT '',
                     receiver_name TEXT NOT NULL DEFAULT '',
                     amount_cent INTEGER NOT NULL DEFAULT 0,
                     created_at TEXT NOT NULL DEFAULT '',
                     updated_at TEXT NOT NULL DEFAULT ''
                 );"
            ).ok();
        }
        Self { db_path }
    }
}

impl OrderCacheStore for SqliteOrderCache {
    fn load_recent_orders(&self, window: &TimeWindow) -> anyhow::Result<Vec<OrderCacheEntry>> {
        let conn = Connection::open(&self.db_path)?;
        let mut stmt = conn.prepare(
            "SELECT order_id, buyer_name, receiver_name, amount_cent, created_at, updated_at
             FROM orders
             WHERE created_at >= ?1 AND created_at <= ?2
             ORDER BY created_at DESC"
        )?;
        let rows = stmt.query_map(params![&window.start_at, &window.end_at], |row| {
            Ok(OrderCacheEntry {
                order_id: row.get(0)?,
                buyer_name: row.get(1)?,
                receiver_name: row.get(2)?,
                amount_cent: row.get(3)?,
                created_at: row.get(4)?,
                updated_at: row.get(5)?,
            })
        })?;
        let mut entries = Vec::new();
        for row in rows {
            entries.push(row?);
        }
        Ok(entries)
    }

    fn save_orders(&self, orders: &[OrderCacheEntry]) -> anyhow::Result<()> {
        let conn = Connection::open(&self.db_path)?;
        let tx = conn.unchecked_transaction()?;
        {
            let mut stmt = tx.prepare(
                "INSERT OR REPLACE INTO orders (order_id, buyer_name, receiver_name, amount_cent, created_at, updated_at)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6)"
            )?;
            for o in orders {
                stmt.execute(params![
                    &o.order_id,
                    &o.buyer_name,
                    &o.receiver_name,
                    o.amount_cent,
                    &o.created_at,
                    &o.updated_at,
                ])?;
            }
        }
        tx.commit()?;
        Ok(())
    }
}
