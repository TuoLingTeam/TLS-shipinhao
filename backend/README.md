# TLS-shipinhao 卡密后端

**正式部署入口为本目录**：在 `backend/` 执行 `npx wrangler deploy`（`wrangler.toml` 会编译 `apps/license-worker` 并指向其 `build/worker/shim.mjs`）。

- **管理后台页面**唯一源文件：`backend/src/admin/admin.html`（由 Rust Worker 在编译期 `include_str!` 嵌入，勿在 `apps/license-worker` 再复制一份正文）。
- **D1 schema / 迁移**：`backend/db/`。
- **遗留 JS 壳**：`backend/src/worker/index.js` 仅作本地对照，生产路由不应再以它为 `main`。

当前线上路由：

```text
https://sphapi.199908.top
```

管理后台入口：

```text
https://sphapi.199908.top/admin
```

## 功能概览

- 客户端激活卡密：`POST /api/activate`
- 客户端校验卡密：`POST /api/verify`
- 客户端申请任务会话：`POST /api/session/issue`
- 客户端刷新任务会话：`POST /api/session/refresh`
- 管理后台登录页：`GET /admin`
- 管理员生成卡密：`POST /api/admin/generate`
- 管理员查看卡密列表与统计：`POST /api/admin/list`
- 管理员吊销卡密：`POST /api/admin/revoke`

**说明**：若线上仍绑定旧版 **仅 `index.js` 壳** 的 Worker，则除静态页外会 **HTTP 410**。请改用本目录 `wrangler.toml` 部署 **Rust Worker**；管理员接口由 `apps/license-worker` 内 D1 逻辑提供（需配置 `ADMIN_SECRET` 与 D1）。
- 管理员重置设备绑定：`POST /api/admin/device/rebind`
- 管理员吊销短期会话：`POST /api/admin/device/revoke_sessions`
- 管理员查看授权审计：`POST /api/admin/audit/list`

## 配置说明

本项目依赖三个 Secret：

- `HMAC_SECRET`
  用于旧卡密格式签名校验。
- `ADMIN_SECRET`
  用于 `/admin` 管理后台登录和管理员接口鉴权。
- `LICENSE_SIGNING_PRIVATE_KEY_B64`
  Ed25519 私钥（Base64 编码的 PKCS8 PEM 文本），用于签发 `device_claims`、`offline_grant`、`session_token`。

客户端内置的公钥位于 [backup/legacy-src/app/settings.py](../backup/legacy-src/app/settings.py) 的 `LICENSE_PUBLIC_KEY`。
如需轮换密钥，请同时更新客户端公钥与后端私钥。

## 首次部署

```bash
cd backend
npx wrangler secret put HMAC_SECRET
npx wrangler secret put ADMIN_SECRET
npx wrangler secret put LICENSE_SIGNING_PRIVATE_KEY_B64
npx wrangler deploy
```

（`wrangler` 会在部署前执行 `[build]`，在 `apps/license-worker` 内运行 `worker-build`。）

## 线上升级到授权协议 V2

### 1. 先执行 D1 迁移

```bash
cd backend
npx wrangler d1 execute tls-license-db --remote --file=./db/migrations/20260415_license_v2.sql
```

### 2. 配置或轮换授权签名私钥

```bash
cd backend
npx wrangler secret put LICENSE_SIGNING_PRIVATE_KEY_B64
```

写入值应为 **Base64 编码后的 PKCS8 PEM 文本**，不要直接提交到仓库。

### 3. 重新部署 Worker

```bash
cd backend
npx wrangler deploy
```

### 4. 升级后客户端迁移行为

- 旧卡密：继续有效
- 旧本地 `license.json`：不再受信任
- 用户首次打开新客户端时：必须联网重新激活一次
- 之后高价值任务会在启动前申请 `session_token`，若网络不可用则拒绝启动

## 本地开发

在 `backend/` 目录创建 `.dev.vars`：

```dotenv
HMAC_SECRET=<旧卡密签名密钥>
ADMIN_SECRET=<本地管理密码>
LICENSE_SIGNING_PRIVATE_KEY_B64=<Ed25519 私钥 Base64>
```

## 客户端 API

### `POST /api/activate`

请求体：

```json
{
  "key": "TLS-XXXX-XXXX-XXXX-XXXX",
  "device_id": "设备指纹哈希（16位）",
  "device_fingerprint": "原始设备信息",
  "client_version": "4.3.0",
  "platform": "darwin",
  "build_channel": "desktop"
}
```

成功响应会返回：
- `device_claims`
- `offline_grant`
- `session_token`
- 各自的过期时间
- `license_version=2`

### `POST /api/verify`

用于在线刷新授权状态与离线票据。

请求体：

```json
{
  "key": "TLS-XXXX-XXXX-XXXX-XXXX",
  "device_id": "设备指纹哈希（16位）",
  "license_version": 2,
  "session_id": "可选，当前短期会话 ID",
  "client_version": "4.3.0"
}
```

### `POST /api/session/issue`

任务启动前申请短期任务令牌。

请求体：

```json
{
  "license_key": "TLS-XXXX-XXXX-XXXX-XXXX",
  "device_id": "设备指纹哈希（16位）",
  "device_claims": "<服务端签发票据>",
  "task_type": "review_find",
  "client_version": "4.3.0"
}
```

### `POST /api/session/refresh`

长任务中刷新短期令牌。

请求体：

```json
{
  "license_key": "TLS-XXXX-XXXX-XXXX-XXXX",
  "device_id": "设备指纹哈希（16位）",
  "session_token": "<旧短期令牌>",
  "task_type": "review_find"
}
```

## D1 数据表

- `activations`：设备绑定、授权总状态、最近签发时间
- `generated_keys`：后台生成过的卡密
- `device_sessions`：短期任务令牌
- `device_registrations`：设备注册记录
- `license_audit_logs`：审计日志

## 迁移说明

- 旧卡密仍可继续使用
- 旧客户端的本地 `license.json` 不再受信任
- 用户升级到新客户端后，必须联网重新激活一次，生成 V2 票据
