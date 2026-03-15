# TLS-shipinhao 卡密后端

基于 Cloudflare Workers + D1 的卡密生成、激活、在线校验与管理后台服务。

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
- 管理后台登录页：`GET /admin`
- 管理员生成卡密：`POST /api/admin/generate`
- 管理员查看卡密列表与统计：`POST /api/admin/list`
- 管理员吊销卡密：`POST /api/admin/revoke`

## 环境要求

- Node.js 18+
- npm
- Cloudflare Wrangler 4

安装依赖：

```bash
cd backend
npm install
```

## 配置说明

Worker 入口与路由定义在 [wrangler.toml](./wrangler.toml)。

本项目依赖两个 Secret：

- `HMAC_SECRET`
  用于卡密签名校验，必须与客户端 [app/src/license.py](../app/src/license.py) 中使用的密钥保持一致。
- `ADMIN_SECRET`
  用于 `/admin` 管理后台登录和管理员接口鉴权。

客户端当前使用的后端地址定义在 [app/src/constants.py](../app/src/constants.py)：

```python
LICENSE_API_BASE_URL = "https://sphapi.199908.top"
```

## 首次部署

### 1. 创建 D1 数据库

如果还没有数据库，先创建：

```bash
npx wrangler d1 create tls-license-db
```

把命令输出里的 `database_id` 写入 [wrangler.toml](./wrangler.toml) 的 `[[d1_databases]]` 段。

### 2. 初始化数据库结构

远程初始化：

```bash
npm run db:init
```

本地初始化：

```bash
npm run db:init:local
```

### 3. 配置 Secrets

```bash
npx wrangler secret put HMAC_SECRET
npx wrangler secret put ADMIN_SECRET
```

`HMAC_SECRET` 必须与客户端一致，当前使用值为：

```text
TLS-shipinhao-2026-LicenseKey-HMAC
```

### 4. 部署

```bash
npm run deploy
```

或者直接使用 Wrangler：

```bash
npx wrangler deploy
```

部署完成后，可直接访问：

- `https://sphapi.199908.top/admin`
- `https://sphapi.199908.top/api/verify`

## 本地开发

### 方式一：使用 `.dev.vars`

在 `backend/` 目录创建 `.dev.vars`：

```dotenv
HMAC_SECRET=TLS-shipinhao-2026-LicenseKey-HMAC
ADMIN_SECRET=your-local-admin-secret
```

初始化本地数据库：

```bash
npm run db:init:local
```

启动本地服务：

```bash
npm run dev
```

### 方式二：命令行临时传入变量

如果不想创建 `.dev.vars`，可以直接用 `--var`：

```bash
npx wrangler dev \
  --local \
  --var HMAC_SECRET:TLS-shipinhao-2026-LicenseKey-HMAC \
  --var ADMIN_SECRET:your-local-admin-secret
```

### 本地访问地址

- 管理后台：`http://127.0.0.1:8787/admin`
- 激活接口：`http://127.0.0.1:8787/api/activate`
- 校验接口：`http://127.0.0.1:8787/api/verify`

Wrangler 本地状态与 D1 数据默认保存在：

```text
backend/.wrangler/
```

这个目录只用于本地开发，不应提交到 Git。

## 管理后台

管理后台使用同源接口，不开放跨域给外部站点调用。

登录后支持：

- 批量生成卡密
- 查看总量、未使用、已激活统计
- 查看最近 200 条卡密记录
- 吊销卡密并同步删除相关激活记录

管理员接口都需要在请求头中带上：

```text
X-Admin-Secret: <ADMIN_SECRET>
```

## API

### `POST /api/activate`

激活卡密并绑定设备。

请求体：

```json
{
  "key": "TLS-XXXX-XXXX-XXXX-XXXX",
  "device_id": "设备指纹哈希（16位）",
  "device_fingerprint": "原始设备信息"
}
```

成功响应：

```json
{
  "success": true,
  "message": "激活成功",
  "activated_at": "2026-03-10T00:00:00+00:00",
  "expires_at": "2026-04-09T00:00:00+00:00",
  "plan_days": 30
}
```

常见失败场景：

- 卡密不存在或已被吊销
- 卡密签名不匹配
- 卡密已绑定其他设备

### `POST /api/verify`

校验已激活卡密状态。

请求体：

```json
{
  "key": "TLS-XXXX-XXXX-XXXX-XXXX",
  "device_id": "设备指纹哈希（16位）"
}
```

成功响应：

```json
{
  "success": true,
  "message": "授权有效",
  "expires_at": "2026-04-09T00:00:00+00:00",
  "plan_days": 30,
  "activated_at": "2026-03-10T00:00:00+00:00"
}
```

### `POST /api/admin/generate`

批量生成卡密。

请求头：

```text
X-Admin-Secret: <ADMIN_SECRET>
```

请求体：

```json
{
  "count": 5,
  "plan_days": 30,
  "note": "可选备注"
}
```

### `POST /api/admin/list`

返回卡密列表和状态统计。

请求头：

```text
X-Admin-Secret: <ADMIN_SECRET>
```

请求体：

```json
{}
```

### `POST /api/admin/revoke`

吊销卡密并删除对应记录。

请求头：

```text
X-Admin-Secret: <ADMIN_SECRET>
```

请求体：

```json
{
  "key": "TLS-XXXX-XXXX-XXXX-XXXX"
}
```

## D1 数据库结构

### `activations`

记录已激活卡密与设备绑定关系。

| 字段 | 类型 | 说明 |
| --- | --- | --- |
| `id` | `INTEGER` | 自增主键 |
| `license_key` | `TEXT` | 卡密，唯一 |
| `device_id` | `TEXT` | 设备指纹哈希 |
| `device_fingerprint` | `TEXT` | 原始设备信息 |
| `plan_days` | `INTEGER` | 有效期天数 |
| `activated_at` | `TEXT` | 激活时间 |
| `expires_at` | `TEXT` | 过期时间 |
| `updated_at` | `TEXT` | 最后更新时间 |

### `generated_keys`

记录后台生成过的卡密。

| 字段 | 类型 | 说明 |
| --- | --- | --- |
| `id` | `INTEGER` | 自增主键 |
| `license_key` | `TEXT` | 卡密，唯一 |
| `plan_days` | `INTEGER` | 有效期天数 |
| `status` | `TEXT` | 状态，当前使用 `unused` / `activated` |
| `created_at` | `TEXT` | 生成时间 |
| `note` | `TEXT` | 管理备注 |

## 常用命令

```bash
# 安装依赖
npm install

# 本地开发
npm run dev

# 初始化远程 D1
npm run db:init

# 初始化本地 D1
npm run db:init:local

# 部署到 Cloudflare
npm run deploy
```
