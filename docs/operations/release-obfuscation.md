# 发版混淆与分发流程

本文档记录「从源码到分发安装包」的完整强力保护链路、每个层级的具体工具与配置，
以及一键回滚开关。新成员加入团队后应先通读本文再参与发版。

## 层级总览

| 层级 | 目标 | 工具 / 配置 | 开关位置 |
|------|------|-------------|----------|
| L1-Rust 字符串 | 加密 URL / API 路径 / 敏感文案 | `obfstr` crate（workspace 依赖） | 源码：逐点 `obfstr::obfstr!(...)` |
| L1-Rust 编译 | 最大化优化 + 清除可读面 | `[profile.release]` lto/strip/panic=abort/debug=0 + 发版 `RUSTFLAGS` path remap | `Cargo.toml` + `scripts/build-release.mjs` |
| L2-JS 混淆 | 前端 stringArray / CFG 扁平 / selfDefending | `javascript-obfuscator@^4.2` | `scripts/obfuscator.config.json` |
| L2-编排 | 一键完成前端构建 → 混淆 → Tauri 打包 | `scripts/build-release.mjs` | 顶层 `package.json` 的 `release:build` |
| L3-Tauri 壳 | CSP / invoke 面 | `tauri.conf.json` + `generate_handler!` | `apps/desktop/src/main.rs` |
| CI 集成 | 打 tag 时产出混淆 Windows exe / macOS dmg | `.github/workflows/build.yml` | 触发：`git push v*` tag |

## 本地发版（与 CI 等价）

```bash
# 纯前端 + 混淆，用于排障
pnpm release:build:plain             # 关闭 JS 混淆，跑原生 dist
pnpm release:build                   # 完整链路，即 node scripts/build-release.mjs

# 只跑前端 + 混淆，跳过 cargo tauri build
node scripts/build-release.mjs --skip-rust-build

# 完整发版（默认启用混淆）
node scripts/build-release.mjs
```

完成后：
- `target/release/desktop(.exe)` —— 便携版可执行文件
- `target/release/bundle/dmg/*.dmg` —— macOS 安装包
- `dist/release/` —— 脚本自动收集的可直接分发的产物

## CI 发版

```bash
git tag v5.2.0
git push --tags
```

`.github/workflows/build.yml` 会在 `v*` tag push 时触发，跑 `node scripts/build-release.mjs`
→ 上传 `TLS-shipinhao-v{ver}-portable.exe` 与 `TLS-shipinhao-v{ver}-mac.dmg` 到 GitHub Release。

CI 额外环境变量：

| Env | 值 | 作用 |
|-----|-----|------|
| `CARGO_PROFILE_RELEASE_LTO` | `thin` | 压缩 Windows 编译时间，产物略增 |
| `CARGO_PROFILE_RELEASE_CODEGEN_UNITS` | `16` | 启用并行 codegen |

注：本地 `Cargo.toml` 的 `lto=true / codegen-units=1` 仍然生效（适合手工出「极致优化版」）。

## 关键决策

### 采用

- **`obfstr` 编译期字符串加密**：纯宏、零运行时依赖，`strings` 扫不出明文
- **`javascript-obfuscator` stringArray + CFG 扁平 + selfDefending**：成熟 NPM 包，社区活跃
- **CI-only profile 覆盖**：`Cargo.toml` 保持最高优化，CI 用 `thin` 换编译速度
- **tag-only 触发 Desktop 构建**：日常 push 不占 GH Actions 免费分钟（Windows 2×、macOS 10×）
- **CSP 白名单显式化**：从 `default-src 'self'` 扩展为每个 directive 明示 + `frame-ancestors 'none'`
- **Tauri invoke 面收缩**：删除未使用 command（`get_app_info` / `get_ui_scale` / `set_ui_scale` / `sync_orders`）

### 不采用

| 方案 | 理由 |
|------|------|
| LLVM Rust 混淆（`cargo-obfuscator`） | 需自编译 rustc 或不稳定 toolchain，维护成本过高 |
| UPX 壳 | 国内 360 / Windows Defender 大概率误报病毒，反而影响用户 |
| `renameGlobals: true` | 破坏 ES module 导出 |
| `renameProperties: true` | Tauri `invoke(name)` 与 Vue props 依赖字符串键名 |
| `disableConsoleOutput: true` | 线上排障仍需 `console.error` |

### 后续可选

- **代码签名证书**：Windows EV 证书 ~$200~$500/年，消除 SmartScreen 警告，抗二次篡改

## Rust 二进制：`strings` 与路径 / panic 元数据

