#!/usr/bin/env python3
# -*- coding: utf-8 -*-
"""统一构建入口。"""

import os
import platform
import subprocess
import sys


PROJECT_ROOT = os.path.dirname(os.path.dirname(os.path.abspath(__file__)))


def main():
    system = platform.system()
    if system == "Darwin":
        cmd = [os.path.join(PROJECT_ROOT, "build_mac.sh")]
    elif system == "Windows":
        cmd = [os.path.join(PROJECT_ROOT, "build_windows.bat")]
    else:
        raise SystemExit(f"不支持的系统: {system}")

    raise SystemExit(subprocess.call(cmd, cwd=PROJECT_ROOT))


if __name__ == "__main__":
    main()
