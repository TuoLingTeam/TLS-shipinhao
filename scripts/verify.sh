#!/usr/bin/env bash
# 发版前严格验证：覆盖 CI 上 -D warnings / vue-tsc / cargo test 全部检查项，
# 确保本地通过等同于 CI 通过，避免 push 后才看到 GitHub Actions 失败。
#
# 被 .husky/pre-push 自动调用；手动执行：pnpm verify 或 ./scripts/verify.sh
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

pretty_step 1 3 "前端类型检查 (vue-tsc --noEmit)"
pnpm --filter tls-shipinhao-ui exec vue-tsc --noEmit

pretty_step 2 3 "Rust clippy 严格模式 (-D warnings)"
cargo clippy --manifest-path apps/desktop/Cargo.toml --quiet -- -D warnings

pretty_step 3 3 "Rust 单元测试 (cargo test)"
cargo test --manifest-path apps/desktop/Cargo.toml --quiet

ts_end=$(date +%s)
printf "\033[32m[verify] 全部通过（耗时 %ds）\033[0m\n" $((ts_end - ts_start))
