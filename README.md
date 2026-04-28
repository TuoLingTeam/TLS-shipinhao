# TLS-shipinhao

微信视频号小店中差评订单查找与物流更新桌面工具。

当前主线技术栈：**Rust + Tauri 2 + Vue 3 + Tailwind CSS v4**，授权后端运行在 **Cloudflare Workers + D1**。

## 功能概览

- **中差评订单匹配**：按商品、SKU、昵称、时间窗口等多维度自动关联评价与订单
- **订单缓存同步**：本地 SQLite 缓存，支持首轮补齐、增量刷新、缺口补抓
- **批量发货更新**：物流公司自动降级、物流快照保留、批量任务进度反馈
- **卡密授权管理**：Ed25519 签名授权、设备绑定、离线宽限、灰度更新控制
- **首次迁移兼容**：可从旧版 Python 客户端迁移 Cookie / 配置 / 历史数据

## 目录结构

```text
TLS-shipinhao/
├── apps/                        # 应用层
│   ├── desktop/                 # Tauri 2 桌面端（Rust 命令、适配器、状态、图标）
│   └── ui/                      # Vue 3 前端（Vite 6 + Pinia + vue-router + Tailwind v4）
├── crates/                      # 桌面共享 Rust 库
│   ├── domain-core/             # 领域模型与错误类型
│   ├── desktop-services/        # 订单缓存、同步、HTTP、SQLite 等业务服务
│   └── security-core/           # 设备指纹、Lease 校验、完整性检查
├── backend/                     # Cloudflare Worker 授权后端
│   ├── api-contracts/           # 前后端共享 API 契约（serde 类型）
│   ├── license-service/         # 授权域逻辑（Lease、卡密、本地验证）
│   ├── license-worker/          # Rust → WASM Cloudflare Worker 入口
│   ├── db/                      # D1 schema / migration
│   ├── scripts/                 # Worker 构建脚本
│   └── wrangler.toml            # Worker 生产部署入口
├── scripts/                     # 发版与构建脚本
│   ├── build-obfuscate.mjs      # 混淆镜像构建（JS obfuscator + cargo tauri build）
│   └── obfuscator.config.json   # javascript-obfuscator 配置
├── tools/                       # 辅助工具
│   ├── build-tools/             # 发版相关构建工具库
│   └── xtask/                   # perf、release、bench 等辅助命令
├── backup/                      # 旧版 Python 资产备份（不参与主线）
├── Cargo.toml                   # 顶层 Rust workspace
├── package.json                 # pnpm 脚本入口
├── pnpm-workspace.yaml          # pnpm workspace 配置
└── .github/workflows/           # CI/CD（tag 触发桌面发版）
```

## 快速开始

### 环境要求

- Rust stable（见 `rust-toolchain.toml`）
- Node.js 22+
- pnpm 10+
- Tauri CLI 2.x

安装 Tauri CLI：

```bash
cargo install tauri-cli --version "^2"
```

### 安装依赖

```bash
pnpm install
```

### 开发模式

```bash
pnpm tauri:dev
```

该命令会：
- 启动 `apps/ui` 的 Vite 开发服务
- 启动 `apps/desktop` 的 Tauri 桌面壳

### 调试与 Devtools

本项目使用 Vue 3 + Pinia + Vue Router，开发期建议挂上以下工具提升调试效率：

| 工具 | 用途 | 安装 / 启用 |
|---|---|---|
| **Vue Devtools** | 组件树、props/state 实时预览、router 路由调试 | Chrome / Edge / Firefox 应用商店搜「Vue.js devtools」(>= v6 支持 Vue 3)；Tauri 内 webview 也可启用，需在 dev 模式下右键「检查」打开 DevTools |
| **Pinia 调试** | store 的 mutation 时间线、state diff、action 重放 | Vue Devtools 内置 Pinia 面板，无需额外安装 |
| **Vite HMR Overlay** | 模板/类型错误覆盖层 | 默认开启，无需配置 |
| **Tauri DevTools** | 主进程 ↔ 前端 IPC 调用追踪 | macOS / Windows 在 dev 模式下右键 webview 点击「检查」即可；release 包默认禁用 |

> Tip：Tauri 2 webview 在 macOS 默认禁用了右键菜单，本项目通过 `tauri.conf.json` 在 dev 模式下放开。如果右键无响应，确认当前是 `pnpm tauri:dev` 启动而非打包后的产物。

## 常用命令

### 前端

```bash
pnpm build
pnpm lint
```

### 测试

