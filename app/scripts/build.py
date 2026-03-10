#!/usr/bin/env python3
# -*- coding: utf-8 -*-
"""桌面应用统一构建入口。

设计目标：
1. 本地单命令构建（自动切换到项目 .venv）。
2. 明确禁止本地跨平台打包（mac 上不能直接产出 exe）。
3. 构建流程拆分清晰，便于 CI 与本地共用。
"""

from __future__ import annotations

import os
import platform
import shutil
import subprocess
import sys
import json
from pathlib import Path

# Windows 终端默认编码对中文输出不友好，统一切成 UTF-8。
if platform.system() == "Windows":
    import io

    sys.stdout = io.TextIOWrapper(sys.stdout.buffer, encoding="utf-8")
    sys.stderr = io.TextIOWrapper(sys.stderr.buffer, encoding="utf-8")


# =========================
# 基础常量
# =========================
MAIN_APP_NAME = "TLS-shipinhao"
BUNDLE_ID = "com.tuoling.tls-shipinhao"
SYSTEM_MACOS = "Darwin"
SYSTEM_WINDOWS = "Windows"

APP_ROOT = Path(__file__).resolve().parent.parent      # app/
REPO_ROOT = APP_ROOT.parent                             # 仓库根目录
APP_DIST = REPO_ROOT / "app-dist"                       # 混淆分发目录
DIST_DIR = REPO_ROOT / "dist"
BUILD_DIR = REPO_ROOT / "build"
MAIN_FILE = APP_ROOT / "main.py"
COOKIE_FILE = REPO_ROOT / "cookie.txt"
SOURCE_ICON_FILE = APP_ROOT / "src" / "favicon.png"
MACOS_ICON_FILE = BUILD_DIR / "app_icon.icns"
WINDOWS_ICON_FILE = BUILD_DIR / "app_icon.ico"
PYINSTALLER_CACHE_DIR = REPO_ROOT / ".pyinstaller"

# 构建时需要的最小依赖集合（避免漏装导致中断）。
BUILD_REQUIREMENTS = ["PySide6_Essentials", "shiboken6", "requests", "pyinstaller", "Pillow"]
PROBE_IMPORTS = (
    "import requests, shiboken6, PyInstaller; "
    "from PIL import Image; "
    "from PySide6.QtCore import QObject; "
    "from PySide6.QtGui import QFont; "
    "from PySide6.QtWidgets import QApplication"
)

# PyInstaller 需要保留的隐藏导入。
HIDDEN_IMPORTS = ["PySide6.QtCore", "PySide6.QtGui", "PySide6.QtWidgets", "PIL.Image"]

# 业务不使用的 Qt 模块，显式排除可显著减小体积。
QT_OPTIONAL_MODULES = (
    "Qt3DAnimation",
    "Qt3DCore",
    "Qt3DExtras",
    "Qt3DInput",
    "Qt3DLogic",
    "QtBluetooth",
    "QtCharts",
    "QtConcurrent",
    "QtDataVisualization",
    "QtDBus",
    "QtDesigner",
    "QtGraphs",
    "QtGraphsWidgets",
    "QtHelp",
    "QtHttpServer",
    "QtLocation",
    "QtMultimedia",
    "QtMultimediaWidgets",
    "QtNetworkAuth",
    "QtNfc",
    "QtOpenGL",
    "QtOpenGLWidgets",
    "QtPdf",
    "QtPdfWidgets",
    "QtPositioning",
    "QtPrintSupport",
    "QtQml",
    "QtQuick",
    "QtQuick3D",
    "QtQuickControls2",
    "QtQuickTest",
    "QtQuickWidgets",
    "QtRemoteObjects",
    "QtScxml",
    "QtSensors",
    "QtSerialBus",
    "QtSerialPort",
    "QtSpatialAudio",
    "QtSql",
    "QtStateMachine",
    "QtSvg",
    "QtSvgWidgets",
    "QtTest",
    "QtTextToSpeech",
    "QtUiTools",
    "QtWebChannel",
    "QtWebEngineCore",
    "QtWebEngineQuick",
    "QtWebEngineWidgets",
    "QtWebSockets",
    "QtWebView",
    "QtXml",
)
EXCLUDED_MODULES = ["bs4", "beautifulsoup4", "pymongo", "openpyxl", *[f"PySide6.{m}" for m in QT_OPTIONAL_MODULES]]

PROFILE_MAIN = "main"

