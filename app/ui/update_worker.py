# -*- coding: utf-8 -*-
"""在线更新检查后台 worker。"""

from PySide6.QtCore import QObject, Signal

from services.update_service import fetch_latest_version_info


class UpdateCheckWorker(QObject):
    finished = Signal(object)
    failed = Signal(str)

    def __init__(self, current_version: str, parent=None):
        super().__init__(parent)
        self._current_version = current_version

    def run(self):
        try:
            info = fetch_latest_version_info(self._current_version)
        except Exception as exc:  # noqa: BLE001
            self.failed.emit(str(exc))
            return
        self.finished.emit(info)
