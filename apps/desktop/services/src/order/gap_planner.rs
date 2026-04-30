//! 订单缓存时间段缺口计算。
//!
//! 对应 PRD §6.4 的三步算法：
//! 1. 裁剪：将已完成段裁剪到 `[start, end]` 内，过滤无效段。
//! 2. 合并：相邻段间隔 `gap <= merge_tolerance` 视为连续覆盖，合并为一段。
//! 3. 缺口：提取合并后仍未覆盖的区间，仅保留宽度 `>= min_gap_width` 的缺口。
//!
//! 参数默认值由 [`crate::order_sync_service`] 提供：
//! `MERGE_TOLERANCE_SECONDS = 120` / `MIN_GAP_WIDTH_SECONDS = 300`。

/// 返回 `[start_timestamp, end_timestamp]` 内未被已完成段覆盖的缺口列表。
///
/// 当输入时间戳非法（非正数或 start > end）时返回空 `Vec`，调用方据此跳过。
pub fn compute_missing_segments(
    start_timestamp: i64,
    end_timestamp: i64,
    merge_tolerance: i64,
    min_gap_width: i64,
    raw_segments: Vec<(i64, i64)>,
) -> Vec<(i64, i64)> {
    if start_timestamp <= 0 || end_timestamp <= 0 || start_timestamp > end_timestamp {
        return Vec::new();
    }

    let mut segments = raw_segments
        .into_iter()
        .map(|(seg_start, seg_end)| (seg_start.max(start_timestamp), seg_end.min(end_timestamp)))
        .filter(|(seg_start, seg_end)| seg_start <= seg_end)
        .collect::<Vec<_>>();

    if segments.is_empty() {
        return vec![(start_timestamp, end_timestamp)];
    }

    segments.sort_by_key(|(start, end)| (*start, *end));
    let mut merged: Vec<(i64, i64)> = Vec::new();
    for (seg_start, seg_end) in segments {
        match merged.last_mut() {
            Some((_, last_end)) if seg_start <= *last_end + merge_tolerance => {
                *last_end = (*last_end).max(seg_end);
            }
            _ => merged.push((seg_start, seg_end)),
        }
    }

    let mut missing = Vec::new();
    let mut cursor = start_timestamp;
    for (seg_start, seg_end) in merged {
        if cursor < seg_start {
            let gap_width = seg_start - cursor;
            if gap_width >= min_gap_width {
                missing.push((cursor, seg_start - 1));
            }
        }
        cursor = cursor.max(seg_end + 1);
    }
    if cursor <= end_timestamp {
        let gap_width = end_timestamp - cursor + 1;
        if gap_width >= min_gap_width {
            missing.push((cursor, end_timestamp));
        }
    }
    missing
}

#[cfg(test)]
mod tests {
    use super::*;

    /// 对应 M1/M3 线上场景的默认参数。
    const MERGE_TOLERANCE: i64 = 120;
    const MIN_GAP_WIDTH: i64 = 300;

    #[test]
    fn no_completed_segments_returns_full_range() {
        assert_eq!(
            compute_missing_segments(1_000, 10_000, MERGE_TOLERANCE, MIN_GAP_WIDTH, vec![]),
            vec![(1_000, 10_000)]
        );
    }

    #[test]
    fn three_segments_covering_entire_range_returns_empty() {
        let segments = vec![(0, 3_000), (3_001, 6_000), (6_001, 10_000)];
        assert!(
            compute_missing_segments(1_000, 10_000, MERGE_TOLERANCE, MIN_GAP_WIDTH, segments)
                .is_empty()
        );
    }

    #[test]
    fn two_segments_with_60s_interval_are_merged_below_tolerance() {
        // 中间 60s 间隔 < 120s → 合并成一段，覆盖 [1000, 6060] → 无缺口
        let segments = vec![(1_000, 3_000), (3_060, 6_060)];
        assert!(
            compute_missing_segments(1_000, 6_060, MERGE_TOLERANCE, MIN_GAP_WIDTH, segments)
                .is_empty()
        );
    }

    #[test]
    fn middle_500s_gap_is_reported_above_min_gap_width() {
        // 中间 500s 缺口 > 300s → 保留
        let segments = vec![(1_000, 3_000), (3_501, 6_000)];
        let gaps = compute_missing_segments(1_000, 6_000, MERGE_TOLERANCE, MIN_GAP_WIDTH, segments);
        assert_eq!(gaps, vec![(3_001, 3_500)]);
    }

    #[test]
    fn middle_200s_gap_is_ignored_below_min_gap_width() {
        // 中间 200s 缺口 < 300s → 过滤
        let segments = vec![(1_000, 3_000), (3_201, 6_000)];
        assert!(
            compute_missing_segments(1_000, 6_000, MERGE_TOLERANCE, MIN_GAP_WIDTH, segments)
                .is_empty()
        );
    }

