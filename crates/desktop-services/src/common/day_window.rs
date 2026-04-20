use chrono::{DateTime, Duration, FixedOffset, TimeZone, Utc};

/// 业务基准时区：中国标准时间（UTC+8）。
///
/// 视频号卖家的"自然日"语义固定为北京时间 00:00:00 – 23:59:59。历史上这组
/// public API 用 `chrono::Local` 读取系统时区——开发机默认 `Asia/Shanghai`
/// 时能侥幸跑对，但在 CI（GitHub Actions runner 默认 UTC）或海外部署机器上
/// 会把"自然日"滑一个偏移，导致订单同步起止时间错位一整天。锁死 UTC+8 之
/// 后，生产行为与单元测试断言的"北京时间日界"一致，跨时区环境不再漂移。
fn china_offset() -> FixedOffset {
    FixedOffset::east_opt(8 * 3600).expect("UTC+8 offset must construct successfully")
}

pub fn start_of_day_timestamp(dt: Option<DateTime<Utc>>) -> i64 {
    let current = dt.unwrap_or_else(Utc::now);
    start_of_day_timestamp_in_timezone(current, &china_offset())
}

pub fn end_of_day_timestamp(dt: Option<DateTime<Utc>>) -> i64 {
    let current = dt.unwrap_or_else(Utc::now);
    end_of_day_timestamp_in_timezone(current, &china_offset())
}

pub fn end_of_previous_day_timestamp(dt: Option<DateTime<Utc>>) -> i64 {
    let current = dt.unwrap_or_else(Utc::now);
    let previous = current - Duration::days(1);
    end_of_day_timestamp_in_timezone(previous, &china_offset())
}

pub fn recent_day_range_timestamps(days: i64, now: Option<DateTime<Utc>>) -> (i64, i64) {
    let current = now.unwrap_or_else(Utc::now);
    let safe_days = days.max(0);
    recent_day_range_timestamps_in_timezone(safe_days, current, &china_offset())
}

fn boundary_timestamp_in_timezone<Tz: TimeZone>(
    dt: DateTime<Utc>,
    timezone: &Tz,
    hour: u32,
    minute: u32,
    second: u32,
) -> i64
where
    Tz::Offset: Copy,
{
    let local_dt = dt.with_timezone(timezone);
    let naive = local_dt
        .date_naive()
        .and_hms_opt(hour, minute, second)
        .expect("valid time");

    timezone
        .from_local_datetime(&naive)
        .single()
        .or_else(|| timezone.from_local_datetime(&naive).earliest())
        .or_else(|| timezone.from_local_datetime(&naive).latest())
        .expect("valid local timestamp")
        .with_timezone(&Utc)
        .timestamp()
}

fn start_of_day_timestamp_in_timezone<Tz: TimeZone>(dt: DateTime<Utc>, timezone: &Tz) -> i64
where
    Tz::Offset: Copy,
{
    boundary_timestamp_in_timezone(dt, timezone, 0, 0, 0)
}

fn end_of_day_timestamp_in_timezone<Tz: TimeZone>(dt: DateTime<Utc>, timezone: &Tz) -> i64
where
    Tz::Offset: Copy,
{
    boundary_timestamp_in_timezone(dt, timezone, 23, 59, 59)
}

fn recent_day_range_timestamps_in_timezone<Tz: TimeZone>(
    days: i64,
    now: DateTime<Utc>,
    timezone: &Tz,
) -> (i64, i64)
where
    Tz::Offset: Copy,
{
    let end_dt = now - Duration::days(1);
    let start_dt = now - Duration::days(days);
    (
        start_of_day_timestamp_in_timezone(start_dt, timezone),
        end_of_day_timestamp_in_timezone(end_dt, timezone),
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::{DateTime, FixedOffset, TimeZone};

    #[test]
    fn recent_day_range_uses_local_natural_day_boundaries() {
        let now = DateTime::parse_from_rfc3339("2026-04-19T06:30:45Z")
            .unwrap()
            .with_timezone(&Utc);
        let china = FixedOffset::east_opt(8 * 3600).unwrap();
        let (start, end) = recent_day_range_timestamps_in_timezone(30, now, &china);
        assert_eq!(
            Utc.timestamp_opt(start, 0).unwrap().to_rfc3339(),
            "2026-03-19T16:00:00+00:00"
        );
        assert_eq!(
            Utc.timestamp_opt(end, 0).unwrap().to_rfc3339(),
            "2026-04-18T15:59:59+00:00"
        );
    }
}