# macOS 包体裁剪配置（删除可选资源，不影响运行）。
QT_PRUNE_DIRS = (
    Path("Qt") / "qml",
    Path("Qt") / "translations",
    Path("Qt") / "metatypes",
    Path("Qt") / "libexec",
    Path("include"),
    Path("typesystems"),
    Path("scripts"),
    Path("support"),
    Path("glue"),
)
QT_PRUNE_PLUGIN_DIRS = (
    "designer",
    "gamepads",
    "geometryloaders",
    "geoservices",
    "networkinformation",
    "position",
    "qmltooling",
    "renderers",
    "renderplugins",
    "sceneparsers",
    "sensorgestures",
    "sqldrivers",
    "texttospeech",
    "tls",
    "wayland-decoration-client",
    "wayland-graphics-integration-client",
    "wayland-shell-integration",
    "webview",
)
QT_PRUNE_FILES = (
    "Assistant.app",
    "Designer.app",
    "Linguist.app",
    "Assistant__dot__app",
    "Designer__dot__app",
    "Linguist__dot__app",
    "balsam",
    "balsamui",
    "lrelease",
    "lupdate",
    "qmlformat",
    "qmllint",
    "qmlls",
    "qsb",
    "svgtoqml",
)
RESOURCE_PRUNE_GLOBS = ("*.dist-info", "*.pyi")


# =========================
# 通用工具函数
# =========================
def run(cmd: list[str], *, cwd: Path | None = None) -> None:
    """执行命令，失败即抛异常。"""
    env = os.environ.copy()
    PYINSTALLER_CACHE_DIR.mkdir(parents=True, exist_ok=True)
    env["PYINSTALLER_CONFIG_DIR"] = str(PYINSTALLER_CACHE_DIR)
    subprocess.run(cmd, cwd=str(cwd or REPO_ROOT), check=True, env=env)


def project_python() -> str:
    """优先返回项目 .venv 的 Python，避免污染系统环境。"""
    if platform.system() == SYSTEM_WINDOWS:
        candidate = REPO_ROOT / ".venv" / "Scripts" / "python.exe"
    else:
        candidate = REPO_ROOT / ".venv" / "bin" / "python"
    return str(candidate) if candidate.exists() else sys.executable


def ensure_running_with_project_python() -> None:
    """当前解释器不是项目 .venv 时，自动重启到项目解释器。"""
    target_python = Path(project_python()).resolve()
    current_python = Path(sys.executable).resolve()
    if target_python == current_python:
        return
    print(f"切换到项目解释器: {target_python}")
    os.execv(str(target_python), [str(target_python), *sys.argv])


def ensure_build_dependencies(python_bin: str) -> None:
    """检查并自动安装打包依赖。"""
    try:
        run([python_bin, "-c", PROBE_IMPORTS])
    except subprocess.CalledProcessError:
        print("安装构建依赖...")
        run([python_bin, "-m", "pip", "install", "-q", *BUILD_REQUIREMENTS])


def clean_build_artifacts() -> None:
    """清理历史产物，保证每次构建可复现。"""
    print("清理旧构建产物...")
    shutil.rmtree(BUILD_DIR, ignore_errors=True)

    spec_file = REPO_ROOT / f"{MAIN_APP_NAME}.spec"
    if spec_file.exists():
        spec_file.unlink()

    shutil.rmtree(DIST_DIR / MAIN_APP_NAME, ignore_errors=True)
    shutil.rmtree(DIST_DIR / f"{MAIN_APP_NAME}.app", ignore_errors=True)

    for legacy_file in (DIST_DIR / f"{MAIN_APP_NAME}.exe", DIST_DIR / MAIN_APP_NAME):
        if legacy_file.exists() and legacy_file.is_file():
            legacy_file.unlink()

    DIST_DIR.mkdir(exist_ok=True)


def cleanup_temp_files(app_name: str) -> None:
    """构建完成后清理中间文件。"""
    if BUILD_DIR.exists():
        shutil.rmtree(BUILD_DIR, ignore_errors=True)
    spec_file = REPO_ROOT / f"{app_name}.spec"
    if spec_file.exists():
        spec_file.unlink()


def remove_path(path: Path) -> bool:
    """删除文件/目录（兼容符号链接），返回是否实际删除。"""
    if not path.exists() and not path.is_symlink():
        return False
    if path.is_dir() and not path.is_symlink():
        shutil.rmtree(path, ignore_errors=True)
    else:
        path.unlink(missing_ok=True)
    return True


def copy_runtime_files(destination: Path) -> None:
    """复制运行时配置文件（cookie）。"""
    destination.mkdir(parents=True, exist_ok=True)
    if COOKIE_FILE.exists():
        shutil.copy2(COOKIE_FILE, destination / COOKIE_FILE.name)