    #[test]
    fn single_segment_leaves_head_and_tail_gaps_when_both_wide_enough() {
        // 单段在中间 → 头尾均为缺口
        let segments = vec![(5_000, 7_000)];
        let gaps =
            compute_missing_segments(1_000, 10_000, MERGE_TOLERANCE, MIN_GAP_WIDTH, segments);
        assert_eq!(gaps, vec![(1_000, 4_999), (7_001, 10_000)]);
    }

    #[test]
    fn segments_overlapping_range_boundaries_are_trimmed() {
        // 段超出 [start, end] 边界，应被裁剪
        let segments = vec![(-10_000, 2_000), (8_000, 100_000)];
        let gaps =
            compute_missing_segments(1_000, 10_000, MERGE_TOLERANCE, MIN_GAP_WIDTH, segments);
        // 裁剪后：[1000, 2000]、[8000, 10000]；中间缺口 2001..7999 → 宽度 5999 > 300
        assert_eq!(gaps, vec![(2_001, 7_999)]);
    }

    #[test]
    fn negative_start_returns_empty_vec() {
        assert!(
            compute_missing_segments(-1, 10_000, MERGE_TOLERANCE, MIN_GAP_WIDTH, vec![]).is_empty()
        );
    }

    #[test]
    fn start_greater_than_end_returns_empty_vec() {
        assert!(
            compute_missing_segments(10_000, 1_000, MERGE_TOLERANCE, MIN_GAP_WIDTH, vec![])
                .is_empty()
        );
    }

    #[test]
    fn zero_end_returns_empty_vec() {
        assert!(
            compute_missing_segments(1_000, 0, MERGE_TOLERANCE, MIN_GAP_WIDTH, vec![]).is_empty()
        );
    }

    #[test]
    fn duplicate_and_out_of_order_segments_are_normalized() {
        // 乱序 + 重复覆盖段 → 正确排序合并
        let segments = vec![
            (5_000, 7_000),
            (1_000, 3_000),
            (5_500, 7_500),
            (1_000, 2_000),
        ];
        let gaps =
            compute_missing_segments(1_000, 10_000, MERGE_TOLERANCE, MIN_GAP_WIDTH, segments);
        // 合并后：[1000,3000]、[5000,7500]；
        // 缺口：3001..4999（宽 1999 保留）、7501..10000（宽 2500 保留）
        assert_eq!(gaps, vec![(3_001, 4_999), (7_501, 10_000)]);
    }

    #[test]
    fn segments_entirely_outside_range_are_filtered_completely() {
        // 两段都在目标范围外 → 等同于空段
        let segments = vec![(0, 500), (20_000, 30_000)];
        let gaps =
            compute_missing_segments(1_000, 10_000, MERGE_TOLERANCE, MIN_GAP_WIDTH, segments);
        assert_eq!(gaps, vec![(1_000, 10_000)]);
    }

    #[test]
    fn tail_gap_exactly_at_min_gap_width_is_kept() {
        // 尾部缺口宽度恰好 = min_gap_width → 保留（>= 语义）
        let segments = vec![(1_000, 9_700)];
        let gaps = compute_missing_segments(
            1_000,
            9_999, // 尾缺口 9701..9999 宽度 299 < 300
            MERGE_TOLERANCE,
            MIN_GAP_WIDTH,
            segments.clone(),
        );
        assert!(gaps.is_empty());

        let gaps_exact = compute_missing_segments(
            1_000,
            10_000, // 尾缺口 9701..10000 宽度 300 = min_gap_width
            MERGE_TOLERANCE,
            MIN_GAP_WIDTH,
            segments,
        );
        assert_eq!(gaps_exact, vec![(9_701, 10_000)]);
    }

    #[test]
    fn contiguous_segments_at_tolerance_boundary_are_merged() {
        // 两段间隔 120s（seg_start = last_end + tolerance）→ 合并条件 `<=` 满足
        let segments = vec![(1_000, 3_000), (3_120, 6_000)];
        assert!(
            compute_missing_segments(1_000, 6_000, MERGE_TOLERANCE, MIN_GAP_WIDTH, segments)
                .is_empty()
        );
    }

    #[test]
    fn gap_exactly_one_above_tolerance_is_not_merged_but_ignored_when_small() {
        // 两段间隔 121s（超出 tolerance=120 一点）→ 不合并
        // 缺口宽度 120 < min_gap_width=300 → 被过滤
        let segments = vec![(1_000, 3_000), (3_121, 6_000)];
        assert!(
            compute_missing_segments(1_000, 6_000, MERGE_TOLERANCE, MIN_GAP_WIDTH, segments)
                .is_empty()
        );
    }
}
