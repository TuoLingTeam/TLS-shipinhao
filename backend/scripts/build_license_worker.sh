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

# worker-build 0.8 与 worker 0.8 是强版本对齐：0.8.x 的 worker-build 内置了
# wasm-bindgen 预编译二进制的自动下载逻辑，脚本不再需要显式管理 wasm-bindgen。
# 官方没有发布 worker-build 本身的预编译二进制，这里仍用 cargo install，
# 但会缓存在仓库本地 `.cache/worker-build-0.8/`，同一机器首次安装后后续跳过。
WORKER_BUILD_ROOT="$BACKEND_DIR/.cache/worker-build-0.8"
WORKER_BUILD_BIN="$WORKER_BUILD_ROOT/bin/worker-build"
if [ ! -x "$WORKER_BUILD_BIN" ]; then
  log "Installing worker-build 0.8.x into $WORKER_BUILD_ROOT ..."
  mkdir -p "$WORKER_BUILD_ROOT"
  cargo install worker-build --version "^0.8" --locked --root "$WORKER_BUILD_ROOT"
fi

# worker-build 0.8 会把自己下载的工具链（wasm-bindgen / wasm-opt 等）放到一个
# 缓存目录下；默认是 $HOME/.cache/worker-build。Cloudflare 构建环境会自动保留
# 该目录，多数情况下首次下载后后续直接命中缓存。如需迁移到仓库本地缓存，
# 可以通过 `WORKER_BUILD_BIN_ROOT` 环境变量指定。
log "Running worker-build 0.8..."
"$WORKER_BUILD_BIN" "$@"

rm -rf "$REAL_WORKER_DIR/build"
cp -R "$TEMP_WORKER_DIR/build" "$REAL_WORKER_DIR/build"
log "Build artifacts copied to $REAL_WORKER_DIR/build"