```bash
pnpm test               # 等价于 test:ui + test:rust，串行执行
pnpm test:ui            # 仅前端 Vitest（apps/ui/src 下 14+ 个 .spec.ts）
pnpm test:ui:watch      # 前端 Vitest watch 模式
pnpm test:rust          # 仅 Rust workspace 单测
cargo test --workspace -- --nocapture   # 直跑（看完整输出）
```

本地查看覆盖率（CI 不强制阈值）：

```bash
# 前端：已集成 @vitest/coverage-v8，直接调脚本即可，输出在 apps/ui/coverage/
pnpm test:ui:coverage

# Rust：按需 cargo install cargo-llvm-cov
cargo llvm-cov --workspace --html
```

前端冒烟 E2E（Playwright，仅本地，CI 默认不跑）：

```bash
# 仅首次：下载 Chromium 浏览器二进制
pnpm --filter tls-shipinhao-ui exec playwright install chromium

# 启动 vite preview + 跑冒烟 spec（apps/ui/e2e/*.spec.ts）
pnpm test:e2e
# 与上一行等价，显式强调「仅 WebView 预览 / IPC mock」，非真 Tauri 壳
pnpm test:e2e:web

# 调试模式（headed + DevTools）
pnpm --filter tls-shipinhao-ui test:e2e:headed
```

骨架仅验证「构建产物能起 + 主要路由能渲染 + 不出现 404」三条粗粒度路径，
不替代 Vitest 单测对业务逻辑的覆盖；后续按需扩 spec。

`apps/ui/e2e/license-ipc.spec.ts` 在浏览器内注入最小 `__TAURI_INTERNALS__` mock，
覆盖设置页「模拟激活 → 已激活」展示链，不启动真实桌面壳。

**L4-4（真 Tauri E2E）**：需 `tauri-driver` / WebDriver 等驱动真实 WebView2/WKWebView，
环境与 CI 成本高；当前仍以 Playwright + `vite preview` 为回归入口，真壳 E2E 单列 backlog。

**`cargo audit`（L4-5 策略）**：CI `test` job 已安装并执行 `cargo audit`，当前为
`continue-on-error: true`。RustSec 数据库与传递依赖会不定期出现 **未修复** 或
**误报** advisory，直接改成阻塞会导致主干频繁不可用；**维持非阻塞**、由维护者
定期阅读日志并择机升级依赖。若连续多个版本周期内本地与 CI 均报告 **0 vulnerabilities**，
可将该两步的 `continue-on-error` 去掉改为硬门禁。

### 桌面端打包

```bash
pnpm tauri:build
```

当前默认行为：
- 执行 `cd apps/desktop && cargo tauri build`
- macOS 默认生成 `.dmg`
- Windows 默认生成 NSIS 安装包，并在安装阶段自动补齐 WebView2 Runtime

## 构建产物

### 发版命令 → 中间目录 / 最终产物对照

| 命令 | 入口脚本 | 中间镜像 / 缓存 | 最终产物目录 | 适用场景 |
|---|---|---|---|---|
| `pnpm tauri:build` | `apps/desktop` 内 `cargo tauri build` | 无（在源码 `target/` 内编译） | `target/release/bundle/{dmg,nsis,...}` | 本地直建，不混淆 |
| `pnpm release:build` | `scripts/build-release.mjs` | `build/obfuscated-ui/`（前端混淆中转） | `dist/release/` + `target/release/bundle/...` | 轻量发版，源树内打包 |
| `pnpm release:build:plain` | `scripts/build-release.mjs --no-obfuscate` | 同上但跳过 JS 混淆 | 同上 | 快速本地预览混淆前包形态 |
| `pnpm release:build:deep` | `scripts/build-obfuscate.mjs` | `TLS-shipinhao-release/`（整树镜像 + JS 深混 + rustflags 重写） | `dist/release/` 内最终安装包 / 便携 exe / dmg | 正式发版主路径 |
| `pnpm release:build:deep:mirror-only` | 同上 + `--skip-build` | 仅生成 `TLS-shipinhao-release/` 镜像，不调 `cargo tauri build` | 无最终产物 | 验证镜像内容是否干净 |
| `pnpm worker:deploy` | `cd backend && npx wrangler deploy` | `backend/build/` | Cloudflare 边缘 | 授权 Worker 上线 |

`TLS-shipinhao-release/` 是 `build-obfuscate.mjs` 全量重建的镜像，**禁止手动修改**——任何对它的改动都会在下次发版被覆盖。

