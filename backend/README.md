# TLS-shipinhao 卡密验证后端

基于 Cloudflare Workers + D1 的卡密在线验证服务。

## 部署步骤

### 1. 安装依赖

```bash
cd backend
npm install
```

### 2. 创建 D1 数据库

```bash
npx wrangler d1 create tls-license-db
```

命令输出会包含 `database_id`，将其填入 `wrangler.toml` 中的 `database_id` 字段。

### 3. 初始化数据库表

```bash
# 远程
npm run db:init

# 本地开发
npm run db:init:local
```

### 4. 设置 HMAC 密钥

```bash
npx wrangler secret put HMAC_SECRET
```

输入值必须与客户端 `license.py` 中的 `_SECRET` 一致：
```
TLS-shipinhao-2026-LicenseKey-HMAC
```

### 5. 部署

```bash
npm run deploy
```

部署成功后将输出 Workers URL，格式为：
```
https://tls-shipinhao-license-api.<your-subdomain>.workers.dev
```

将此 URL 更新到客户端 `src/constants.py` 的 `LICENSE_API_BASE_URL`。

### 6. 本地开发

```bash
npm run dev
```

## API 接口

### POST /api/activate

激活卡密，绑定设备。

**请求体：**
```json
{
  "key": "TLS-XXXX-XXXX-XXXX-XXXX",
  "device_id": "设备指纹哈希（16位）",
  "device_fingerprint": "原始设备信息"
}
```

**成功响应：**
```json
{
  "success": true,
  "message": "激活成功",
  "activated_at": "2026-03-10T00:00:00+00:00",
  "expires_at": "2026-04-09T00:00:00+00:00",
  "plan_days": 30
}
```

**失败响应（设备冲突）：**
```json
{
  "success": false,
  "message": "该卡密已在其他设备激活，不允许更换设备。如需帮助请联系作者。"
}
```

### POST /api/verify

验证已激活的卡密状态。

**请求体：**
```json
{
  "key": "TLS-XXXX-XXXX-XXXX-XXXX",
  "device_id": "设备指纹哈希（16位）"
}
```

**成功响应：**
```json
{
  "success": true,
  "message": "授权有效",
  "expires_at": "2026-04-09T00:00:00+00:00",
  "plan_days": 30,
  "activated_at": "2026-03-10T00:00:00+00:00"
}
```

## D1 数据库结构

| 字段               | 类型    | 说明               |
|--------------------|---------|--------------------|
| id                 | INTEGER | 自增主键           |
| license_key        | TEXT    | 卡密（唯一）       |
| device_id          | TEXT    | 设备指纹哈希       |
| device_fingerprint | TEXT    | 原始设备信息       |
| plan_days          | INTEGER | 有效期天数         |
| activated_at       | TEXT    | 激活时间（ISO）    |
| expires_at         | TEXT    | 过期时间（ISO）    |
| updated_at         | TEXT    | 最后更新时间（ISO）|
