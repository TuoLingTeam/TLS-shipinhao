# -*- coding: utf-8 -*-
"""TLS-shipinhao 后台批量任务执行器。"""

import threading
import time

from PySide6.QtCore import QObject, Signal

from core.license import issue_or_refresh_session_token
from services.delivery_api import create_session, update_single_order
from settings import ConfigNotFoundError, LICENSE_TASK_BATCH_DELIVERY


class BatchWorker(QObject):
    """后台批量执行器。"""

    started = Signal(int)
    step_started = Signal(int, int, str)
    step_succeeded = Signal(int, int, str, str, str)
    step_failed = Signal(int, int, str, str, str)
    fatal_error = Signal(str)
    missing_config = Signal(str)
    finished = Signal(int, int, int, bool)

    _WAIT_POLL_INTERVAL = 0.5
    _STEP_INTERVAL = 1.0

    def __init__(self, order_ids, tracking_numbers):
        super().__init__()
        self.order_ids = order_ids
        self.tracking_numbers = tracking_numbers
        self._resume_event = threading.Event()
        self._resume_event.set()
        self._stopped = False

    def pause(self):
        self._resume_event.clear()

    def resume(self):
        self._resume_event.set()

    def stop(self):
        self._stopped = True
        self._resume_event.set()

    def _wait_for_resume(self):
        while not self._resume_event.wait(timeout=self._WAIT_POLL_INTERVAL):
            if self._stopped:
                return False
        return not self._stopped

    def _sleep_between_steps(self, index: int, total_count: int) -> None:
        if index < total_count and not self._stopped:
            time.sleep(self._STEP_INTERVAL)

    def _emit_step_failure(self, index, total_count, order_id, tracking_number, error):
        self.step_failed.emit(index, total_count, order_id, tracking_number, str(error))

    def _emit_step_success(self, index, total_count, order_id, tracking_number, old_waybill):
        self.step_succeeded.emit(
            index,
            total_count,
            order_id,
            tracking_number,
            old_waybill or "无原物流单号",
        )

    def run(self):
        success_count = 0
        failure_count = 0
        total_count = len(self.order_ids)
        self.started.emit(total_count)
        last_session_refresh = time.monotonic()

        _, reason = issue_or_refresh_session_token(LICENSE_TASK_BATCH_DELIVERY, force=True)
        if reason != "ok":
            self.fatal_error.emit("授权会话已失效，请联网后重试。")
            self.finished.emit(0, total_count, total_count, True)
            return

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
                    if not self._wait_for_resume():
                        break
                    if index == 1 or (index - 1) % 10 == 0 or (time.monotonic() - last_session_refresh) >= 60:
                        _, reason = issue_or_refresh_session_token(LICENSE_TASK_BATCH_DELIVERY)
                        if reason != "ok":
                            raise RuntimeError("授权会话已失效，请联网后重试。")
                        last_session_refresh = time.monotonic()

                    self.step_started.emit(index, total_count, order_id)
                    try:
                        old_waybill = update_single_order(order_id, tracking_number, session)
                    except Exception as exc:  # noqa: BLE001
                        failure_count += 1
                        self._emit_step_failure(index, total_count, order_id, tracking_number, exc)
                        self._sleep_between_steps(index, total_count)
                        continue

                    success_count += 1
                    self._emit_step_success(index, total_count, order_id, tracking_number, old_waybill)
                    self._sleep_between_steps(index, total_count)
        except Exception as exc:  # noqa: BLE001
            failure_count += total_count - success_count - failure_count
            self.fatal_error.emit(str(exc))
        finally:
            self.finished.emit(success_count, failure_count, total_count, self._stopped)
