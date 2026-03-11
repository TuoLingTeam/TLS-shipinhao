# -*- coding: utf-8 -*-
"""TLS-shipinhao 中差评查找后台执行器。"""

import threading

from PySide6.QtCore import QObject, Signal

from .config import ConfigNotFoundError, get_cookie, get_magic
from .review_matcher import BadReviewOrderFinder


class ReviewMatcherWorker(QObject):
    """后台中差评查找执行器。"""

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

        finder = BadReviewOrderFinder(cookie_str, magic)

        def _progress(msg):
            if not self._stopped:
                self.progress.emit(msg)

        # 阶段 1: 获取差评
        try:
            _progress("=== 开始获取差评数据 ===")
            bad_evaluations = finder.get_bad_evaluations(
                days=self.days, on_progress=_progress
            )
        except Exception as exc:  # noqa: BLE001
            self.error.emit(f"获取差评数据失败: {exc}")
            self.finished.emit(0, 0)
            return

        if self._stopped:
            self.finished.emit(0, 0)
            return

        if not bad_evaluations:
            _progress("未找到差评数据。")
            self.finished.emit(0, 0)
            return

        _progress(f"差评获取完成，共 {len(bad_evaluations)} 条。")

        # 阶段 2: 获取订单
        try:
            _progress("=== 开始获取订单数据 ===")
            orders = finder.get_orders(on_progress=_progress)
        except Exception as exc:  # noqa: BLE001
            self.error.emit(f"获取订单数据失败: {exc}")
            self.finished.emit(0, len(bad_evaluations))
            return

        if self._stopped:
            self.finished.emit(0, len(bad_evaluations))
            return

        if not orders:
            _progress("未获取到订单数据。")
            self.finished.emit(0, len(bad_evaluations))
            return

        _progress(f"订单获取完成，共 {len(orders)} 个。")

        # 阶段 3: 匹配
        try:
            _progress("=== 开始匹配差评和订单 ===")
            results = finder.match_orders_with_evaluations(
                bad_evaluations, orders, on_progress=_progress
            )
        except Exception as exc:  # noqa: BLE001
            self.error.emit(f"匹配过程出错: {exc}")
            self.finished.emit(0, len(bad_evaluations))
            return

        # 提取匹配到的订单号
        matched_results = [r for r in results if r["matched"]]
        matched_order_ids = [r["orderId"] for r in matched_results if r["orderId"]]

        # 输出匹配摘要
        _progress(f"\n=== 匹配完成 ===")
        _progress(
            f"共 {len(bad_evaluations)} 条差评，"
            f"匹配到 {len(matched_results)} 个订单。"
        )

        if matched_results:
            _progress("\n匹配成功的订单:")
            for r in matched_results:
                _progress(
                    f"  订单 {r['orderId']} | "
                    f"买家: {r['buyerNickname']} | "
                    f"商品: {r['productName']} | "
                    f"得分: {r['matchScore']}"
                )

        self.results_ready.emit(results)
        self.order_ids_ready.emit(matched_order_ids)
        self.finished.emit(len(matched_results), len(bad_evaluations))
