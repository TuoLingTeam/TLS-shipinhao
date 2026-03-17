#!/usr/bin/env python3
# -*- coding: utf-8 -*-
"""桌面应用统一构建入口。

设计目标：
1. 本地单命令构建（自动切换到项目 .venv）。
2. 明确禁止本地跨平台打包（mac 上不能直接产出 exe）。
3. 构建流程拆分清晰，便于 CI 与本地共用。
"""

from __future__ import annotations

import json
import os
import platform
import plistlib
import re
import shutil
import subprocess
import sys
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
CONSTANTS_PY = APP_ROOT / "src" / "constants.py"

# 构建时需要的最小依赖集合（避免漏装导致中断）。
BUILD_REQUIREMENTS = [
    "PySide6_Essentials",
    "PySide6_Addons",
    "shiboken6",
    "requests",
    "pyinstaller",
    "Pillow",
]
PROBE_IMPORTS = (
    "import requests, shiboken6, PyInstaller; "
    "from PIL import Image; "
    "from PySide6.QtCore import QObject; "
    "from PySide6.QtGui import QFont; "
    "from PySide6.QtWidgets import QApplication; "
    "from PySide6.QtWebEngineWidgets import QWebEngineView"
)

# PyInstaller 需要保留的隐藏导入。
HIDDEN_IMPORTS = [
    "PySide6.QtCore",
    "PySide6.QtGui",
    "PySide6.QtWidgets",
    "PySide6.QtPrintSupport",
    "PySide6.QtWebChannel",
    "PySide6.QtWebEngineCore",
    "PySide6.QtWebEngineWidgets",
]

# Cython 编译后的模块（使用混淆源时需要）
CYTHON_MODULES = [
    "src.app",
    "src.config",
    "src.constants",
    "src.core",
    "src.core.api",
    "src.core.cookie_browser",
    "src.core.http_utils",
    "src.core.license",
    "src.services",
    "src.services.order_cache",
    "src.services.order_sync",
    "src.services.review_matcher",
    "src.ui",
    "src.ui.review_worker",
    "src.ui.widgets",
    "src.ui.window",
    "src.ui.worker",
]

# Cython 模块依赖的标准库（使用混淆源时需要）
CYTHON_STDLIB_DEPS = [
    "charset_normalizer",
    "concurrent.futures",
    "datetime",
    "functools",
    "hmac",
    "hashlib",
    "json",
    "logging",
    "os",
    "re",
    "sqlite3",
    "threading",
    "requests",
    "subprocess",
    "time",
    "typing",
]

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
    "QtPdf",
    "QtPdfWidgets",
    "QtQuick3D",
    "QtQuickControls2",
    "QtQuickTest",
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
WEBENGINE_RESOURCE_RELATIVE_DIR = Path("Qt") / "lib" / "QtWebEngineCore.framework" / "Versions" / "A" / "Resources"
WEBENGINE_KEEP_LOCALES = {"en-US.pak", "zh-CN.pak", "zh-TW.pak"}
WEBENGINE_PRUNE_FILES = ("qtwebengine_devtools_resources.pak",)
PYSIDE_KEEP_BINDINGS = {
    "QtCore.abi3.so",
    "QtGui.abi3.so",
    "QtNetwork.abi3.so",
    "QtPrintSupport.abi3.so",
    "QtWebChannel.abi3.so",
    "QtWebEngineCore.abi3.so",
    "QtWebEngineWidgets.abi3.so",
    "QtWidgets.abi3.so",
}
PYSIDE_PRUNE_LIBRARIES = {"libpyside6qml.abi3.6.10.dylib"}
QT_KEEP_PLUGIN_FILES = {
    Path("platforms") / "libqcocoa.dylib",
    Path("styles") / "libqmacstyle.dylib",
}
QT_KEEP_FRAMEWORKS = {
    "QtCore",
    "QtDBus",
    "QtGui",
    "QtNetwork",
    "QtOpenGL",
    "QtPositioning",
    "QtPrintSupport",
    "QtQml",
    "QtQmlMeta",
    "QtQmlModels",
    "QtQmlWorkerScript",
    "QtQuick",
    "QtQuickWidgets",
    "QtWebChannel",
    "QtWebEngineCore",
    "QtWebEngineWidgets",
    "QtWidgets",
}


