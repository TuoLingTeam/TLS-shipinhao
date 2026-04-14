# -*- coding: utf-8 -*-
"""TLS-shipinhao 订单本地缓存存取。"""

import os
import sqlite3
import threading
import time

from settings import get_home_config_dir
from settings import ORDER_CACHE_DB_NAME, ORDER_CACHE_SCOPE


class OrderCacheRepository:
    """订单缓存仓库（SQLite，线程安全）。"""

    def __init__(self, db_path: str | None = None):
        base_dir = get_home_config_dir()
        self.db_path = db_path or os.path.join(base_dir, ORDER_CACHE_DB_NAME)
        self._connection = None
        self._initialized = False
        self._lock = threading.Lock()

    def _connect(self):
        if self._connection is not None:
            return self._connection
        os.makedirs(os.path.dirname(self.db_path) or ".", exist_ok=True)
        self._connection = sqlite3.connect(self.db_path, check_same_thread=False)
        self._connection.row_factory = sqlite3.Row
        self._connection.execute("PRAGMA journal_mode=WAL")
        return self._connection

    def initialize(self) -> None:
        """初始化缓存数据库结构（幂等，重复调用自动跳过）。"""
        if self._initialized:
            return
        with self._lock, self._connect() as connection:
            connection.executescript(
                """
                CREATE TABLE IF NOT EXISTS orders (
                    order_id TEXT PRIMARY KEY,
                    buyer_nickname TEXT NOT NULL DEFAULT '',
                    normalized_nickname TEXT NOT NULL DEFAULT '',
                    create_time INTEGER NOT NULL DEFAULT 0,
                    confirm_receipt_time INTEGER NOT NULL DEFAULT 0,
                    is_waybill_received INTEGER NOT NULL DEFAULT 0,
                    waybill_received_time INTEGER NOT NULL DEFAULT 0,
                    is_education_order INTEGER NOT NULL DEFAULT 0,
                    order_status INTEGER NOT NULL DEFAULT 0,
                    openid TEXT NOT NULL DEFAULT '',
                    raw_source TEXT NOT NULL DEFAULT 'order_api',
                    updated_at INTEGER NOT NULL DEFAULT 0
                );

                CREATE TABLE IF NOT EXISTS order_products (
                    id INTEGER PRIMARY KEY AUTOINCREMENT,
                    order_id TEXT NOT NULL,
                    product_id TEXT NOT NULL DEFAULT '',
                    sku_id TEXT NOT NULL DEFAULT '',
                    sale_param TEXT NOT NULL DEFAULT '',
                    product_name TEXT NOT NULL DEFAULT '',
                    thumb_img TEXT NOT NULL DEFAULT '',
                    FOREIGN KEY(order_id) REFERENCES orders(order_id) ON DELETE CASCADE
                );

                CREATE TABLE IF NOT EXISTS sync_state (
                    scope TEXT PRIMARY KEY,
                    coverage_start INTEGER NOT NULL DEFAULT 0,
                    coverage_end INTEGER NOT NULL DEFAULT 0,
                    last_incremental_start INTEGER NOT NULL DEFAULT 0,
                    last_incremental_end INTEGER NOT NULL DEFAULT 0,
                    last_success_at INTEGER NOT NULL DEFAULT 0,
                    last_mode TEXT NOT NULL DEFAULT '',
                    last_error TEXT NOT NULL DEFAULT ''
                );

                CREATE TABLE IF NOT EXISTS cache_segments (
                    scope TEXT NOT NULL,
                    start_ts INTEGER NOT NULL,
                    end_ts INTEGER NOT NULL,
                    status TEXT NOT NULL DEFAULT 'complete',
                    updated_at INTEGER NOT NULL DEFAULT 0,
                    PRIMARY KEY (scope, start_ts, end_ts)
                );

                CREATE INDEX IF NOT EXISTS idx_orders_create_time
                ON orders(create_time DESC);

                CREATE INDEX IF NOT EXISTS idx_products_order_id
                ON order_products(order_id);

                CREATE INDEX IF NOT EXISTS idx_cache_segments_scope_start
                ON cache_segments(scope, start_ts, end_ts);
                """
            )
        self._initialized = True

    @staticmethod
    def _first_non_empty(data, keys):
        for key in keys:
            value = data.get(key)
            if isinstance(value, str):
                if value.strip():
                    return value.strip()
                continue
            if value not in (None, [], {}):
                return value
        return ""

    @staticmethod
    def _normalize_sale_param(raw_value):
        """将 saleParam 序列化为纯文本，避免 str(list) 污染。"""
        if isinstance(raw_value, list):
            return "|".join(str(v).strip() for v in raw_value if str(v).strip())
        if raw_value is None:
            return ""
        return str(raw_value).strip()

    def _normalize_order(self, order, *, raw_source):
        common_info = order.get("commonInfo", {}) or {}
        order_id = str(common_info.get("orderId", "") or "").strip()
        if not order_id:
            return None, []

        buyer_nickname = str(order.get("buyerInfo", {}).get("nickName", "") or "").strip()
        accept_info = order.get("acceptInfo", {}) or {}
        confirm_receipt_time = accept_info.get("confirmReceiptTime", "")
        confirm_receipt_timestamp = 0
        if confirm_receipt_time and str(confirm_receipt_time).isdigit():
            confirm_receipt_timestamp = int(confirm_receipt_time)

        auto_confirm_info = order.get("orderStatus", {}).get("autoConfirmInfo", {}) or {}
        updated_at = int(time.time())
        order_row = (
            order_id,
            buyer_nickname,
            buyer_nickname,
            int(common_info.get("createTime", 0) or 0),
            confirm_receipt_timestamp,
            1 if auto_confirm_info.get("isWaybillReceived", False) else 0,
            int(auto_confirm_info.get("waybillReceivedTime", 0) or 0),
            1 if common_info.get("isEducationOrder", False) else 0,
            int(common_info.get("status", 0) or 0),
            str(common_info.get("openid", "") or "").strip(),
            raw_source,
            updated_at,
        )

        product_rows = []
        product_list = order.get("orderProductInfo", []) or order.get("productInfos", []) or []
        for product in product_list:
            raw_sale_param = self._first_non_empty(
                product, ("saleParam", "sale_param", "skuName", "specName", "spec"),
            )
            product_rows.append(
                (
                    order_id,
                    str(self._first_non_empty(product, ("productId", "product_id", "spuId", "spu_id"))),
                    str(self._first_non_empty(product, ("skuId", "sku_id"))),
                    self._normalize_sale_param(raw_sale_param),
                    str(self._first_non_empty(product, ("title", "spuName", "productName", "name"))),
                    str(self._first_non_empty(product, ("thumbImg", "imgUrl", "image", "imageUrl"))),
                )
            )
        return order_row, product_rows

    def has_dirty_sale_param(self) -> bool:
        """检测是否存在 str(list) 污染的 sale_param 历史数据。"""
        self.initialize()
        with self._lock, self._connect() as connection:
            row = connection.execute(
                "SELECT COUNT(*) AS cnt FROM order_products WHERE sale_param LIKE '[%'"
            ).fetchone()
        return (row["cnt"] if row else 0) > 0

    def clear_all(self) -> None:
        """清空全部缓存表。"""
        self.initialize()
        with self._lock, self._connect() as connection:
            connection.execute("DELETE FROM order_products")
            connection.execute("DELETE FROM orders")
            connection.execute("DELETE FROM sync_state")
            connection.execute("DELETE FROM cache_segments")

    def upsert_orders(self, orders: list[dict], *, raw_source: str = "order_api") -> int:
        """写入或更新订单缓存。返回成功落库的订单数。"""
        self.initialize()
        order_rows = []
        product_rows = []
        order_ids = []
        for order in orders:
            order_row, normalized_products = self._normalize_order(order, raw_source=raw_source)
            if order_row is None:
                continue
            order_rows.append(order_row)
            order_ids.append(order_row[0])
            product_rows.extend(normalized_products)

        if not order_rows:
            return 0

        with self._lock, self._connect() as connection:
            connection.executemany(
                "DELETE FROM order_products WHERE order_id = ?",
                [(order_id,) for order_id in order_ids],
            )
            connection.executemany(
                """
                INSERT OR REPLACE INTO orders (
                    order_id,
                    buyer_nickname,
                    normalized_nickname,
                    create_time,
                    confirm_receipt_time,
                    is_waybill_received,
                    waybill_received_time,
                    is_education_order,
                    order_status,
                    openid,
                    raw_source,
                    updated_at
                ) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)
                """,
                order_rows,
            )
            if product_rows:
                connection.executemany(
                    """
                    INSERT INTO order_products (
                        order_id,
                        product_id,
                        sku_id,
                        sale_param,
                        product_name,
                        thumb_img
                    ) VALUES (?, ?, ?, ?, ?, ?)
                    """,
                    product_rows,
                )

        return len(order_rows)

    def get_state(self, scope: str = ORDER_CACHE_SCOPE) -> dict | None:
        """读取同步状态。"""
        self.initialize()
        with self._lock, self._connect() as connection:
            row = connection.execute(
                "SELECT * FROM sync_state WHERE scope = ?",
                (scope,),
            ).fetchone()
        return dict(row) if row else None

    def save_state(
        self,
        *,
        scope=ORDER_CACHE_SCOPE,
        coverage_start=0,
        coverage_end=0,
        last_incremental_start=0,
        last_incremental_end=0,
        last_success_at=0,
        last_mode="",
        last_error="",
    ):
        """写入同步状态。"""
        self.initialize()
        with self._lock, self._connect() as connection:
            connection.execute(
                """
                INSERT OR REPLACE INTO sync_state (
                    scope,
                    coverage_start,
                    coverage_end,
                    last_incremental_start,
                    last_incremental_end,
                    last_success_at,
                    last_mode,
                    last_error
                ) VALUES (?, ?, ?, ?, ?, ?, ?, ?)
                """,
                (
                    scope,
                    int(coverage_start or 0),
                    int(coverage_end or 0),
                    int(last_incremental_start or 0),
                    int(last_incremental_end or 0),
                    int(last_success_at or 0),
                    last_mode,
                    last_error,
                ),
            )

    def mark_segment_complete(self, start_timestamp, end_timestamp, *, scope=ORDER_CACHE_SCOPE):
        """标记某个时间窗口已完整写入缓存。"""
        self.initialize()
        now_ts = int(time.time())
        with self._lock, self._connect() as connection:
            connection.execute(
                """
                INSERT OR REPLACE INTO cache_segments (
                    scope,
                    start_ts,
                    end_ts,
                    status,
                    updated_at
                ) VALUES (?, ?, ?, 'complete', ?)
                """,
                (
                    scope,
                    int(start_timestamp or 0),
                    int(end_timestamp or 0),
                    now_ts,
                ),
            )

    def get_complete_segments(self, start_timestamp, end_timestamp, *, scope=ORDER_CACHE_SCOPE):
        """返回指定范围内已完成的缓存窗口。"""
        self.initialize()
        start_timestamp = int(start_timestamp or 0)
        end_timestamp = int(end_timestamp or 0)
        with self._lock, self._connect() as connection:
            rows = connection.execute(
                """
                SELECT scope, start_ts, end_ts, status, updated_at
                FROM cache_segments
                WHERE scope = ?
                  AND status = 'complete'
                  AND end_ts >= ?
                  AND start_ts <= ?
                ORDER BY start_ts ASC, end_ts ASC
                """,
                (scope, start_timestamp, end_timestamp),
            ).fetchall()
        return [dict(row) for row in rows]

    def get_missing_segments(
        self,
        start_timestamp,
        end_timestamp,
        *,
        scope=ORDER_CACHE_SCOPE,
        merge_tolerance=120,
        min_gap_width=300,
    ):
        """计算指定范围内尚未完成缓存覆盖的时间缺口。

        Args:
            merge_tolerance: 合并相邻段的容差秒数，小于此间距的缝隙视为连续。
            min_gap_width: 最终缺口宽度低于此阈值的直接丢弃，不触发补齐。
        """
        start_timestamp = int(start_timestamp or 0)
        end_timestamp = int(end_timestamp or 0)
        if start_timestamp <= 0 or end_timestamp <= 0 or start_timestamp > end_timestamp:
            return []

        segments = []
        for segment in self.get_complete_segments(start_timestamp, end_timestamp, scope=scope):
            seg_start = max(start_timestamp, int(segment["start_ts"] or 0))
            seg_end = min(end_timestamp, int(segment["end_ts"] or 0))
            if seg_start <= seg_end:
                segments.append((seg_start, seg_end))

        if not segments:
            return [(start_timestamp, end_timestamp)]

        merged = []
        for seg_start, seg_end in segments:
            if not merged or seg_start > merged[-1][1] + merge_tolerance:
                merged.append([seg_start, seg_end])
            else:
                merged[-1][1] = max(merged[-1][1], seg_end)

        missing = []
        cursor = start_timestamp
        for seg_start, seg_end in merged:
            if cursor < seg_start:
                gap_width = seg_start - cursor
                if gap_width >= min_gap_width:
                    missing.append((cursor, seg_start - 1))
            cursor = max(cursor, seg_end + 1)
        if cursor <= end_timestamp:
            gap_width = end_timestamp - cursor + 1
            if gap_width >= min_gap_width:
                missing.append((cursor, end_timestamp))
        return missing

    def delete_older_than(self, cutoff_timestamp: int) -> int:
        """删除缓存范围外的旧订单。"""
        self.initialize()
        cutoff_timestamp = int(cutoff_timestamp or 0)
        with self._lock, self._connect() as connection:
            expired_order_rows = connection.execute(
                "SELECT order_id FROM orders WHERE create_time < ?",
                (cutoff_timestamp,),
            ).fetchall()
            if not expired_order_rows:
                order_ids = []
            else:
                order_ids = [row["order_id"] for row in expired_order_rows]
                connection.executemany(
                    "DELETE FROM order_products WHERE order_id = ?",
                    [(order_id,) for order_id in order_ids],
                )
                connection.executemany(
                    "DELETE FROM orders WHERE order_id = ?",
                    [(order_id,) for order_id in order_ids],
                )

            connection.execute(
                "DELETE FROM cache_segments WHERE scope = ? AND end_ts < ?",
                (ORDER_CACHE_SCOPE, cutoff_timestamp),
            )
            connection.execute(
                """
                UPDATE cache_segments
                SET start_ts = ?, updated_at = ?
                WHERE scope = ? AND start_ts < ? AND end_ts >= ?
                """,
                (
                    cutoff_timestamp,
                    int(time.time()),
                    ORDER_CACHE_SCOPE,
                    cutoff_timestamp,
                    cutoff_timestamp,
                ),
            )
        return len(order_ids)

    def fetch_orders_in_range(self, start_timestamp: int, end_timestamp: int) -> list[dict]:
        """按时间范围读取订单并回组装为匹配器可消费的结构。"""
        self.initialize()
        start_timestamp = int(start_timestamp or 0)
        end_timestamp = int(end_timestamp or 0)
        with self._lock, self._connect() as connection:
            order_rows = connection.execute(
                """
                SELECT *
                FROM orders
                WHERE create_time >= ? AND create_time <= ?
                ORDER BY create_time DESC, order_id DESC
                """,
                (start_timestamp, end_timestamp),
            ).fetchall()
            product_rows = connection.execute(
                """
                SELECT p.*
                FROM order_products p
                JOIN orders o ON o.order_id = p.order_id
                WHERE o.create_time >= ? AND o.create_time <= ?
                ORDER BY o.create_time DESC, p.order_id ASC, p.id ASC
                """,
                (start_timestamp, end_timestamp),
            ).fetchall()

        products_by_order = {}
        for row in product_rows:
            raw_sp = row["sale_param"] or ""
            if raw_sp.startswith("[") and raw_sp.endswith("]"):
                try:
                    import ast
                    parsed = ast.literal_eval(raw_sp)
                    if isinstance(parsed, list):
                        raw_sp = "|".join(str(v).strip() for v in parsed if str(v).strip())
                except (ValueError, SyntaxError):
                    pass
            products_by_order.setdefault(row["order_id"], []).append(
                {
                    "productId": row["product_id"],
                    "skuId": row["sku_id"],
                    "saleParam": raw_sp,
                    "title": row["product_name"],
                    "thumbImg": row["thumb_img"],
                }
            )

        orders = []
        for row in order_rows:
            confirm_receipt_time = str(row["confirm_receipt_time"]) if row["confirm_receipt_time"] else ""
            orders.append(
                {
                    "commonInfo": {
                        "orderId": row["order_id"],
                        "createTime": int(row["create_time"] or 0),
                        "status": int(row["order_status"] or 0),
                        "openid": row["openid"] or "",
                        "isEducationOrder": bool(row["is_education_order"]),
                    },
                    "buyerInfo": {"nickName": row["buyer_nickname"] or ""},
                    "acceptInfo": {"confirmReceiptTime": confirm_receipt_time},
                    "orderStatus": {
                        "autoConfirmInfo": {
                            "isWaybillReceived": bool(row["is_waybill_received"]),
                            "waybillReceivedTime": int(row["waybill_received_time"] or 0),
                        }
                    },
                    "orderProductInfo": products_by_order.get(row["order_id"], []),
                }
            )
        return orders
