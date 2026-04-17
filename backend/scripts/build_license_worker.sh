#!/bin/sh
set -eu

SCRIPT_PATH=$0
case "$SCRIPT_PATH" in
  /*) ;;
  *) SCRIPT_PATH="$(pwd)/$SCRIPT_PATH" ;;
esac
SCRIPT_DIR=$(CDPATH= cd -- "$(dirname -- "$SCRIPT_PATH")" && pwd)
BACKEND_DIR=$(CDPATH= cd -- "$SCRIPT_DIR/.." && pwd)
REPO_ROOT=$(CDPATH= cd -- "$BACKEND_DIR/.." && pwd)
TEMP_WORKSPACE=$(mktemp -d "${TMPDIR:-/tmp}/license-worker-workspace.XXXXXX")
REAL_WORKER_DIR="$BACKEND_DIR/license-worker"
TEMP_BACKEND_DIR="$TEMP_WORKSPACE/backend"
TEMP_WORKER_DIR="$TEMP_BACKEND_DIR/license-worker"

cleanup() {
  rm -rf "$TEMP_WORKSPACE"
}
trap cleanup EXIT HUP INT TERM

mkdir -p "$TEMP_BACKEND_DIR/crates" "$TEMP_BACKEND_DIR/src/admin"
cp -R "$REAL_WORKER_DIR" "$TEMP_WORKER_DIR"
cp -R "$BACKEND_DIR/crates/api-contracts" "$TEMP_BACKEND_DIR/crates/api-contracts"
cp -R "$BACKEND_DIR/crates/license-service" "$TEMP_BACKEND_DIR/crates/license-service"
cp "$BACKEND_DIR/src/admin/admin.html" "$TEMP_BACKEND_DIR/src/admin/admin.html"

if [ -f "$REPO_ROOT/Cargo.lock" ]; then
  cp "$REPO_ROOT/Cargo.lock" "$TEMP_WORKSPACE/Cargo.lock"
fi

cat > "$TEMP_WORKSPACE/Cargo.toml" <<'EOF'
[workspace]
resolver = "2"
members = [
  "backend/license-worker",
  "backend/crates/api-contracts",
  "backend/crates/license-service",
]

[workspace.package]
edition = "2021"
version = "5.1.0"
license = "Proprietary"
authors = ["TLS-801"]

[workspace.dependencies]
anyhow = "1"
chrono = { version = "0.4", features = ["serde"] }
serde = { version = "1", features = ["derive"] }
serde_json = "1"
sha2 = "0.10"
thiserror = "2"
tokio = { version = "1", features = ["full"] }
EOF

cd "$TEMP_WORKER_DIR"
WORKER_BUILD_ROOT="$BACKEND_DIR/tmp/worker-build-0.1"
WORKER_BUILD_BIN="$WORKER_BUILD_ROOT/bin/worker-build"
if [ ! -x "$WORKER_BUILD_BIN" ]; then
  mkdir -p "$WORKER_BUILD_ROOT"
  cargo install worker-build --version "^0.1" --locked --root "$WORKER_BUILD_ROOT" >/dev/null 2>&1
fi
WASM_BINDGEN_ROOT="$BACKEND_DIR/tmp/wasm-bindgen-0.2.118"
WASM_BINDGEN_BIN="$WASM_BINDGEN_ROOT/bin/wasm-bindgen"
if [ ! -x "$WASM_BINDGEN_BIN" ]; then
  mkdir -p "$WASM_BINDGEN_ROOT"
  cargo install wasm-bindgen-cli --version "0.2.118" --root "$WASM_BINDGEN_ROOT" >/dev/null 2>&1
fi
export WASM_BINDGEN_BIN
"$WORKER_BUILD_BIN" "$@"

rm -rf "$REAL_WORKER_DIR/build"
cp -R "$TEMP_WORKER_DIR/build" "$REAL_WORKER_DIR/build"
