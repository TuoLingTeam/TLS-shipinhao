//! `SqliteOrderCacheRepository` 与 schema 迁移辅助的回归测试。

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
fn upsert_cancelled_order_deletes_existing_cache_record() {
    let dir = tempdir().unwrap();
    let path = dir.path().join("order_cache.sqlite3");
    let repo = open_repo(&path);
    repo.initialize().unwrap();
    repo.upsert_orders(&[sample_order()]).unwrap();
    assert!(repo.fetch_order("o-1").unwrap().is_some());

    let mut cancelled = sample_order();
    cancelled.order_status = 250;
    repo.upsert_orders(&[cancelled]).unwrap();

    assert!(repo.fetch_order("o-1").unwrap().is_none());
    assert_eq!(repo.count_orders().unwrap(), 0);
}

#[test]
fn legacy_cancelled_order_rows_are_excluded_from_reads() {
    let dir = tempdir().unwrap();
    let path = dir.path().join("order_cache.sqlite3");
    let repo = open_repo(&path);
    repo.initialize().unwrap();
    repo.with_connection(|conn| {
        conn.execute(
            r#"
            INSERT INTO orders (
                order_id, buyer_nickname, normalized_nickname, amount_cent, create_time,
                confirm_receipt_time, is_waybill_received, waybill_received_time,
                is_education_order, order_status, openid, raw_source, updated_at
            ) VALUES ('cancelled-legacy', 'buyer', 'buyer', 100, 1000, 0, 0, 0, 0, 250, '', 'order_api', 1000)
            "#,
            [],
        )?;
        Ok(())
    })
    .unwrap();

    assert!(repo.fetch_order("cancelled-legacy").unwrap().is_none());
    assert!(repo.fetch_orders_in_range(900, 1_100).unwrap().is_empty());
    assert_eq!(repo.count_orders().unwrap(), 0);
    assert_eq!(repo.count_orders_in_range(900, 1_100).unwrap(), 0);
    assert_eq!(
        repo.max_order_create_time_in_range(900, 1_100).unwrap(),
        None
    );
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
    assert_eq!(
        repo.max_order_create_time_in_range(900, 2_100).unwrap(),
        Some(2_000)
    );
    assert_eq!(
        repo.max_order_create_time_in_range(2_100, 3_000).unwrap(),
        None
    );
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
        let user_version: i32 = conn.pragma_query_value(None, "user_version", |row| row.get(0))?;
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

        let user_version: i32 = conn.pragma_query_value(None, "user_version", |row| row.get(0))?;
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
        let user_version: i32 = conn.pragma_query_value(None, "user_version", |row| row.get(0))?;
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
