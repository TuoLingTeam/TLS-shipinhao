# -*- coding: utf-8 -*-
"""TLS-shipinhao 后台批量任务执行器。"""

import threading

from PySide6.QtCore import QObject, Signal

from .api import create_session, update_single_order
from .config import ConfigNotFoundError


class BatchWorker(QObject):
    """后台批量执行器。"""

    started = Signal(int)
    step_started = Signal(int, int, str)
    step_succeeded = Signal(int, int, str, str, str)
    step_failed = Signal(int, int, str, str, str)
    fatal_error = Signal(str)
    missing_config = Signal(str)
    finished = Signal(int, int, int, bool)

    def __init__(self, order_ids, tracking_numbers):
        super().__init__()
        self.order_ids = order_ids
        self.tracking_numbers = tracking_numbers
        self._resume_event = threading.Event()
        self._resume_event.set()

    def pause(self):
        """暂停后续任务。"""
        self._resume_event.clear()

    def resume(self):
        """恢复任务。"""
        self._resume_event.set()

    def run(self):
        """后台线程执行入口。"""
        success_count = 0
        failure_count = 0
        total_count = len(self.order_ids)
        self.started.emit(total_count)

        try:
            session = create_session()
        except ConfigNotFoundError as exc:
            self.missing_config.emit("\n".join(exc.searched_dirs))
            self.finished.emit(0, 0, total_count, True)
            return

        try:
            with session:
                for index, (order_id, tracking_number) in enumerate(
                    zip(self.order_ids, self.tracking_numbers), start=1
                ):
                    self._resume_event.wait()
                    self.step_started.emit(index, total_count, order_id)
                    try:
                        old_waybill = update_single_order(order_id, tracking_number, session)
                    except Exception as exc:  # noqa: BLE001
                        failure_count += 1
                        self.step_failed.emit(
                            index,
                            total_count,
                            order_id,
                            tracking_number,
                            str(exc),
                        )
                        continue

                    success_count += 1
                    self.step_succeeded.emit(
                        index,
                        total_count,
                        order_id,
                        tracking_number,
                        old_waybill or "无原物流单号",
                    )
        except Exception as exc:  # noqa: BLE001
            failure_count += total_count - success_count - failure_count
            self.fatal_error.emit(str(exc))
        finally:
            self.finished.emit(success_count, failure_count, total_count, False)
