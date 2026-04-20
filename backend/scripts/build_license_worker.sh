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

bootstrap_rust
ensure_rust_target

log "Using cargo: $(command -v cargo)"
log "Using rustc: $(command -v rustc)"
log "Working backend dir: $BACKEND_DIR"

mkdir -p "$TEMP_BACKEND_DIR"
cp -R "$REAL_WORKER_DIR" "$TEMP_WORKER_DIR"
cp -R "$BACKEND_DIR/api-contracts" "$TEMP_BACKEND_DIR/api-contracts"
cp -R "$BACKEND_DIR/license-service" "$TEMP_BACKEND_DIR/license-service"

# license-service 的 [dev-dependencies] 仍然引用 `../../crates/security-core`
# 做跨 crate 一致性测试。wasm target 下不会编译 dev-dep，但 `cargo metadata`
# 必须能读到 security-core 的 manifest，否则 worker-build 在解析 workspace
# 时就会 "failed to load manifest for dependency security_core" 报错。
# 因此把 crates/security-core 也复制进临时 workspace 并作为 member 声明。
mkdir -p "$TEMP_WORKSPACE/crates"
cp -R "$REPO_ROOT/crates/security-core" "$TEMP_WORKSPACE/crates/security-core"

if [ -f "$REPO_ROOT/Cargo.lock" ]; then
  cp "$REPO_ROOT/Cargo.lock" "$TEMP_WORKSPACE/Cargo.lock"
fi

cat > "$TEMP_WORKSPACE/Cargo.toml" <<'EOF'
[workspace]
resolver = "2"
members = [
  "backend/license-worker",
  "backend/api-contracts",
  "backend/license-service",
  "crates/security-core",
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

# ---- worker-build（无官方预编译，只能 cargo install；下一次升级到 worker-build 0.7 可换预编译） ----
WORKER_BUILD_ROOT="$BACKEND_DIR/.cache/worker-build-0.1"
WORKER_BUILD_BIN="$WORKER_BUILD_ROOT/bin/worker-build"
if [ ! -x "$WORKER_BUILD_BIN" ]; then
  log "Installing worker-build into $WORKER_BUILD_ROOT ..."
  mkdir -p "$WORKER_BUILD_ROOT"
  cargo install worker-build --version "^0.1" --locked --root "$WORKER_BUILD_ROOT"
fi

# ---- wasm-bindgen-cli（优先使用官方预编译二进制，节省 ~2 分钟 cargo install 时间） ----
WASM_BINDGEN_VERSION="0.2.118"
WASM_BINDGEN_ROOT="$BACKEND_DIR/.cache/wasm-bindgen-${WASM_BINDGEN_VERSION}"
WASM_BINDGEN_BIN="$WASM_BINDGEN_ROOT/bin/wasm-bindgen"

detect_wasm_bindgen_triple() {
  uname_s=$(uname -s 2>/dev/null || echo "")
  uname_m=$(uname -m 2>/dev/null || echo "")
  case "${uname_s}-${uname_m}" in
    Linux-x86_64)  echo "x86_64-unknown-linux-musl" ;;
    Linux-aarch64) echo "aarch64-unknown-linux-musl" ;;
    Darwin-x86_64) echo "x86_64-apple-darwin" ;;
    Darwin-arm64)  echo "aarch64-apple-darwin" ;;
    *) echo "" ;;
  esac
}

install_wasm_bindgen_prebuilt() {
  triple=$(detect_wasm_bindgen_triple)
  if [ -z "$triple" ]; then
    return 1
  fi
  archive_name="wasm-bindgen-${WASM_BINDGEN_VERSION}-${triple}.tar.gz"
  url="https://github.com/rustwasm/wasm-bindgen/releases/download/${WASM_BINDGEN_VERSION}/${archive_name}"
  tmpdir=$(mktemp -d "${TMPDIR:-/tmp}/wasm-bindgen-dl.XXXXXX")
  log "Fetching prebuilt wasm-bindgen: $url"
  if ensure_cmd curl; then
    curl -fsSL "$url" -o "$tmpdir/$archive_name" || return 1
  elif ensure_cmd wget; then
    wget -qO "$tmpdir/$archive_name" "$url" || return 1
  else
    return 1
  fi
  tar -xzf "$tmpdir/$archive_name" -C "$tmpdir" || return 1
  mkdir -p "$WASM_BINDGEN_ROOT/bin"
  extracted="$tmpdir/wasm-bindgen-${WASM_BINDGEN_VERSION}-${triple}/wasm-bindgen"
  if [ ! -x "$extracted" ]; then
    # Windows tar 可能解成子目录名不同，尽量找一下
    extracted=$(find "$tmpdir" -maxdepth 3 -name wasm-bindgen -type f | head -1)
  fi
  if [ -z "$extracted" ] || [ ! -x "$extracted" ]; then
    return 1
  fi
  cp "$extracted" "$WASM_BINDGEN_BIN"
  chmod +x "$WASM_BINDGEN_BIN"
  rm -rf "$tmpdir"
  return 0
}

if [ ! -x "$WASM_BINDGEN_BIN" ]; then
  log "Installing wasm-bindgen-cli ${WASM_BINDGEN_VERSION} into $WASM_BINDGEN_ROOT ..."
  mkdir -p "$WASM_BINDGEN_ROOT"
  if install_wasm_bindgen_prebuilt; then
    log "wasm-bindgen-cli ${WASM_BINDGEN_VERSION} installed from prebuilt release."
  else
    log "Prebuilt wasm-bindgen not available for this platform, falling back to cargo install."
    cargo install wasm-bindgen-cli --version "${WASM_BINDGEN_VERSION}" --root "$WASM_BINDGEN_ROOT"
  fi
fi
export WASM_BINDGEN_BIN

log "Running worker-build..."
"$WORKER_BUILD_BIN" "$@"

rm -rf "$REAL_WORKER_DIR/build"
cp -R "$TEMP_WORKER_DIR/build" "$REAL_WORKER_DIR/build"
log "Build artifacts copied to $REAL_WORKER_DIR/build"
