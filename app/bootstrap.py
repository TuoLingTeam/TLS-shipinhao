# -*- coding: utf-8 -*-
"""TLS-shipinhao 工具启动入口。"""

from __future__ import annotations

import sys
from pathlib import Path

from PySide6.QtCore import QTimer
from PySide6.QtGui import QIcon
from PySide6.QtWidgets import QApplication


def _load_runtime_objects():
    """兼容源码态与打包态的导入路径。"""
    try:
        import settings as _settings  # noqa: F401
        from ui.window import MainWindow
        from core.license import check_stored_license_local
    except ModuleNotFoundError:
        from app import settings as _settings  # noqa: F401
        from app.ui.window import MainWindow
        from app.core.license import check_stored_license_local
    return MainWindow, check_stored_license_local


def _resolve_app_icon_path() -> Path | None:
    """定位应用图标路径，兼容源码态与打包态。"""
    base_dir = Path(__file__).resolve().parent
    candidates = [
        base_dir / "assets" / "favicon.png",
        base_dir.parent / "app" / "assets" / "favicon.png",
    ]
    for candidate in candidates:
        if candidate.exists():
            return candidate
    return None


def _apply_app_icon(app: QApplication) -> None:
    """为应用设置统一图标。"""
    icon_path = _resolve_app_icon_path()
    if icon_path is None:
        return
    icon = QIcon(str(icon_path))
    if not icon.isNull():
        app.setWindowIcon(icon)


def main():
    """程序入口。"""
    MainWindow, check_stored_license_local = _load_runtime_objects()
    app = QApplication(sys.argv)
    _apply_app_icon(app)
    app.setStyle("Fusion")
    info, reason = check_stored_license_local()
    window = MainWindow(license_reason=reason, license_info=info)
    window.show()
    if reason in {"ok", "renewal_due"}:
        QTimer.singleShot(1200, window.trigger_background_update_check)
    else:
        window.prompt_license_on_startup()
    sys.exit(app.exec())


if __name__ == "__main__":
    main()
