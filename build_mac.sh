#!/bin/bash
# macOS 打包脚本：生成 .app 应用
set -e

SCRIPT_DIR="$(cd "$(dirname "$0")" && pwd)"
APP_NAME="订单物流信息更新"
BUNDLE_ID="com.tuoling.order-logistics-updater"
DIST_DIR="$SCRIPT_DIR/dist"
BUILD_DIR="$SCRIPT_DIR/build"
SPEC_FILE="$SCRIPT_DIR/${APP_NAME}.spec"
APP_BUNDLE="$DIST_DIR/${APP_NAME}.app"
BIN_FILE="$DIST_DIR/${APP_NAME}"
MAC_BUILD_VENV="$SCRIPT_DIR/.venv-mac-build"
VENV_DIR="$SCRIPT_DIR/.venv"
if [ -x "$MAC_BUILD_VENV/bin/python" ]; then
  PYTHON_BIN="$MAC_BUILD_VENV/bin/python"
  PYINSTALLER_BIN="$MAC_BUILD_VENV/bin/pyinstaller"
elif [ -x "$VENV_DIR/bin/python" ]; then
  PYTHON_BIN="$VENV_DIR/bin/python"
  PYINSTALLER_BIN="$VENV_DIR/bin/pyinstaller"
else
  PYTHON_BIN="$(command -v python3)"
  PYINSTALLER_BIN=""
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

if [ -n "$PYINSTALLER_BIN" ] && [ -x "$PYINSTALLER_BIN" ]; then
  PYINSTALLER_CMD="$PYINSTALLER_BIN"
else
  echo "安装 PyInstaller（若未安装）..."
  "$PYTHON_BIN" -m pip install pyinstaller -q
  PYINSTALLER_CMD="$PYTHON_BIN -m PyInstaller"
fi

echo "开始打包 macOS 应用..."
$PYINSTALLER_CMD \
  --clean \
  --noconfirm \
  --collect-all charset_normalizer \
  --osx-bundle-identifier "$BUNDLE_ID" \
  --windowed \
  --name "$APP_NAME" \
  main.py

echo "清理临时产物..."
rm -rf "$BUILD_DIR" "$SPEC_FILE" "$BIN_FILE"

echo "打包完成。"
echo "应用位置: $APP_BUNDLE"
echo "使用前：将 cookie.txt 和 biz_magic.txt 放在与 .app 同目录（dist/）即可。"
