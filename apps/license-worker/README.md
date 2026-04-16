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
cd apps/license-worker
npx wrangler dev
```

（本目录 `wrangler.toml` **不配置** `routes`，避免误占用线上域名。）

## 已实现能力

- `GET /admin`、`POST /api/admin/list|generate|revoke`（D1 + `ADMIN_SECRET`），见源码 `src/admin_d1.rs`。
- `/api/activate`、`/api/verify` 仍返回 `rust_worker_repository_pending`，待接 D1 版 `LicenseRepository`。
