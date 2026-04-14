# -*- coding: utf-8 -*-
"""按自然日计算查询时间窗口。"""

from __future__ import annotations

from datetime import datetime, timedelta


def start_of_day_timestamp(dt: datetime | None = None) -> int:
    """返回指定时间所在自然日 00:00:00 的时间戳。"""
    current = dt or datetime.now()
    return int(current.replace(hour=0, minute=0, second=0, microsecond=0).timestamp())


def end_of_day_timestamp(dt: datetime | None = None) -> int:
    """返回指定时间所在自然日 23:59:59 的时间戳。"""
    current = dt or datetime.now()
    return int(current.replace(hour=23, minute=59, second=59, microsecond=0).timestamp())


def recent_day_range_timestamps(days: int, now: datetime | None = None) -> tuple[int, int]:
    """返回最近 N 天查询窗口，按自然日边界计算。"""
    current = now or datetime.now()
    safe_days = max(int(days or 0), 0)
    start_dt = current - timedelta(days=safe_days)
    return start_of_day_timestamp(start_dt), end_of_day_timestamp(current)
