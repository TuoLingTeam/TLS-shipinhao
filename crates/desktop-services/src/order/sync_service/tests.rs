//! `OrderSyncService` 与重复消除工具的回归测试。
//!
//! 历史上和 `mod.rs` 同文件，2026 年起按 A1 大文件拆分外移到本文件，
//! 行为完全等价（`super::*` 仍指向 sync_service 模块顶层）。

use super::*;
use crate::order_cache_repository::{CacheOrderProduct, CacheOrderRecord};
use crate::order_cache_storage::SqliteOrderCacheRepository;
use chrono::{DateTime, TimeZone, Utc};
use tempfile::tempdir;

fn open_shared_repo(path: &std::path::Path) -> Arc<dyn OrderCacheRepository> {
    let repo = SqliteOrderCacheRepository::open(path).unwrap();
    Arc::new(repo)
}

#[derive(Default)]
struct FakeFinder {
    stopped: bool,
    responses: Vec<CacheFetchResult>,
    calls: Vec<(i64, i64, i64)>,
    /// 设为 `Some(msg)` 时，下一次 `get_orders_for_cache` 消耗该值并返回错误；
    /// 允许按顺序塞多条错误（FIFO），验证 sync_range 对 finder 错误的传递行为。
    errors: std::collections::VecDeque<String>,
}

impl FakeFinder {
    fn with_responses(responses: Vec<CacheFetchResult>) -> Self {
        Self {
            responses,
            ..Self::default()
        }
    }

    fn with_error(message: &str) -> Self {
        let mut errors = std::collections::VecDeque::new();
        errors.push_back(message.to_string());
        Self {
            errors,
            ..Self::default()
        }
    }
}

impl CacheOrderFinder for FakeFinder {
    fn stop(&mut self) {
        self.stopped = true;
    }

    fn get_orders_for_cache(
        &mut self,
        earliest_time: i64,
        create_time_start: i64,
        create_time_end: i64,
    ) -> anyhow::Result<CacheFetchResult> {
        self.calls
            .push((earliest_time, create_time_start, create_time_end));
        if let Some(message) = self.errors.pop_front() {
            return Err(anyhow::anyhow!(message));
        }
        Ok(if self.responses.is_empty() {
            CacheFetchResult::default()
        } else {
            self.responses.remove(0)
        })
    }
}

fn sample_order(order_id: &str, create_time: i64) -> CacheOrderRecord {
    CacheOrderRecord {
        order_id: order_id.into(),
        buyer_nickname: "buyer".into(),
        normalized_nickname: "buyer".into(),
        amount_cent: 3990,
        create_time,
        confirm_receipt_time: create_time + 100,
        is_waybill_received: true,
        waybill_received_time: create_time + 50,
        is_education_order: false,
        order_status: 20,
        openid: "openid".into(),
        raw_source: "order_api".into(),
        updated_at: create_time + 200,
        products: vec![CacheOrderProduct {
            product_id: "p1".into(),
            sku_id: "s1".into(),
            sale_param: "默认规格".into(),
            product_name: "仁和洗发水".into(),
            thumb_img: String::new(),
        }],
    }
}

fn sample_window(
    window_id: &str,
    start_ts: i64,
    end_ts: i64,
    orders: Vec<CacheOrderRecord>,
) -> SyncWindowOrders {
    SyncWindowOrders {
        window_id: window_id.into(),
        start_ts,
        end_ts,
        orders,
    }
}

#[test]
fn sync_range_persists_orders_and_state_via_rebuild() {
    let dir = tempdir().unwrap();
    let path = dir.path().join("order_cache.sqlite3");
    let finder = FakeFinder::with_responses(vec![CacheFetchResult {
        windows: vec![sample_window(
            "w1",
            432_000,
            3_110_399,
            vec![sample_order("o-1", 500_000)],
        )],
        warnings: vec!["warn-1".into()],
    }]);
    let repo = open_shared_repo(&path);
    let mut service = OrderSyncService::new(finder, repo);
    let now = DateTime::parse_from_rfc3339("1970-02-05T16:30:45Z")
        .unwrap()
        .with_timezone(&Utc);
    let (written, warnings) = service.rebuild_cache(Some(now)).unwrap();
    assert_eq!(written, 1);
    assert_eq!(warnings, vec!["warn-1"]);
    let state = service
        .repository
        .get_state(ORDER_CACHE_SCOPE)
        .unwrap()
        .unwrap();
    assert_eq!(state.last_mode, "rebuild");
    assert!(service.repository.fetch_order("o-1").unwrap().is_some());
}

