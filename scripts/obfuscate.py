#!/usr/bin/env python3
# -*- coding: utf-8 -*-
"""将 app/ 源码编译后输出到 app-dist/ 分发目录。

使用 Cython 将 Python 源码编译为 .so/.pyd 二进制扩展模块，
编译后无法反编译出源码，保护强度高。
混淆后的代码可直接用于 PyInstaller 打包。

用法：
    python scripts/obfuscate.py          # 编译
    python scripts/obfuscate.py --clean   # 仅清理输出目录
"""

from __future__ import annotations

import os
import platform
import shutil
import subprocess
import sys
import textwrap
from pathlib import Path
from typing import Iterable

# =========================
# 路径常量
# =========================
REPO_ROOT = Path(__file__).resolve().parent.parent       # 仓库根目录
APP_ROOT = REPO_ROOT / "app"                             # app/
DIST_SRC = REPO_ROOT / "app-dist"                        # 混淆输出目录

SRC_DIR = APP_ROOT
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
    """Clean old distribution directory."""
    if DIST_SRC.exists():
        shutil.rmtree(DIST_SRC)
    DIST_SRC.mkdir(parents=True)
    print(f"Cleaned output directory: {DIST_SRC}")


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

    print(f"Copied {count} resource files")


# =========================
# Cython 编译
# =========================
def ensure_cython() -> None:
    """Ensure Cython and setuptools are installed."""
    python_bin = _project_python()
    try:
        subprocess.run(
            [python_bin, "-c", "import Cython; import setuptools"],
            capture_output=True, check=True,
        )
    except subprocess.CalledProcessError:
        print("Installing Cython + setuptools...")
        subprocess.run(
            [python_bin, "-m", "pip", "install", "Cython", "setuptools"],
            check=True,
        )


def collect_python_sources() -> list[Path]:
    """按稳定顺序递归收集 app/ 下需要编译的 Python 文件。"""
    return sorted(
        path
        for path in SRC_DIR.rglob("*.py")
        if path.name != "__init__.py"
    )


def module_name_from_path(path: Path) -> str:
    """从源文件路径提取点分模块名（相对于 APP_ROOT）。

    例如 services/delivery_api.py -> services.delivery_api，settings.py -> settings
    """
    rel = path.relative_to(SRC_DIR)
    parts = list(rel.parent.parts) + [rel.stem]
    return ".".join(parts)


def verify_compiled_modules(module_names: Iterable[str]) -> None:
    """校验每个源模块都已生成对应的二进制扩展。

    module_names 使用点分格式，如 "services.delivery_api"、"config"。
    """
    module_names = list(module_names)
    compiled_dir = DIST_SRC
    missing_modules: list[str] = []

    for module_name in module_names:
        parts = module_name.split(".")
        stem = parts[-1]
        parent = compiled_dir.joinpath(*parts[:-1]) if len(parts) > 1 else compiled_dir
        candidates = list(parent.glob(f"{stem}.*"))
        if not any(c.suffix.lower() in {".so", ".pyd"} for c in candidates):
            missing_modules.append(module_name)

    if missing_modules:
        raise RuntimeError(
            "以下模块未生成二进制扩展: " + ", ".join(missing_modules)
        )

    print(f"Verified compiled modules: {len(module_names)}")


def compile_with_cython() -> None:
    """使用 Cython 将 app/ 下所有 .py 编译为 .so/.pyd（支持子包）。"""
    ensure_cython()
    python_bin = _project_python()

    source_files = collect_python_sources()
    if not source_files:
        print("No .py files found for compilation")
        return
    py_files = [str(path.relative_to(APP_ROOT)) for path in source_files]
    module_names = [module_name_from_path(path) for path in source_files]

    print(f"Files to compile: {len(py_files)}")
    for module_name, source_file in zip(module_names, py_files):
        print(f"  {source_file} -> {module_name}")

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

        print("\nStarting Cython compilation...")
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
        print("Cython compilation completed")
    finally:
        setup_file.unlink(missing_ok=True)
        if build_temp.exists():
            shutil.rmtree(build_temp)
        for c_file in SRC_DIR.rglob("*.c"):
            c_file.unlink()

    # 复制所有 __init__.py 以保持包/子包结构
    for init_file in SRC_DIR.rglob("__init__.py"):
        rel = init_file.relative_to(SRC_DIR)
        init_dst = DIST_SRC / rel
        init_dst.parent.mkdir(parents=True, exist_ok=True)
        shutil.copy2(init_file, init_dst)

    main_content = MAIN_FILE.read_text(encoding="utf-8")
    (DIST_SRC / "main.py").write_text(main_content, encoding="utf-8")

    fix_cython_imports()
    verify_compiled_modules(module_names)


# =========================
# 修复导入问题
# =========================
def fix_cython_imports() -> None:
    """删除编译后残留的 .py 源文件，只保留 .so/.pyd 和 __init__.py。"""
    print("Fixing Cython import issues...")
    for py_file in DIST_SRC.rglob("*.py"):
        if py_file.name != "__init__.py":
            py_file.unlink()
            print(f"  Removed {py_file.relative_to(DIST_SRC)} (using .so version)")


# =========================
# 验证
# =========================
def show_tree() -> None:
    """Show distribution directory structure."""
    print("\nDistribution directory structure:")
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
    print("TLS-shipinhao Code Compilation Tool (Cython)")
    print("=" * 50)
    print(f"Source directory: {APP_ROOT}")
    print(f"Output directory: {DIST_SRC}")
    print()

    if "--clean" in sys.argv:
        clean_dist()
        print("Cleanup completed.")
        return

    clean_dist()
    copy_resources()
    compile_with_cython()
    show_tree()

    print(f"\nDistribution directory generated: {DIST_SRC}")
    print("You can use build.py --dist to build from the compiled source code.")


if __name__ == "__main__":
    main()
