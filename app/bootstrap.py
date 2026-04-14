# -*- coding: utf-8 -*-
"""TLS-shipinhao工具入口。"""

import sys

# 预加载顶层模块，确保 frozen 环境中子包的相对 import 能正确解析
import settings as _constants  # noqa: F401
import settings as _config  # noqa: F401

from PySide6.QtWidgets import QApplication

from ui.window import MainWindow
from core.license import check_stored_license


def main():
    """程序入口。"""
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
