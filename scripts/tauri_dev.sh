#!/bin/sh
set -eu

PORT="${TLS_SHIPINHAO_UI_PORT:-5173}"
SCRIPT_PATH=$0
case "$SCRIPT_PATH" in
  /*) ;;
  *) SCRIPT_PATH="$(pwd)/$SCRIPT_PATH" ;;
esac
SCRIPT_DIR=$(CDPATH= cd -- "$(dirname -- "$SCRIPT_PATH")" && pwd)
REPO_ROOT=$(CDPATH= cd -- "$SCRIPT_DIR/.." && pwd)
APP_DESKTOP_DIR="$REPO_ROOT/apps/desktop"
PROJECT_MARKER="$REPO_ROOT/apps/ui"

log() {
  printf '[tauri-dev] %s\n' "$*"
}

listener_pids() {
  lsof -tiTCP:"$PORT" -sTCP:LISTEN 2>/dev/null || true
}

is_project_vite_pid() {
  pid="$1"
  cmd=$(ps -p "$pid" -o command= 2>/dev/null || true)
  case "$cmd" in
    *vite*"$PROJECT_MARKER"*|*"$PROJECT_MARKER"*vite*|*vite/bin/vite.js*"$PROJECT_MARKER"*|*"$REPO_ROOT"*vite*)
      return 0
      ;;
    *)
      return 1
      ;;
  esac
}

terminate_pid() {
  pid="$1"
  if ! kill -0 "$pid" 2>/dev/null; then
    return 0
  fi

  log "清理旧实例 PID=$pid"
  kill "$pid" 2>/dev/null || true
  i=0
  while [ "$i" -lt 20 ]; do
    if ! kill -0 "$pid" 2>/dev/null; then
      return 0
    fi
    sleep 0.25
    i=$((i + 1))
  done

  log "旧实例未及时退出，升级为强制终止 PID=$pid"
  kill -9 "$pid" 2>/dev/null || true
}

cleanup_conflicting_vite() {
  pids=$(listener_pids)
  [ -n "$pids" ] || return 0

  for pid in $pids; do
    if is_project_vite_pid "$pid"; then
      terminate_pid "$pid"
    else
      cmd=$(ps -p "$pid" -o command= 2>/dev/null || true)
      log "端口 $PORT 已被非当前项目进程占用，未自动清理：PID=$pid CMD=$cmd"
      exit 1
    fi
  done
}

wait_for_port_release() {
  i=0
  while [ "$i" -lt 20 ]; do
    pids=$(listener_pids)
    [ -z "$pids" ] && return 0
    sleep 0.25
    i=$((i + 1))
  done
  log "端口 $PORT 仍未释放，请手动检查占用进程"
  exit 1
}

cleanup_conflicting_vite
wait_for_port_release

log "启动 cargo tauri dev"
cd "$APP_DESKTOP_DIR"
exec cargo tauri dev "$@"