- **已做**：`[profile.release]` 的 `strip` / `panic = "abort"` / `debug = 0`；workspace 里 `tracing` 使用 `release_max_level_off`，减少宏展开带入的 file/line 静态串（见根 `Cargo.toml` 注释）。
- **发版脚本追加（稳定工具链）**：`node scripts/build-release.mjs` 在调用 `cargo tauri build` 时合并 `RUSTFLAGS`，包含  
  `-C remap-path-prefix=<仓库根目录>=.`，并在设置了 `CARGO_HOME` / `RUSTUP_HOME` 时把依赖缓存路径映射为 `/.cargo`、`/.rustup`，减轻 `strings` 扫出本机绝对路径与 registry 路径。
- **可选（需 nightly 或支持该 unstable 的工具链）**：在环境中设置 `TLS_RELEASE_RUSTFLAGS=-Zlocation-detail=none`，可进一步削弱 panic / caller 相关位置信息（与 `remap-path-prefix` 互补）。当前 `rust-toolchain.toml` 为 **stable**，默认 CI/本地不启用该项。
- **serde「短 tag / 短字段名」**：桌面端与授权服务之间存在 **稳定 JSON 契约**（例如 `domain-core::TaskKind` 与 `api_contracts` 任务字面量、Tauri `invoke` 载荷字段名）。把 `rename_all` 或枚举序列化字面量改成短 id **会同步破坏** 前端、已发放 Lease、以及服务端校验，因此不在此做「全局短名」式混淆；若将来仅有 **纯 Rust 内部、永不序列化到 IPC/磁盘 JSON** 的结构，再对单结构评估 `#[serde(rename = "…")]`。

## 产物特征（阶段 5 实测 / macOS arm64）

| 指标 | 数值 |
|------|------|
| 构建时长 | 1m49s（Rust release 1m31s + JS 混淆 ~3s + bundle） |
| `target/release/desktop` | 11 MB |
| `build/obfuscated-ui/` | 2.1 MB（vs apps/ui/dist 304 KB，~7×） |
| `dist/release/*.dmg` | 6.2 MB |
| `strings binary` 命中敏感 URL | **0 条**（`sphapi.*` / `store.weixin.qq.com/shop-faas/*` / `gitee.com/tuolingshe/*` 全部消失） |

## 回滚开关

### 彻底跳过 JS 混淆

```bash
node scripts/build-release.mjs --no-obfuscate
```

或在 `package.json` 配置的别名：

```bash
pnpm release:build:plain
```

### 单独关闭 selfDefending / debugProtection

编辑 `scripts/obfuscator.config.json` 把对应字段改 `false`。改动仅影响运行时反调试，不影响 stringArray / CFG 扁平。

### 把 `obfstr` 临时改回明文 const

```rust
// 调试期可临时改回（别提交）
// fn license_api_base_urls() -> Vec<String> { vec!["https://...".to_string()] }
```

**禁止将明文 URL 提交到 main 分支**。

### CI 禁用混淆链路

```yaml
# .github/workflows/build.yml 临时把下面一行换回去（需自行 revert）
# run: cargo tauri build
# working-directory: apps/desktop
```

## 常见问题

### Q. 混淆后 Tauri 运行时白屏？

90% 是 `selfDefending` / `debugProtection` 在未开 DevTools 时也误伤。
先把 `obfuscator.config.json` 里这两项改为 `false`，再二分排查哪个混淆选项引起的。

### Q. 本地 `node scripts/build-release.mjs` 报 `--ci` invalid？

本地 shell（特别是 oh-my-zsh 默认）可能把 `CI=1` 导出到 env。
脚本自 `b0f24eb` 起会自动规整（`CI=1` 会被重写成 `true`）。如果升级脚本后还有问题，
直接 `CI=true node scripts/build-release.mjs`。

### Q. 增量构建后混淆失效？

`scripts/build-release.mjs` 每次会 `cleanDir(build/obfuscated-ui)` + 重新跑 obfuscator，不存在增量问题。
但如果你只跑 `cargo tauri build`（不经过脚本），就会直接用 `apps/ui/dist/` 未混淆产物。
**发版一律走脚本**。

## 相关 commit（按阶段顺序）

| 阶段 | 提交 | 说明 |
|------|------|------|
| 0 | `1f55ec2` | 搭建 scripts/build-release.mjs 骨架 |
| 1a | `0539637` | 引入 obfstr 到 workspace |
| 1b | `ed1d354` | URL / Referer 字符串加密 |
| 1c | `c71f37f` | release profile 补强 |
| 2a | `cc3018d` | 引入 javascript-obfuscator + 配置 |
| 3a | `0890ac0` | tauri.conf.json CSP 收紧 |
| 3b | `c8f9d2c` | 删除 4 个未用 command |
| 4 | `ed10423` | build.yml 走混淆链路 |
| 5 | `b0f24eb` | CI env 规整 + 本地验证 |