#[test]
fn rebuild_cache_records_actual_completion_time_as_last_success_at() {
    let dir = tempdir().unwrap();
    let path = dir.path().join("order_cache.sqlite3");
    let finder = FakeFinder::with_responses(vec![CacheFetchResult {
        windows: vec![sample_window(
            "w1",
            1_776_403_200,
            1_776_489_599,
            vec![sample_order("o-actual-sync", 1_776_410_070)],
        )],
        warnings: vec![],
    }]);
    let repo = open_shared_repo(&path);
    let mut service = OrderSyncService::new(finder, repo);
    let now = DateTime::parse_from_rfc3339("2026-04-19T03:34:30Z")
        .unwrap()
        .with_timezone(&Utc);

    let (written, warnings) = service.rebuild_cache(Some(now)).unwrap();

    assert_eq!(written, 1);
    assert!(warnings.is_empty());
    let state = service
        .repository
        .get_state(ORDER_CACHE_SCOPE)
        .unwrap()
        .unwrap();
    assert_eq!(state.last_mode, "rebuild");
    assert_eq!(state.last_success_at, now.timestamp());
}

#[test]
fn refresh_cache_uses_gap_windows_and_trims_retention() {
    let dir = tempdir().unwrap();
    let path = dir.path().join("order_cache.sqlite3");
    let finder = FakeFinder::with_responses(vec![CacheFetchResult {
        windows: vec![sample_window(
            "gap",
            3_196_920,
            3_542_399,
            vec![sample_order("o-2", 3_250_000)],
        )],
        warnings: vec![],
    }]);
    let repo = open_shared_repo(&path);
    repo.initialize().unwrap();
    repo.save_state(&SyncStateRecord {
        scope: ORDER_CACHE_SCOPE.into(),
        coverage_start: 864_000,
        coverage_end: 3_542_399,
        last_incremental_start: 3_196_800,
        last_incremental_end: 3_283_200,
        last_success_at: 3_283_200,
        last_mode: "incremental".into(),
        last_error: String::new(),
    })
    .unwrap();
    repo.mark_segment_complete(ORDER_CACHE_SCOPE, 3_196_800, 3_196_919)
        .unwrap();
    let mut service = OrderSyncService::new(finder, repo);
    let now = DateTime::parse_from_rfc3339("1970-02-10T00:35:00Z")
        .unwrap()
        .with_timezone(&Utc);
    let end_timestamp = service.sync_now_timestamp(Some(now));
    let (written, warnings) = service.refresh_cache(Some(now)).unwrap();
    assert_eq!(written, 1);
    assert!(warnings.is_empty());
    assert_eq!(service.finder.calls.len(), 1);
    assert_eq!(
        service.finder.calls[0],
        (3_196_920, 3_196_920, end_timestamp)
    );
}

#[test]
fn review_incremental_cache_bootstrap_only_fetches_recent_incremental_window() {
    let dir = tempdir().unwrap();
    let path = dir.path().join("order_cache.sqlite3");
    let finder = FakeFinder::with_responses(vec![CacheFetchResult {
        windows: vec![sample_window(
            "recent-incremental",
            518_400,
            863_999,
            vec![sample_order("o-incremental", 700_000)],
        )],
        warnings: vec![],
    }]);
    let repo = open_shared_repo(&path);
    let mut service = OrderSyncService::new(finder, repo);
    let now = DateTime::parse_from_rfc3339("1970-01-10T08:00:00Z")
        .unwrap()
        .with_timezone(&Utc);
    let end_timestamp = service.sync_now_timestamp(Some(now));
    let expected_start = incremental_refresh_start(end_timestamp, None);
    let retention_start = service.retention_start_timestamp(end_timestamp);

    let (written, warnings, actual_retention_start, actual_end) =
        service.refresh_recent_incremental_cache(Some(now)).unwrap();

    assert_eq!(written, 1);
    assert!(warnings.is_empty());
    assert_eq!(actual_retention_start, retention_start);
    assert_eq!(actual_end, end_timestamp);
    assert_eq!(service.finder.calls.len(), 1);
    assert_eq!(
        service.finder.calls[0],
        (expected_start, expected_start, end_timestamp)
    );

    let state = service
        .repository
        .get_state(ORDER_CACHE_SCOPE)
        .unwrap()
        .unwrap();
    assert_eq!(state.last_mode, "incremental");
    assert_eq!(state.last_incremental_start, expected_start);
    assert_eq!(state.last_incremental_end, end_timestamp);
    assert!(service
        .repository
        .fetch_order("o-incremental")
        .unwrap()
        .is_some());
}

