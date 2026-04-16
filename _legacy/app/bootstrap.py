# -*- coding: utf-8 -*-
"""兼容期 Python 启动入口：委托 Rust desktop-app。"""

from __future__ import annotations

import shutil
import subprocess
import sys
from pathlib import Path

REPO_ROOT = Path(__file__).resolve().parents[1]
TARGET_DIR = REPO_ROOT / "target"
DESKTOP_BINARY_NAME = "desktop-app.exe" if sys.platform.startswith("win") else "desktop-app"


def resolve_rust_desktop_command(extra_args: list[str] | None = None) -> list[str]:
    args = extra_args or []
    for candidate in (
        TARGET_DIR / "release" / DESKTOP_BINARY_NAME,
        TARGET_DIR / "debug" / DESKTOP_BINARY_NAME,
    ):
        if candidate.exists():
            return [str(candidate), *args]

    cargo = shutil.which("cargo")
    if cargo:
        return [cargo, "run", "-p", "desktop-app", "--", *args]

    raise RuntimeError(
        "未找到 Rust desktop-app 可执行文件，也未检测到 cargo；请先运行 `cargo run -p desktop-app`。"
    )


def launch_rust_desktop(extra_args: list[str] | None = None) -> int:
    command = resolve_rust_desktop_command(extra_args)
    completed = subprocess.run(command, check=False)
    return completed.returncode


def main() -> None:
    raise SystemExit(launch_rust_desktop(sys.argv[1:]))


if __name__ == "__main__":
    main()
