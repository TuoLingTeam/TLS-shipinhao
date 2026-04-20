# TLS-shipinhao 卡密后端

**正式部署入口为本目录**：在 `backend/` 执行 `npx wrangler deploy`（`wrangler.toml` 会编译 `backend/license-worker` 并指向其 `license-worker/build/worker/shim.mjs`）。

- **管理后台页面**唯一源文件：`backend/license-worker/assets/admin.html`（由 Rust Worker 在编译期 `include_str!` 嵌入）。
- **D1 schema / 迁移**：`backend/db/`。
- **遗留 JS 壳**：已从仓库移除，当前仅保留 Rust Worker 作为正式实现。

当前线上路由：

```text
https://sphapi.199908.top
```

管理后台入口：

```text
https://sphapi.199908.top/admin
```

## 功能概览

运行时协议全部由 `backend/license-worker/src/messages.rs` 定义，下述 5 条客户端路由 + 4 条管理端路由就是全部对外表面：

### 客户端路由

- 激活卡密：`POST /api/activate`
- 校验授权：`POST /api/verify`
- 续约 Lease：`POST /api/lease/refresh`
- 管理员吊销 Lease：`POST /api/lease/revoke`（要求 `X-Admin-Secret`）
- 任务级授权：`POST /api/task/authorize`

### 管理端路由

- 管理后台登录页：`GET /admin`
- 查看卡密列表与统计：`POST /api/admin/list`
- 生成卡密：`POST /api/admin/generate`
- 吊销卡密：`POST /api/admin/revoke`

管理端路由全部要求 `X-Admin-Secret: <ADMIN_SECRET>` 请求头；具体鉴权逻辑见 `backend/license-worker/src/admin.rs`。

## 配置说明

本项目使用两个 Secret（通过 `npx wrangler secret put` 写入）：

- `ADMIN_SECRET`
  用于 `/admin` 管理后台登录和所有管理员接口鉴权。
- `LICENSE_SIGNING_PRIVATE_KEY_B64`
  Ed25519 私钥，Base64 编码的 PKCS8 DER（或带 `-----BEGIN PRIVATE KEY-----` 的 PEM 文本）。Worker 用它对 `LeasePayload` 做 Ed25519 签名，签发客户端用的 Lease Token。

> **历史兼容**：仓库早期版本还声明过 `HMAC_SECRET`，在当前 Rust Worker 代码中未被读取；出于安全回滚考虑**不主动清理**现有 Cloudflare Secret 值，但新部署不再需要设置它。

客户端内置的验签公钥常量在 `backend/license-service/src/service.rs` 的 `LICENSE_PUBLIC_KEY_B64`。**轮换签名私钥前，必须同步更新该常量与所有已发布客户端**，否则旧设备的 Lease Token 将全部进入 `LicenseState::Invalid`，被迫重新激活。

## 首次部署

```bash
cd backend
npx wrangler secret put ADMIN_SECRET
npx wrangler secret put LICENSE_SIGNING_PRIVATE_KEY_B64
npx wrangler deploy
```

（`wrangler` 会在部署前执行 `[build]`，在 `backend/license-worker` 内运行 `worker-build`。）

## 线上升级到授权协议 V2

> 当前已全部运行在协议 V2（`LICENSE_PROTOCOL_VERSION = 3`）下。以下步骤仅作为历史记录保留。

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

写入值应为 **Base64 编码后的 PKCS8 DER**（或 `-----BEGIN PRIVATE KEY-----` PEM 文本的 Base64），**不要直接提交到仓库**。

### 3. 重新部署 Worker

```bash
cd backend
npx wrangler deploy
```

## 本地开发

在 `backend/` 目录创建 `.dev.vars`：

```dotenv
ADMIN_SECRET=<本地管理密码>
LICENSE_SIGNING_PRIVATE_KEY_B64=<Ed25519 私钥 Base64>
```

## 客户端 API

所有请求/响应均为 JSON，字段名使用 `snake_case`。字段定义位于 `backend/license-worker/src/messages.rs` 与 `backend/license-service/src/model.rs`。

### `POST /api/activate`

首次激活：把卡密与设备绑定，成功后返回已签名的 Lease Token。

请求体（`ActivationInput`）：

```json
{
  "license_key": "TLS-XXXXXXXXXXXXXXXX",
  "device_id": "设备指纹哈希（16 位 hex）",
  "device_fingerprint": "原始设备指纹",
  "client_version": "5.1.0"
}
```

成功响应（`SignedLicenseApiResponse`）：

