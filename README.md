# TLS-shipinhao

微信小商店中差评订单查找与物流更新桌面工具。

**技术栈**：Rust + Tauri 2.0 + Vue 3 + Tailwind CSS v4

## 目录结构

```text
TLS-shipinhao/
├── apps/
│   ├── desktop/                  # Tauri 桌面壳（commands + adapters + state）
│   ├── ui/                       # Vue 3 前端（Vite + Pinia + vue-router）
│   │   ├── src/
│   │   ├── package.json
│   │   └── vite.config.ts
│   ├── license-worker/           # Rust Cloudflare Worker 授权服务
│   ├── legacy-src/               # 兼容期 Python 源码快照
│   ├── legacy-runtime/           # 兼容期 Python 运行时残留
│   └── legacy-dist/              # 兼容期 Python 编译产物
├── backend/
│   ├── crates/                   # Rust 共享库与核心业务
│   ├── docs/                     # 验收、性能、发布文档
│   ├── src/                      # Worker 管理页与兼容壳
│   ├── tests/                    # Rust / Python 回归测试与夹具
│   ├── xtask/                    # 构建 / manifest / 发布命令
│   └── db/                       # D1 schema / migration
├── Cargo.toml                    # 顶层 Rust workspace
├── openspec/                     # 变更提案与迁移设计
└── .github/workflows/            # CI/CD
```

## 功能特性

- **中差评订单匹配**：多属性评分算法，自动关联差评与订单
- **订单缓存同步**：SQLite 本地缓存，支持全量/增量同步
- **批量发货更新**：一键更新物流信息，支持快递单号校验
- **卡密授权管理**：Ed25519 签名 lease，设备绑定 + 离线宽限

## 环境要求

- Rust stable（推荐 1.80+）
- Node.js 22+
- pnpm 10+
- Tauri CLI 2.x（`cargo install tauri-cli --version "^2"`）

## 快速开始

### 开发模式

```bash
pnpm install
pnpm tauri:dev
```

### 运行测试

```bash
cargo test --workspace
pnpm --filter tls-shipinhao-ui lint
```

### 构建发布包

```bash
pnpm tauri:build
```

产物位于 `target/release/bundle/`（Windows NSIS / macOS DMG）。

### 后端部署

```bash
cd backend && npm install && npm run deploy
```

详见 [backend/README.md](backend/README.md) 与 [apps/license-worker/README.md](apps/license-worker/README.md)。

## 注意事项

- Cookie 信息需要定期更新（包含 `biz_magic` 值）
- CI 自动在 push 到 `main` 时构建 Windows + macOS 安装包
