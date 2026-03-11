# -*- coding: utf-8 -*-
"""TLS-shipinhao 中差评查找后台执行器。"""

import time
from concurrent.futures import ThreadPoolExecutor, as_completed

from PySide6.QtCore import QObject, Signal

from .config import ConfigNotFoundError, get_cookie, get_magic
from .review_matcher import BadReviewOrderFinder

# 订单并发拉取线程数（降低并发数防 429 限流）
ORDER_FETCH_WORKERS = 3


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

    def __init__(self, days=30):
        super().__init__()
        self.days = days
        self._stopped = False

    def stop(self):
        """请求终止任务（安全退出）。"""
        self._stopped = True

    def run(self):
        """后台线程执行入口。"""
        try:
            cookie_data = get_cookie()
        except ConfigNotFoundError as exc:
            self.missing_config.emit("\n".join(exc.searched_dirs))
            self.finished.emit(0, 0)
            return
        except Exception as exc:  # noqa: BLE001
            self.error.emit(f"读取配置失败: {exc}")
            self.finished.emit(0, 0)
            return

        try:
            magic = get_magic(cookie_data)
        except Exception as exc:  # noqa: BLE001
            self.error.emit(f"提取 biz_magic 失败: {exc}")
            self.finished.emit(0, 0)
            return

        # 将 cookie 字典还原为字符串
        cookie_str = "; ".join(f"{k}={v}" for k, v in cookie_data.items())

        def _progress(msg):
            if not self._stopped:
                self.progress.emit(msg)

        # 订单时间窗口下限 = 评价天数 + 30 天缓冲
        earliest_time = int(time.time()) - (self.days + 30) * 86400

        # ---------------------------------------------------------------
        # 阶段 1+2: 并行获取 差评 & 订单（订单内部再多线程）
        # ---------------------------------------------------------------
        _progress(
            f"=== 并行获取差评 + 订单（{ORDER_FETCH_WORKERS} 线程并发）==="
        )

        bad_evaluations = None
        orders = None
        eval_error = None
        order_error = None

        def _fetch_evaluations():
            nonlocal bad_evaluations, eval_error
            try:
                finder = BadReviewOrderFinder(cookie_str, magic)
                _progress("[差评] 开始获取...")
                raw_evaluations = finder.get_bad_evaluations(
                    days=self.days, on_progress=_progress
                )
                # 优化6：过滤系统自动评价（autoEvaluation=1），只保留主动评价
                # 追评和系统自动评价均视为无效评价，不参与匹配
                bad_evaluations = [
                    e for e in raw_evaluations
                    if e.get("evaluationInfo", {})
                       .get("firstEvaluationInfo", {})
                       .get("buyerEvaluationInfo", {})
                       .get("autoEvaluation", 0) == 0
                ]
                auto_count = len(raw_evaluations) - len(bad_evaluations)
                _progress(
                    f"[差评] 完成，共 {len(raw_evaluations)} 条，"
                    f"过滤系统自动评价 {auto_count} 条，"
                    f"有效主动评价 {len(bad_evaluations)} 条。"
                )
            except Exception as exc:  # noqa: BLE001
                eval_error = str(exc)


        def _fetch_orders():
            nonlocal orders, order_error
            try:
                finder = BadReviewOrderFinder(cookie_str, magic)
                orders = finder.get_orders_concurrent(
                    earliest_time=earliest_time,
                    num_workers=ORDER_FETCH_WORKERS,
                    on_progress=_progress,
                )
            except Exception as exc:  # noqa: BLE001
                order_error = str(exc)

        # 差评和订单阶段并行
        with ThreadPoolExecutor(max_workers=2, thread_name_prefix="phase") as pool:
            futures = [
                pool.submit(_fetch_evaluations),
                pool.submit(_fetch_orders),
            ]
            for f in as_completed(futures):
                pass

        if self._stopped:
            self.finished.emit(0, 0)
            return

        if eval_error:
            self.error.emit(f"获取差评数据失败: {eval_error}")
            self.finished.emit(0, 0)
            return
        if order_error:
            self.error.emit(f"获取订单数据失败: {order_error}")
            self.finished.emit(0, len(bad_evaluations or []))
            return

        if not bad_evaluations:
            _progress("未找到差评数据。")
            self.finished.emit(0, 0)
            return
        if not orders:
            _progress("未获取到订单数据。")
            self.finished.emit(0, len(bad_evaluations))
            return

        _progress(
            f"数据获取完成: {len(bad_evaluations)} 条差评, "
            f"{len(orders)} 个订单。"
        )

        # ---------------------------------------------------------------
        # 阶段 3: 匹配
        # ---------------------------------------------------------------
        try:
            _progress("=== 开始匹配差评和订单 ===")
            matcher = BadReviewOrderFinder(cookie_str, magic)
            results = matcher.match_orders_with_evaluations(
                bad_evaluations, orders, on_progress=_progress
            )
        except Exception as exc:  # noqa: BLE001
            self.error.emit(f"匹配过程出错: {exc}")
            self.finished.emit(0, len(bad_evaluations))
            return

        # 提取匹配到的订单号
        matched_results = [r for r in results if r["matched"]]
        autofill_order_ids = [r["orderId"] for r in matched_results if r["orderId"] and r["matchScore"] >= 100]
        manual_review_count = len(matched_results) - len(autofill_order_ids)

        # 输出匹配摘要
        _progress(f"\n=== 匹配完成 ===")
        _progress(
            f"共 {len(bad_evaluations)} 条差评，"
            f"初步匹配到 {len(matched_results)} 个订单。"
        )
        
        if matched_results:
            _progress(
                f"其中 {len(autofill_order_ids)} 个得分达标(100分)已自动填入，"
                f"{manual_review_count} 个需人工核对。"
            )
            _progress("\n匹配成功的订单明细:")
            for r in matched_results:
                score = r["matchScore"]
                mark = "✅ 自动填入" if score >= 100 else "⚠️ 需人工核对"
                _progress(
                    f"  {mark} | 得分: {score} | "
                    f"订单: {r['orderId']} | "
                    f"买家: {r['buyerNickname']} | "
                    f"商品: {r['productName']}"
                )

        self.results_ready.emit(results)
        self.order_ids_ready.emit(autofill_order_ids)
        # finished 发送的还是找到的总疑似订单数
        self.finished.emit(len(matched_results), len(bad_evaluations))