# =========================
# 图标与 PyInstaller 参数
# =========================
def prepare_icon(system: str, python_bin: str) -> Path | None:
    """根据目标系统生成图标文件。"""
    if not SOURCE_ICON_FILE.exists():
        print(f"警告: 图标源文件不存在: {SOURCE_ICON_FILE}")
        return None

    if system == SYSTEM_MACOS:
        fmt = "ICNS"
        output_path = MACOS_ICON_FILE
        sizes = [(16, 16), (32, 32), (64, 64), (128, 128), (256, 256), (512, 512), (1024, 1024)]
    elif system == SYSTEM_WINDOWS:
        fmt = "ICO"
        output_path = WINDOWS_ICON_FILE
        sizes = [(16, 16), (24, 24), (32, 32), (48, 48), (64, 64), (128, 128), (256, 256)]
    else:
        return None

    BUILD_DIR.mkdir(exist_ok=True)
    try:
        run(
            [
                python_bin,
                "-c",
                (
                    "import json,sys;"
                    "from PIL import Image;"
                    "src,dst,fmt,sizes_json=sys.argv[1:5];"
                    "sizes=[tuple(x) for x in json.loads(sizes_json)];"
                    "img=Image.open(src).convert('RGBA');"
                    "img.save(dst,format=fmt,sizes=sizes)"
                ),
                str(SOURCE_ICON_FILE),
                str(output_path),
                fmt,
                json.dumps(sizes),
            ]
        )
        print(f"使用图标: {output_path}")
        return output_path
    except Exception as exc:  # noqa: BLE001
        print(f"警告: 生成图标失败: {exc}")
        return None


def build_pyinstaller_base_cmd(python_bin: str, system: str, profile: str) -> list[str]:
    """组装 PyInstaller 公共参数。"""
    cmd = [python_bin, "-m", "PyInstaller", "--clean", "--noconfirm"]

    icon_file = prepare_icon(system, python_bin)
    if icon_file:
        cmd.extend(["--icon", str(icon_file)])

    # 主程序使用 PySide6，需显式补齐/裁剪导入。
    if profile == PROFILE_MAIN:
        for module in HIDDEN_IMPORTS:
            cmd.extend(["--hidden-import", module])
        for module in EXCLUDED_MODULES:
            cmd.extend(["--exclude-module", module])

        # macOS/Linux 下可 strip 降体积；Windows 不使用该参数。
        if system != SYSTEM_WINDOWS:
            cmd.append("--strip")

    return cmd


# =========================
# 平台构建逻辑
# =========================
def prune_macos_bundle(app_bundle: Path) -> None:
    """裁剪 macOS bundle 中的可选资源，减小体积。"""
    removed_count = 0
    pyside_roots = (
        app_bundle / "Contents" / "Frameworks" / "PySide6",
        app_bundle / "Contents" / "Resources" / "PySide6",
    )

    for root in pyside_roots:
        if not root.exists():
            continue

        for rel_path in QT_PRUNE_DIRS:
            if remove_path(root / rel_path):
                removed_count += 1

        plugin_root = root / "Qt" / "plugins"
        for name in QT_PRUNE_PLUGIN_DIRS:
            if remove_path(plugin_root / name):
                removed_count += 1

        for name in QT_PRUNE_FILES:
            if remove_path(root / name):
                removed_count += 1

        for pattern in RESOURCE_PRUNE_GLOBS:
            for target in root.glob(pattern):
                if remove_path(target):
                    removed_count += 1

    for parent in (app_bundle / "Contents" / "Resources", app_bundle / "Contents" / "Frameworks"):
        for target in parent.glob("*.dist-info"):
            if remove_path(target):
                removed_count += 1

    if removed_count:
        print(f"已裁剪 macOS bundle 中的 {removed_count} 个非运行时资源。")


def build_macos(python_bin: str, app_name: str, entry_file: Path, profile: str) -> Path:
    """构建 macOS .app。"""
    print(f"开始打包 macOS 应用: {app_name}")
    cmd = build_pyinstaller_base_cmd(python_bin, SYSTEM_MACOS, profile)
    cmd.extend(
        [
            "--windowed",
            "--osx-bundle-identifier",
            BUNDLE_ID,
            "--name",
            app_name,
            str(entry_file),
        ]
    )
    run(cmd)

    app_bundle = DIST_DIR / f"{app_name}.app"
    if not app_bundle.exists():
        raise FileNotFoundError(f"未找到 macOS 构建产物: {app_bundle}")

    if profile == PROFILE_MAIN:
        prune_macos_bundle(app_bundle)
    cleanup_temp_files(app_name)

    print(f"打包完成。\n应用位置: {app_bundle}")
    if profile == PROFILE_MAIN:
        print("使用前：启动应用后点击「选择配置目录」设置 cookie.txt 的路径。")
    return app_bundle


