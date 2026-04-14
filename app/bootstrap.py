# -*- coding: utf-8 -*-
"""TLS-shipinhao 工具启动入口。"""

import sys

try:
    import settings as _settings  # noqa: F401
    from ui.window import MainWindow
    from core.license import check_stored_license
except ModuleNotFoundError:
    from app import settings as _settings  # noqa: F401
    from app.ui.window import MainWindow
    from app.core.license import check_stored_license

from PySide6.QtWidgets import QApplication


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
