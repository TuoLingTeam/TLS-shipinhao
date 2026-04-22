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

## 常用命令

### 前端

```bash
pnpm build
pnpm lint
```

### Rust 测试

```bash
cargo test --workspace -- --nocapture
```

### 桌面端打包（Mac 默认输出 `.app`）

```bash
pnpm tauri:build
```

当前默认行为：
- 执行 `cd apps/desktop && cargo tauri build -b app`
- 默认生成 **Mac `.app`**，不默认生成 `.dmg`

## 构建产物

### Mac 桌面应用

```text
target/release/bundle/macos/驼铃·视频小店差评处理.app
```

### Rust 可执行文件

```text
target/release/desktop
```

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

更详细的后端说明见 [backend/README.md](backend/README.md)。

## 备份目录说明

`backup/` 下的内容用于保留旧版 Python 客户端资产，便于：
- 迁移对照
- 回归参考
- 历史实现追溯

这些目录**不是当前正式运行路径**。

## 注意事项

- 桌面图标资源位于 `apps/desktop/icons/`
- JS 混淆配置已关闭 `controlFlowFlattening` / `deadCodeInjection` / `selfDefending` / `debugProtection`，避免破坏 Vue 3 runtime 或触发 Tauri CSP 拦截
- Windows 下 `std::process::Command` 调用系统工具（如 `wmic`）需加 `CREATE_NO_WINDOW` 防止黑窗闪现
- `backup/` 为旧版 Python 资产，不参与当前主运行链路
