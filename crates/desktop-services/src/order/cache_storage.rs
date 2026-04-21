//! 订单富缓存的 SQLite 实现 + DDL/迁移辅助。
//!
//! trait `OrderCacheRepository` 与 DTO 定义在 [`crate::order_cache_repository`]。
//! 这里通过 `pub use` 对外兼容导出，避免破坏旧的 import 路径
//! `desktop_services::order_cache_storage::{CacheOrderRecord, ...}`。

use anyhow::Context;
use rusqlite::{params, Connection, OptionalExtension};
use std::collections::{HashMap, HashSet};
use std::path::Path;
use std::sync::Mutex;
use std::time::{SystemTime, UNIX_EPOCH};

pub use crate::order_cache_repository::{
    CacheOrderProduct, CacheOrderRecord, OrderCacheRepository, SyncStateRecord,
};

pub const CURRENT_SCHEMA_VERSION: i32 = 2;

const CREATE_ORDERS_SQL: &str = r#"
CREATE TABLE IF NOT EXISTS orders (
    order_id TEXT PRIMARY KEY,
    buyer_nickname TEXT NOT NULL DEFAULT '',
    normalized_nickname TEXT NOT NULL DEFAULT '',
    amount_cent INTEGER NOT NULL DEFAULT 0,
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
"#;

const CREATE_ORDER_PRODUCTS_SQL: &str = r#"
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
"#;

const CREATE_SYNC_STATE_SQL: &str = r#"
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
"#;

const CREATE_CACHE_SEGMENTS_SQL: &str = r#"
CREATE TABLE IF NOT EXISTS cache_segments (
    scope TEXT NOT NULL,
    start_ts INTEGER NOT NULL,
    end_ts INTEGER NOT NULL,
    status TEXT NOT NULL DEFAULT 'complete',
    updated_at INTEGER NOT NULL DEFAULT 0,
    PRIMARY KEY (scope, start_ts, end_ts)
);
"#;

const CREATE_INDEXES_SQL: &str = r#"
CREATE INDEX IF NOT EXISTS idx_orders_create_time ON orders(create_time DESC);
CREATE INDEX IF NOT EXISTS idx_products_order_id ON order_products(order_id);
CREATE INDEX IF NOT EXISTS idx_cache_segments_scope_start ON cache_segments(scope, start_ts, end_ts);
"#;

/// orders 表在 v2 schema 下必须存在的列清单。v1 单表数据库可能缺少这些列，
/// 迁移器通过反射 `PRAGMA table_info` 来增量补齐。
const ORDERS_V2_COLUMNS: &[(&str, &str)] = &[
    ("buyer_nickname", "TEXT NOT NULL DEFAULT ''"),
    ("normalized_nickname", "TEXT NOT NULL DEFAULT ''"),
    ("amount_cent", "INTEGER NOT NULL DEFAULT 0"),
    ("create_time", "INTEGER NOT NULL DEFAULT 0"),
    ("confirm_receipt_time", "INTEGER NOT NULL DEFAULT 0"),
    ("is_waybill_received", "INTEGER NOT NULL DEFAULT 0"),
    ("waybill_received_time", "INTEGER NOT NULL DEFAULT 0"),
    ("is_education_order", "INTEGER NOT NULL DEFAULT 0"),
    ("order_status", "INTEGER NOT NULL DEFAULT 0"),
    ("openid", "TEXT NOT NULL DEFAULT ''"),
    ("raw_source", "TEXT NOT NULL DEFAULT 'order_api'"),
    ("updated_at", "INTEGER NOT NULL DEFAULT 0"),
];

/// `OrderCacheRepository` 的 SQLite 实现。
///
/// 内部使用 `Mutex<Connection>` 串行化对单文件数据库的访问，
/// 使得 trait 方法可以保持 `&self` 并通过 `Arc<dyn OrderCacheRepository>`
/// 在多个 Tauri 线程间共享。
pub struct SqliteOrderCacheRepository {
    connection: Mutex<Connection>,
}

impl SqliteOrderCacheRepository {
    pub fn open(db_path: &Path) -> anyhow::Result<Self> {
        if let Some(parent) = db_path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        let connection = Connection::open(db_path)?;
        connection.pragma_update(None, "journal_mode", "WAL")?;
        // 打开外键约束：order_products → orders 的 ON DELETE CASCADE 需要这一步。
        connection.pragma_update(None, "foreign_keys", "ON")?;
        Ok(Self {
            connection: Mutex::new(connection),
        })
    }

    fn with_connection<R>(
        &self,
        f: impl FnOnce(&Connection) -> anyhow::Result<R>,
    ) -> anyhow::Result<R> {
        let guard = self
            .connection
            .lock()
            .map_err(|e| anyhow::anyhow!("order cache connection mutex poisoned: {e}"))?;
        f(&guard)
    }

    fn with_connection_mut<R>(
        &self,
        f: impl FnOnce(&mut Connection) -> anyhow::Result<R>,
    ) -> anyhow::Result<R> {
        let mut guard = self
            .connection
            .lock()
            .map_err(|e| anyhow::anyhow!("order cache connection mutex poisoned: {e}"))?;
        f(&mut guard)
    }

    fn read_user_version(conn: &Connection) -> anyhow::Result<i32> {
        let value: i32 = conn.pragma_query_value(None, "user_version", |row| row.get(0))?;
        Ok(value)
    }

    fn write_user_version(conn: &Connection, version: i32) -> anyhow::Result<()> {
        // PRAGMA user_version 不支持参数占位符；version 来自常量，不受外部输入污染。
        conn.execute_batch(&format!("PRAGMA user_version = {version};"))?;
        Ok(())
    }

    /// v0（空库）/ v1（单表 orders，字段不全）→ v2（4 表 + 索引）统一迁移入口。
    /// 所有变更在单事务内完成，失败回滚；成功后写入 user_version。
    fn migrate_to_current(conn: &Connection) -> anyhow::Result<()> {
        let tx = conn.unchecked_transaction()?;
        tx.execute_batch(CREATE_ORDERS_SQL)?;
        tx.execute_batch(CREATE_ORDER_PRODUCTS_SQL)?;
        tx.execute_batch(CREATE_SYNC_STATE_SQL)?;
        tx.execute_batch(CREATE_CACHE_SEGMENTS_SQL)?;

        let existing_columns = columns_of_table(&tx, "orders")?;
        for (column_name, column_ddl) in ORDERS_V2_COLUMNS {
            if !existing_columns.contains(*column_name) {
                tx.execute(
                    &format!("ALTER TABLE orders ADD COLUMN {column_name} {column_ddl}"),
                    [],
                )?;
            }
        }

        tx.execute_batch(CREATE_INDEXES_SQL)?;
        tx.commit()?;
        Ok(())
    }
}

impl OrderCacheRepository for SqliteOrderCacheRepository {
    fn initialize(&self) -> anyhow::Result<()> {
        self.with_connection(|conn| {
            let existing_version = Self::read_user_version(conn)?;
            if existing_version >= CURRENT_SCHEMA_VERSION {
                // 已处于目标版本：仅确保索引幂等创建，兼容手工删除过索引的场景。
                conn.execute_batch(CREATE_INDEXES_SQL)
                    .context("ensure indexes on up-to-date schema")?;
                return Ok(());
            }

            Self::migrate_to_current(conn)
                .with_context(|| format!("migrate order cache schema from v{existing_version}"))?;
            Self::write_user_version(conn, CURRENT_SCHEMA_VERSION)?;
            Ok(())
        })
    }

    fn clear_all(&self) -> anyhow::Result<()> {
        self.with_connection(|conn| {
            conn.execute("DELETE FROM order_products", [])?;
            conn.execute("DELETE FROM orders", [])?;
            conn.execute("DELETE FROM sync_state", [])?;
            conn.execute("DELETE FROM cache_segments", [])?;
            Ok(())
        })
    }

    fn upsert_orders(&self, orders: &[CacheOrderRecord]) -> anyhow::Result<usize> {
        if orders.is_empty() {
            return Ok(0);
        }
        self.with_connection_mut(|conn| {
            let tx = conn.transaction()?;
            for order in orders {
                tx.execute(
                    "DELETE FROM order_products WHERE order_id = ?1",
                    params![order.order_id],
                )?;
                tx.execute(
                    r#"
                    INSERT OR REPLACE INTO orders (
                        order_id, buyer_nickname, normalized_nickname, create_time,
                        amount_cent, confirm_receipt_time, is_waybill_received, waybill_received_time,
                        is_education_order, order_status, openid, raw_source, updated_at
                    ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13)
                    "#,
                    params![
                        order.order_id,
                        order.buyer_nickname,
                        order.normalized_nickname,
                        order.create_time,
                        order.amount_cent,
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
        })
    }

    fn save_state(&self, state: &SyncStateRecord) -> anyhow::Result<()> {
        self.with_connection(|conn| {
            conn.execute(
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
        })
    }

    fn get_state(&self, scope: &str) -> anyhow::Result<Option<SyncStateRecord>> {
        self.with_connection(|conn| {
            conn.query_row(
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
        })
    }

    fn fetch_order(&self, order_id: &str) -> anyhow::Result<Option<CacheOrderRecord>> {
        self.with_connection(|conn| {
            let order = conn
                .query_row(
                    r#"
                    SELECT order_id, buyer_nickname, normalized_nickname, amount_cent, create_time,
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
                            amount_cent: row.get(3)?,
                            create_time: row.get(4)?,
                            confirm_receipt_time: row.get(5)?,
                            is_waybill_received: int_to_bool(row.get::<_, i64>(6)?),
                            waybill_received_time: row.get(7)?,
                            is_education_order: int_to_bool(row.get::<_, i64>(8)?),
                            order_status: row.get(9)?,
                            openid: row.get(10)?,
                            raw_source: row.get(11)?,
                            updated_at: row.get(12)?,
                            products: Vec::new(),
                        })
                    },
                )
                .optional()?;

            let Some(mut order) = order else {
                return Ok(None);
            };
            let mut stmt = conn.prepare(
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
        })
    }

    fn mark_segment_complete(
        &self,
        scope: &str,
        start_timestamp: i64,
        end_timestamp: i64,
    ) -> anyhow::Result<()> {
        self.with_connection(|conn| {
            conn.execute(
                r#"
                INSERT OR REPLACE INTO cache_segments (scope, start_ts, end_ts, status, updated_at)
                VALUES (?1, ?2, ?3, 'complete', ?4)
                "#,
                params![scope, start_timestamp, end_timestamp, now_epoch_seconds()],
            )?;
            Ok(())
        })
    }

    fn get_complete_segments(
        &self,
        scope: &str,
        start_timestamp: i64,
        end_timestamp: i64,
    ) -> anyhow::Result<Vec<(i64, i64)>> {
        self.with_connection(|conn| {
            let mut stmt = conn.prepare(
                r#"
                SELECT start_ts, end_ts
                FROM cache_segments
                WHERE scope = ?1 AND status = 'complete' AND end_ts >= ?2 AND start_ts <= ?3
                ORDER BY start_ts ASC, end_ts ASC
                "#,
            )?;
            let rows = stmt.query_map(params![scope, start_timestamp, end_timestamp], |row| {
                Ok((row.get::<_, i64>(0)?, row.get::<_, i64>(1)?))
            })?;
            Ok(rows.collect::<rusqlite::Result<Vec<_>>>()?)
        })
    }

    fn get_missing_segments(
        &self,
        scope: &str,
        start_timestamp: i64,
        end_timestamp: i64,
        merge_tolerance: i64,
        min_gap_width: i64,
    ) -> anyhow::Result<Vec<(i64, i64)>> {
        if start_timestamp <= 0 || end_timestamp <= 0 || start_timestamp > end_timestamp {
            return Ok(Vec::new());
        }

        let raw_segments = self.get_complete_segments(scope, start_timestamp, end_timestamp)?;
        Ok(crate::order_gap_planner::compute_missing_segments(
            start_timestamp,
            end_timestamp,
            merge_tolerance,
            min_gap_width,
            raw_segments,
        ))
    }

    fn has_dirty_sale_param(&self) -> anyhow::Result<bool> {
        self.with_connection(|conn| {
            let count: i64 = conn.query_row(
                "SELECT COUNT(*) FROM order_products WHERE sale_param LIKE '[%'",
                [],
                |row| row.get(0),
            )?;
            Ok(count > 0)
        })
    }

    fn delete_older_than(&self, scope: &str, cutoff_timestamp: i64) -> anyhow::Result<usize> {
        self.with_connection(|conn| {
            let mut stmt = conn.prepare("SELECT order_id FROM orders WHERE create_time < ?1")?;
            let expired_rows =
                stmt.query_map(params![cutoff_timestamp], |row| row.get::<_, String>(0))?;
            let order_ids = expired_rows.collect::<rusqlite::Result<Vec<_>>>()?;
            if !order_ids.is_empty() {
                let tx = conn.unchecked_transaction()?;
                for order_id in &order_ids {
                    tx.execute(
                        "DELETE FROM order_products WHERE order_id = ?1",
                        params![order_id],
                    )?;
                    tx.execute("DELETE FROM orders WHERE order_id = ?1", params![order_id])?;
                }
                tx.execute(
                    "DELETE FROM cache_segments WHERE scope = ?1 AND end_ts < ?2",
                    params![scope, cutoff_timestamp],
                )?;
                tx.execute(
                    r#"
                    UPDATE cache_segments
                    SET start_ts = ?1, updated_at = ?2
                    WHERE scope = ?3 AND start_ts < ?1 AND end_ts >= ?1
                    "#,
                    params![cutoff_timestamp, now_epoch_seconds(), scope],
                )?;
                tx.commit()?;
            }
            Ok(order_ids.len())
        })
    }

    fn fetch_orders_in_range(
        &self,
        start_timestamp: i64,
        end_timestamp: i64,
    ) -> anyhow::Result<Vec<CacheOrderRecord>> {
        self.with_connection(|conn| {
            let mut order_stmt = conn.prepare(
                r#"
                SELECT order_id, buyer_nickname, normalized_nickname, amount_cent, create_time,
                       confirm_receipt_time, is_waybill_received, waybill_received_time,
                       is_education_order, order_status, openid, raw_source, updated_at
                FROM orders
                WHERE create_time >= ?1 AND create_time <= ?2
                ORDER BY create_time DESC, order_id DESC
                "#,
            )?;
            let order_rows = order_stmt.query_map(params![start_timestamp, end_timestamp], |row| {
                Ok(CacheOrderRecord {
                    order_id: row.get(0)?,
                    buyer_nickname: row.get(1)?,
                    normalized_nickname: row.get(2)?,
                    amount_cent: row.get(3)?,
                    create_time: row.get(4)?,
                    confirm_receipt_time: row.get(5)?,
                    is_waybill_received: int_to_bool(row.get::<_, i64>(6)?),
                    waybill_received_time: row.get(7)?,
                    is_education_order: int_to_bool(row.get::<_, i64>(8)?),
                    order_status: row.get(9)?,
                    openid: row.get(10)?,
                    raw_source: row.get(11)?,
                    updated_at: row.get(12)?,
                    products: Vec::new(),
                })
            })?;
            let mut orders = order_rows.collect::<rusqlite::Result<Vec<_>>>()?;

            let mut product_stmt = conn.prepare(
                r#"
                SELECT p.order_id, p.product_id, p.sku_id, p.sale_param, p.product_name, p.thumb_img
                FROM order_products p
                JOIN orders o ON o.order_id = p.order_id
                WHERE o.create_time >= ?1 AND o.create_time <= ?2
                ORDER BY o.create_time DESC, p.order_id ASC, p.id ASC
                "#,
            )?;
            let product_rows =
                product_stmt.query_map(params![start_timestamp, end_timestamp], |row| {
                    Ok((
                        row.get::<_, String>(0)?,
                        CacheOrderProduct {
                            product_id: row.get(1)?,
                            sku_id: row.get(2)?,
                            sale_param: row.get(3)?,
                            product_name: row.get(4)?,
                            thumb_img: row.get(5)?,
                        },
                    ))
                })?;
            let mut products_by_order: HashMap<String, Vec<CacheOrderProduct>> = HashMap::new();
            for row in product_rows {
                let (order_id, product) = row?;
                products_by_order.entry(order_id).or_default().push(product);
            }
            for order in &mut orders {
                order.products = products_by_order
                    .remove(&order.order_id)
                    .unwrap_or_default();
            }
            Ok(orders)
        })
    }

    fn count_orders(&self) -> anyhow::Result<usize> {
        self.with_connection(|conn| {
            let count = conn.query_row("SELECT COUNT(*) FROM orders", [], |row| {
                row.get::<_, i64>(0)
            })?;
            Ok(count.max(0) as usize)
        })
    }
}

fn bool_to_int(value: bool) -> i64 {
    if value {
        1
    } else {
        0
    }
}

fn int_to_bool(value: i64) -> bool {
    value != 0
}

/// 反射目标表的列集合，供 schema 迁移判断缺失字段。
fn columns_of_table(conn: &Connection, table: &str) -> anyhow::Result<HashSet<String>> {
    let mut stmt = conn.prepare(&format!("PRAGMA table_info({table})"))?;
    let rows = stmt.query_map([], |row| row.get::<_, String>(1))?;
    let mut columns = HashSet::new();
    for name in rows {
        columns.insert(name?);
    }
    Ok(columns)
}

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
            amount_cent: 3990,
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

    fn open_repo(path: &Path) -> SqliteOrderCacheRepository {
        SqliteOrderCacheRepository::open(path).unwrap()
    }

    #[test]
    fn initialize_and_persist_state() {
        let dir = tempdir().unwrap();
        let path = dir.path().join("order_cache.sqlite3");
        let repo = open_repo(&path);
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
        let repo = open_repo(&path);
        repo.initialize().unwrap();
        repo.upsert_orders(&[sample_order()]).unwrap();
        let loaded = repo.fetch_order("o-1").unwrap().unwrap();
        assert_eq!(loaded.order_id, "o-1");
        assert_eq!(loaded.products.len(), 1);
        assert_eq!(loaded.products[0].product_id, "p1");
        assert!(loaded.is_waybill_received);
    }

    #[test]
    fn computes_missing_segments_and_range_fetch() {
        let dir = tempdir().unwrap();
        let path = dir.path().join("order_cache.sqlite3");
        let repo = open_repo(&path);
        repo.initialize().unwrap();
        let mut first = sample_order();
        first.create_time = 1_000;
        let mut second = sample_order();
        second.order_id = "o-2".into();
        second.create_time = 2_000;
        repo.upsert_orders(&[first, second]).unwrap();
        repo.mark_segment_complete("tls_order_cache", 900, 1500)
            .unwrap();
        repo.mark_segment_complete("tls_order_cache", 1700, 2100)
            .unwrap();
        let missing = repo
            .get_missing_segments("tls_order_cache", 900, 2100, 120, 1)
            .unwrap();
        assert_eq!(missing, vec![(1501, 1699)]);
        let orders = repo.fetch_orders_in_range(900, 2_100).unwrap();
        assert_eq!(orders.len(), 2);
        assert_eq!(orders[0].order_id, "o-2");
    }

    #[test]
    fn delete_older_than_removes_old_orders_and_trims_segments() {
        let dir = tempdir().unwrap();
        let path = dir.path().join("order_cache.sqlite3");
        let repo = open_repo(&path);
        repo.initialize().unwrap();
        let mut first = sample_order();
        first.create_time = 1_000;
        let mut second = sample_order();
        second.order_id = "o-2".into();
        second.create_time = 2_000;
        repo.upsert_orders(&[first, second]).unwrap();
        repo.mark_segment_complete("tls_order_cache", 900, 2100)
            .unwrap();
        let deleted = repo.delete_older_than("tls_order_cache", 1_500).unwrap();
        assert_eq!(deleted, 1);
        let remaining = repo.fetch_orders_in_range(0, 3_000).unwrap();
        assert_eq!(remaining.len(), 1);
        assert_eq!(remaining[0].order_id, "o-2");
        let segments = repo
            .get_complete_segments("tls_order_cache", 0, 3_000)
            .unwrap();
        assert_eq!(segments, vec![(1_500, 2_100)]);
    }

    #[test]
    fn clear_all_removes_records() {
        let dir = tempdir().unwrap();
        let path = dir.path().join("order_cache.sqlite3");
        let repo = open_repo(&path);
        repo.initialize().unwrap();
        repo.upsert_orders(&[sample_order()]).unwrap();
        repo.clear_all().unwrap();
        assert!(repo.fetch_order("o-1").unwrap().is_none());
    }

    fn table_exists(repo: &SqliteOrderCacheRepository, name: &str) -> bool {
        repo.with_connection(|conn| {
            Ok(conn
                .query_row(
                    "SELECT 1 FROM sqlite_master WHERE type='table' AND name = ?1",
                    params![name],
                    |row| row.get::<_, i64>(0),
                )
                .optional()
                .unwrap()
                .is_some())
        })
        .unwrap()
    }

    fn index_exists(repo: &SqliteOrderCacheRepository, name: &str) -> bool {
        repo.with_connection(|conn| {
            Ok(conn
                .query_row(
                    "SELECT 1 FROM sqlite_master WHERE type='index' AND name = ?1",
                    params![name],
                    |row| row.get::<_, i64>(0),
                )
                .optional()
                .unwrap()
                .is_some())
        })
        .unwrap()
    }

    #[test]
    fn fresh_schema_contains_all_tables_indexes_and_wal_mode() {
        let dir = tempdir().unwrap();
        let path = dir.path().join("order_cache.sqlite3");
        let repo = open_repo(&path);
        repo.initialize().unwrap();

        for table in ["orders", "order_products", "sync_state", "cache_segments"] {
            assert!(table_exists(&repo, table), "missing table {table}");
        }
        for index in [
            "idx_orders_create_time",
            "idx_products_order_id",
            "idx_cache_segments_scope_start",
        ] {
            assert!(index_exists(&repo, index), "missing index {index}");
        }

        repo.with_connection(|conn| {
            let columns = columns_of_table(conn, "orders")?;
            for (column_name, _) in ORDERS_V2_COLUMNS {
                assert!(
                    columns.contains(*column_name),
                    "orders missing column {column_name}"
                );
            }
            let user_version: i32 =
                conn.pragma_query_value(None, "user_version", |row| row.get(0))?;
            assert_eq!(user_version, CURRENT_SCHEMA_VERSION);

            let journal_mode: String =
                conn.pragma_query_value(None, "journal_mode", |row| row.get(0))?;
            assert_eq!(journal_mode.to_lowercase(), "wal");
            Ok(())
        })
        .unwrap();
    }

    #[test]
    fn v1_single_table_schema_migrates_to_v2() {
        let dir = tempdir().unwrap();
        let path = dir.path().join("order_cache.sqlite3");
        {
            let legacy = Connection::open(&path).unwrap();
            legacy
                .execute_batch(
                    r#"
                    CREATE TABLE orders (
                        order_id TEXT PRIMARY KEY,
                        buyer_nickname TEXT NOT NULL DEFAULT '',
                        create_time INTEGER NOT NULL DEFAULT 0
                    );
                    INSERT INTO orders (order_id, buyer_nickname, create_time)
                    VALUES ('legacy-1', 'old buyer', 42);
                    "#,
                )
                .unwrap();
        }

        let repo = open_repo(&path);
        repo.initialize().unwrap();

        for table in ["order_products", "sync_state", "cache_segments"] {
            assert!(table_exists(&repo, table), "missing table {table}");
        }

        repo.with_connection(|conn| {
            let columns = columns_of_table(conn, "orders")?;
            for (column_name, _) in ORDERS_V2_COLUMNS {
                assert!(
                    columns.contains(*column_name),
                    "orders missing migrated column {column_name}"
                );
            }

            let legacy_row = conn.query_row(
                "SELECT buyer_nickname, raw_source FROM orders WHERE order_id = 'legacy-1'",
                [],
                |row| Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?)),
            )?;
            assert_eq!(legacy_row.0, "old buyer");
            assert_eq!(legacy_row.1, "order_api");

            let user_version: i32 =
                conn.pragma_query_value(None, "user_version", |row| row.get(0))?;
            assert_eq!(user_version, CURRENT_SCHEMA_VERSION);
            Ok(())
        })
        .unwrap();
    }

    #[test]
    fn foreign_key_cascade_deletes_products_when_order_removed() {
        let dir = tempdir().unwrap();
        let path = dir.path().join("order_cache.sqlite3");
        let repo = open_repo(&path);
        repo.initialize().unwrap();
        repo.upsert_orders(&[sample_order()]).unwrap();

        repo.with_connection(|conn| {
            conn.execute("DELETE FROM orders WHERE order_id = 'o-1'", [])?;
            let remaining: i64 = conn.query_row(
                "SELECT COUNT(*) FROM order_products WHERE order_id = 'o-1'",
                [],
                |row| row.get(0),
            )?;
            assert_eq!(
                remaining, 0,
                "ON DELETE CASCADE 必须级联删除 order_products"
            );
            Ok(())
        })
        .unwrap();
    }

    #[test]
    fn initialize_is_idempotent_when_already_at_current_version() {
        let dir = tempdir().unwrap();
        let path = dir.path().join("order_cache.sqlite3");
        let repo = open_repo(&path);
        repo.initialize().unwrap();
        repo.initialize().unwrap();
        repo.with_connection(|conn| {
            let user_version: i32 =
                conn.pragma_query_value(None, "user_version", |row| row.get(0))?;
            assert_eq!(user_version, CURRENT_SCHEMA_VERSION);
            Ok(())
        })
        .unwrap();
    }

    #[test]
    fn detects_dirty_sale_param_when_legacy_json_value_present() {
        let dir = tempdir().unwrap();
        let path = dir.path().join("order_cache.sqlite3");
        let repo = open_repo(&path);
        repo.initialize().unwrap();
        repo.upsert_orders(&[sample_order()]).unwrap();
        assert!(!repo.has_dirty_sale_param().unwrap());

        let mut dirty = sample_order();
        dirty.order_id = "o-dirty".into();
        dirty.products[0].sale_param = "[\"legacy-json\"]".into();
        repo.upsert_orders(&[dirty]).unwrap();
        assert!(repo.has_dirty_sale_param().unwrap());
    }
}
