use crate::order_cache_repository::{CacheOrderRecord, OrderCacheRepository, SyncStateRecord};
use crate::order_sync_planner::{
    incremental_refresh_start, retention_start, sync_now, SyncPlannerState,
};
use anyhow::Context;
use serde::{Deserialize, Serialize};
use std::sync::Arc;

pub const ORDER_CACHE_SCOPE: &str = "tls_order_cache";
pub const MERGE_TOLERANCE_SECONDS: i64 = 120;
pub const MIN_GAP_WIDTH_SECONDS: i64 = 300;
pub const ONE_DAY_SECONDS: i64 = 86_400;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Default)]
#[serde(rename_all = "camelCase")]
pub struct SyncWindowOrders {
    pub window_id: String,
    pub start_ts: i64,
    pub end_ts: i64,
    pub orders: Vec<CacheOrderRecord>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Default)]
#[serde(rename_all = "camelCase")]
pub struct CacheFetchResult {
    pub windows: Vec<SyncWindowOrders>,
    #[serde(default)]
    pub warnings: Vec<String>,
}

pub trait CacheOrderFinder {
    fn stop(&mut self);
    fn get_orders_for_cache(
        &mut self,
        earliest_time: i64,
        create_time_start: i64,
        create_time_end: i64,
    ) -> anyhow::Result<CacheFetchResult>;
}

pub struct OrderSyncService<F> {
    pub finder: F,
    pub repository: Arc<dyn OrderCacheRepository>,
    stopped: bool,
}

