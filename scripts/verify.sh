#!/usr/bin/env bash
# 发版前严格验证：与 .github/workflows/build.yml 的 test job 完全对齐
#（fmt --all --check / clippy --workspace -D warnings / test --workspace
# / vue-tsc），确保本地通过等同于 CI 通过，避免 push 后才看到 Actions 失败。
#
# 被 .husky/pre-push 自动调用；手动执行：pnpm verify 或 ./scripts/verify.sh
#
# 历史变更：早期版本仅扫 apps/desktop 范围，2026 年起扩展到整个
# Rust workspace 与 cargo fmt，因为 backend/license-worker、
# crates/* 上的 lint/test 漂移多次成为 push 后才暴露的事故源。
set -euo pipefail

# 进入脚本所在仓库根目录，保证相对路径健壮
cd "$(dirname "$0")/.."

ts_start=$(date +%s)
pretty_step() {
  # 青色前缀，白色正文；tty 下为了易读，非 tty（如 CI 重定向）自动退化为纯文本
  if [[ -t 1 ]]; then
    printf "\033[36m[verify][%d/%d]\033[0m %s\n" "$1" "$2" "$3"
  else
    printf "[verify][%d/%d] %s\n" "$1" "$2" "$3"
  fi
}

pretty_step 1 5 "前端类型检查 (vue-tsc --noEmit)"
pnpm --filter tls-shipinhao-ui exec vue-tsc --noEmit

pretty_step 2 5 "LicenseState SSoT 一致性 (check:license-state)"
node scripts/check-license-state-sync.mjs

pretty_step 3 5 "Rust 格式检查 (cargo fmt --all --check)"
cargo fmt --all --check

pretty_step 4 5 "Rust clippy 严格模式 (cargo clippy --workspace --all-targets -- -D warnings)"
cargo clippy --workspace --all-targets --quiet -- -D warnings

pretty_step 5 5 "Rust 单元测试 (cargo test --workspace)"
cargo test --workspace --quiet

ts_end=$(date +%s)
printf "\033[32m[verify] 全部通过（耗时 %ds）\033[0m\n" $((ts_end - ts_start))
