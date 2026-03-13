# -*- coding: utf-8 -*-
"""TLS-shipinhao工具入口。"""

import sys

from PySide6.QtCore import Qt
from PySide6.QtGui import QGuiApplication
from PySide6.QtWidgets import QApplication

from .window import MainWindow

from .license import check_stored_license


def main():
    """程序入口。"""
    # 高 DPI 舍入策略必须在 QApplication 创建前设置，否则 4K/150% 等缩放下表现不一致
    QGuiApplication.setHighDpiScaleFactorRoundingPolicy(
        Qt.HighDpiScaleFactorRoundingPolicy.PassThrough
    )
    app = QApplication(sys.argv)
    app.setStyle("Fusion")
    info, reason = check_stored_license()
    window = MainWindow(license_reason=reason, license_info=info)
    window.show()
    if reason != "ok":
        window.prompt_license_on_startup()
    sys.exit(app.exec())


if __name__ == "__main__":
    main()
