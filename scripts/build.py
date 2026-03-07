#!/usr/bin/env python3
# -*- coding: utf-8 -*-
"""统一构建入口。"""

from __future__ import annotations

import platform
import shutil
import subprocess
import sys
from pathlib import Path

if platform.system() == "Windows":
    import io

    sys.stdout = io.TextIOWrapper(sys.stdout.buffer, encoding="utf-8")
    sys.stderr = io.TextIOWrapper(sys.stderr.buffer, encoding="utf-8")


APP_NAME = "TLS-shipinhao"
BUNDLE_ID = "com.tuoling.tls-shipinhao"
PROJECT_ROOT = Path(__file__).resolve().parent.parent
DIST_DIR = PROJECT_ROOT / "dist"
BUILD_DIR = PROJECT_ROOT / "build"
SPEC_FILE = PROJECT_ROOT / f"{APP_NAME}.spec"
MAIN_FILE = PROJECT_ROOT / "main.py"
COOKIE_FILE = PROJECT_ROOT / "cookie.txt"
MAGIC_FILE = PROJECT_ROOT / "biz_magic.txt"
BUILD_REQUIREMENTS = ["PySide6", "requests", "pyinstaller"]
HIDDEN_IMPORTS = ["PySide6.QtCore", "PySide6.QtGui", "PySide6.QtWidgets"]
EXCLUDED_MODULES = [
    "bs4",
    "beautifulsoup4",
    "pymongo",
    "openpyxl",
    "PySide6.Qt3DAnimation",
    "PySide6.Qt3DCore",
    "PySide6.Qt3DExtras",
    "PySide6.Qt3DInput",
    "PySide6.Qt3DLogic",
    "PySide6.QtBluetooth",
    "PySide6.QtCharts",
    "PySide6.QtDataVisualization",
    "PySide6.QtDesigner",
    "PySide6.QtGraphs",
    "PySide6.QtGraphsWidgets",
    "PySide6.QtHelp",
    "PySide6.QtHttpServer",
    "PySide6.QtLocation",
    "PySide6.QtMultimedia",
    "PySide6.QtMultimediaWidgets",
    "PySide6.QtNetworkAuth",
    "PySide6.QtNfc",
    "PySide6.QtOpenGL",
    "PySide6.QtPdf",
    "PySide6.QtPdfWidgets",
    "PySide6.QtPositioning",
    "PySide6.QtQml",
    "PySide6.QtQuick",
    "PySide6.QtQuick3D",
    "PySide6.QtQuickControls2",
    "PySide6.QtQuickWidgets",
    "PySide6.QtRemoteObjects",
    "PySide6.QtScxml",
    "PySide6.QtSensors",
    "PySide6.QtSerialBus",
    "PySide6.QtSerialPort",
    "PySide6.QtSql",
    "PySide6.QtStateMachine",
    "PySide6.QtSvg",
    "PySide6.QtSvgWidgets",
    "PySide6.QtTest",
    "PySide6.QtTextToSpeech",
    "PySide6.QtUiTools",
    "PySide6.QtWebChannel",
    "PySide6.QtWebEngineCore",
    "PySide6.QtWebEngineQuick",
    "PySide6.QtWebEngineWidgets",
    "PySide6.QtWebSockets",
    "PySide6.QtWebView",
    "PySide6.QtXml",
]


def project_python() -> str:
    if platform.system() == "Windows":
        candidate = PROJECT_ROOT / ".venv" / "Scripts" / "python.exe"
    else:
        candidate = PROJECT_ROOT / ".venv" / "bin" / "python"
    return str(candidate) if candidate.exists() else sys.executable


def run(cmd: list[str], *, cwd: Path | None = None) -> None:
    subprocess.run(cmd, cwd=str(cwd or PROJECT_ROOT), check=True)


def ensure_build_dependencies(python_bin: str) -> None:
    probe = [python_bin, "-c", "import PySide6, requests, PyInstaller"]
    try:
        run(probe)
    except subprocess.CalledProcessError:
        print("安装构建依赖...")
        run([python_bin, "-m", "pip", "install", "-q", *BUILD_REQUIREMENTS])


