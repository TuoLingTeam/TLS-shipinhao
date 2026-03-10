#!/usr/bin/env python3
# -*- coding: utf-8 -*-
"""将 app/ 源码编译后输出到 app-dist/ 分发目录。

使用 Cython 将 Python 源码编译为 .so/.pyd 二进制扩展模块，
编译后无法反编译出源码，保护强度高。
混淆后的代码可直接用于 PyInstaller 打包。

用法：
    python app/scripts/obfuscate.py          # 编译
    python app/scripts/obfuscate.py --clean   # 仅清理输出目录
"""

from __future__ import annotations

import os
import platform
import shutil
import subprocess
import sys
import textwrap
from pathlib import Path

# =========================
# 路径常量
# =========================
APP_ROOT = Path(__file__).resolve().parent.parent       # app/
REPO_ROOT = APP_ROOT.parent                              # 仓库根目录
DIST_SRC = REPO_ROOT / "app-dist"                        # 混淆输出目录

SRC_DIR = APP_ROOT / "src"
MAIN_FILE = APP_ROOT / "main.py"


def _project_python() -> str:
    """优先返回项目 .venv 的 Python。"""
    if platform.system() == "Windows":
        candidate = REPO_ROOT / ".venv" / "Scripts" / "python.exe"
    else:
        candidate = REPO_ROOT / ".venv" / "bin" / "python"
    return str(candidate) if candidate.exists() else sys.executable


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
# Cython 编译
# =========================
def ensure_cython() -> None:
    """确保 Cython 和 setuptools 已安装。"""
    python_bin = _project_python()
    try:
        subprocess.run(
            [python_bin, "-c", "import Cython; import setuptools"],
            capture_output=True, check=True,
        )
    except subprocess.CalledProcessError:
        print("安装 Cython + setuptools...")
        subprocess.run(
            [python_bin, "-m", "pip", "install", "Cython", "setuptools"],
            check=True,
        )


def compile_with_cython() -> None:
    """使用 Cython 将 src/ 下所有 .py 编译为 .so/.pyd。"""
    ensure_cython()
    python_bin = _project_python()

    # 收集需要编译的 .py 文件（排除 __init__.py，保留为纯 .py 以确保包可导入）
    py_files = [
        f"src/{f}" for f in os.listdir(SRC_DIR)
        if f.endswith(".py") and f != "__init__.py"
    ]
    if not py_files:
        print("没有找到需要编译的 .py 文件")
        return

    print(f"待编译文件: {len(py_files)} 个")
    for f in py_files:
        print(f"  {f}")

    # 生成临时 setup.py
    setup_content = textwrap.dedent(f"""\
        from setuptools import setup
        from Cython.Build import cythonize

        setup(
            ext_modules=cythonize(
                {py_files!r},
                compiler_directives={{
                    "language_level": "3",
                }},
            ),
        )
    """)

    setup_file = APP_ROOT / "_cython_setup.py"
    build_temp = APP_ROOT / "_cython_temp"

    try:
        setup_file.write_text(setup_content, encoding="utf-8")

        print("\n开始 Cython 编译...")
        subprocess.run(
            [
                python_bin, str(setup_file),
                "build_ext",
                "--build-lib", str(DIST_SRC),
                "--build-temp", str(build_temp),
            ],
            cwd=str(APP_ROOT),
            check=True,
        )
        print("Cython 编译完成")
    finally:
        # 清理临时文件
        setup_file.unlink(missing_ok=True)
        if build_temp.exists():
            shutil.rmtree(build_temp)
        # 清理 Cython 在源目录生成的 .c 文件
        for c_file in SRC_DIR.glob("*.c"):
            c_file.unlink()

    # 复制 __init__.py（保持包结构）
    init_src = SRC_DIR / "__init__.py"
    if init_src.exists():
        init_dst = DIST_SRC / "src" / "__init__.py"
        init_dst.parent.mkdir(parents=True, exist_ok=True)
        shutil.copy2(init_src, init_dst)

    # 复制 main.py 入口（PyInstaller 需要 .py 入口文件）
    shutil.copy2(MAIN_FILE, DIST_SRC / "main.py")
    
    # 修复 Cython 编译后的导入问题
    fix_cython_imports()


# =========================
# 修复导入问题
# =========================
def fix_cython_imports() -> None:
    """修复 Cython 编译后的导入问题。"""
    print("修复 Cython 导入问题...")
    
    # 读取原始 app.py 内容
    app_py_src = SRC_DIR / "app.py"
    if not app_py_src.exists():
        return
        
    app_content = app_py_src.read_text(encoding="utf-8")
    
    # 将相对导入改为绝对导入
    fixed_content = app_content.replace("from .window import", "from src.window import")
    fixed_content = fixed_content.replace("from .license import", "from src.license import")
    fixed_content = fixed_content.replace("from .config import", "from src.config import")
    fixed_content = fixed_content.replace("from .constants import", "from src.constants import")
    fixed_content = fixed_content.replace("from .widgets import", "from src.widgets import")
    fixed_content = fixed_content.replace("from .api import", "from src.api import")
    fixed_content = fixed_content.replace("from .worker import", "from src.worker import")
    
    # 写入修复后的 app.py
    app_py_dist = DIST_SRC / "src" / "app.py"
    app_py_dist.write_text(fixed_content, encoding="utf-8")
    
    # 检查其他文件的相对导入并修复
    for py_file in SRC_DIR.glob("*.py"):
        if py_file.name in ["__init__.py"]:
            continue
            
        content = py_file.read_text(encoding="utf-8")
        original_content = content
        
        # 修复相对导入
        content = content.replace("from . import", "from src import")
        content = content.replace("from .window import", "from src.window import")
        content = content.replace("from .license import", "from src.license import")
        content = content.replace("from .config import", "from src.config import")
        content = content.replace("from .constants import", "from src.constants import")
        content = content.replace("from .widgets import", "from src.widgets import")
        content = content.replace("from .api import", "from src.api import")
        content = content.replace("from .worker import", "from src.worker import")
        content = content.replace("from .app import", "from src.app import")
        
        if content != original_content:
            dist_py_file = DIST_SRC / "src" / py_file.name
            dist_py_file.write_text(content, encoding="utf-8")
            print(f"  修复了 {py_file.name} 的导入")


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
    print("TLS-shipinhao 代码编译工具 (Cython)")
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
    compile_with_cython()
    show_tree()

    print(f"\n分发目录已生成: {DIST_SRC}")
    print("可使用 build.py --dist 从编译后的源码构建打包产物。")


if __name__ == "__main__":
    main()