#[test]
fn ensure_window_covered_returns_zero_for_invalid_range() {
    let dir = tempdir().unwrap();
    let path = dir.path().join("order_cache.sqlite3");
    let finder = FakeFinder::with_responses(vec![]);
    let repo = open_shared_repo(&path);
    let mut service = OrderSyncService::new(finder, repo);
    let now = DateTime::parse_from_rfc3339("1970-02-10T00:35:00Z")
        .unwrap()
        .with_timezone(&Utc);

    let (written, warnings, candidate_start, candidate_end) = service
        .ensure_window_covered(0, 1_000_000, Some(now))
        .unwrap();
    assert_eq!(written, 0);
    assert!(warnings.is_empty());
    assert_eq!(candidate_start, 0);
    assert_eq!(candidate_end, 1_000_000);

    let (written, _, _, _) = service
        .ensure_window_covered(2_000_000, 1_000_000, Some(now))
        .unwrap();
    assert_eq!(written, 0);
    assert!(
        service.finder.calls.is_empty(),
        "无效窗口不应触发任何 finder 调用"
    );
}

#[test]
fn ensure_window_covered_fills_missing_segments_when_cache_empty() {
    let dir = tempdir().unwrap();
    let path = dir.path().join("order_cache.sqlite3");
    let finder = FakeFinder::with_responses(vec![CacheFetchResult {
        windows: vec![sample_window(
            "window-fill",
            3_456_000,
            3_542_399,
            vec![sample_order("o-window-fill", 3_500_000)],
        )],
        warnings: vec!["window_fill_warn".into()],
    }]);
    let repo = open_shared_repo(&path);
    let mut service = OrderSyncService::new(finder, repo);
    let now = DateTime::parse_from_rfc3339("1970-02-10T00:35:00Z")
        .unwrap()
        .with_timezone(&Utc);

    let (written, warnings, candidate_start, candidate_end) = service
        .ensure_window_covered(3_456_000, 3_542_399, Some(now))
        .unwrap();
    assert_eq!(written, 1);
    assert_eq!(warnings, vec!["window_fill_warn"]);
    assert_eq!(candidate_end, 3_542_399);
    assert!(
        candidate_start <= 3_456_000,
        "candidate_start 应不晚于 target_start，实际 {candidate_start}"
    );
    assert_eq!(service.finder.calls.len(), 1);
    let state = service
        .repository
        .get_state(ORDER_CACHE_SCOPE)
        .unwrap()
        .unwrap();
    assert_eq!(state.last_mode, "window_fill");
    assert!(service
        .repository
        .fetch_order("o-window-fill")
        .unwrap()
        .is_some());
}

#[test]
fn ensure_window_covered_skips_fetch_when_segment_already_covered() {
    let dir = tempdir().unwrap();
    let path = dir.path().join("order_cache.sqlite3");
    let finder = FakeFinder::with_responses(vec![]);
    let repo = open_shared_repo(&path);
    repo.initialize().unwrap();
    repo.mark_segment_complete(ORDER_CACHE_SCOPE, 3_456_000, 3_542_399)
        .unwrap();
    let mut service = OrderSyncService::new(finder, repo);
    let now = DateTime::parse_from_rfc3339("1970-02-10T00:35:00Z")
        .unwrap()
        .with_timezone(&Utc);

    let (written, warnings, _, candidate_end) = service
        .ensure_window_covered(3_456_000, 3_542_399, Some(now))
        .unwrap();
    assert_eq!(written, 0, "窗口已完全覆盖时不应再发 finder 请求");
    assert!(warnings.is_empty());
    assert_eq!(candidate_end, 3_542_399);
    assert!(
        service.finder.calls.is_empty(),
        "已 covered 不应触发 fetch，实际调用 {} 次",
        service.finder.calls.len()
    );
}

