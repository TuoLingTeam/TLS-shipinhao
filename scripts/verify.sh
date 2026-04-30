#!/usr/bin/env bash
# 发版前严格验证，与 GitHub Actions test job 对齐。
set -euo pipefail

cd "$(dirname "$0")/.."

ts_start=$(date +%s)
pretty_step() {
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
