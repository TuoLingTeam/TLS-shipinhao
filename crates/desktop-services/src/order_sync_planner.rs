use crate::day_window::{end_of_day_timestamp, recent_day_range_timestamps};
use serde::{Deserialize, Serialize};

pub const ORDER_CACHE_COVERAGE_DAYS: i64 = 30;
pub const ORDER_CACHE_INCREMENTAL_DAYS: i64 = 3;
pub const ORDER_CACHE_INCREMENTAL_OVERLAP_DAYS: i64 = 1;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Default)]
#[serde(rename_all = "camelCase")]
pub struct SyncPlannerState {
    pub last_incremental_end: i64,
}

pub fn retention_start(now_end_of_day: i64) -> i64 {
    let fake_now = chrono::DateTime::from_timestamp(now_end_of_day, 0)
        .expect("valid timestamp")
        .with_timezone(&chrono::Utc);
    recent_day_range_timestamps(ORDER_CACHE_COVERAGE_DAYS, Some(fake_now)).0
}

pub fn sync_now(now: Option<chrono::DateTime<chrono::Utc>>) -> i64 {
    end_of_day_timestamp(now)
}

pub fn incremental_refresh_start(end_timestamp: i64, state: Option<&SyncPlannerState>) -> i64 {
    let overlap_seconds = ORDER_CACHE_INCREMENTAL_OVERLAP_DAYS * 86_400;
    let default_start =
        end_timestamp - (ORDER_CACHE_INCREMENTAL_DAYS + ORDER_CACHE_INCREMENTAL_OVERLAP_DAYS) * 86_400;
    let last_incremental_end = state.map(|value| value.last_incremental_end).unwrap_or(0);
    let preferred = if last_incremental_end > 0 {
        last_incremental_end - overlap_seconds
    } else {
        default_start
    };
    preferred.max(default_start)
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::{DateTime, TimeZone, Utc};

    #[test]
    fn retention_start_uses_end_of_day_coverage_window() {
        let now = DateTime::parse_from_rfc3339("2026-04-14T23:59:59Z").unwrap().with_timezone(&Utc);
        let start = retention_start(now.timestamp());
        assert_eq!(Utc.timestamp_opt(start, 0).unwrap().to_rfc3339(), "2026-03-15T00:00:00+00:00");
    }

    #[test]
    fn sync_now_uses_end_of_day_timestamp() {
        let now = DateTime::parse_from_rfc3339("2026-04-14T16:30:45Z").unwrap().with_timezone(&Utc);
        let end = sync_now(Some(now));
        assert_eq!(Utc.timestamp_opt(end, 0).unwrap().to_rfc3339(), "2026-04-14T23:59:59+00:00");
    }

    #[test]
    fn incremental_refresh_uses_overlap_from_previous_end() {
        let end_timestamp = 1_712_137_599;
        let state = SyncPlannerState { last_incremental_end: 1_712_051_200 };
        assert_eq!(incremental_refresh_start(end_timestamp, Some(&state)), 1_711_964_800);
    }

    #[test]
    fn incremental_refresh_falls_back_to_default_window() {
        let end_timestamp = 1_712_137_599;
        assert_eq!(incremental_refresh_start(end_timestamp, None), 1_711_791_999);
    }
}