#[test]
fn ensure_orders_reads_recent_cache_after_bootstrap() {
    let dir = tempdir().unwrap();
    let path = dir.path().join("order_cache.sqlite3");
    let finder = FakeFinder::with_responses(vec![CacheFetchResult {
        windows: vec![sample_window(
            "recent",
            864_000,
            3_542_399,
            vec![sample_order("o-1", 900_000), sample_order("o-2", 1_200_000)],
        )],
        warnings: vec!["bootstrapped".into()],
    }]);
    let repo = open_shared_repo(&path);
    let mut service = OrderSyncService::new(finder, repo);
    let now = DateTime::parse_from_rfc3339("1970-02-10T00:35:00Z")
        .unwrap()
        .with_timezone(&Utc);
    let (orders, warnings) = service.ensure_orders(880_000, Some(now)).unwrap();
    assert_eq!(orders.len(), 2);
    assert_eq!(warnings, vec!["bootstrapped"]);
}

#[test]
fn ensure_recent_and_today_cache_fetches_today_without_marking_complete() {
    let dir = tempdir().unwrap();
    let path = dir.path().join("order_cache.sqlite3");
    let now = DateTime::parse_from_rfc3339("2026-04-29T04:00:00Z")
        .unwrap()
        .with_timezone(&Utc);
    let recent_end = sync_now(Some(now));
    let recent_start = retention_start(recent_end);
    let today_start = crate::day_window::start_of_day_timestamp(Some(now));
    let today_end = crate::day_window::end_of_day_timestamp(Some(now));
    let finder = FakeFinder::with_responses(vec![
        CacheFetchResult {
            windows: vec![sample_window(
                "recent",
                recent_start,
                recent_end,
                vec![sample_order("o-recent", recent_start + 3_600)],
            )],
            warnings: vec![],
        },
        CacheFetchResult {
            windows: vec![sample_window(
                "today",
                today_start,
                today_end,
                vec![sample_order("o-today", today_start + 3_600)],
            )],
            warnings: vec!["today-warning".into()],
        },
    ]);
    let repo = open_shared_repo(&path);
    let mut service = OrderSyncService::new(finder, repo);

    let (written, warnings, coverage_start, coverage_end) =
        service.ensure_recent_and_today_cache(Some(now)).unwrap();

    assert_eq!(written, 2);
    assert_eq!(warnings, vec!["today-warning"]);
    assert_eq!(
        Utc.timestamp_opt(coverage_start, 0).unwrap().to_rfc3339(),
        "2026-03-29T16:00:00+00:00"
    );
    assert_eq!(
        Utc.timestamp_opt(coverage_end, 0).unwrap().to_rfc3339(),
        "2026-04-28T15:59:59+00:00"
    );
    assert_eq!(service.finder.calls.len(), 2);
    assert_eq!(
        Utc.timestamp_opt(service.finder.calls[0].1, 0)
            .unwrap()
            .to_rfc3339(),
        "2026-03-29T16:00:00+00:00"
    );
    assert_eq!(
        Utc.timestamp_opt(service.finder.calls[0].2, 0)
            .unwrap()
            .to_rfc3339(),
        "2026-04-28T15:59:59+00:00"
    );
    assert_eq!(
        Utc.timestamp_opt(service.finder.calls[1].1, 0)
            .unwrap()
            .to_rfc3339(),
        "2026-04-28T16:00:00+00:00"
    );
    assert_eq!(
        Utc.timestamp_opt(service.finder.calls[1].2, 0)
            .unwrap()
            .to_rfc3339(),
        "2026-04-29T15:59:59+00:00"
    );
    assert!(service
        .repository
        .fetch_order("o-recent")
        .unwrap()
        .is_some());
    assert!(service.repository.fetch_order("o-today").unwrap().is_some());

    let today_segments = service
        .repository
        .get_complete_segments(ORDER_CACHE_SCOPE, today_start, today_end)
        .unwrap();
    assert!(
        today_segments.is_empty(),
        "今天仍在进行中，不能标记为完整覆盖段，否则稍后新增订单会被跳过"
    );

    let state = service
        .repository
        .get_state(ORDER_CACHE_SCOPE)
        .unwrap()
        .unwrap();
    assert_eq!(
        Utc.timestamp_opt(state.coverage_end, 0)
            .unwrap()
            .to_rfc3339(),
        "2026-04-28T15:59:59+00:00"
    );
}