```json
{
  "success": true,
  "message": "激活成功",
  "license_state": "active",
  "license_lease": "<base64url(payload).base64url(signature)>",
  "license_expires_at": "2026-05-16T00:00:00Z",
  "activated_at": "2026-04-16T00:00:00Z",
  "device_id": "...",
  "license_key": "TLS-...",
  "lease_expires_at": "2026-04-19T00:00:00Z",
  "renew_after": "2026-04-17T00:00:00Z",
  "issued_at": "2026-04-16T00:00:00Z",
  "license_status": "active",
  "task_policy": ["review_find", "review_full_scan", "quality_refund", "batch_delivery", "cache_manage"]
}
```

### `POST /api/verify`

在线复检，返回新的 Lease Token。

请求体（`VerifyInput`）：

```json
{
  "license_key": "TLS-XXXXXXXXXXXXXXXX",
  "device_id": "设备指纹哈希（16 位 hex）",
  "client_version": "5.1.0"
}
```

响应与 `/api/activate` 一致。

### `POST /api/lease/refresh`

Lease 进入软刷新窗口（`now >= renew_after`）后调用；硬过期（`now >= exp`）只能走 `/api/verify`。

请求体（`LeaseRefreshRequest`）：

```json
{
  "license_key": "TLS-XXXXXXXXXXXXXXXX",
  "device_id": "设备指纹哈希（16 位 hex）",
  "current_issued_at": 1718000000
}
```

响应（`LeaseRefreshResponse`）：

```json
{
  "success": true,
  "message": "lease_refreshed",
  "new_token": "<base64url(payload).base64url(signature)>"
}
```

### `POST /api/lease/revoke`

管理员在客户端侧吊销某个 `license_key`。**必须**携带 `X-Admin-Secret: <ADMIN_SECRET>` 头。

请求体（`LeaseRevokeRequest`）：

```json
{
  "license_key": "TLS-XXXXXXXXXXXXXXXX",
  "device_id": "设备指纹哈希（16 位 hex）",
  "reason": "admin_revoke"
}
```

响应为 `SignedLicenseApiResponse`，`license_state = "revoked"`。

### `POST /api/task/authorize`

高风险任务启动前申请 `RuntimeGrant`。

请求体（`TaskAuthorizeRequest`）：

```json
{
  "license_key": "TLS-XXXXXXXXXXXXXXXX",
  "device_id": "设备指纹哈希（16 位 hex）",
  "task_type": "review_find",
  "client_version": "5.1.0"
}
```

响应（`RuntimeGrant`）：

```json
{
  "task_type": "review_find",
  "granted": true,
  "grant_id": "worker-grant-1718000000000-1",
  "valid_until": "2026-04-16T00:30:00Z",
  "risk_level": "low",
  "degraded_reason": null
}
```

支持的 `task_type` 白名单在 `backend/api-contracts/src/lib.rs` 的 `SUPPORTED_TASKS`：`review_find` / `review_full_scan` / `quality_refund` / `batch_delivery` / `cache_manage`。

## 管理员 API

全部要求 `X-Admin-Secret: <ADMIN_SECRET>` 请求头。

### `POST /api/admin/list`

无请求体。返回 `generated_keys` 按状态分组统计 + 全量卡密清单（LEFT JOIN `activations`，按创建时间倒序）。

### `POST /api/admin/generate`

```json
{
  "count": 10,
  "plan_days": 30,
  "note": "可选备注"
}
```

`count` 会被钳制到 `[1, 100]`。响应包含生成的 `keys` 数组。

### `POST /api/admin/revoke`

```json
{
  "key": "TLS-XXXXXXXXXXXXXXXX"
}
```

注意：管理端使用 `key` 字段名（历史协议），客户端 API 统一使用 `license_key`。

## D1 数据表

- `activations`：设备绑定、授权总状态、Lease 过期时间
- `generated_keys`：后台生成过的卡密
- `device_sessions`：历史遗留的短期任务令牌表（当前运行时仅在吊销时做副作用写入，线上保留以供后续兼容或审计查询）
- `device_registrations`：设备注册记录（激活与吊销时被写入，审计用）
- `license_audit_logs`：授权动作审计日志

## 迁移说明

- 旧卡密继续有效
- 升级到 V2 后的客户端必须联网重新激活一次，才能换取 V2 的 Lease Token
- 轮换签名私钥 = 旧客户端全部需要重新激活（在客户端公钥常量与 Worker secret **同步更新**前，不要执行私钥轮换）