# =========================
# 版本号（与 app/src/constants.py 中 APP_VERSION 保持一致）
# =========================
def get_app_version() -> str:
    """从 constants.py 读取 APP_VERSION。"""
    if not CONSTANTS_PY.exists():
        return "0.0.0"
    text = CONSTANTS_PY.read_text(encoding="utf-8")
    match = re.search(r'APP_VERSION\s*=\s*["\']([^"\']+)["\']', text)
    return match.group(1).strip() if match else "0.0.0"


def patch_macos_bundle_version(app_bundle: Path, version: str) -> None:
    """修补 macOS .app 的 Info.plist，设置 CFBundleShortVersionString 与 CFBundleVersion。"""
    plist_path = app_bundle / "Contents" / "Info.plist"
    if not plist_path.exists():
        return
    with open(plist_path, "rb") as f:
        plist = plistlib.load(f)
    plist["CFBundleShortVersionString"] = version
    plist["CFBundleVersion"] = version
    with open(plist_path, "wb") as f:
        plistlib.dump(plist, f)
    print(f"已设置 macOS 应用版本: {version}")


def prepare_windows_version_file(version: str) -> Path:
    """生成 Windows 版本资源文件，返回文件路径。PyInstaller 会 eval 该文件并注入 VSVersionInfo 等。"""
    parts = [int(x) for x in version.split(".")[:4]]
    while len(parts) < 4:
        parts.append(0)
    vers_tuple = tuple(parts)
    BUILD_DIR.mkdir(exist_ok=True)
    path = BUILD_DIR / "version_info.txt"
    # 仅包含 VSVersionInfo(...)，类名由 PyInstaller eval 时注入
    content = f'''# UTF-8
VSVersionInfo(
  ffi=FixedFileInfo(
    filevers={vers_tuple},
    prodvers={vers_tuple},
    mask=0x3F,
    flags=0x0,
    OS=0x40004,
    fileType=0x1,
    subtype=0x0,
    date=(0, 0),
  ),
  kids=[
    StringFileInfo([
      StringTable(
        "040904B0",
        [
          StringStruct("CompanyName", "驼铃"),
          StringStruct("FileDescription", "驼铃视频小店中差评处理"),
          StringStruct("FileVersion", "{version}"),
          StringStruct("InternalName", "{MAIN_APP_NAME}"),
          StringStruct("LegalCopyright", ""),
          StringStruct("OriginalFilename", "{MAIN_APP_NAME}.exe"),
          StringStruct("ProductName", "驼铃视频小店中差评处理"),
          StringStruct("ProductVersion", "{version}"),
        ],
      ),
    ]),
    VarFileInfo([VarStruct("Translation", [0, 1200])]),
  ],
)
'''
    path.write_text(content, encoding="utf-8")
    return path


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


def prune_webengine_resources(root: Path) -> int:
    """裁剪 QtWebEngine 的非核心资源，避免误删运行时依赖。"""
    removed_count = 0
    resources_dir = root / WEBENGINE_RESOURCE_RELATIVE_DIR
    if not resources_dir.exists():
        return removed_count

    for name in WEBENGINE_PRUNE_FILES:
        if remove_path(resources_dir / name):
            removed_count += 1

    locales_dir = resources_dir / "qtwebengine_locales"
    if not locales_dir.exists():
        return removed_count

    for locale_file in locales_dir.glob("*.pak"):
        if locale_file.name in WEBENGINE_KEEP_LOCALES:
            continue
        if remove_path(locale_file):
            removed_count += 1

    return removed_count