/// 内存 Mock：不依赖 sqlite，只实现测试覆盖流程所需的行为。
/// M3-02 AC 要求 Mock Repository 能跑通 OrderSyncService 主流程。
#[derive(Default)]
struct InMemoryRepository {
    inner: std::sync::Mutex<InMemoryData>,
}

#[derive(Default)]
struct InMemoryData {
    orders: std::collections::BTreeMap<String, CacheOrderRecord>,
    states: std::collections::BTreeMap<String, SyncStateRecord>,
    segments: std::collections::BTreeMap<String, Vec<(i64, i64)>>,
    initialized: bool,
}

impl OrderCacheRepository for InMemoryRepository {
    fn initialize(&self) -> anyhow::Result<()> {
        self.inner.lock().unwrap().initialized = true;
        Ok(())
    }

    fn clear_all(&self) -> anyhow::Result<()> {
        let mut data = self.inner.lock().unwrap();
        data.orders.clear();
        data.states.clear();
        data.segments.clear();
        Ok(())
    }

    fn upsert_orders(&self, orders: &[CacheOrderRecord]) -> anyhow::Result<usize> {
        let mut data = self.inner.lock().unwrap();
        for order in orders {
            data.orders.insert(order.order_id.clone(), order.clone());
        }
        Ok(orders.len())
    }

    fn save_state(&self, state: &SyncStateRecord) -> anyhow::Result<()> {
        self.inner
            .lock()
            .unwrap()
            .states
            .insert(state.scope.clone(), state.clone());
        Ok(())
    }

    fn get_state(&self, scope: &str) -> anyhow::Result<Option<SyncStateRecord>> {
        Ok(self.inner.lock().unwrap().states.get(scope).cloned())
    }

    fn fetch_order(&self, order_id: &str) -> anyhow::Result<Option<CacheOrderRecord>> {
        Ok(self.inner.lock().unwrap().orders.get(order_id).cloned())
    }

    fn mark_segment_complete(
        &self,
        scope: &str,
        start_timestamp: i64,
        end_timestamp: i64,
    ) -> anyhow::Result<()> {
        self.inner
            .lock()
            .unwrap()
            .segments
            .entry(scope.to_string())
            .or_default()
            .push((start_timestamp, end_timestamp));
        Ok(())
    }

    fn get_complete_segments(
        &self,
        scope: &str,
        start_timestamp: i64,
        end_timestamp: i64,
    ) -> anyhow::Result<Vec<(i64, i64)>> {
        let data = self.inner.lock().unwrap();
        let Some(items) = data.segments.get(scope) else {
            return Ok(Vec::new());
        };
        let mut filtered = items
            .iter()
            .filter(|(s, e)| *e >= start_timestamp && *s <= end_timestamp)
            .copied()
            .collect::<Vec<_>>();
        filtered.sort_by_key(|(s, e)| (*s, *e));
        Ok(filtered)
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
        let raw = self.get_complete_segments(scope, start_timestamp, end_timestamp)?;
        Ok(crate::order_gap_planner::compute_missing_segments(
            start_timestamp,
            end_timestamp,
            merge_tolerance,
            min_gap_width,
            raw,
        ))
    }

    fn has_dirty_sale_param(&self) -> anyhow::Result<bool> {
        let data = self.inner.lock().unwrap();
        Ok(data.orders.values().any(|order| {
            order
                .products
                .iter()
                .any(|product| product.sale_param.starts_with('['))
        }))
    }

