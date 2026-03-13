# -*- coding: utf-8 -*-
"""TLS-shipinhao工具入口。"""

import sys

from PySide6.QtWidgets import QApplication

from .window import MainWindow

from .license import check_stored_license


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
