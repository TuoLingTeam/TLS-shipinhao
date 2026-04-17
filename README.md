# TLS-shipinhao

微信小商店中差评订单查找与物流更新桌面工具。

当前主线技术栈：**Rust + Tauri 2 + Vue 3 + Tailwind CSS v4**。

## 功能概览

- **中差评订单匹配**：按商品、SKU、昵称、时间窗口等多维度自动关联评价与订单
- **订单缓存同步**：本地 SQLite 缓存，支持首轮补齐、增量刷新、缺口补抓
- **批量发货更新**：物流公司自动降级、物流快照保留、批量任务进度反馈
- **卡密授权管理**：Ed25519 签名授权、设备绑定、离线宽限、灰度更新控制
- **首次迁移兼容**：可从旧版 Python 客户端迁移 Cookie / 配置 / 历史数据

## 目录结构

```text
TLS-shipinhao/
├── apps/                        # 当前正式应用层
│   ├── desktop/                 # Tauri 桌面端（命令、适配器、状态、图标）
│   ├── ui/                      # Vue 3 前端（Vite + Pinia + vue-router）
│   └── license-worker/          # Rust Cloudflare Worker 授权服务
├── backend/                     # 后端与共享实现
│   ├── crates/                  # Rust 共享库与核心业务能力
│   ├── db/                      # D1 schema / migration
│   ├── dist/                    # 发布元数据输出（如 version.json）
│   ├── docs/                    # PRD、验收、性能、发布文档
│   ├── src/                     # Worker 管理页与兼容壳
│   ├── tests/                   # Rust / Python 回归测试与夹具
│   ├── xtask/                   # 构建、manifest、性能、发布辅助命令
│   └── wrangler.toml            # Cloudflare Worker 生产部署入口
├── backup/                      # 旧版 Python 资产备份（不参与当前主运行链路）
│   ├── legacy-src/              # 旧版 Python 源码快照
│   ├── legacy-runtime/          # 旧版运行时残留
│   └── legacy-dist/             # 旧版编译产物 / 分发残留
├── openspec/                    # 变更提案与迁移设计记录
├── Cargo.toml                   # 顶层 Rust workspace
├── package.json                 # 前端 / 桌面常用脚本入口
└── .github/workflows/           # CI/CD 工作流
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

生产部署入口位于：

```text
backend/wrangler.toml
```

常见操作：

```bash
cd backend
npx wrangler deploy
```

更详细的后端说明见：
- [backend/README.md](backend/README.md)
- [apps/license-worker/README.md](apps/license-worker/README.md)

## 备份目录说明

`backup/` 下的内容用于保留旧版 Python 客户端资产，便于：
- 迁移对照
- 回归参考
- 历史实现追溯

这些目录**不是当前正式运行路径**。

## 相关文档

- 产品/任务卡片：`backend/docs/`
- 发布手册：`backend/docs/release-runbook.md`
- 回归矩阵：`backend/docs/regression-matrix.md`
- 变更设计：`openspec/changes/`

## 注意事项

- 当前桌面图标资源位于 `apps/desktop/icons/`
- 品牌图标来源已对齐旧版资源，并会打入 `.app` 包内
- Cookie、迁移数据、旧版残留资产请不要再放回 `apps/` 主运行目录
