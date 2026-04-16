use anyhow::Context;
use rusqlite::{params, Connection, OptionalExtension};
use serde::{Deserialize, Serialize};
use std::path::Path;
use std::time::{SystemTime, UNIX_EPOCH};

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

pub struct OrderCacheRepository {
    connection: Connection,
}

impl OrderCacheRepository {
    pub fn open(db_path: &Path) -> anyhow::Result<Self> {
        if let Some(parent) = db_path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        let connection = Connection::open(db_path)?;
        connection.pragma_update(None, "journal_mode", "WAL")?;
        Ok(Self { connection })
    }

    pub fn initialize(&self) -> anyhow::Result<()> {
        self.connection.execute_batch(
            r#"
            CREATE TABLE IF NOT EXISTS orders (
                order_id TEXT PRIMARY KEY,
                buyer_nickname TEXT NOT NULL DEFAULT '',
                normalized_nickname TEXT NOT NULL DEFAULT '',
                create_time INTEGER NOT NULL DEFAULT 0,
                confirm_receipt_time INTEGER NOT NULL DEFAULT 0,
                is_waybill_received INTEGER NOT NULL DEFAULT 0,
                waybill_received_time INTEGER NOT NULL DEFAULT 0,
                is_education_order INTEGER NOT NULL DEFAULT 0,
                order_status INTEGER NOT NULL DEFAULT 0,
                openid TEXT NOT NULL DEFAULT '',
                raw_source TEXT NOT NULL DEFAULT 'order_api',
                updated_at INTEGER NOT NULL DEFAULT 0
            );

            CREATE TABLE IF NOT EXISTS order_products (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                order_id TEXT NOT NULL,
                product_id TEXT NOT NULL DEFAULT '',
                sku_id TEXT NOT NULL DEFAULT '',
                sale_param TEXT NOT NULL DEFAULT '',
                product_name TEXT NOT NULL DEFAULT '',
                thumb_img TEXT NOT NULL DEFAULT '',
                FOREIGN KEY(order_id) REFERENCES orders(order_id) ON DELETE CASCADE
            );

            CREATE TABLE IF NOT EXISTS sync_state (
                scope TEXT PRIMARY KEY,
                coverage_start INTEGER NOT NULL DEFAULT 0,
                coverage_end INTEGER NOT NULL DEFAULT 0,
                last_incremental_start INTEGER NOT NULL DEFAULT 0,
                last_incremental_end INTEGER NOT NULL DEFAULT 0,
                last_success_at INTEGER NOT NULL DEFAULT 0,
                last_mode TEXT NOT NULL DEFAULT '',
                last_error TEXT NOT NULL DEFAULT ''
            );

            CREATE TABLE IF NOT EXISTS cache_segments (
                scope TEXT NOT NULL,
                start_ts INTEGER NOT NULL,
                end_ts INTEGER NOT NULL,
                status TEXT NOT NULL DEFAULT 'complete',
                updated_at INTEGER NOT NULL DEFAULT 0,
                PRIMARY KEY (scope, start_ts, end_ts)
            );

            CREATE INDEX IF NOT EXISTS idx_orders_create_time ON orders(create_time DESC);
            CREATE INDEX IF NOT EXISTS idx_products_order_id ON order_products(order_id);
            CREATE INDEX IF NOT EXISTS idx_cache_segments_scope_start ON cache_segments(scope, start_ts, end_ts);
            "#,
        )?;
        Ok(())
    }

    pub fn clear_all(&self) -> anyhow::Result<()> {
        self.connection.execute("DELETE FROM order_products", [])?;
        self.connection.execute("DELETE FROM orders", [])?;
        self.connection.execute("DELETE FROM sync_state", [])?;
        self.connection.execute("DELETE FROM cache_segments", [])?;
        Ok(())
    }

    pub fn upsert_orders(&mut self, orders: &[CacheOrderRecord]) -> anyhow::Result<usize> {
        if orders.is_empty() {
            return Ok(0);
        }
        let tx = self.connection.transaction()?;
        for order in orders {
            tx.execute("DELETE FROM order_products WHERE order_id = ?1", params![order.order_id])?;
            tx.execute(
                r#"
                INSERT OR REPLACE INTO orders (
                    order_id, buyer_nickname, normalized_nickname, create_time,
                    confirm_receipt_time, is_waybill_received, waybill_received_time,
                    is_education_order, order_status, openid, raw_source, updated_at
                ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12)
                "#,
                params![
                    order.order_id,
                    order.buyer_nickname,
                    order.normalized_nickname,
                    order.create_time,
                    order.confirm_receipt_time,
                    bool_to_int(order.is_waybill_received),
                    order.waybill_received_time,
                    bool_to_int(order.is_education_order),
                    order.order_status,
                    order.openid,
                    order.raw_source,
                    order.updated_at,
                ],
            )?;
            for product in &order.products {
                tx.execute(
                    r#"
                    INSERT INTO order_products (
                        order_id, product_id, sku_id, sale_param, product_name, thumb_img
                    ) VALUES (?1, ?2, ?3, ?4, ?5, ?6)
                    "#,
                    params![
                        order.order_id,
                        product.product_id,
                        product.sku_id,
                        product.sale_param,
                        product.product_name,
                        product.thumb_img,
                    ],
                )?;
            }
        }
        tx.commit()?;
        Ok(orders.len())
    }

    pub fn save_state(&self, state: &SyncStateRecord) -> anyhow::Result<()> {
        self.connection.execute(
            r#"
            INSERT OR REPLACE INTO sync_state (
                scope, coverage_start, coverage_end, last_incremental_start,
                last_incremental_end, last_success_at, last_mode, last_error
            ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)
            "#,
            params![
                state.scope,
                state.coverage_start,
                state.coverage_end,
                state.last_incremental_start,
                state.last_incremental_end,
                state.last_success_at,
                state.last_mode,
                state.last_error,
            ],
        )?;
        Ok(())
    }