impl<F> OrderSyncService<F>
where
    F: CacheOrderFinder,
{
    /// 新建实例，持有一个共享的仓储实现（trait object）。
    ///
    /// 传入 `Arc<dyn OrderCacheRepository>` 允许业务层复用同一 sqlite 连接，
    /// 也方便单元测试用内存 Mock 替换真实数据库。
    pub fn new(finder: F, repository: Arc<dyn OrderCacheRepository>) -> Self {
        Self {
            finder,
            repository,
            stopped: false,
        }
    }

    pub fn stop(&mut self) {
        self.stopped = true;
        self.finder.stop();
    }

    pub fn sync_now_timestamp(&self, now: Option<chrono::DateTime<chrono::Utc>>) -> i64 {
        sync_now(now)
    }

    pub fn retention_start_timestamp(&self, now_end_of_day: i64) -> i64 {
        retention_start(now_end_of_day)
    }

    pub fn rebuild_cache(
        &mut self,
        now: Option<chrono::DateTime<chrono::Utc>>,
    ) -> anyhow::Result<(usize, Vec<String>)> {
        if self.stopped {
            return Ok((0, Vec::new()));
        }
        let end_timestamp = sync_now(now);
        let start_timestamp = retention_start(end_timestamp);
        self.repository.initialize()?;
        self.repository.clear_all()?;
        let (written_count, warnings) =
            self.sync_range(start_timestamp, end_timestamp, "rebuild")?;
        let _ = self
            .repository
            .delete_older_than(ORDER_CACHE_SCOPE, start_timestamp)?;
        Ok((written_count, warnings))
    }

    pub fn refresh_cache(
        &mut self,
        now: Option<chrono::DateTime<chrono::Utc>>,
    ) -> anyhow::Result<(usize, Vec<String>)> {
        if self.stopped {
            return Ok((0, Vec::new()));
        }
        self.repository.initialize()?;
        let end_timestamp = sync_now(now);
        let state = self.repository.get_state(ORDER_CACHE_SCOPE)?;
        let Some(state) = state else {
            return self.rebuild_cache(now);
        };

        let planner_state = SyncPlannerState {
            last_incremental_end: state.last_incremental_end,
        };
        let start_timestamp = incremental_refresh_start(end_timestamp, Some(&planner_state));
        let gaps = self.repository.get_missing_segments(
            ORDER_CACHE_SCOPE,
            start_timestamp,
            end_timestamp,
            MERGE_TOLERANCE_SECONDS,
            MIN_GAP_WIDTH_SECONDS,
        )?;
        if gaps.is_empty() {
            let cutoff = retention_start(end_timestamp);
            let _ = self
                .repository
                .delete_older_than(ORDER_CACHE_SCOPE, cutoff)?;
            return Ok((0, Vec::new()));
        }

        let mut total_written = 0;
        let mut all_warnings = Vec::new();
        for (gap_start, gap_end) in gaps {
            if self.stopped {
                break;
            }
            let (written_count, warnings) = self.sync_range(gap_start, gap_end, "incremental")?;
            total_written += written_count;
            all_warnings.extend(warnings);
        }
        let cutoff = retention_start(end_timestamp);
        let _ = self
            .repository
            .delete_older_than(ORDER_CACHE_SCOPE, cutoff)?;
        Ok((total_written, all_warnings))
    }

    pub fn refresh_recent_incremental_cache(
        &mut self,
        now: Option<chrono::DateTime<chrono::Utc>>,
    ) -> anyhow::Result<(usize, Vec<String>, i64, i64)> {
        self.repository.initialize()?;
        let end_timestamp = sync_now(now);
        let retention = retention_start(end_timestamp);
        if self.stopped {
            return Ok((0, Vec::new(), retention, end_timestamp));
        }

        let state = self.repository.get_state(ORDER_CACHE_SCOPE)?;
        let planner_state = state.as_ref().map(|state| SyncPlannerState {
            last_incremental_end: state.last_incremental_end,
        });
        let start_timestamp = incremental_refresh_start(end_timestamp, planner_state.as_ref());
        let gaps = self.repository.get_missing_segments(
            ORDER_CACHE_SCOPE,
            start_timestamp,
            end_timestamp,
            MERGE_TOLERANCE_SECONDS,
            MIN_GAP_WIDTH_SECONDS,
        )?;

        let mut total_written = 0;
        let mut warnings = Vec::new();
        for (segment_start, segment_end) in gaps {
            if self.stopped {
                break;
            }
            let (written_count, gap_warnings) =
                self.sync_range(segment_start, segment_end, "incremental")?;
            total_written += written_count;
            warnings.extend(gap_warnings);
        }
        let _ = self
            .repository
            .delete_older_than(ORDER_CACHE_SCOPE, retention)?;
        Ok((total_written, warnings, retention, end_timestamp))
    }

    pub fn ensure_recent_cache(
        &mut self,
        now: Option<chrono::DateTime<chrono::Utc>>,
    ) -> anyhow::Result<(usize, Vec<String>, i64, i64)> {
        self.repository.initialize()?;
        let end_timestamp = sync_now(now);
        let start_timestamp = retention_start(end_timestamp);

        let state = self.repository.get_state(ORDER_CACHE_SCOPE)?;
        if state.is_none() {
            let (written, warnings) = self.rebuild_cache(now)?;
            return Ok((written, warnings, start_timestamp, end_timestamp));
        }

        let mut total_written = 0;
        let mut warnings = Vec::new();
        let gaps = self.repository.get_missing_segments(
            ORDER_CACHE_SCOPE,
            start_timestamp,
            end_timestamp,
            MERGE_TOLERANCE_SECONDS,
            MIN_GAP_WIDTH_SECONDS,
        )?;
        for (segment_start, segment_end) in gaps {
            if self.stopped {
                break;
            }
            let (written_count, gap_warnings) =
                self.sync_range(segment_start, segment_end, "gap_fill")?;
            total_written += written_count;
            warnings.extend(gap_warnings);
        }
        let (refresh_written, refresh_warnings) = self.refresh_cache(now)?;
        total_written += refresh_written;
        warnings.extend(refresh_warnings);
        Ok((total_written, warnings, start_timestamp, end_timestamp))
    }

    pub fn ensure_orders(
        &mut self,
        earliest_time: i64,
        now: Option<chrono::DateTime<chrono::Utc>>,
    ) -> anyhow::Result<(Vec<CacheOrderRecord>, Vec<String>)> {
        let (_, warnings, recent_start, recent_end) = self.ensure_recent_cache(now)?;
        let fetch_start = earliest_time.max(recent_start);
        let orders = self
            .repository
            .fetch_orders_in_range(fetch_start, recent_end)?;
        Ok((orders, warnings))
    }

    pub fn fetch_full_scan_orders(
        &mut self,
        earliest_time: i64,
        now: Option<chrono::DateTime<chrono::Utc>>,
    ) -> anyhow::Result<(Vec<CacheOrderRecord>, Vec<String>)> {
        let (_, mut warnings, recent_start, recent_end) = self.ensure_recent_cache(now)?;
        let mut recent_orders = self
            .repository
            .fetch_orders_in_range(earliest_time.max(recent_start), recent_end)?;
        if earliest_time >= recent_start {
            return Ok((recent_orders, warnings));
        }

        let temporary_end = recent_start - 1;
        let temporary = self
            .finder
            .get_orders_for_cache(earliest_time, earliest_time, temporary_end)
            .context("fetch temporary full-scan orders")?;
        warnings.extend(temporary.warnings);
        let mut combined = Vec::new();
        for window in temporary.windows {
            combined.extend(window.orders);
        }
        combined.append(&mut recent_orders);
        Ok((deduplicate_orders_by_id(combined), warnings))
    }

    fn sync_range(
        &mut self,
        start_timestamp: i64,
        end_timestamp: i64,
        mode: &str,
    ) -> anyhow::Result<(usize, Vec<String>)> {
        if self.stopped
            || start_timestamp <= 0
            || end_timestamp <= 0
            || start_timestamp > end_timestamp
        {
            return Ok((0, Vec::new()));
        }
        let fetched = self
            .finder
            .get_orders_for_cache(start_timestamp, start_timestamp, end_timestamp)
            .with_context(|| {
                format!("fetch cache orders for {start_timestamp}..{end_timestamp}")
            })?;
        let mut persisted_orders = Vec::new();
        for window in &fetched.windows {
            if self.stopped || window.orders.is_empty() {
                continue;
            }
            self.repository.upsert_orders(&window.orders)?;
            persisted_orders.extend(window.orders.clone());
        }
        self.repository
            .mark_segment_complete(ORDER_CACHE_SCOPE, start_timestamp, end_timestamp)?;
        let unique_written = count_unique_order_ids(&persisted_orders);
        let now_ts = sync_now(None);
        let retention = retention_start(now_ts);
        self.repository.save_state(&SyncStateRecord {
            scope: ORDER_CACHE_SCOPE.to_string(),
            coverage_start: retention,
            coverage_end: now_ts,
            last_incremental_start: if matches!(mode, "incremental" | "rebuild") {
                start_timestamp
            } else {
                0
            },
            last_incremental_end: if matches!(mode, "incremental" | "rebuild") {
                end_timestamp
            } else {
                0
            },
            last_success_at: now_ts,
            last_mode: mode.to_string(),
            last_error: String::new(),
        })?;
        Ok((unique_written, fetched.warnings))
    }
}

