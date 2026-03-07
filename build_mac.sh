#!/bin/bash
set -e

SCRIPT_DIR="$(cd "$(dirname "$0")" && pwd)"
PYTHON_BIN="$SCRIPT_DIR/.venv/bin/python"

if [ ! -x "$PYTHON_BIN" ]; then
  PYTHON_BIN="$(command -v python3)"
fi

exec "$PYTHON_BIN" "$SCRIPT_DIR/scripts/build.py" macos
