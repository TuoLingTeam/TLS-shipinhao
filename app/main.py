#!/usr/bin/env python3
# -*- coding: utf-8 -*-
"""TLS-shipinhao 工具入口。"""

import sys
from pathlib import Path

APP_DIR = Path(__file__).resolve().parent
if str(APP_DIR) not in sys.path:
    sys.path.insert(0, str(APP_DIR))

from bootstrap import main


if __name__ == "__main__":
    main()