    fn delete_older_than(&self, _scope: &str, cutoff_timestamp: i64) -> anyhow::Result<usize> {
        let mut data = self.inner.lock().unwrap();
        let ids = data
            .orders
            .iter()
            .filter(|(_, order)| order.create_time < cutoff_timestamp)
            .map(|(id, _)| id.clone())
            .collect::<Vec<_>>();
        let removed = ids.len();
        for id in ids {
            data.orders.remove(&id);
        }
        Ok(removed)
    }

    fn fetch_orders_in_range(
        &self,
        start_timestamp: i64,
        end_timestamp: i64,
    ) -> anyhow::Result<Vec<CacheOrderRecord>> {
        let data = self.inner.lock().unwrap();
        let mut orders = data
            .orders
            .values()
            .filter(|order| {
                order.create_time >= start_timestamp && order.create_time <= end_timestamp
            })
            .cloned()
            .collect::<Vec<_>>();
        orders.sort_by(|a, b| {
            b.create_time
                .cmp(&a.create_time)
                .then_with(|| b.order_id.cmp(&a.order_id))
        });
        Ok(orders)
    }

    fn count_orders(&self) -> anyhow::Result<usize> {
        Ok(self.inner.lock().unwrap().orders.len())
    }

    fn count_orders_in_range(
        &self,
        start_timestamp: i64,
        end_timestamp: i64,
    ) -> anyhow::Result<usize> {
        let data = self.inner.lock().unwrap();
        Ok(data
            .orders
            .values()
            .filter(|order| {
                order.create_time >= start_timestamp && order.create_time <= end_timestamp
            })
            .count())
    }

    fn max_order_create_time_in_range(
        &self,
        start_timestamp: i64,
        end_timestamp: i64,
    ) -> anyhow::Result<Option<i64>> {
        let data = self.inner.lock().unwrap();
        Ok(data
            .orders
            .values()
            .filter(|order| {
                order.create_time >= start_timestamp && order.create_time <= end_timestamp
            })
            .map(|order| order.create_time)
            .max())
    }
}

#[test]
fn rebuild_and_refresh_work_with_in_memory_repository_mock() {
    let finder = FakeFinder::with_responses(vec![CacheFetchResult {
        windows: vec![sample_window(
            "w1",
            432_000,
            3_110_399,
            vec![sample_order("o-1", 500_000)],
        )],
        warnings: vec![],
    }]);
    let repo: Arc<dyn OrderCacheRepository> = Arc::new(InMemoryRepository::default());
    let mut service = OrderSyncService::new(finder, repo);
    let now = DateTime::parse_from_rfc3339("1970-02-05T16:30:45Z")
        .unwrap()
        .with_timezone(&Utc);

    let (written, warnings) = service.rebuild_cache(Some(now)).unwrap();
    assert_eq!(written, 1);
    assert!(warnings.is_empty());

    let state = service
        .repository
        .get_state(ORDER_CACHE_SCOPE)
        .unwrap()
        .unwrap();
    assert_eq!(state.last_mode, "rebuild");
    assert!(service.repository.fetch_order("o-1").unwrap().is_some());
    assert_eq!(service.repository.count_orders().unwrap(), 1);
}

#[test]
fn full_scan_combines_temporary_and_recent_orders_without_duplicates() {
    let dir = tempdir().unwrap();
    let path = dir.path().join("order_cache.sqlite3");
    let finder = FakeFinder::with_responses(vec![
        CacheFetchResult {
            windows: vec![sample_window(
                "recent",
                864_000,
                3_542_399,
                vec![sample_order("o-1", 900_000), sample_order("o-2", 1_200_000)],
            )],
            warnings: vec![],
        },
        CacheFetchResult {
            windows: vec![sample_window(
                "old",
                120_000,
                863_999,
                vec![sample_order("o-0", 120_000), sample_order("o-1", 900_000)],
            )],
            warnings: vec!["temporary".into()],
        },
    ]);
    let repo = open_shared_repo(&path);
    let mut service = OrderSyncService::new(finder, repo);
    let now = DateTime::parse_from_rfc3339("1970-02-10T00:35:00Z")
        .unwrap()
        .with_timezone(&Utc);
    let (orders, warnings) = service.fetch_full_scan_orders(120_000, Some(now)).unwrap();
    assert_eq!(orders.len(), 3);
    assert_eq!(warnings, vec!["temporary"]);
    assert_eq!(orders[0].order_id, "o-0");
}

