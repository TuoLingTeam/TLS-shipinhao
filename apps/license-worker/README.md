# license worker shell

Cloudflare Rust Worker 部署入口位于本目录。

## 本地开发

```bash
cd apps/license-worker
npx wrangler dev
```

## 部署

```bash
cd apps/license-worker
npx wrangler secret put HMAC_SECRET
npx wrangler secret put ADMIN_SECRET
npx wrangler secret put LICENSE_SIGNING_PRIVATE_KEY_B64
npx wrangler deploy
```

当前 Worker 入口已经切换到 Rust `fetch` 事件与 `license-service` 主逻辑适配层，旧 `backend/src/worker/index.js` 仅保留兼容提示壳。

## 管理员 API（D1）

已实现（需部署本 Worker 且 D1 已按 `backend/db/schema.sql` 初始化）：

- `GET /admin`：内嵌 `backend/src/admin/admin.html`
- `POST /api/admin/list`：统计 + 卡密列表（`X-Admin-Secret` 与 `ADMIN_SECRET` 一致）
- `POST /api/admin/generate`：批量写入 `generated_keys`
- `POST /api/admin/revoke`：将 `generated_keys.status` 置为 `revoked`

`/api/activate`、`/api/verify` 仍返回 `rust_worker_repository_pending`，下一迭代可接入 D1 版 `LicenseRepository`。
