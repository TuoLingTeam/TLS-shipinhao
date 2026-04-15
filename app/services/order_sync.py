# -*- coding: utf-8 -*-
"""TLS-shipinhao 订单缓存同步服务。"""

from core.day_window import end_of_day_timestamp, recent_day_range_timestamps
from settings import (
    ORDER_CACHE_COVERAGE_DAYS,
    ORDER_CACHE_INCREMENTAL_DAYS,
    ORDER_CACHE_INCREMENTAL_OVERLAP_DAYS,
    ORDER_CACHE_SCOPE,
)
from services.order_cache import OrderCacheRepository


class OrderSyncService:
    """订单缓存同步与读取服务。"""

    def __init__(self, finder, repository=None):
        self.finder = finder
        self.repository = repository or OrderCacheRepository()
        self._stopped = False

    def stop(self):
        """请求停止当前同步任务。"""
        self._stopped = True
        self.finder.stop()

    def _progress(self, on_progress, message):
        if on_progress:
            on_progress(message)


    def _save_state(self, *, mode, last_error="", coverage_start=None, coverage_end=None, incremental_start=None, incremental_end=None):
        now_ts = end_of_day_timestamp()
        retention_start, _ = recent_day_range_timestamps(ORDER_CACHE_COVERAGE_DAYS)
        self.repository.save_state(
            scope=ORDER_CACHE_SCOPE,
            coverage_start=int(coverage_start if coverage_start is not None else retention_start),
            coverage_end=int(coverage_end if coverage_end is not None else now_ts),
            last_incremental_start=int(incremental_start or 0),
            last_incremental_end=int(incremental_end or 0),
            last_success_at=0 if last_error else now_ts,
            last_mode=mode,
            last_error=last_error,
        )

    def _sync_range(self, start_timestamp, end_timestamp, *, mode, on_progress=None):
        if self._stopped:
            return 0, []

        start_timestamp = int(start_timestamp or 0)
        end_timestamp = int(end_timestamp or 0)
        if start_timestamp <= 0 or end_timestamp <= 0 or start_timestamp > end_timestamp:
            return 0, []

        self._progress(
            on_progress,
            f"[缓存] 开始同步窗口：{start_timestamp} ~ {end_timestamp}（{mode}）",
        )
        try:
            persisted_orders = []

            def _persist_window(window, window_orders):
                if self._stopped or not window_orders:
                    return
                self.repository.upsert_orders(window_orders)
                persisted_orders.extend(window_orders)
                self._progress(
                    on_progress,
                    f"[缓存] 已持久化窗口 {window.window_id}：{window.start_ts} ~ {window.end_ts}，"
                    f"写入 {len(window_orders)} 个订单。",
                )

            orders, warnings = self.finder.get_orders_for_cache(
                earliest_time=start_timestamp,
                create_time_start=start_timestamp,
                create_time_end=end_timestamp,
                on_progress=on_progress,
                on_window_completed=_persist_window,
            )
            self.repository.mark_segment_complete(
                start_timestamp, end_timestamp, scope=ORDER_CACHE_SCOPE,
            )
            written_count = len({order.get("commonInfo", {}).get("orderId") for order in persisted_orders if order.get("commonInfo", {}).get("orderId")})
            self._save_state(
                mode=mode,
                coverage_start=recent_day_range_timestamps(ORDER_CACHE_COVERAGE_DAYS)[0],
                coverage_end=end_of_day_timestamp(),
                incremental_start=start_timestamp if mode in ("incremental", "rebuild") else 0,
                incremental_end=end_timestamp if mode in ("incremental", "rebuild") else 0,
            )
            self._progress(on_progress, f"[缓存] 窗口同步完成：写入 {written_count} 个订单。")
            return written_count, warnings
        except Exception as exc:
            self._save_state(mode=mode, last_error=str(exc))
            raise

    def rebuild_cache(self, on_progress=None):
        """重建最近覆盖范围内的全部订单缓存。"""
        if self._stopped:
            return 0, []

        start_timestamp, end_timestamp = recent_day_range_timestamps(ORDER_CACHE_COVERAGE_DAYS)
        self._progress(on_progress, "[缓存] 正在重建最近 30 天订单缓存...")
        self.repository.clear_all()
        written_count, warnings = self._sync_range(
            start_timestamp,
            end_timestamp,
            mode="rebuild",
            on_progress=on_progress,
        )
        self.repository.delete_older_than(start_timestamp)
        return written_count, warnings

    def refresh_cache(self, on_progress=None):
        """增量刷新最近订单缓存，仅补齐未覆盖的部分。"""
        if self._stopped:
            return 0, []

        state = self.repository.get_state(scope=ORDER_CACHE_SCOPE)
        if not state:
            return self.rebuild_cache(on_progress=on_progress)

        end_timestamp = self._now()
        overlap_seconds = ORDER_CACHE_INCREMENTAL_OVERLAP_DAYS * 86400
        default_start = end_timestamp - (ORDER_CACHE_INCREMENTAL_DAYS + ORDER_CACHE_INCREMENTAL_OVERLAP_DAYS) * 86400
        last_incremental_end = int(state.get("last_incremental_end", 0) or 0)
        start_timestamp = max(default_start, (last_incremental_end - overlap_seconds) if last_incremental_end else default_start)

        gaps = self.repository.get_missing_segments(
            start_timestamp, end_timestamp, scope=ORDER_CACHE_SCOPE,
        )
        if not gaps:
            self._progress(on_progress, "[缓存] 最近 3 天缓存已完整覆盖，跳过增量刷新。")
            self.repository.delete_older_than(end_timestamp - ORDER_CACHE_COVERAGE_DAYS * 86400)
            return 0, []

        total_written = 0
        all_warnings: list[str] = []
        for gap_start, gap_end in gaps:
            if self._stopped:
                break
            self._progress(
                on_progress,
                f"[缓存] 增量刷新缺口：{gap_start} ~ {gap_end}",
            )
            written_count, warnings = self._sync_range(
                gap_start, gap_end, mode="incremental", on_progress=on_progress,
            )
            total_written += written_count
            all_warnings.extend(warnings)

        self.repository.delete_older_than(end_timestamp - ORDER_CACHE_COVERAGE_DAYS * 86400)
        return total_written, all_warnings

    def _ensure_recent_cache(self, on_progress=None):
        """确保最近 30 天缓存可用，并只补齐缺口。"""
        self.repository.initialize()
        start_timestamp, end_timestamp = recent_day_range_timestamps(ORDER_CACHE_COVERAGE_DAYS)
        warnings = []

        state = self.repository.get_state(scope=ORDER_CACHE_SCOPE)
        need_rebuild = not state
        if state and self.repository.has_dirty_sale_param():
            self._progress(on_progress, "[缓存] 检测到历史数据格式异常，自动清空并重建缓存。")
            need_rebuild = True
        if need_rebuild:
            if not state:
                self._progress(on_progress, "[缓存] 未发现本地订单缓存，将自动构建最近 30 天缓存。")
            written_count, rebuild_warnings = self.rebuild_cache(on_progress=on_progress)
            warnings.extend(rebuild_warnings)
            return written_count, warnings, start_timestamp, end_timestamp

        missing_segments = self.repository.get_missing_segments(
            start_timestamp,
            end_timestamp,
            scope=ORDER_CACHE_SCOPE,
        )
        total_written = 0
        for segment_start, segment_end in missing_segments:
            if self._stopped:
                break
            self._progress(
                on_progress,
                f"[缓存] 检测到最近 30 天缓存缺口，准备补齐：{segment_start} ~ {segment_end}",
            )
            written_count, gap_warnings = self._sync_range(
                segment_start,
                segment_end,
                mode="gap_fill",
                on_progress=on_progress,
            )
            total_written += written_count
            warnings.extend(gap_warnings)

        refresh_written, refresh_warnings = self.refresh_cache(on_progress=on_progress)
        total_written += refresh_written
        warnings.extend(refresh_warnings)
        return total_written, warnings, start_timestamp, end_timestamp

    def ensure_orders(self, earliest_time, on_progress=None):
        """确保缓存覆盖当前查询窗口，并返回本地订单。"""
        _, warnings, recent_start, recent_end = self._ensure_recent_cache(on_progress=on_progress)
        fetch_start = max(int(earliest_time or 0), recent_start)
        orders = self.repository.fetch_orders_in_range(fetch_start, recent_end)
        self._progress(on_progress, f"[缓存] 已从本地缓存读取 {len(orders)} 个订单。")
        return orders, warnings

    def fetch_full_scan_orders(self, earliest_time, on_progress=None):
        """执行手动完整补查：最近 30 天用缓存，更早范围临时抓取。"""
        _, warnings, recent_start, recent_end = self._ensure_recent_cache(on_progress=on_progress)
        recent_orders = self.repository.fetch_orders_in_range(max(int(earliest_time or 0), recent_start), recent_end)

        if int(earliest_time or 0) >= recent_start:
            self._progress(on_progress, f"[缓存] 完整补查命中最近 30 天缓存 {len(recent_orders)} 个订单。")
            return recent_orders, warnings

        temporary_end = recent_start - 1
        self._progress(on_progress, "[缓存] 开始补查 30 天前的临时订单（本次使用，不写入长期缓存）。")
        temporary_orders, temp_warnings = self.finder.get_orders_for_cache(
            earliest_time=int(earliest_time or 0),
            create_time_start=int(earliest_time or 0),
            create_time_end=temporary_end,
            on_progress=on_progress,
            on_window_completed=None,
        )
        warnings.extend(temp_warnings)
        combined_orders = self.finder.deduplicate_orders_by_id(temporary_orders + recent_orders)
        self._progress(
            on_progress,
            f"[缓存] 完整补查已合并最近 30 天缓存与历史临时订单，共 {len(combined_orders)} 个订单。",
        )
        return combined_orders, warnings
