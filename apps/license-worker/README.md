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
