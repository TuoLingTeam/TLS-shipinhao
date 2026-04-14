# -*- coding: utf-8 -*-
"""TLS-shipinhao 中差评查找后台执行器。"""

import threading
from concurrent.futures import ThreadPoolExecutor, as_completed

from PySide6.QtCore import QObject, Signal

from ..config import ConfigNotFoundError, get_cookie, get_magic, serialize_cookie_data
from ..constants import ORDER_CACHE_COVERAGE_DAYS
from ..core.day_window import recent_day_range_timestamps
from ..services.order_sync import OrderSyncService
from ..services.review_matcher import AUTO_FILL_SCORE_THRESHOLD, BadReviewOrderFinder

ORDER_FETCH_BUFFER_DAYS = 30
TASK_REVIEW_MATCH = "review_match"
TASK_REVIEW_FULL_SCAN = "review_full_scan"
TASK_QUALITY_REFUND = "quality_refund"
TASK_CACHE_REFRESH = "cache_refresh"
TASK_CACHE_REBUILD = "cache_rebuild"
TERMINAL_STATUS_SUCCESS = "success"
TERMINAL_STATUS_WARNING = "warning"
TERMINAL_STATUS_ERROR = "error"
TERMINAL_STATUS_CANCELLED = "cancelled"


class ReviewMatcherWorker(QObject):
    """后台中差评查找执行器。

    性能优化策略:
      1. 差评获取 和 订单获取 **阶段并行执行**
      2. 订单获取使用 ``nextKey`` 顺序翻页，优先保证稳定性
      3. 差评翻页间隔缩短至 0.3 秒
      4. 订单翻页加 0.3 秒限速防止 429 限流
    """

    progress = Signal(str)
    results_ready = Signal(list)
    order_ids_ready = Signal(list)
    error = Signal(str)
    missing_config = Signal(str)
    finished = Signal(str, str, int, int)  # (status, message, matched_count, total_count)

    def __init__(self, days=30, task_type=TASK_REVIEW_MATCH):
        super().__init__()
        self.days = days
        self.task_type = task_type
        self._stopped = False
        self._finder_lock = threading.Lock()
        self._active_finders = []
        self._active_order_sync_service = None

    def stop(self):
        """请求终止任务（安全退出）。"""
        self._stopped = True
        with self._finder_lock:
            active_service = self._active_order_sync_service
            active_finders = list(self._active_finders)
        if active_service is not None:
            active_service.stop()
        for finder in active_finders:
            finder.stop()

    def _register_finder(self, finder):
        with self._finder_lock:
            self._active_finders.append(finder)
        return finder

    def _release_finder(self, finder):
        with self._finder_lock:
            if finder in self._active_finders:
                self._active_finders.remove(finder)

    def _set_active_order_sync_service(self, service):
        with self._finder_lock:
            self._active_order_sync_service = service

    @staticmethod
    def _filter_active_evaluations(raw_evaluations):
        """过滤系统自动评价，仅保留主动评价。"""
        return [
            e
            for e in raw_evaluations
            if e.get("evaluationInfo", {})
            .get("firstEvaluationInfo", {})
            .get("buyerEvaluationInfo", {})
            .get("autoEvaluation", 0)
            == 0
        ]

    def _progress_emitter(self):
        def _progress(msg):
            if not self._stopped:
                self.progress.emit(msg)

        return _progress

    def _emit_terminal(self, status, message="", matched_count=0, total_count=0):
        self.finished.emit(status, message, matched_count, total_count)

    def _emit_empty_results(self, progress, message):
        """输出空结果状态并同步结束信号。"""
        progress(message)
        self.results_ready.emit([])
        self.order_ids_ready.emit([])
        self._emit_terminal(TERMINAL_STATUS_SUCCESS)

    @staticmethod
    def _collect_unique_order_ids(orders):
        """从订单列表中提取去重后的 orderId。"""
        order_ids = []
        for order in orders:
            order_id = order.get("commonInfo", {}).get("orderId")
            if order_id and order_id not in order_ids:
                order_ids.append(order_id)
        return order_ids

    def _load_matcher_credentials(self):
        """读取配置并返回匹配所需凭据。"""
        try:
            cookie_data = get_cookie()
        except ConfigNotFoundError as exc:
            self.missing_config.emit("\n".join(exc.searched_dirs))
            self._emit_terminal(TERMINAL_STATUS_CANCELLED)
            return None
        except Exception as exc:  # noqa: BLE001
            self._emit_terminal(TERMINAL_STATUS_ERROR, f"读取配置失败: {exc}")
            return None

        try:
            magic = get_magic(cookie_data)
        except Exception as exc:  # noqa: BLE001
            self._emit_terminal(TERMINAL_STATUS_ERROR, f"提取 biz_magic 失败: {exc}")
            return None

        return serialize_cookie_data(cookie_data), magic

    def _build_order_earliest_time(self):
        """计算订单抓取的时间窗口下限（自然日 00:00:00）。"""
        start_ts, _ = recent_day_range_timestamps(self.days + ORDER_FETCH_BUFFER_DAYS)
        return start_ts

    def _build_cache_order_earliest_time(self):
        """自动查单默认只读取最近 30 天持久缓存（自然日 00:00:00 起）。"""
        start_ts, _ = recent_day_range_timestamps(ORDER_CACHE_COVERAGE_DAYS)
        return start_ts

    def _build_quality_refund_earliest_time(self):
        """计算品质退款订单筛选下限（自然日 00:00:00）。"""
        start_ts, _ = recent_day_range_timestamps(self.days)
        return start_ts

    def _fetch_active_evaluations(self, cookie_str, magic, progress):
        """获取并过滤有效主动评价。"""
        finder = self._register_finder(BadReviewOrderFinder(cookie_str, magic))
        try:
            progress("[差评] 开始获取...")
            raw_evaluations = finder.get_bad_evaluations(
                days=self.days,
                on_progress=progress,
            )
            bad_evaluations = self._filter_active_evaluations(raw_evaluations)
            auto_count = len(raw_evaluations) - len(bad_evaluations)
            progress(
                f"[差评] 完成，共 {len(raw_evaluations)} 条，"
                f"过滤系统自动评价 {auto_count} 条，"
                f"有效主动评价 {len(bad_evaluations)} 条。"
            )
            return bad_evaluations
        finally:
            self._release_finder(finder)

    def _fetch_orders(self, cookie_str, magic, earliest_time, progress):
        """获取订单列表。"""
        finder = self._register_finder(BadReviewOrderFinder(cookie_str, magic))
        sync_service = OrderSyncService(finder)
        self._set_active_order_sync_service(sync_service)
        try:
            orders, warnings = sync_service.ensure_orders(
                earliest_time=earliest_time,
                on_progress=progress,
            )
            return orders, warnings
        finally:
            self._set_active_order_sync_service(None)
            self._release_finder(finder)

    def _fetch_full_scan_orders(self, cookie_str, magic, earliest_time, progress):
        """执行完整补查：最近 30 天命中缓存，更早订单临时抓取。"""
        finder = self._register_finder(BadReviewOrderFinder(cookie_str, magic))
        sync_service = OrderSyncService(finder)
        self._set_active_order_sync_service(sync_service)
        try:
            orders, warnings = sync_service.fetch_full_scan_orders(
                earliest_time=earliest_time,
                on_progress=progress,
            )
            return orders, warnings
        finally:
            self._set_active_order_sync_service(None)
            self._release_finder(finder)

    def _fetch_quality_refund_orders(self, cookie_str, magic, earliest_time, progress):
        """获取品质退款订单列表。"""
        finder = self._register_finder(BadReviewOrderFinder(cookie_str, magic))
        try:
            return finder.get_quality_refund_orders(
                earliest_time=earliest_time,
                on_progress=progress,
            )
        finally:
            self._release_finder(finder)

    def _fetch_data_in_parallel(self, cookie_str, magic, earliest_time, progress, order_fetcher=None):
        """并行获取差评和订单数据。"""
        order_fetcher = order_fetcher or self._fetch_orders
        results = {
            "bad_evaluations": None,
            "orders": None,
            "order_warnings": [],
        }
        errors = {
            "bad_evaluations": None,
            "orders": None,
        }
        tasks = {
            "bad_evaluations": lambda: self._fetch_active_evaluations(
                cookie_str,
                magic,
                progress,
            ),
            "orders": lambda: order_fetcher(
                cookie_str,
                magic,
                earliest_time,
                progress,
            ),
        }

        with ThreadPoolExecutor(max_workers=2, thread_name_prefix="phase") as pool:
            future_to_key = {
                pool.submit(task): key
                for key, task in tasks.items()
            }
            for future in as_completed(future_to_key):
                key = future_to_key[future]
                try:
                    value = future.result()
                    if key == "orders":
                        results["orders"], results["order_warnings"] = value
                    else:
                        results[key] = value
                except Exception as exc:  # noqa: BLE001
                    errors[key] = str(exc)

        return (
            results["bad_evaluations"],
            results["orders"],
            results["order_warnings"],
            errors["bad_evaluations"],
            errors["orders"],
        )

    def _match_orders(self, cookie_str, magic, bad_evaluations, orders, progress):
        """执行差评与订单匹配。"""
        progress("=== 开始匹配差评和订单 ===")
        matcher = self._register_finder(BadReviewOrderFinder(cookie_str, magic))
        try:
            return matcher.match_orders_with_evaluations(
                bad_evaluations,
                orders,
                on_progress=progress,
            )
        finally:
            self._release_finder(matcher)

    def _emit_match_summary(self, bad_evaluations, matched_results, autofill_order_ids, progress):
        """输出匹配完成后的汇总日志。"""
        manual_review_count = len(matched_results) - len(autofill_order_ids)

        progress("\n=== 匹配完成 ===")
        progress(
            f"共 {len(bad_evaluations)} 条差评，"
            f"初步匹配到 {len(matched_results)} 个订单。"
        )

        if not matched_results:
            return

        progress(
            f"其中 {len(autofill_order_ids)} 个得分达标"
            f"({AUTO_FILL_SCORE_THRESHOLD}分)已自动填入，"
            f"{manual_review_count} 个需人工核对。"
        )
        progress("匹配进度明细已在上方 [i/n] 匹配日志输出。")

        need_review_items = [
            item for item in matched_results
            if int(item.get("matchScore", 0) or 0) < 100
        ]
        if not need_review_items:
            return

        progress("\n⚠️ 重点核对明细（仅展示 <100 分）:")
        need_review_items.sort(key=lambda x: int(x.get("matchScore", 0) or 0))

        for index, item in enumerate(need_review_items, 1):
            score = int(item.get("matchScore", 0) or 0)
            order_id = item.get("orderId") or "-"
            eval_nickname = item.get("buyerNickname") or ""
            order_nickname = item.get("orderBuyerNickname") or ""
            reasons = item.get("matchReasons") or []

            progress(f"  [{index}] 订单: {order_id} | 得分: {score}")
            progress(f"      评价的买家昵称: {eval_nickname}")
            progress(f"      订单的买家昵称: {order_nickname}")

            if reasons:
                progress("      扣分/判定原因:")
                for reason in reasons:
                    progress(f"        - {reason}")

    def _emit_quality_refund_summary(self, orders, order_ids, progress):
        """输出品质退款订单获取汇总日志。"""
        progress("\n=== 品退订单获取完成 ===")
        progress(f"共获取到 {len(orders)} 个品质退款订单。")

        if not order_ids:
            return

        progress("\n品退订单明细:")
        for order in orders:
            order_info = order.get("commonInfo", {})
            product_list = order.get("orderProductInfo", []) or []
            product = product_list[0] if product_list else {}
            refund_info = order.get("qualityRefundInfo", {})
            progress(
                f"  订单: {order_info.get('orderId', '')} | "
                f"商品: {product.get('title', '')} | "
                f"规格: {product.get('saleParam', '')} | "
                f"原因: {refund_info.get('reason', '')}"
            )

    def _run_quality_refund_task(self, cookie_str, magic, progress):
        """执行品质退款订单获取任务。"""
        earliest_time = self._build_quality_refund_earliest_time()

        try:
            orders = self._fetch_quality_refund_orders(
                cookie_str,
                magic,
                earliest_time,
                progress,
            )
        except Exception as exc:  # noqa: BLE001
            self._emit_terminal(TERMINAL_STATUS_ERROR, f"获取品质退款订单失败: {exc}")
            return

        if self._stopped:
            self._emit_terminal(TERMINAL_STATUS_CANCELLED)
            return

        if not orders:
            self._emit_empty_results(
                progress,
                f"近 {self.days} 天没有品质退款订单。",
            )
            return

        order_ids = self._collect_unique_order_ids(orders)

        self._emit_quality_refund_summary(orders, order_ids, progress)
        self.results_ready.emit(orders)
        self.order_ids_ready.emit(order_ids)
        self._emit_terminal(
            TERMINAL_STATUS_SUCCESS,
            matched_count=len(order_ids),
            total_count=len(orders),
        )

    def _run_cache_task(self, cookie_str, magic, progress):
        """执行订单缓存刷新 / 重建任务。"""
        finder = self._register_finder(BadReviewOrderFinder(cookie_str, magic))
        sync_service = OrderSyncService(finder)
        self._set_active_order_sync_service(sync_service)
        try:
            if self.task_type == TASK_CACHE_REBUILD:
                progress(f"=== 开始重建订单缓存（最近 {ORDER_CACHE_COVERAGE_DAYS} 天）===")
                written_count, warnings = sync_service.rebuild_cache(on_progress=progress)
            else:
                progress("=== 开始增量刷新订单缓存（最近 3 天）===")
                written_count, warnings = sync_service.refresh_cache(on_progress=progress)
        except Exception as exc:  # noqa: BLE001
            self._emit_terminal(TERMINAL_STATUS_ERROR, f"订单缓存任务失败: {exc}")
            return
        finally:
            self._set_active_order_sync_service(None)
            self._release_finder(finder)

        if self._stopped:
            self._emit_terminal(TERMINAL_STATUS_CANCELLED)
            return

        for warning in warnings:
            progress(f"⚠️ {warning}")

        status = TERMINAL_STATUS_WARNING if warnings else TERMINAL_STATUS_SUCCESS
        self._emit_terminal(
            status,
            "\n".join(warnings),
            matched_count=written_count,
            total_count=written_count,
        )

    def run(self):
        """后台线程执行入口。"""
        credentials = self._load_matcher_credentials()
        if credentials is None:
            return

        cookie_str, magic = credentials
        progress = self._progress_emitter()

        if self.task_type == TASK_QUALITY_REFUND:
            self._run_quality_refund_task(cookie_str, magic, progress)
            return

        if self.task_type in (TASK_CACHE_REFRESH, TASK_CACHE_REBUILD):
            self._run_cache_task(cookie_str, magic, progress)
            return

        if self.task_type == TASK_REVIEW_FULL_SCAN:
            earliest_time = self._build_order_earliest_time()
        else:
            earliest_time = self._build_cache_order_earliest_time()

        # ---------------------------------------------------------------
        # 阶段 1+2: 并行获取 差评 & 订单（订单优先使用本地缓存，缺口自动分片抓取）
        # ---------------------------------------------------------------
        if self.task_type == TASK_REVIEW_FULL_SCAN:
            progress("=== 并行获取差评 + 订单（最近 30 天用缓存，超出范围执行完整补查）===")
        else:
            progress("=== 并行获取差评 + 订单（订单优先使用最近 30 天缓存，缺口自动补齐）===")

        order_fetcher = self._fetch_full_scan_orders if self.task_type == TASK_REVIEW_FULL_SCAN else self._fetch_orders
        bad_evaluations, orders, order_warnings, eval_error, order_error = self._fetch_data_in_parallel(
            cookie_str,
            magic,
            earliest_time,
            progress,
            order_fetcher=order_fetcher,
        )

        if self._stopped:
            self._emit_terminal(TERMINAL_STATUS_CANCELLED)
            return

        if eval_error:
            self._emit_terminal(TERMINAL_STATUS_ERROR, f"获取差评数据失败: {eval_error}")
            return

        if order_error:
            self._emit_terminal(
                TERMINAL_STATUS_ERROR,
                f"获取订单数据失败: {order_error}",
                matched_count=0,
                total_count=len(bad_evaluations or []),
            )
            return

        if not bad_evaluations:
            progress("未找到差评数据。")
            self.results_ready.emit([])
            self.order_ids_ready.emit([])
            self._emit_terminal(TERMINAL_STATUS_SUCCESS)
            return

        if not orders:
            progress("未获取到订单数据。")
            self.results_ready.emit([])
            self.order_ids_ready.emit([])
            self._emit_terminal(
                TERMINAL_STATUS_SUCCESS,
                matched_count=0,
                total_count=len(bad_evaluations),
            )
            return

        for warning in order_warnings:
            progress(f"⚠️ {warning}")
        progress(f"数据获取完成: {len(bad_evaluations)} 条差评, {len(orders)} 个订单。")

        # ---------------------------------------------------------------
        # 阶段 3: 匹配
        # ---------------------------------------------------------------
        try:
            results = self._match_orders(
                cookie_str,
                magic,
                bad_evaluations,
                orders,
                progress,
            )
        except Exception as exc:  # noqa: BLE001
            self._emit_terminal(
                TERMINAL_STATUS_ERROR,
                f"匹配过程出错: {exc}",
                matched_count=0,
                total_count=len(bad_evaluations),
            )
            return

        matched_results = [r for r in results if r["matched"]]
        autofill_order_ids = [
            r["orderId"]
            for r in matched_results
            if r["orderId"] and r["matchScore"] >= AUTO_FILL_SCORE_THRESHOLD
        ]
        self._emit_match_summary(
            bad_evaluations,
            matched_results,
            autofill_order_ids,
            progress,
        )

        self.results_ready.emit(results)
        self.order_ids_ready.emit(autofill_order_ids)
        if self.task_type == TASK_REVIEW_MATCH and (self.days > ORDER_CACHE_COVERAGE_DAYS or len(matched_results) < len(bad_evaluations)):
            order_warnings = list(order_warnings)
            order_warnings.append("当前结果仅基于最近 30 天订单缓存；如需更完整结果，请点击“完整补查订单”。")
        self._emit_terminal(
            TERMINAL_STATUS_WARNING if order_warnings else TERMINAL_STATUS_SUCCESS,
            "\n".join(order_warnings),
            matched_count=len(matched_results),
            total_count=len(bad_evaluations),
        )
