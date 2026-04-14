#!/usr/bin/env python3
# -*- coding: utf-8 -*-
"""TLS-shipinhao 工具入口。"""

import sys
import importlib
from pathlib import Path

APP_DIR = Path(__file__).resolve().parent
REPO_ROOT = APP_DIR.parent

for candidate in (REPO_ROOT, APP_DIR):
    candidate_str = str(candidate)
    if candidate_str not in sys.path:
        sys.path.insert(0, candidate_str)

try:
    settings_module = importlib.import_module("app.settings")
    sys.modules.setdefault("settings", settings_module)

    bootstrap_module = importlib.import_module("app.bootstrap")
    sys.modules.setdefault("bootstrap", bootstrap_module)
    main = bootstrap_module.main
except ModuleNotFoundError:
    from bootstrap import main


if __name__ == "__main__":
    main()