### Mac 桌面应用

```text
target/release/bundle/dmg/*.dmg
```

### Windows 安装包

```text
target/release/bundle/nsis/*.exe
```

### Rust 可执行文件

```text
target/release/desktop
```

Windows 端请优先分发 NSIS 安装包，不要把 `target/release/desktop.exe` 作为正式用户包；便携 exe 无法替用户安装 WebView2，缺少 Runtime 的机器会在启动时弹出错误提示。

### Windows 便携包内置 WebView2

如确需免安装便携包，可使用 Microsoft WebView2 Fixed Version Runtime：

```bash
TLS_WEBVIEW2_FIXED_RUNTIME_DIR=/path/to/fixed-runtime pnpm release:build:deep
```

构建脚本会生成：

```text
dist/release/TLS-shipinhao-portable/
├── TLS-shipinhao.exe
└── WebView2Runtime/
```

`TLS-shipinhao.exe` 启动时会优先探测同目录下的 `WebView2Runtime`，并通过 `WEBVIEW2_BROWSER_EXECUTABLE_FOLDER` 指向其中包含 `msedgewebview2.exe` 的目录。该模式不需要安装 WebView2，但包体会明显增大；Windows 10 + WebView2 Fixed Runtime 120+ 还需要确保运行时目录具备应用容器读取权限。

### 发布元数据

```bash
cargo run -p xtask -- release 5.1.0
```

默认输出到：

```text
backend/dist/release/version.json
```

## 后端部署

授权 Worker 部署入口位于 `backend/wrangler.toml`。

```bash
cd backend
npx wrangler deploy
```

当前 Worker 路由：

| 域名 | 加速方式 | 说明 |
|---|---|---|
| `sphapi-cn.199908.top` | EdgeOne 中国节点加速 | 国内用户首选 |
| `sphapi.199908.top` | Cloudflare Worker 直连 | 主域名 |
| `sphapi.tuoling.ccwu.cc` | Cloudflare Worker 直连 | 备用 |
| `sphapi.tuoling.eu.cc` | Cloudflare Worker 直连 | 备用 |

### 新增 / 调整备用域名检查清单

`apps/desktop/src/app_settings.rs::license_api_base_urls` 与上方路由表是同一份
策略的「客户端视角」与「边缘视角」，任一处漂移都会让客户端配了域名却被
Cloudflare 判 1014（unknown host），或 Worker 绑了路由却没人调。新增 / 调整
任一备用域名时按以下三步同步执行：

1. **客户端**：编辑 `apps/desktop/src/app_settings.rs::license_api_base_urls`，
   用 `obfstr::obfstr!("...")` 包裹新 URL 加入返回数组（顺序代表优先级）。
2. **Cloudflare 路由**：在 `backend/wrangler.toml` 增补 `routes = [{ pattern = "...", custom_domain = true }]`，
   再 `cd backend && npx wrangler deploy` 让边缘绑定生效。
3. **DNS / 加速层**：把新域名 CNAME / 接入到对应加速通道（EdgeOne / Cloudflare），
   在浏览器手动访问 `https://<新域名>/` 确认返回授权服务的健康响应而非默认页。

完成后回到本表格末尾追加一行，让运营同学也能看到当前期望的域名拓扑。

## 备份目录说明

`backup/` 下的内容用于保留旧版 Python 客户端资产，便于：
- 迁移对照
- 回归参考
- 历史实现追溯

这些目录**不是当前正式运行路径**。

## 注意事项

- 桌面图标资源位于 `apps/desktop/icons/`
- JS 混淆配置已关闭 `controlFlowFlattening` / `deadCodeInjection` / `selfDefending` / `debugProtection`，避免破坏 Vue 3 runtime 或触发 Tauri CSP 拦截。**详细原因与触发的真实故障链路**见 [`scripts/obfuscator.config.json`](scripts/obfuscator.config.json) 中 `_note_cff_dci_off` 字段；**不要**为「更难被反混淆」打开这两个开关，否则下次发版会以非常间歇且难复现的方式炸掉
- Windows 下 `std::process::Command` 调用系统工具（如 `wmic`）需加 `CREATE_NO_WINDOW` 防止黑窗闪现
- `backup/` 为旧版 Python 资产，不参与当前主运行链路
- 新增 / 调整授权 Worker 域名前请通读上文「新增 / 调整备用域名检查清单」，避免客户端 `apps/desktop/src/app_settings.rs::license_api_base_urls` 与 `backend/wrangler.toml` 路由对不上