def prune_pyside_bindings(root: Path) -> int:
    """只保留运行时会实际导入的 PySide6 扩展模块。"""
    removed_count = 0
    if not root.exists():
        return removed_count

    for target in root.glob("*.abi3.so"):
        if target.name in PYSIDE_KEEP_BINDINGS:
            continue
        if remove_path(target):
            removed_count += 1

    for name in PYSIDE_PRUNE_LIBRARIES:
        if remove_path(root / name):
            removed_count += 1

    return removed_count


def prune_qt_plugins(root: Path) -> int:
    """删除主程序不依赖的 Qt 插件，避免它们继续拖入附带 framework。"""
    removed_count = 0
    plugin_root = root / "Qt" / "plugins"
    if not plugin_root.exists():
        return removed_count

    for target in plugin_root.rglob("*.dylib"):
        relative_path = target.relative_to(plugin_root)
        if relative_path in QT_KEEP_PLUGIN_FILES:
            continue
        if remove_path(target):
            removed_count += 1

    # 清理被裁空的插件目录，避免包里残留无内容目录。
    for directory in sorted(plugin_root.rglob("*"), reverse=True):
        if directory.is_dir() and not any(directory.iterdir()):
            directory.rmdir()

    return removed_count


def prune_qt_frameworks(root: Path) -> int:
    """删除不在主程序运行闭包中的 Qt frameworks。"""
    removed_count = 0
    framework_root = root / "Qt" / "lib"
    if not framework_root.exists():
        return removed_count

    for target in framework_root.glob("*.framework"):
        name = target.name.split(".framework", 1)[0]
        if name in QT_KEEP_FRAMEWORKS:
            continue
        if remove_path(target):
            removed_count += 1

    return removed_count


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


def _generate_runtime_hook() -> Path:
    """生成 runtime hook 脚本，在 frozen 环境启动前预初始化子包层级。

    PyInstaller 的 FrozenImporter 对 Cython .so 的相对 import 支持不完整，
    需要在主脚本执行前确保所有子包的 __init__ 已被加载。
    """
    BUILD_DIR.mkdir(exist_ok=True)
    hook_path = BUILD_DIR / "_rthook_init_packages.py"
    hook_path.write_text(
        "import src\n"
        "import src.core\n"
        "import src.services\n"
        "import src.ui\n",
        encoding="utf-8",
    )
    return hook_path


def build_pyinstaller_base_cmd(python_bin: str, system: str, profile: str, use_dist: bool = False) -> list[str]:
    """组装 PyInstaller 公共参数。"""
    cmd = [python_bin, "-m", "PyInstaller", "--clean", "--noconfirm"]

    icon_file = prepare_icon(system, python_bin)
    if icon_file:
        cmd.extend(["--icon", str(icon_file)])

    # 主程序使用 PySide6，需显式补齐/裁剪导入。
    if profile == PROFILE_MAIN:
        for module in HIDDEN_IMPORTS:
            cmd.extend(["--hidden-import", module])
        
        # 使用混淆源时，需要显式声明所有 Cython 编译的模块及其标准库依赖
        if use_dist:
            for module in resolve_cython_modules():
                cmd.extend(["--hidden-import", module])
            for module in CYTHON_STDLIB_DEPS:
                cmd.extend(["--hidden-import", module])
            cmd.extend(["--collect-submodules", "src"])
            for init_file in (APP_DIST / "src").rglob("__init__.py"):
                dest = str(init_file.parent.relative_to(APP_DIST))
                cmd.extend(["--add-data", f"{init_file}{os.pathsep}{dest}"])
            # runtime hook：在 frozen 环境启动前预先初始化包层级，
            # 使 Cython .so 的相对 import 能正确解析
            hook = _generate_runtime_hook()
            cmd.extend(["--runtime-hook", str(hook)])
        
        for module in EXCLUDED_MODULES:
            cmd.extend(["--exclude-module", module])

        # macOS/Linux 下可 strip 降体积；Windows 不使用该参数。
        if system != SYSTEM_WINDOWS:
            cmd.append("--strip")

    return cmd


