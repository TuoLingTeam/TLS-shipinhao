# license-worker（Rust 源码）

Cloudflare Worker 的 **Rust 实现**位于本目录；**生产部署请勿在本目录执行 `wrangler deploy`**。

## 生产部署（唯一入口）

在仓库 **`backend/`** 目录使用 `backend/wrangler.toml`：

```bash
cd backend
npx wrangler secret put ADMIN_SECRET
npx wrangler deploy
```

管理页 HTML 的**唯一源文件**为 `backend/src/admin/admin.html`（编译期嵌入 Worker）。

## 仅本地调试本 crate

```bash
cd backend/license-worker
npx wrangler dev
```

（本目录 `wrangler.toml` **不配置** `routes`，避免误占用线上域名。）

## 已实现能力

- `GET /admin`、`POST /api/admin/list|generate|revoke`（D1 + `ADMIN_SECRET`），见源码 `src/admin_d1.rs`。
- `POST /api/lease/refresh`
  - Cloudflare 路径已接入 D1 查询 + `LICENSE_SIGNING_PRIVATE_KEY_B64`
  - 返回真实签名 `new_token`
- `POST /api/task/authorize`
  - Cloudflare 路径已接入 D1 查询
  - 返回真实 `RuntimeGrant`

## 当前仍未完成

- `/api/activate`、`/api/verify` 的 Cloudflare D1 运行时仍返回 `rust_worker_repository_pending`
- 原因：`license-service::LicenseRepository` 目前是同步 trait，而 Worker D1 API 是 async；
  本轮只先打通 refresh / task authorize 两条联调链路