fn count_unique_order_ids(orders: &[CacheOrderRecord]) -> usize {
    let mut unique = std::collections::BTreeSet::new();
    for order in orders {
        if !order.order_id.is_empty() {
            unique.insert(order.order_id.clone());
        }
    }
    unique.len()
}

pub fn deduplicate_orders_by_id(orders: Vec<CacheOrderRecord>) -> Vec<CacheOrderRecord> {
    let mut seen = std::collections::BTreeSet::new();
    let mut deduplicated = Vec::new();
    for order in orders {
        if order.order_id.is_empty() || seen.insert(order.order_id.clone()) {
            deduplicated.push(order);
        }
    }
    deduplicated
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::order_cache_repository::{CacheOrderProduct, CacheOrderRecord};
    use crate::order_cache_storage::SqliteOrderCacheRepository;
    use chrono::{DateTime, Utc};
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
    }

    impl FakeFinder {
        fn with_responses(responses: Vec<CacheFetchResult>) -> Self {
            Self {
                responses,
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
            receiver_name: "李**".into(),
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
        let (written, warnings) = service.refresh_cache(Some(now)).unwrap();
        assert_eq!(written, 1);
        assert!(warnings.is_empty());
        assert_eq!(service.finder.calls.len(), 1);
        assert_eq!(service.finder.calls[0], (3_196_920, 3_196_920, 3_542_399));
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
}