def clean_build_artifacts() -> None:
    print("清理旧构建产物...")
    shutil.rmtree(BUILD_DIR, ignore_errors=True)
    if SPEC_FILE.exists():
        SPEC_FILE.unlink()

    shutil.rmtree(DIST_DIR / APP_NAME, ignore_errors=True)
    shutil.rmtree(DIST_DIR / f"{APP_NAME}.app", ignore_errors=True)

    for legacy_file in (DIST_DIR / f"{APP_NAME}.exe", DIST_DIR / APP_NAME):
        if legacy_file.exists() and legacy_file.is_file():
            legacy_file.unlink()

    DIST_DIR.mkdir(exist_ok=True)


def pyinstaller_command(python_bin: str) -> list[str]:
    cmd = [python_bin, "-m", "PyInstaller", "--clean", "--noconfirm"]
    for mod in HIDDEN_IMPORTS:
        cmd.extend(["--hidden-import", mod])
    for mod in EXCLUDED_MODULES:
        cmd.extend(["--exclude-module", mod])
    if platform.system() != "Windows":
        cmd.append("--strip")
    return cmd


def copy_runtime_files(destination: Path) -> None:
    destination.mkdir(parents=True, exist_ok=True)
    for source in (COOKIE_FILE, MAGIC_FILE):
        if source.exists():
            shutil.copy2(source, destination / source.name)


def build_macos(python_bin: str) -> Path:
    print("开始打包 macOS 应用...")
    cmd = pyinstaller_command(python_bin)
    cmd.extend(
        [
            "--windowed",
            "--osx-bundle-identifier",
            BUNDLE_ID,
            "--name",
            APP_NAME,
            str(MAIN_FILE),
        ]
    )
    run(cmd)

    app_bundle = DIST_DIR / f"{APP_NAME}.app"
    if not app_bundle.exists():
        raise FileNotFoundError(f"未找到 macOS 构建产物: {app_bundle}")

    if BUILD_DIR.exists():
        shutil.rmtree(BUILD_DIR, ignore_errors=True)
    if SPEC_FILE.exists():
        SPEC_FILE.unlink()

    print(f"打包完成。\n应用位置: {app_bundle}")
    print("使用前：将 cookie.txt 和 biz_magic.txt 放在与 .app 同目录（dist/）即可。")
    return app_bundle


def build_windows(python_bin: str) -> Path:
    print("Building Windows package...")
    cmd = pyinstaller_command(python_bin)
    cmd.extend(
        [
            "--onefile",
            "--windowed",
            "--name",
            APP_NAME,
            str(MAIN_FILE),
        ]
    )
    run(cmd)

    exe_file = DIST_DIR / f"{APP_NAME}.exe"
    if not exe_file.exists():
        raise FileNotFoundError(f"未找到 Windows 构建产物: {exe_file}")

    if BUILD_DIR.exists():
        shutil.rmtree(BUILD_DIR, ignore_errors=True)
    if SPEC_FILE.exists():
        SPEC_FILE.unlink()

    copy_runtime_files(DIST_DIR)
    print(f"Build complete.\nExecutable: {exe_file}")
    print("cookie.txt and biz_magic.txt will be copied automatically when they exist in the project root.")
    return exe_file


def build(target: str | None = None) -> Path:
    python_bin = project_python()
    system = target or platform.system()

    ensure_build_dependencies(python_bin)
    clean_build_artifacts()

    if system == "Darwin":
        return build_macos(python_bin)
    if system == "Windows":
        return build_windows(python_bin)
    raise SystemExit(f"不支持的系统: {system}")


def main() -> None:
    target = None
    if len(sys.argv) > 1:
        arg = sys.argv[1].lower()
        if arg in {"mac", "macos", "darwin"}:
            target = "Darwin"
        elif arg in {"win", "windows"}:
            target = "Windows"
        else:
            raise SystemExit(f"不支持的构建目标: {sys.argv[1]}")

    try:
        artifact = build(target)
    except subprocess.CalledProcessError as exc:
        raise SystemExit(exc.returncode) from exc
    except Exception as exc:  # noqa: BLE001
        print(f"Build failed: {exc}")
        raise SystemExit(1) from exc

    print(f"构建成功: {artifact}")


if __name__ == "__main__":
    main()