    pub fn get_state(&self, scope: &str) -> anyhow::Result<Option<SyncStateRecord>> {
        self.connection
            .query_row(
                r#"
                SELECT scope, coverage_start, coverage_end, last_incremental_start,
                       last_incremental_end, last_success_at, last_mode, last_error
                FROM sync_state WHERE scope = ?1
                "#,
                params![scope],
                |row| {
                    Ok(SyncStateRecord {
                        scope: row.get(0)?,
                        coverage_start: row.get(1)?,
                        coverage_end: row.get(2)?,
                        last_incremental_start: row.get(3)?,
                        last_incremental_end: row.get(4)?,
                        last_success_at: row.get(5)?,
                        last_mode: row.get(6)?,
                        last_error: row.get(7)?,
                    })
                },
            )
            .optional()
            .context("query sync_state")
    }

    pub fn fetch_order(&self, order_id: &str) -> anyhow::Result<Option<CacheOrderRecord>> {
        let order = self
            .connection
            .query_row(
                r#"
                SELECT order_id, buyer_nickname, normalized_nickname, create_time,
                       confirm_receipt_time, is_waybill_received, waybill_received_time,
                       is_education_order, order_status, openid, raw_source, updated_at
                FROM orders WHERE order_id = ?1
                "#,
                params![order_id],
                |row| {
                    Ok(CacheOrderRecord {
                        order_id: row.get(0)?,
                        buyer_nickname: row.get(1)?,
                        normalized_nickname: row.get(2)?,
                        create_time: row.get(3)?,
                        confirm_receipt_time: row.get(4)?,
                        is_waybill_received: int_to_bool(row.get::<_, i64>(5)?),
                        waybill_received_time: row.get(6)?,
                        is_education_order: int_to_bool(row.get::<_, i64>(7)?),
                        order_status: row.get(8)?,
                        openid: row.get(9)?,
                        raw_source: row.get(10)?,
                        updated_at: row.get(11)?,
                        products: Vec::new(),
                    })
                },
            )
            .optional()?;

        let Some(mut order) = order else { return Ok(None) };
        let mut stmt = self.connection.prepare(
            r#"SELECT product_id, sku_id, sale_param, product_name, thumb_img
               FROM order_products WHERE order_id = ?1 ORDER BY id ASC"#,
        )?;
        let rows = stmt.query_map(params![order_id], |row| {
            Ok(CacheOrderProduct {
                product_id: row.get(0)?,
                sku_id: row.get(1)?,
                sale_param: row.get(2)?,
                product_name: row.get(3)?,
                thumb_img: row.get(4)?,
            })
        })?;
        order.products = rows.collect::<rusqlite::Result<Vec<_>>>()?;
        Ok(Some(order))
    }
}

fn bool_to_int(value: bool) -> i64 { if value { 1 } else { 0 } }
fn int_to_bool(value: i64) -> bool { value != 0 }

pub fn now_epoch_seconds() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs() as i64)
        .unwrap_or(0)
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    fn sample_order() -> CacheOrderRecord {
        CacheOrderRecord {
            order_id: "o-1".into(),
            buyer_nickname: "buyer".into(),
            normalized_nickname: "buyer".into(),
            create_time: 100,
            confirm_receipt_time: 120,
            is_waybill_received: true,
            waybill_received_time: 110,
            is_education_order: false,
            order_status: 20,
            openid: "openid".into(),
            raw_source: "order_api".into(),
            updated_at: 999,
            products: vec![CacheOrderProduct {
                product_id: "p1".into(),
                sku_id: "s1".into(),
                sale_param: "默认规格".into(),
                product_name: "仁和洗发水".into(),
                thumb_img: String::new(),
            }],
        }
    }

    #[test]
    fn initialize_and_persist_state() {
        let dir = tempdir().unwrap();
        let path = dir.path().join("order_cache.sqlite3");
        let repo = OrderCacheRepository::open(&path).unwrap();
        repo.initialize().unwrap();
        let state = SyncStateRecord {
            scope: "tls_order_cache".into(),
            coverage_start: 1,
            coverage_end: 2,
            last_incremental_start: 3,
            last_incremental_end: 4,
            last_success_at: 5,
            last_mode: "rebuild".into(),
            last_error: String::new(),
        };
        repo.save_state(&state).unwrap();
        assert_eq!(repo.get_state("tls_order_cache").unwrap(), Some(state));
    }

    #[test]
    fn upsert_and_fetch_order() {
        let dir = tempdir().unwrap();
        let path = dir.path().join("order_cache.sqlite3");
        let mut repo = OrderCacheRepository::open(&path).unwrap();
        repo.initialize().unwrap();
        repo.upsert_orders(&[sample_order()]).unwrap();
        let loaded = repo.fetch_order("o-1").unwrap().unwrap();
        assert_eq!(loaded.order_id, "o-1");
        assert_eq!(loaded.products.len(), 1);
        assert_eq!(loaded.products[0].product_id, "p1");
        assert!(loaded.is_waybill_received);
    }

    #[test]
    fn clear_all_removes_records() {
        let dir = tempdir().unwrap();
        let path = dir.path().join("order_cache.sqlite3");
        let mut repo = OrderCacheRepository::open(&path).unwrap();
        repo.initialize().unwrap();
        repo.upsert_orders(&[sample_order()]).unwrap();
        repo.clear_all().unwrap();
        assert!(repo.fetch_order("o-1").unwrap().is_none());
    }
}
