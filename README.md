# TLS-shipinhao

微信小商店中差评订单查找与物流更新桌面工具。

**技术栈**：Rust + Tauri 2.0 + Vue 3 + Tailwind CSS v4

## 目录结构

```text
TLS-shipinhao/
├── crates/
│   ├── domain-core/              # 领域模型（OrderMatchResult、DeliveryUpdateRequest 等）
│   ├── api-contracts/            # 前后端共享契约（LicenseLease、RuntimeGrant 等）
│   ├── security-core/            # 设备指纹 + Ed25519 签名验证 + 完整性校验
│   ├── desktop-services/         # 业务逻辑（评价匹配、订单缓存、发货更新、订单同步）
│   ├── license-service/          # 授权激活 / 验证 / lease 签发
│   └── build-tools/              # 构建完整性清单生成
├── apps/
│   ├── desktop/                  # Tauri 桌面壳（commands + adapters + state）
│   └── license-worker/           # Cloudflare Worker 授权后端
├── ui/                           # Vue 3 前端（Vite + Pinia + vue-router）
│   ├── src/
│   │   ├── views/                # 6 个页面（Dashboard / Review / Order / Delivery / License / Settings）
│   │   ├── components/           # 通用 + 业务组件
│   │   ├── composables/          # useTauriInvoke / useReview / useOrder 等
│   │   ├── stores/               # Pinia 状态管理
│   │   └── types/                # TypeScript 类型定义（镜像 Rust 契约）
│   └── vite.config.ts
├── backend/                      # Cloudflare Workers 授权后端（JS）
├── xtask/                        # 构建 / manifest / 发布命令
└── .github/workflows/build.yml   # CI：测试 + Windows/macOS 打包
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
cargo tauri dev
```

### 运行测试

```bash
cargo test --workspace
pnpm --filter tls-shipinhao-ui lint
```

### 构建发布包

```bash
cargo tauri build
```

产物位于 `target/release/bundle/`（Windows NSIS / macOS DMG）。

### 后端部署

```bash
cd backend && npm install && npm run deploy
```

详见 [backend/README.md](backend/README.md)。

## 注意事项

- Cookie 信息需要定期更新（包含 `biz_magic` 值）
- CI 自动在 push 到 `main` 时构建 Windows + macOS 安装包
