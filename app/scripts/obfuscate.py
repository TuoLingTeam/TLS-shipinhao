#!/usr/bin/env python3
# -*- coding: utf-8 -*-
"""将 app/ 源码混淆后输出到 app-dist/ 分发目录。

使用 pyarmor 对 Python 源码进行混淆加密，保留非 Python 资源文件。
混淆后的代码可直接用于 PyInstaller 打包。

用法：
    python app/scripts/obfuscate.py          # 默认混淆
    python app/scripts/obfuscate.py --clean   # 仅清理输出目录
"""

from __future__ import annotations

import os
import shutil
import subprocess
import sys
from pathlib import Path

# =========================
# 路径常量
# =========================
APP_ROOT = Path(__file__).resolve().parent.parent       # app/
REPO_ROOT = APP_ROOT.parent                              # 仓库根目录
DIST_SRC = REPO_ROOT / "app-dist"                        # 混淆输出目录

SRC_DIR = APP_ROOT / "src"
MAIN_FILE = APP_ROOT / "main.py"

# 不复制到分发目录的文件夹
SKIP_DIRS = {"__pycache__", "scripts", ".git"}
# 需要原样复制的资源文件扩展名
RESOURCE_SUFFIXES = {".png", ".ico", ".icns", ".jpg", ".jpeg", ".gif", ".svg"}


# =========================
# 清理
# =========================
def clean_dist() -> None:
    """清理旧的分发目录。"""
    if DIST_SRC.exists():
        shutil.rmtree(DIST_SRC)
    DIST_SRC.mkdir(parents=True)
    print(f"已清理输出目录: {DIST_SRC}")


# =========================
# 资源复制
# =========================
def copy_resources() -> None:
    """将非 Python 资源文件（图标等）复制到分发目录。"""
    count = 0
    for root, dirs, files in os.walk(APP_ROOT):
        dirs[:] = [d for d in dirs if d not in SKIP_DIRS]
        rel = Path(root).relative_to(APP_ROOT)
        for filename in files:
            if Path(filename).suffix.lower() in RESOURCE_SUFFIXES:
                dest = DIST_SRC / rel / filename
                dest.parent.mkdir(parents=True, exist_ok=True)
                shutil.copy2(Path(root) / filename, dest)
                count += 1

    # requirements.txt
    req = APP_ROOT / "requirements.txt"
    if req.exists():
        shutil.copy2(req, DIST_SRC / "requirements.txt")
        count += 1

    print(f"已复制 {count} 个资源文件")


# =========================
# pyarmor 混淆
# =========================
def ensure_pyarmor() -> None:
    """确保 pyarmor 已安装。"""
    try:
        subprocess.run(
            [sys.executable, "-m", "pyarmor", "--version"],
            capture_output=True, check=True,
        )
    except (subprocess.CalledProcessError, FileNotFoundError):
        print("pyarmor 未安装，正在安装...")
        subprocess.run(
            [sys.executable, "-m", "pip", "install", "pyarmor"],
            check=True,
        )


def obfuscate() -> None:
    """使用 pyarmor 混淆所有 Python 源码。"""
    ensure_pyarmor()

    # 混淆 src/ 包（递归处理所有子模块）
    print("混淆 src/ 包...")
    subprocess.run(
        [
            sys.executable, "-m", "pyarmor", "gen",
            "-O", str(DIST_SRC),
            "-r",
            str(SRC_DIR),
        ],
        cwd=str(APP_ROOT),
        check=True,
    )

    # 混淆入口 main.py
    print("混淆 main.py...")
    subprocess.run(
        [
            sys.executable, "-m", "pyarmor", "gen",
            "-O", str(DIST_SRC),
            str(MAIN_FILE),
        ],
        cwd=str(APP_ROOT),
        check=True,
    )

    print("混淆完成")


# =========================
# 验证
# =========================
def show_tree() -> None:
    """显示分发目录结构。"""
    print("\n分发目录结构:")
    for root, dirs, files in os.walk(DIST_SRC):
        dirs.sort()
        level = len(Path(root).relative_to(DIST_SRC).parts)
        indent = "  " * level
        dirname = Path(root).name if root != str(DIST_SRC) else "app-dist"
        print(f"{indent}{dirname}/")
        for f in sorted(files):
            print(f"{indent}  {f}")


# =========================
# 入口
# =========================
def main() -> None:
    print("=" * 50)
    print("TLS-shipinhao 代码混淆工具")
    print("=" * 50)
    print(f"源码目录: {APP_ROOT}")
    print(f"输出目录: {DIST_SRC}")
    print()

    if "--clean" in sys.argv:
        clean_dist()
        print("清理完成。")
        return

    clean_dist()
    copy_resources()
    obfuscate()
    show_tree()

    print(f"\n✅ 分发目录已生成: {DIST_SRC}")
    print("可使用 build.py --dist 从混淆源码构建打包产物。")


if __name__ == "__main__":
    main()