def build_windows(python_bin: str, app_name: str, entry_file: Path, profile: str) -> Path:
    """构建 Windows .exe。"""
    print(f"开始打包 Windows 应用: {app_name}")
    cmd = build_pyinstaller_base_cmd(python_bin, SYSTEM_WINDOWS, profile)
    cmd.extend(["--onefile", "--windowed", "--name", app_name, str(entry_file)])
    run(cmd)

    exe_file = DIST_DIR / f"{app_name}.exe"
    if not exe_file.exists():
        raise FileNotFoundError(f"未找到 Windows 构建产物: {exe_file}")

    cleanup_temp_files(app_name)
    if profile == PROFILE_MAIN:
        copy_runtime_files(DIST_DIR)

    print(f"打包完成。\n可执行文件: {exe_file}")
    if profile == PROFILE_MAIN:
        print("使用前：启动应用后点击「选择配置目录」设置 cookie.txt 的路径。")
    return exe_file


# =========================
# 参数与流程编排
# =========================
def parse_args(argv: list[str]) -> tuple[str | None, str, bool]:
    """解析命令行参数，返回 (目标平台, 构建档位, 是否使用混淆源)。"""
    target = None
    profile = PROFILE_MAIN
    use_dist = False

    for raw_arg in argv[1:]:
        arg = raw_arg.lower()
        if arg in {"mac", "macos", "darwin"}:
            target = SYSTEM_MACOS
            continue
        if arg in {"win", "windows"}:
            target = SYSTEM_WINDOWS
            continue
        if arg in {"main", "app"}:
            profile = PROFILE_MAIN
            continue
        if arg in {"--dist", "dist"}:
            use_dist = True
            continue
        raise SystemExit(f"不支持的参数: {raw_arg}")

    return target, profile, use_dist


def resolve_profile(profile: str, use_dist: bool = False) -> tuple[str, Path]:
    """根据构建档位返回 (应用名称, 入口文件)。"""
    if profile == PROFILE_MAIN:
        if use_dist:
            entry = APP_DIST / "main.py"
            if not entry.exists():
                raise SystemExit(
                    f"混淆分发目录不存在: {APP_DIST}\n"
                    "请先运行: python app/scripts/obfuscate.py"
                )
            return MAIN_APP_NAME, entry
        return MAIN_APP_NAME, MAIN_FILE
    raise SystemExit(f"不支持的构建档位: {profile}")


def build(target: str | None = None, profile: str = PROFILE_MAIN, use_dist: bool = False) -> Path:
    """统一构建入口。"""
    python_bin = project_python()
    current_system = platform.system()
    system = target or current_system
    app_name, entry_file = resolve_profile(profile, use_dist)
    if use_dist:
        print(f"使用混淆分发目录: {APP_DIST}")

    # 本地禁止跨平台打包，避免“构建了半天才发现产物不存在”。
    if target and target != current_system:
        raise SystemExit(
            "不支持跨平台本地打包："
            f"当前系统是 {current_system}，目标是 {target}。\n"
            "请在对应系统上构建，或使用 GitHub Actions 的对应 Runner：\n"
            "- Windows 包：windows-latest\n"
            "- macOS 包：macos-latest"
        )

    if not entry_file.exists():
        raise FileNotFoundError(f"入口文件不存在: {entry_file}")

    ensure_build_dependencies(python_bin)
    clean_build_artifacts()

    if system == SYSTEM_MACOS:
        return build_macos(python_bin, app_name, entry_file, profile)
    if system == SYSTEM_WINDOWS:
        return build_windows(python_bin, app_name, entry_file, profile)
    raise SystemExit(f"不支持的系统: {system}")


def main() -> None:
    """脚本主入口。"""
    ensure_running_with_project_python()
    target, profile, use_dist = parse_args(sys.argv)

    try:
        artifact = build(target, profile, use_dist)
    except subprocess.CalledProcessError as exc:
        raise SystemExit(exc.returncode) from exc
    except Exception as exc:  # noqa: BLE001
        print(f"Build failed: {exc}")
        raise SystemExit(1) from exc

    print(f"构建成功: {artifact}")


if __name__ == "__main__":
    main()
