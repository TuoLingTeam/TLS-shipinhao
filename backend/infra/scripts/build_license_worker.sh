#!/bin/sh
set -eu

log() {
  printf '[license-worker-build] %s\n' "$*"
}

ensure_cmd() {
  command -v "$1" >/dev/null 2>&1
}

bootstrap_rust() {
  if ensure_cmd cargo && ensure_cmd rustup; then
    return 0
  fi

  log "Rust toolchain not found, bootstrapping rustup..."
  RUSTUP_HOME=${RUSTUP_HOME:-$HOME/.rustup}
  CARGO_HOME=${CARGO_HOME:-$HOME/.cargo}
  export RUSTUP_HOME CARGO_HOME

  if ensure_cmd curl; then
    curl https://sh.rustup.rs -sSf | sh -s -- -y --profile minimal --default-toolchain stable
  elif ensure_cmd wget; then
    wget -qO- https://sh.rustup.rs | sh -s -- -y --profile minimal --default-toolchain stable
  else
    log "Neither curl nor wget is available; cannot bootstrap Rust toolchain."
    exit 127
  fi

  # shellcheck disable=SC1090
  . "$CARGO_HOME/env"
}

ensure_rust_target() {
  if ! rustup target list --installed | grep -q '^wasm32-unknown-unknown$'; then
    log "Installing wasm32-unknown-unknown target..."
    rustup target add wasm32-unknown-unknown
  fi
}

SCRIPT_PATH=$0
case "$SCRIPT_PATH" in
  /*) ;;
  *) SCRIPT_PATH="$(pwd)/$SCRIPT_PATH" ;;
esac
SCRIPT_DIR=$(CDPATH= cd -- "$(dirname -- "$SCRIPT_PATH")" && pwd)
BACKEND_DIR=$(CDPATH= cd -- "$SCRIPT_DIR/../.." && pwd)
REPO_ROOT=$(CDPATH= cd -- "$BACKEND_DIR/.." && pwd)
TEMP_WORKSPACE=$(mktemp -d "${TMPDIR:-/tmp}/license-worker-workspace.XXXXXX")
REAL_WORKER_DIR="$BACKEND_DIR/apps/license-worker"
TEMP_BACKEND_DIR="$TEMP_WORKSPACE/backend"
TEMP_WORKER_DIR="$TEMP_BACKEND_DIR/apps/license-worker"

cleanup() {
  rm -rf "$TEMP_WORKSPACE"
}
trap cleanup EXIT HUP INT TERM

bootstrap_rust
ensure_rust_target

log "Using cargo: $(command -v cargo)"
log "Using rustc: $(command -v rustc)"
log "Working backend dir: $BACKEND_DIR"

mkdir -p "$TEMP_BACKEND_DIR/shared" "$TEMP_BACKEND_DIR/modules" "$TEMP_BACKEND_DIR/apps"
cp -R "$REAL_WORKER_DIR" "$TEMP_WORKER_DIR"
mkdir -p "$TEMP_BACKEND_DIR/shared" && cp -R "$BACKEND_DIR/shared/api-contracts" "$TEMP_BACKEND_DIR/shared/api-contracts"
cp -R "$BACKEND_DIR/modules/license-service" "$TEMP_BACKEND_DIR/modules/license-service"

if [ -f "$REPO_ROOT/Cargo.lock" ]; then
  cp "$REPO_ROOT/Cargo.lock" "$TEMP_WORKSPACE/Cargo.lock"
fi

cat > "$TEMP_WORKSPACE/Cargo.toml" <<'EOF'
[workspace]
resolver = "2"
members = [
  "backend/apps/license-worker",
  "backend/shared/api-contracts",
  "backend/modules/license-service",
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
WORKER_BUILD_ROOT="$BACKEND_DIR/.cache/worker-build-0.1"
WORKER_BUILD_BIN="$WORKER_BUILD_ROOT/bin/worker-build"
if [ ! -x "$WORKER_BUILD_BIN" ]; then
  log "Installing worker-build into $WORKER_BUILD_ROOT ..."
  mkdir -p "$WORKER_BUILD_ROOT"
  cargo install worker-build --version "^0.1" --locked --root "$WORKER_BUILD_ROOT"
fi
WASM_BINDGEN_ROOT="$BACKEND_DIR/.cache/wasm-bindgen-0.2.118"
WASM_BINDGEN_BIN="$WASM_BINDGEN_ROOT/bin/wasm-bindgen"
if [ ! -x "$WASM_BINDGEN_BIN" ]; then
  log "Installing wasm-bindgen-cli 0.2.118 into $WASM_BINDGEN_ROOT ..."
  mkdir -p "$WASM_BINDGEN_ROOT"
  cargo install wasm-bindgen-cli --version "0.2.118" --root "$WASM_BINDGEN_ROOT"
fi
export WASM_BINDGEN_BIN
log "Running worker-build..."
"$WORKER_BUILD_BIN" "$@"

rm -rf "$REAL_WORKER_DIR/build"
cp -R "$TEMP_WORKER_DIR/build" "$REAL_WORKER_DIR/build"
log "Build artifacts copied to $REAL_WORKER_DIR/build"
