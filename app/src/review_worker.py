# -*- coding: utf-8 -*-
"""TLS-shipinhao 中差评查找后台执行器。"""

import time
from concurrent.futures import ThreadPoolExecutor, as_completed

from PySide6.QtCore import QObject, Signal

from .config import ConfigNotFoundError, get_cookie, get_magic, serialize_cookie_data
from .review_matcher import AUTO_FILL_SCORE_THRESHOLD, BadReviewOrderFinder

# 订单并发拉取线程数（降低并发数防 429 限流）
ORDER_FETCH_WORKERS = 3
ORDER_FETCH_BUFFER_DAYS = 30
TASK_REVIEW_MATCH = "review_match"
TASK_QUALITY_REFUND = "quality_refund"


class ReviewMatcherWorker(QObject):
    """后台中差评查找执行器。

    性能优化策略:
      1. 差评获取 和 订单获取 **阶段并行执行**
      2. 订单获取内部再拆成 3 个线程按时间段并发拉取
      3. 差评翻页间隔缩短至 0.3 秒
      4. 订单翻页加 0.3 秒限速防止 429 限流
    """

    progress = Signal(str)
    results_ready = Signal(list)
    order_ids_ready = Signal(list)
    error = Signal(str)
    missing_config = Signal(str)
    finished = Signal(int, int)  # (matched_count, total_count)

    def __init__(self, days=30, task_type=TASK_REVIEW_MATCH):
        super().__init__()
        self.days = days
        self.task_type = task_type
        self._stopped = False

    def stop(self):
        """请求终止任务（安全退出）。"""
        self._stopped = True

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

    def _emit_error_and_finish(self, message, matched_count=0, total_count=0):
        self.error.emit(message)
        self.finished.emit(matched_count, total_count)

    def _emit_empty_results(self, progress, message):
        """输出空结果状态并同步结束信号。"""
        progress(message)
        self.results_ready.emit([])
        self.order_ids_ready.emit([])
        self.finished.emit(0, 0)

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
            self.finished.emit(0, 0)
            return None
        except Exception as exc:  # noqa: BLE001
            self._emit_error_and_finish(f"读取配置失败: {exc}")
            return None

        try:
            magic = get_magic(cookie_data)
        except Exception as exc:  # noqa: BLE001
            self._emit_error_and_finish(f"提取 biz_magic 失败: {exc}")
            return None

        return serialize_cookie_data(cookie_data), magic

    def _build_order_earliest_time(self):
        """计算订单抓取的时间窗口下限。"""
        return int(time.time()) - (self.days + ORDER_FETCH_BUFFER_DAYS) * 86400

    def _build_quality_refund_earliest_time(self):
        """计算品质退款订单筛选下限。"""
        return int(time.time()) - self.days * 86400

    def _fetch_active_evaluations(self, cookie_str, magic, progress):
        """获取并过滤有效主动评价。"""
        finder = BadReviewOrderFinder(cookie_str, magic)
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

    def _fetch_orders(self, cookie_str, magic, earliest_time, progress):
        """获取订单列表。"""
        finder = BadReviewOrderFinder(cookie_str, magic)
        return finder.get_orders_concurrent(
            earliest_time=earliest_time,
            num_workers=ORDER_FETCH_WORKERS,
            on_progress=progress,
        )

    def _fetch_quality_refund_orders(self, cookie_str, magic, earliest_time, progress):
        """获取品质退款订单列表。"""
        finder = BadReviewOrderFinder(cookie_str, magic)
        return finder.get_quality_refund_orders(
            earliest_time=earliest_time,
            on_progress=progress,
        )

    def _fetch_data_in_parallel(self, cookie_str, magic, earliest_time, progress):
        """并行获取差评和订单数据。"""
        results = {
            "bad_evaluations": None,
            "orders": None,
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
            "orders": lambda: self._fetch_orders(
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
                    results[key] = future.result()
                except Exception as exc:  # noqa: BLE001
                    errors[key] = str(exc)

        return (
            results["bad_evaluations"],
            results["orders"],
            errors["bad_evaluations"],
            errors["orders"],
        )

    def _match_orders(self, cookie_str, magic, bad_evaluations, orders, progress):
        """执行差评与订单匹配。"""
        progress("=== 开始匹配差评和订单 ===")
        matcher = BadReviewOrderFinder(cookie_str, magic)
        return matcher.match_orders_with_evaluations(
            bad_evaluations,
            orders,
            on_progress=progress,
        )

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
        progress("\n匹配成功的订单明细:")
        for item in matched_results:
            score = item["matchScore"]
            mark = (
                "✅ 自动填入"
                if score >= AUTO_FILL_SCORE_THRESHOLD
                else "⚠️ 需人工核对"
            )
            progress(
                f"  {mark} | 得分: {score} | "
                f"订单: {item['orderId']} | "
                f"买家: {item['buyerNickname']} | "
                f"商品: {item['productName']}"
            )

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
            self._emit_error_and_finish(f"获取品质退款订单失败: {exc}")
            return

        if self._stopped:
            self.finished.emit(0, 0)
            return

        if not orders:
            self._emit_empty_results(progress, "未找到品质退款订单。")
            return

        order_ids = self._collect_unique_order_ids(orders)

        self._emit_quality_refund_summary(orders, order_ids, progress)
        self.results_ready.emit(orders)
        self.order_ids_ready.emit(order_ids)
        self.finished.emit(len(order_ids), len(orders))

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

        earliest_time = self._build_order_earliest_time()

        # ---------------------------------------------------------------
        # 阶段 1+2: 并行获取 差评 & 订单（订单内部再多线程）
        # ---------------------------------------------------------------
        progress(f"=== 并行获取差评 + 订单（{ORDER_FETCH_WORKERS} 线程并发）===")
        bad_evaluations, orders, eval_error, order_error = self._fetch_data_in_parallel(
            cookie_str,
            magic,
            earliest_time,
            progress,
        )

        if self._stopped:
            self.finished.emit(0, 0)
            return

        if eval_error:
            self._emit_error_and_finish(f"获取差评数据失败: {eval_error}")
            return

        if order_error:
            self._emit_error_and_finish(
                f"获取订单数据失败: {order_error}",
                matched_count=0,
                total_count=len(bad_evaluations or []),
            )
            return

        if not bad_evaluations:
            progress("未找到差评数据。")
            self.finished.emit(0, 0)
            return

        if not orders:
            progress("未获取到订单数据。")
            self.finished.emit(0, len(bad_evaluations))
            return

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
            self._emit_error_and_finish(
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
        # finished 发送的还是找到的总疑似订单数
        self.finished.emit(len(matched_results), len(bad_evaluations))