// ---- 错误分支 / 停任务路径回归（Pass 4 · T15） ------------------------

#[test]
fn rebuild_cache_surfaces_finder_error_with_context() {
    // finder 首次调用抛错时，sync_range 通过 .with_context(...) 将其包装为带窗口范围
    // 的 anyhow::Error 向上传递；错误不能被吞成 Ok((0, []))
    let dir = tempdir().unwrap();
    let path = dir.path().join("order_cache.sqlite3");
    let finder = FakeFinder::with_error("风控限流拒绝请求");
    let repo = open_shared_repo(&path);
    let mut service = OrderSyncService::new(finder, repo);
    let now = DateTime::parse_from_rfc3339("1970-02-05T16:30:45Z")
        .unwrap()
        .with_timezone(&Utc);

    let err = service.rebuild_cache(Some(now)).unwrap_err();
    let text = format!("{err:?}");
    assert!(
        text.contains("风控限流拒绝请求"),
        "原始错误消息必须保留：{text}"
    );
    assert!(
        text.contains("fetch cache orders"),
        "应带 sync_range 的 with_context 前缀：{text}",
    );
    // finder 被调用过一次（抛错的那一次）
    assert_eq!(service.finder.calls.len(), 1);
    // 失败不写 DB：订单库应为空
    assert!(service.repository.fetch_order("any").unwrap().is_none());
}

#[test]
fn rebuild_cache_short_circuits_and_does_not_touch_finder_after_stop() {
    // stop() 后 rebuild_cache 必须早退：不调用 finder、也不改状态表
    let dir = tempdir().unwrap();
    let path = dir.path().join("order_cache.sqlite3");
    let finder = FakeFinder::with_responses(vec![CacheFetchResult::default()]);
    let repo = open_shared_repo(&path);
    let mut service = OrderSyncService::new(finder, repo);

    service.stop();

    let now = DateTime::parse_from_rfc3339("1970-02-05T16:30:45Z")
        .unwrap()
        .with_timezone(&Utc);
    let (written, warnings) = service.rebuild_cache(Some(now)).unwrap();
    assert_eq!(written, 0);
    assert!(warnings.is_empty());
    assert!(service.finder.calls.is_empty(), "stop 后不应派发给 finder");
    // stop 后短路不会调用 repository.initialize()，因此 sync_state 表根本没创建。
    // 不断言 `get_state()`：返回的是查询错误而非 `None`，真正关键语义是「finder 零调用 + 返回 (0, [])」。
}

#[test]
fn sync_range_rejects_illegal_window_without_calling_finder() {
    // 非法时间窗（start > end / start <= 0 / end <= 0）直接返回 (0, []) 且不调 finder。
    // 通过直接调用 private sync_range 验证 —— #[cfg(test)] 同模块内可见。
    let dir = tempdir().unwrap();
    let path = dir.path().join("order_cache.sqlite3");
    let repo = open_shared_repo(&path);
    let mut service = OrderSyncService::new(FakeFinder::with_responses(vec![]), Arc::clone(&repo));

    // start > end
    let (written, warnings) = service.sync_range(5_000, 1_000, "rebuild", None).unwrap();
    assert_eq!(written, 0);
    assert!(warnings.is_empty());

    // start <= 0
    let (written, warnings) = service.sync_range(0, 1_000, "rebuild", None).unwrap();
    assert_eq!(written, 0);
    assert!(warnings.is_empty());

    // end <= 0
    let (written, warnings) = service.sync_range(100, 0, "rebuild", None).unwrap();
    assert_eq!(written, 0);
    assert!(warnings.is_empty());

    assert!(service.finder.calls.is_empty(), "非法窗口不应派发给 finder");
}
