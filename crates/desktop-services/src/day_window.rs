use chrono::{DateTime, Datelike, Duration, NaiveDate, TimeZone, Utc};

pub fn start_of_day_timestamp(dt: Option<DateTime<Utc>>) -> i64 {
    let current = dt.unwrap_or_else(Utc::now);
    let naive = NaiveDate::from_ymd_opt(current.year(), current.month(), current.day())
        .expect("valid date")
        .and_hms_opt(0, 0, 0)
        .expect("valid time");
    Utc.from_utc_datetime(&naive).timestamp()
}

pub fn end_of_day_timestamp(dt: Option<DateTime<Utc>>) -> i64 {
    let current = dt.unwrap_or_else(Utc::now);
    let naive = NaiveDate::from_ymd_opt(current.year(), current.month(), current.day())
        .expect("valid date")
        .and_hms_opt(23, 59, 59)
        .expect("valid time");
    Utc.from_utc_datetime(&naive).timestamp()
}

pub fn recent_day_range_timestamps(days: i64, now: Option<DateTime<Utc>>) -> (i64, i64) {
    let current = now.unwrap_or_else(Utc::now);
    let safe_days = days.max(0);
    let start_dt = current - Duration::days(safe_days);
    (
        start_of_day_timestamp(Some(start_dt)),
        end_of_day_timestamp(Some(current)),
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::DateTime;

    #[test]
    fn recent_day_range_uses_natural_day_boundaries() {
        let now = DateTime::parse_from_rfc3339("2026-04-14T16:30:45Z")
            .unwrap()
            .with_timezone(&Utc);
        let (start, end) = recent_day_range_timestamps(15, Some(now));
        assert_eq!(
            Utc.timestamp_opt(start, 0).unwrap().to_rfc3339(),
            "2026-03-30T00:00:00+00:00"
        );
        assert_eq!(
            Utc.timestamp_opt(end, 0).unwrap().to_rfc3339(),
            "2026-04-14T23:59:59+00:00"
        );
    }
}
