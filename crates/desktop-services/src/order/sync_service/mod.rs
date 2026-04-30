use crate::day_window::{end_of_day_timestamp, start_of_day_timestamp};
use crate::order_cache_repository::{
    is_cancelled_order_status, CacheOrderRecord, OrderCacheRepository, SyncStateRecord,
};
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

fn sync_completed_at(now: Option<chrono::DateTime<chrono::Utc>>) -> i64 {
    now.unwrap_or_else(chrono::Utc::now).timestamp()
}

impl<F> OrderSyncService<F>
where
    F: CacheOrderFinder,
{
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
            self.sync_range(start_timestamp, end_timestamp, "rebuild", now)?;
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
            let (written_count, warnings) =
                self.sync_range(gap_start, gap_end, "incremental", now)?;
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
                self.sync_range(segment_start, segment_end, "incremental", now)?;
            total_written += written_count;
            warnings.extend(gap_warnings);
        }
        let _ = self
            .repository
            .delete_older_than(ORDER_CACHE_SCOPE, retention)?;
        Ok((total_written, warnings, retention, end_timestamp))
    }

    /// 补齐指定窗口缺口，供评价 / 品退匹配前保障缓存覆盖。
    ///
    /// 返回的 `candidate_start` 会扩展到保留窗口起点，允许匹配早于查询范围的老订单。
    pub fn ensure_window_covered(
        &mut self,
        target_start: i64,
        target_end: i64,
        now: Option<chrono::DateTime<chrono::Utc>>,
    ) -> anyhow::Result<(usize, Vec<String>, i64, i64)> {
        self.repository.initialize()?;
        if self.stopped || target_start <= 0 || target_end <= 0 || target_start > target_end {
            return Ok((0, Vec::new(), target_start, target_end));
        }

        let gaps = self.repository.get_missing_segments(
            ORDER_CACHE_SCOPE,
            target_start,
            target_end,
            MERGE_TOLERANCE_SECONDS,
            MIN_GAP_WIDTH_SECONDS,
        )?;

        let mut total_written = 0;
        let mut warnings = Vec::new();
        for (gap_start, gap_end) in gaps {
            if self.stopped {
                break;
            }
            let (written, gap_warnings) =
                self.sync_range(gap_start, gap_end, "window_fill", now)?;
            total_written += written;
            warnings.extend(gap_warnings);
        }

        let now_ts = sync_now(now);
        let cutoff = retention_start(now_ts);
        let _ = self
            .repository
            .delete_older_than(ORDER_CACHE_SCOPE, cutoff)?;

        let candidate_start = cutoff.min(target_start);
        Ok((total_written, warnings, candidate_start, target_end))
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
                self.sync_range(segment_start, segment_end, "gap_fill", now)?;
            total_written += written_count;
            warnings.extend(gap_warnings);
        }
        let (refresh_written, refresh_warnings) = self.refresh_cache(now)?;
        total_written += refresh_written;
        warnings.extend(refresh_warnings);
        Ok((total_written, warnings, start_timestamp, end_timestamp))
    }

    /// 手动同步时补齐近 30 天稳定窗口，并额外拉取今天自然日。
    ///
    /// 今天不写 sync_state，避免当天新增订单被误判为已覆盖。
    pub fn ensure_recent_and_today_cache(
        &mut self,
        now: Option<chrono::DateTime<chrono::Utc>>,
    ) -> anyhow::Result<(usize, Vec<String>, i64, i64)> {
        let current = now.unwrap_or_else(chrono::Utc::now);
        let (recent_written, mut warnings, recent_start, recent_end) =
            self.ensure_recent_cache(Some(current))?;
        if self.stopped {
            return Ok((recent_written, warnings, recent_start, recent_end));
        }

        let today_start = start_of_day_timestamp(Some(current));
        let today_end = end_of_day_timestamp(Some(current));
        let (today_written, today_warnings) =
            self.sync_range_without_state(today_start, today_end, "today", Some(current))?;
        warnings.extend(today_warnings);

        Ok((
            recent_written + today_written,
            warnings,
            recent_start,
            recent_end,
        ))
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
        now: Option<chrono::DateTime<chrono::Utc>>,
    ) -> anyhow::Result<(usize, Vec<String>)> {
        self.sync_range_inner(start_timestamp, end_timestamp, mode, now, true, true)
    }

    fn sync_range_without_state(
        &mut self,
        start_timestamp: i64,
        end_timestamp: i64,
        mode: &str,
        now: Option<chrono::DateTime<chrono::Utc>>,
    ) -> anyhow::Result<(usize, Vec<String>)> {
        self.sync_range_inner(start_timestamp, end_timestamp, mode, now, false, false)
    }

    fn sync_range_inner(
        &mut self,
        start_timestamp: i64,
        end_timestamp: i64,
        mode: &str,
        now: Option<chrono::DateTime<chrono::Utc>>,
        mark_segment_complete: bool,
        save_sync_state: bool,
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
        if mark_segment_complete {
            self.repository.mark_segment_complete(
                ORDER_CACHE_SCOPE,
                start_timestamp,
                end_timestamp,
            )?;
        }
        let unique_written = count_unique_order_ids(&persisted_orders);
        if save_sync_state {
            let now_ts = sync_now(now);
            let completed_at = sync_completed_at(now);
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
                last_success_at: completed_at,
                last_mode: mode.to_string(),
                last_error: String::new(),
            })?;
        }
        Ok((unique_written, fetched.warnings))
    }
}

fn count_unique_order_ids(orders: &[CacheOrderRecord]) -> usize {
    let mut unique = std::collections::BTreeSet::new();
    for order in orders {
        if !order.order_id.is_empty() && !is_cancelled_order_status(order.order_status) {
            unique.insert(order.order_id.clone());
        }
    }
    unique.len()
}

pub fn deduplicate_orders_by_id(orders: Vec<CacheOrderRecord>) -> Vec<CacheOrderRecord> {
    let mut seen = std::collections::BTreeSet::new();
    let mut deduplicated = Vec::new();
    for order in orders {
        if is_cancelled_order_status(order.order_status) {
            continue;
        }
        if order.order_id.is_empty() || seen.insert(order.order_id.clone()) {
            deduplicated.push(order);
        }
    }
    deduplicated
}

#[cfg(test)]
mod tests;