def resolve_cython_modules() -> list[str]:
    """解析混淆产物中的 Cython 模块列表（递归扫描子包）。"""
    dist_src_dir = APP_DIST / "src"
    if not dist_src_dir.exists():
        return list(CYTHON_MODULES)

    modules: list[str] = []
    seen: set[str] = set()
    for artifact in sorted(dist_src_dir.rglob("*")):
        if artifact.suffix.lower() not in {".so", ".pyd"}:
            continue
        module_stem = artifact.name.split(".", 1)[0]
        rel_parent = artifact.parent.relative_to(APP_DIST)
        hidden_import = str(rel_parent / module_stem).replace(os.sep, ".")
        if hidden_import in seen:
            continue
        seen.add(hidden_import)
        modules.append(hidden_import)

    return modules or list(CYTHON_MODULES)


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

        removed_count += prune_webengine_resources(root)
        removed_count += prune_pyside_bindings(root)
        removed_count += prune_qt_plugins(root)
        removed_count += prune_qt_frameworks(root)

    for parent in (app_bundle / "Contents" / "Resources", app_bundle / "Contents" / "Frameworks"):
        for target in parent.glob("*.dist-info"):
            if remove_path(target):
                removed_count += 1

    if removed_count:
        print(f"已裁剪 macOS bundle 中的 {removed_count} 个非运行时资源。")


def build_macos(python_bin: str, app_name: str, entry_file: Path, profile: str, use_dist: bool = False) -> Path:
    """构建 macOS .app。"""
    print(f"开始打包 macOS 应用: {app_name}")
    cmd = build_pyinstaller_base_cmd(python_bin, SYSTEM_MACOS, profile, use_dist)
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
        patch_macos_bundle_version(app_bundle, get_app_version())

    # PyInstaller --windowed 会同时产出展开目录，只保留 .app bundle
    loose_dir = DIST_DIR / app_name
    if loose_dir.is_dir() and not str(loose_dir).endswith(".app"):
        shutil.rmtree(loose_dir, ignore_errors=True)

    cleanup_temp_files(app_name)

    print(f"打包完成。\n应用位置: {app_bundle}")
    if profile == PROFILE_MAIN:
        print("使用前：可点击「自动获取 Cookie」完成登录并生成 cookie.txt，或手动选择配置目录。")
    return app_bundle


def build_windows(python_bin: str, app_name: str, entry_file: Path, profile: str, use_dist: bool = False) -> Path:
    """构建 Windows .exe。"""
    print(f"开始打包 Windows 应用: {app_name}")
    cmd = build_pyinstaller_base_cmd(python_bin, SYSTEM_WINDOWS, profile, use_dist)
    version = get_app_version()
    version_file = prepare_windows_version_file(version)
    cmd.extend(["--onefile", "--windowed", "--name", app_name, "--version-file", str(version_file), str(entry_file)])
    run(cmd)

    exe_file = DIST_DIR / f"{app_name}.exe"
    if not exe_file.exists():
        raise FileNotFoundError(f"未找到 Windows 构建产物: {exe_file}")

    cleanup_temp_files(app_name)
    if profile == PROFILE_MAIN:
        copy_runtime_files(DIST_DIR)

    print(f"打包完成。\n可执行文件: {exe_file}")
    if profile == PROFILE_MAIN:
        print("使用前：可点击「自动获取 Cookie」完成登录并生成 cookie.txt，或手动选择配置目录。")
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
        return build_macos(python_bin, app_name, entry_file, profile, use_dist)
    if system == SYSTEM_WINDOWS:
        return build_windows(python_bin, app_name, entry_file, profile, use_dist)
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
