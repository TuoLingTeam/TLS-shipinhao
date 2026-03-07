#!/bin/bash
# macOS 打包脚本：生成 .app 应用
set -e

SCRIPT_DIR="$(cd "$(dirname "$0")" && pwd)"
APP_NAME="TLS-shipinhao"
BUNDLE_ID="com.tuoling.tls-shipinhao"
DIST_DIR="$SCRIPT_DIR/dist"
BUILD_DIR="$SCRIPT_DIR/build"
SPEC_FILE="$SCRIPT_DIR/${APP_NAME}.spec"
APP_BUNDLE="$DIST_DIR/${APP_NAME}.app"
BIN_FILE="$DIST_DIR/${APP_NAME}"
VENV_DIR="$SCRIPT_DIR/.venv"
if [ -x "$VENV_DIR/bin/python" ]; then
  PYTHON_BIN="$VENV_DIR/bin/python"
else
  PYTHON_BIN="$(command -v python3)"
fi
CLEAN_TARGETS=(
  "$BUILD_DIR"
  "$SPEC_FILE"
  "$APP_BUNDLE"
  "$BIN_FILE"
)

clean_build_artifacts() {
  rm -rf "${CLEAN_TARGETS[@]}"
}

cd "$SCRIPT_DIR"

echo "清理旧构建产物..."
clean_build_artifacts
mkdir -p "$DIST_DIR"

if ! "$PYTHON_BIN" -m PyInstaller --version >/dev/null 2>&1; then
  echo "安装 PyInstaller（若未安装）..."
  "$PYTHON_BIN" -m pip install pyinstaller -q
fi

echo "开始打包 macOS 应用..."
"$PYTHON_BIN" -m PyInstaller \
  --clean \
  --noconfirm \
  --collect-all charset_normalizer \
  --collect-all shiboken6 \
  --collect-all PySide6 \
  --osx-bundle-identifier "$BUNDLE_ID" \
  --windowed \
  --name "$APP_NAME" \
  main.py

echo "清理临时产物..."
rm -rf "$BUILD_DIR" "$SPEC_FILE" "$BIN_FILE"

echo "打包完成。"
echo "应用位置: $APP_BUNDLE"
echo "使用前：将 cookie.txt 和 biz_magic.txt 放在与 .app 同目录（dist/）即可。"
