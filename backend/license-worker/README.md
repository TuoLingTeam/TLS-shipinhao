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
- `POST /api/activate`
  - Cloudflare 路径已接入 D1 查询/写入
  - 激活成功后返回真实签名 Lease Token
- `POST /api/verify`
  - Cloudflare 路径已接入 D1 校验
  - 校验成功后返回真实签名 Lease Token
- `POST /api/lease/refresh`
  - Cloudflare 路径已接入 D1 查询 + `LICENSE_SIGNING_PRIVATE_KEY_B64`
  - 返回真实签名 `new_token`
- `POST /api/task/authorize`
  - Cloudflare 路径已接入 D1 查询
  - 返回真实 `RuntimeGrant`

## 当前 Worker 授权架构

- `handle_async_runtime_json()` 统一路由：
  - `activate`
  - `verify`
  - `lease_refresh`
  - `task_authorize`
- `runtime_activate()` / `runtime_verify()` / `runtime_refresh_lease()` / `runtime_task_authorize()`
  共享同一套运行时授权逻辑
- `D1RuntimeRepo` 作为 Cloudflare 运行时实现，负责把 D1 表结构映射到异步仓库接口
- 单元测试中的内存仓库也实现相同 `AsyncRuntimeRepository`，用于验证统一路径

## 当前边界

- Cloudflare 线上路径已改为 `D1RuntimeRepo + handle_async_runtime_json()` 统一路径
- 本 crate 内的测试入口也已切换到异步主路径，不再依赖旧的同步兼容入口
- `/api/lease/revoke` 仍保留占位响应，管理员吊销继续走 `/api/admin/*`
