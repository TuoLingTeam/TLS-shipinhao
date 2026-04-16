#!/usr/bin/env python3
# -*- coding: utf-8 -*-
"""TLS-shipinhao 工具入口。"""

from __future__ import annotations

import importlib
import sys
from pathlib import Path

APP_DIR = Path(__file__).resolve().parent
REPO_ROOT = APP_DIR.parent


def _prepend_import_paths() -> None:
    """兼容源码运行与打包运行的导入路径。"""
    for candidate in (REPO_ROOT, APP_DIR):
        candidate_str = str(candidate)
        if candidate_str not in sys.path:
            sys.path.insert(0, candidate_str)


def _load_main_entry():
    """优先按包结构加载入口，失败时再回退到平铺模块。"""
    try:
        settings_module = importlib.import_module("app.settings")
        sys.modules.setdefault("settings", settings_module)

        bootstrap_module = importlib.import_module("app.bootstrap")
        sys.modules.setdefault("bootstrap", bootstrap_module)
        return bootstrap_module.main
    except ModuleNotFoundError:
        from bootstrap import main as bootstrap_main
        return bootstrap_main


_prepend_import_paths()
main = _load_main_entry()


if __name__ == "__main__":
    main()
