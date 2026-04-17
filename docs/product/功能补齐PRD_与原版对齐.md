# TLS-shipinhao 功能补齐 PRD — 与原版 Python 4.3.0 完全对齐

> **文档类型**：Product Requirements Document  
> **目标版本**：`5.1.0`（基于 Rust + Vue 技术栈）  
> **对齐版本**：Python `4.3.0`  
> **文档状态**：Final Draft  
> **生成时间**：2026-04-16

---

## 目录

1. [文档目标与原则](#一文档目标与原则)
2. [用户价值](#二用户价值)
3. [交付目标总览](#三交付目标总览)
4. [产品品牌与身份](#四产品品牌与身份)
5. [授权与安全模块](#五授权与安全模块prd)
6. [订单同步模块](#六订单同步模块prd)
7. [评价匹配模块](#七评价匹配模块prd)
8. [发货管理模块](#八发货管理模块prd)
9. [Cookie 与配置模块](#九cookie-与配置模块prd)
10. [在线更新模块](#十在线更新模块prd)
11. [UI 规范](#十一ui-规范)
12. [Tauri IPC 契约](#十二tauri-ipc-契约)
13. [数据模型](#十三数据模型)
14. [反风控策略](#十四反风控策略)
15. [迁移方案](#十五迁移方案)
16. [验收标准](#十六验收标准)
17. [工作分解与排期](#十七工作分解与排期)
18. [风险与假设](#十八风险与假设)

---

## 一、文档目标与原则

### 1.1 一句话目标

> 将当前 Rust 重构版 `5.0.0` 补齐到与 Python 原版 `4.3.0` **功能 100% 对等**的水平，使现有用户迁移过来**不感知任何功能减损**，只感受到**启动更快、UI 更现代**。

### 1.2 核心原则

| 原则 | 说明 |
|---|---|
| **行为兼容** | 所有原版业务行为必须 1:1 还原（API 参数、请求头、错误处理、降级策略） |
| **数据兼容** | 用户从 Python 版迁移过来，本地缓存数据应自动导入，不需要重新同步 |
| **体验升级** | 在行为一致的前提下，UI 用 Vue 3 + Tailwind 提升视觉，但保留品牌色（翠绿） |
| **技术栈** | 主进程 Rust（Tauri 2） + 前端 Vue 3；授权服务 Cloudflare Worker（Rust WASM） |
| **非需求** | 新增功能不在本 PRD 范围内；本文档只谈"补齐" |

### 1.3 成功指标

- [ ] Python 原版识别的 **20+ 功能点** 在 Rust 版全部可用
- [ ] 老用户升级后能直接使用，**无需重新同步订单、无需重新激活**
- [ ] 反风控策略测试通过（模拟 429/430 响应）
- [ ] 安装包体积 < 30 MB
- [ ] 冷启动时间 < 2 秒
- [ ] **回归测试**：使用原 Python 版的真实用户数据作为测试样本，匹配结果一致

---

## 二、用户价值

### 2.1 目标用户

视频号小店**卖家、运营、客服**，现有 Python 版 4.3.0 用户。

### 2.2 用户 Why

- **"我已经用惯了 4.3.0 的功能，升级到 5.0 不能退化"**
- **"差评匹配准不准关系到我能否及时回复客户"**
- **"发货操作不能出错，否则我要赔客户运费"**
- **"授权不能老是失效，影响我做生意"**
- **"我不想被微信封号"**

### 2.3 用户 Journey 对齐

升级后用户的完整体验必须与原版一致：

```
启动 App
  → 自动迁移旧版缓存（~/.tls-shipinhao/order_cache.sqlite3）
  → 自动沿用原卡密（从旧版 ~/.tls-shipinhao/license.json 读取）
  → 自动识别现有 Cookie（cookie.txt）
  → 显示同样的翠绿主题界面
  → 功能操作路径与旧版一致
```

---

## 三、交付目标总览

### 3.1 模块覆盖矩阵

| 模块 | Python 功能点 | Rust 现状 | 补齐工作量 |
|---|---|---|---|
| 授权与安全 | 12 | 3 | **9 项** |
| 订单同步 | 15 | 6 | **9 项** |
| 评价匹配 | 14 | 6 | **8 项** |
| 发货管理 | 8 | 5 | **3 项** |
| Cookie 管理 | 6 | 5 | **1 项** |
| 在线更新 | 3 | 0 | **3 项** |
| UI/UX | 8 | 3 | **5 项** |
| **合计** | **66** | **28** | **38 项** |

### 3.2 里程碑

| 里程碑 | 目标 | 时间 |
|---|---|---|
| **M1 反风控** | 限流重试 + 风控降级 + UA 平台化 | Week 1-3 |
| **M2 授权安全** | Lease + 设备指纹 + Keychain + 完整性 | Week 4-7 |
| **M3 数据兼容** | 缓存 Schema 对齐 + 迁移脚本 + dirty 检测 | Week 8-10 |
| **M4 业务细节** | 智能昵称 + 策略分级 + 快递降级 + 全量扫描 | Week 11-13 |
| **M5 UI 还原** | 翠绿主题 + UI 缩放 + 多布局 + 更新检查 | Week 14-16 |
| **M6 回归测试** | 用真实数据跑对比测试 | Week 17-18 |

---

## 四、产品品牌与身份

### 4.1 品牌信息（必须还原）

| 项 | Python 原版 | Rust 当前 | 要求 |
|---|---|---|---|
| 产品名 | 驼铃·视频小店差评处理 | TLS-shipinhao | **改回** |
| 窗口标题 | 驼铃·视频小店差评处理 5.x.x | TLS-shipinhao | **改回** |
| 作者微信 | TLS-801 | 无 | **显示** |
| 图标 | 翠绿驼铃 | 默认 | **沿用** |
| 标识符 | 沿用 `com.tls.shipinhao` | `com.tls.shipinhao` | ✅ 保留 |

### 4.2 应用信息常量（Rust 实现）

```rust
// crates/domain-core/src/brand.rs
pub const APP_NAME: &str = "驼铃·视频小店差评处理";
pub const APP_NAME_EN: &str = "TLS-shipinhao";
pub const APP_VERSION: &str = env!("CARGO_PKG_VERSION");
pub const AUTHOR_WECHAT: &str = "TLS-801";
pub const WINDOW_TITLE_TEMPLATE: &str = "{app_name} {version}";

pub fn get_window_title() -> String {
    format!("{} {}", APP_NAME, APP_VERSION)
}
```

```typescript
// ui/src/constants/brand.ts
export const BRAND = {
  appName: '驼铃·视频小店差评处理',
  appNameEn: 'TLS-shipinhao',
  authorWechat: 'TLS-801',
  version: APP_VERSION, // 从 tauri 读取
} as const;
```

---

## 五、授权与安全模块 PRD

### 5.1 模块目标

建立与 Python 版**完全一致**的授权协议（Protocol Version 3），包括多域名容灾、Lease 租约、任务级授权、设备指纹、Keychain 存储、完整性校验。

### 5.2 授权协议 v3 配置常量

```rust
// crates/license-service/src/config.rs
pub const LICENSE_API_BASE_URLS: &[&str] = &[
    "https://sphapi.199908.top",
    "https://sphapi.tuoling.ccwu.cc",
    "https://sphapi.tuoling.us.ci",
    "https://sphapi.tuoling.eu.cc",
];
pub const LICENSE_API_TIMEOUT_SECS: u64 = 10;
pub const LICENSE_STATUS_CACHE_TTL_SECS: u64 = 60;
pub const LICENSE_PROTOCOL_VERSION: u32 = 3;
pub const LICENSE_LEASE_RENEWAL_HOURS: i64 = 24;
pub const LICENSE_LEASE_HARD_EXPIRY_HOURS: i64 = 72;
pub const LICENSE_RUNTIME_GRANT_MINUTES: i64 = 30;
pub const LICENSE_REQUIRE_ONLINE_FOR_TASKS: bool = true;
pub const LICENSE_PUBLIC_KEY_B64: &str = "H0KTidHIXV0nvzkUNmssrx5t5IrUvEQi1WVelkuCJm8";
pub const INTEGRITY_MANIFEST_FILE_NAME: &str = "integrity_manifest.json";

pub const LICENSE_TASK_REVIEW_FIND: &str = "review_find";
pub const LICENSE_TASK_REVIEW_FULL_SCAN: &str = "review_full_scan";
pub const LICENSE_TASK_QUALITY_REFUND: &str = "quality_refund";
pub const LICENSE_TASK_BATCH_DELIVERY: &str = "batch_delivery";
pub const LICENSE_TASK_CACHE_MANAGE: &str = "cache_manage";
```

### 5.3 授权状态机

**完整状态常量**（与 Python 对齐）：

```rust
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum LicenseReason {
    Ok,
    NotFound,
    Invalid,
    Expired,
    DeviceMismatch,
    ReactivationRequired,
    Revoked,
    OnlineRefreshRequired,
    RenewalDue,
    Compromised,
}
```

**允许本地使用的状态**：`Ok` + `RenewalDue`（与 Python 的 `_ALLOWED_LOCAL_REASONS` 一致）

**前端状态映射**：

```typescript
// ui/src/types/license.ts
export type LicenseState =
  | 'ok'
  | 'not_found'
  | 'invalid'
  | 'expired'
  | 'device_mismatch'
  | 'reactivation_required'
  | 'revoked'
  | 'online_refresh_required'
  | 'renewal_due'
  | 'compromised';

export const LICENSE_STATE_LABELS: Record<LicenseState, string> = {
  ok: '已授权',
  not_found: '未激活',
  invalid: '卡密无效',
  expired: '已过期',
  device_mismatch: '设备不匹配',
  reactivation_required: '需要重新激活',
  revoked: '已吊销',
  online_refresh_required: '需要联网续约',
  renewal_due: '待续期',
  compromised: '完整性异常',
};
```

### 5.4 功能 #1：多域名容灾 ⭐

#### 5.4.1 用户故事

**作为** 用户  
**我希望** 当主域名访问不畅时系统自动切换到备用域名  
**所以** 授权服务不会因单点故障而中断

#### 5.4.2 行为规约

1. 激活、校验、续约等所有接口调用，按 `LICENSE_API_BASE_URLS` 顺序尝试
2. 每个域名超时 10 秒
3. 连接/超时错误（网络级）→ 自动切换下一个
4. HTTP 4xx/5xx（业务级）→ 不切换，直接返回错误
5. 所有域名都失败 → 返回网络错误

#### 5.4.3 Rust 实现

```rust
// crates/license-service/src/http_client.rs
pub struct MultiDomainClient {
    client: reqwest::Client,
    bases: &'static [&'static str],
}

impl MultiDomainClient {
    pub async fn post_json<T: Serialize, R: DeserializeOwned>(
        &self,
        path: &str,
        body: &T,
    ) -> Result<R, LicenseError> {
        let mut last_network_err = None;
        for base in self.bases {
            let url = format!("{}{}", base, path);
            match self.client
                .post(&url)
                .json(body)
                .timeout(Duration::from_secs(LICENSE_API_TIMEOUT_SECS))
                .send()
                .await
            {
                Ok(resp) if resp.status().is_success() => {
                    return resp.json().await.map_err(Into::into);
                }
                Ok(resp) => {
                    return Err(LicenseError::HttpError(resp.status()));
                }
                Err(e) if e.is_timeout() || e.is_connect() => {
                    tracing::warn!("授权域名 {} 访问失败，尝试下一个: {}", base, e);
                    last_network_err = Some(e);
                    continue;
                }
                Err(e) => return Err(LicenseError::NetworkError(e.to_string())),
            }
        }
        Err(LicenseError::AllDomainsFailed(
            last_network_err.map(|e| e.to_string()).unwrap_or_default()
        ))
    }
}
```

#### 5.4.4 验收标准

- ✅ 域名 1 超时 → 自动切到域名 2 成功
- ✅ 4 个域名都超时 → 前端显示"网络连接失败"
- ✅ 域名 1 返回 401 → 直接返回"卡密无效"（不切换）
- ✅ 单元测试模拟各场景覆盖

---

### 5.5 功能 #2：Lease 租约机制 ⭐⭐⭐

#### 5.5.1 核心概念

**Lease**（租约）是一个由服务端签发的、带 Ed25519 签名的凭证，格式：

```
<base64url(payload)>.<base64url(signature)>

payload = {
  "kind": "license_lease",
  "license_key": "xxxx-xxxx",
  "device_id": "abc123...",
  "issued_at": 1730000000,
  "exp": 1730172800,           // 硬过期（72h）
  "renew_after": 1730086400,   // 软刷新（24h）
  "task_policy": ["review_find", "batch_delivery", ...],
  "risk_level": "low"
}
```

#### 5.5.2 生命周期

```
[Activate] ──────► 服务端签发 Lease ─────► 存入 Keychain
    │                                          │
    │                                          ▼
    │                                    [读取 Lease]
    │                                          │
    │                                          ▼
    │                               ┌─── now < renew_after ?
    │                               │       │
    │                               │       ▼
    │                               │    [继续使用，无需联网]
    │                               │
    │                               └─── renew_after < now < exp ?
    │                                       │
    │                                       ▼
    │                                  [refresh_lease_if_due]
    │                                       │
    │                                       ▼
    │                                  [签发新 Lease]
    │                                       │
    │                                       ▼
    │                                  [覆盖 Keychain]
    │
    └─── now >= exp ?
            │
            ▼
         [requires reactivation]
```

#### 5.5.3 Rust 实现规范

```rust
// crates/license-service/src/lease.rs
use ed25519_dalek::{Signature, Verifier, VerifyingKey};

#[derive(Debug, Serialize, Deserialize)]
pub struct LeasePayload {
    pub kind: String,          // "license_lease"
    pub license_key: String,
    pub device_id: String,
    pub issued_at: i64,
    pub exp: i64,
    pub renew_after: i64,
    pub task_policy: Vec<String>,
    pub risk_level: String,
}

pub struct LeaseVerifier {
    public_key: VerifyingKey,
}

impl LeaseVerifier {
    pub fn verify(
        &self,
        token: &str,
        expected_device_id: Option<&str>,
        allow_expired: bool,
    ) -> Result<LeasePayload, LeaseError> {
        let (payload_b64, sig_b64) = token.split_once('.')
            .ok_or(LeaseError::InvalidFormat)?;
        
        let signature_bytes = base64_url_decode(sig_b64)?;
        let signature = Signature::from_slice(&signature_bytes)
            .map_err(|_| LeaseError::InvalidFormat)?;
        
        self.public_key.verify(payload_b64.as_bytes(), &signature)
            .map_err(|_| LeaseError::InvalidSignature)?;
        
        let payload_bytes = base64_url_decode(payload_b64)?;
        let payload: LeasePayload = serde_json::from_slice(&payload_bytes)?;
        
        if payload.kind != "license_lease" {
            return Err(LeaseError::InvalidKind);
        }
        
        if let Some(device_id) = expected_device_id {
            if payload.device_id != device_id {
                return Err(LeaseError::DeviceMismatch);
            }
        }
        
        if !allow_expired {
            let now = chrono::Utc::now().timestamp();
            if now >= payload.exp {
                return Err(LeaseError::Expired);
            }
        }
        
        Ok(payload)
    }
}

pub async fn refresh_lease_if_due(
    current: &LeasePayload,
    client: &MultiDomainClient,
) -> Result<Option<String>, LicenseError> {
    let now = chrono::Utc::now().timestamp();
    if now < current.renew_after {
        return Ok(None);   // 还没到刷新窗口
    }
    if now >= current.exp {
        return Err(LicenseError::LeaseExpired);
    }
    
    let response: RefreshLeaseResponse = client.post_json(
        "/api/lease/refresh",
        &RefreshLeaseRequest {
            license_key: current.license_key.clone(),
            device_id: current.device_id.clone(),
            current_issued_at: current.issued_at,
        },
    ).await?;
    
    Ok(Some(response.new_token))
}
```

#### 5.5.4 Worker 端 API（需实现）

| 端点 | 方法 | 说明 |
|---|---|---|
| `/api/activate` | POST | 激活卡密，签发初始 Lease |
| `/api/verify` | POST | 校验 Lease 状态 |
| `/api/lease/refresh` | POST | 续约 Lease |
| `/api/lease/revoke` | POST | 吊销（管理员） |
| `/api/task/authorize` | POST | 任务级授权 |

#### 5.5.5 验收标准

- ✅ 首次激活成功，收到 Lease，验签通过
- ✅ 24h 内重启，无需联网直接使用
- ✅ 24h 后自动续约，用户无感
- ✅ 72h 后必须重新激活
- ✅ 篡改 Lease 签名 → 检测为 `invalid`
- ✅ 设备指纹不一致 → 检测为 `device_mismatch`

---

### 5.6 功能 #3：任务级授权 ⭐⭐

#### 5.6.1 概念

每个危险操作（差评查询、批量发货等）执行前，向服务端申请 30 分钟有效的"任务令牌"。

#### 5.6.2 支持的任务类型

```rust
pub const SUPPORTED_TASKS: &[&str] = &[
    "review_find",        // 差评查询
    "review_full_scan",   // 全量评价扫描
    "quality_refund",     // 品退查询
    "batch_delivery",     // 批量发货
    "cache_manage",       // 缓存管理
];
```

#### 5.6.3 执行流程

```
用户点击"查询差评"
  ↓
commands::review::find_reviews 被调用
  ↓
LicenseService::authorize_task("review_find")
  ├─ 检查本地 Lease 是否有效
  ├─ 检查 task_policy 是否包含 "review_find"
  ├─ 若本地足够，返回本地授权（30 分钟内缓存）
  └─ 若需要，调服务端 /api/task/authorize 申请 RuntimeGrant
  ↓
RuntimeGrant { task_type, granted: true, grant_id, valid_until }
  ↓
执行实际的 HTTP 请求（带 grant_id 头）
  ↓
完成后可选地调用 /api/task/complete 释放
```

#### 5.6.4 数据结构

```rust
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RuntimeGrant {
    pub task_type: String,
    pub granted: bool,
    pub grant_id: String,
    pub valid_until: DateTime<Utc>,
    pub risk_level: String,           // low/medium/high
    pub degraded_reason: Option<String>,
}
```

#### 5.6.5 验收标准

- ✅ 每个危险操作前都调用 `authorize_task`
- ✅ 服务端可通过拒绝 RuntimeGrant 精确限制功能
- ✅ 风险级别 `high` 时限流 3 倍

---

### 5.7 功能 #4：设备指纹 ⭐⭐⭐

#### 5.7.1 三平台实现

```rust
// crates/security-core/src/device_id.rs
use std::process::Command;

pub fn collect_raw_fingerprint() -> String {
    #[cfg(target_os = "macos")]
    return fingerprint_macos();
    
    #[cfg(target_os = "windows")]
    return fingerprint_windows();
    
    #[cfg(target_os = "linux")]
    return fingerprint_linux();
}

#[cfg(target_os = "macos")]
fn fingerprint_macos() -> String {
    match Command::new("ioreg")
        .args(&["-rd1", "-c", "IOPlatformExpertDevice"])
        .output()
    {
        Ok(output) => {
            let stdout = String::from_utf8_lossy(&output.stdout);
            for line in stdout.lines() {
                if line.contains("IOPlatformSerialNumber") {
                    if let Some(value) = line.split('=').nth(1) {
                        return value.trim().trim_matches('"').to_string();
                    }
                }
            }
            fallback_fingerprint()
        }
        Err(_) => fallback_fingerprint(),
    }
}

#[cfg(target_os = "windows")]
fn fingerprint_windows() -> String {
    // 方案 1: wmic
    if let Ok(output) = Command::new("wmic")
        .args(&["csproduct", "get", "UUID"])
        .output()
    {
        let stdout = String::from_utf8_lossy(&output.stdout);
        if let Some(line) = extract_first_non_header_line(&stdout, &["UUID"]) {
            return line;
        }
    }
    
    // 方案 2: PowerShell 回退
    if let Ok(output) = Command::new("powershell")
        .args(&["-Command", "(Get-CimInstance Win32_ComputerSystemProduct).UUID"])
        .output()
    {
        let stdout = String::from_utf8_lossy(&output.stdout);
        if let Some(line) = extract_first_non_header_line(&stdout, &["UUID"]) {
            return line;
        }
    }
    
    fallback_fingerprint()
}

#[cfg(target_os = "linux")]
fn fingerprint_linux() -> String {
    for path in &["/etc/machine-id", "/var/lib/dbus/machine-id"] {
        if let Ok(content) = std::fs::read_to_string(path) {
            let trimmed = content.trim();
            if !trimmed.is_empty() {
                return trimmed.to_string();
            }
        }
    }
    fallback_fingerprint()
}

fn fallback_fingerprint() -> String {
    use sysinfo::System;
    format!(
        "{}-{}-{}",
        System::host_name().unwrap_or_default(),
        System::cpu_arch().unwrap_or_default(),
        System::name().unwrap_or_default(),
    )
}

pub fn get_device_id() -> String {
    use sha2::{Sha256, Digest};
    let raw = collect_raw_fingerprint();
    let hash = Sha256::digest(raw.as_bytes());
    hex::encode(&hash[..8])   // 16 位 hex
}
```

#### 5.7.2 验收标准

- ✅ macOS 返回 IOPlatformSerialNumber 的 SHA256 前 16 位
- ✅ Windows 返回 UUID 的 SHA256 前 16 位
- ✅ 同一机器多次调用结果一致
- ✅ 硬件信息获取失败时有降级方案

---

### 5.8 功能 #5：Keychain / Credential Manager 存储 ⭐⭐

#### 5.8.1 依赖

```toml
# apps/desktop/Cargo.toml
keyring = "3"
```

#### 5.8.2 存储包装

```rust
// apps/desktop/src/adapters/secure_storage.rs
use keyring::Entry;

const KEYCHAIN_SERVICE: &str = "com.tuoling.tls-shipinhao.runtime";
const KEYCHAIN_ACCOUNT: &str = "runtime_bundle";

pub struct SecureStorage {
    entry: Entry,
}

impl SecureStorage {
    pub fn new() -> Result<Self, StorageError> {
        let entry = Entry::new(KEYCHAIN_SERVICE, KEYCHAIN_ACCOUNT)?;
        Ok(Self { entry })
    }
    
    pub fn set(&self, value: &str) -> Result<(), StorageError> {
        self.entry.set_password(value).map_err(Into::into)
    }
    
    pub fn get(&self) -> Result<Option<String>, StorageError> {
        match self.entry.get_password() {
            Ok(v) => Ok(Some(v)),
            Err(keyring::Error::NoEntry) => Ok(None),
            Err(e) => Err(e.into()),
        }
    }
    
    pub fn delete(&self) -> Result<(), StorageError> {
        match self.entry.delete_password() {
            Ok(_) | Err(keyring::Error::NoEntry) => Ok(()),
            Err(e) => Err(e.into()),
        }
    }
}
```

#### 5.8.3 文件后备

当 Keychain 不可用（CI 环境、Linux 无 seahorse 等）时，降级到**加密文件**：

```rust
// 用设备指纹作为 AES-256-GCM 的密钥
// 加密存到 ~/Library/.../TLS-shipinhao/runtime_bundle.enc
```

---

### 5.9 功能 #6：完整性校验 Manifest ⭐⭐

#### 5.9.1 概念

启动时检查**所有关键文件**的 SHA256 是否与**服务端签名的清单**一致，防止篡改。

#### 5.9.2 Manifest 格式

```json
{
  "version": "5.1.0",
  "build": 510,
  "generated_at": "2026-04-16T12:00:00Z",
  "files": [
    { "path": "apps/desktop/desktop-app", "sha256": "abc..." },
    { "path": "ui/dist/index.html", "sha256": "def..." },
    { "path": "ui/dist/assets/main.js", "sha256": "ghi..." }
  ],
  "signature": "<base64url(ed25519_signature)>"
}
```

#### 5.9.3 检查流程

```rust
pub fn validate_runtime_continuity() -> Result<(), IntegrityError> {
    let manifest_path = get_app_runtime_dir().join(INTEGRITY_MANIFEST_FILE_NAME);
    let manifest: SignedManifest = serde_json::from_str(
        &std::fs::read_to_string(&manifest_path)?
    )?;
    
    // 1. 验证签名
    let public_key = load_public_key(INTEGRITY_MANIFEST_PUBLIC_KEY)?;
    let canonical = canonicalize_manifest(&manifest.payload)?;
    public_key.verify(canonical.as_bytes(), &manifest.signature)
        .map_err(|_| IntegrityError::InvalidSignature)?;
    
    // 2. 验证文件哈希
    for file in &manifest.payload.files {
        let full_path = get_app_runtime_dir().join(&file.path);
        let content = std::fs::read(&full_path)?;
        let hash = Sha256::digest(&content);
        if hex::encode(hash) != file.sha256 {
            return Err(IntegrityError::FileModified(file.path.clone()));
        }
    }
    
    Ok(())
}
```

#### 5.9.4 触发条件

1. App 启动时
2. 每次 `refresh_lease_if_due` 前
3. 每次 `authorize_task` 前

**失败处理**：
- 设置 `RuntimeState.compromised = true`
- 禁用所有业务功能
- 显示严重警告："检测到程序被篡改，请重新下载"

---

### 5.10 功能 #7：本地授权校验（离线续航）⭐

#### 5.10.1 功能

`check_stored_license_local()`：**不联网**校验本地 Lease，允许用户在离线状态下继续使用（72 小时内）。

#### 5.10.2 流程

```rust
pub fn check_stored_license_local() -> RuntimeState {
    let lease_token = SecureStorage::new()
        .and_then(|s| s.get())
        .unwrap_or_default();
    
    if lease_token.is_empty() {
        return RuntimeState::not_found();
    }
    
    let device_id = get_device_id();
    let verifier = LeaseVerifier::new();
    
    match verifier.verify(&lease_token, Some(&device_id), false) {
        Ok(payload) => RuntimeState {
            license_key: payload.license_key,
            device_id: payload.device_id,
            reason: LicenseReason::Ok,
            status_hint: LicenseReason::Ok,
            license_expires_at: payload.exp_iso(),
            lease_expires_at: payload.exp_iso(),
            renew_after: payload.renew_after_iso(),
            task_policy: payload.task_policy,
            risk_level: payload.risk_level,
            runtime_backend: "rust".to_string(),
            compromised: false,
            last_verify_at: "".to_string(),
        },
        Err(LeaseError::Expired) => RuntimeState::reason_only(LicenseReason::Expired),
        Err(LeaseError::DeviceMismatch) => RuntimeState::reason_only(LicenseReason::DeviceMismatch),
        Err(_) => RuntimeState::reason_only(LicenseReason::Invalid),
    }
}
```

---

## 六、订单同步模块 PRD

### 6.1 目标

完整还原 Python 版的**缓存段管理**、**缺口补齐**、**dirty 检测**、**全量扫描**等能力。

### 6.2 数据库 Schema 补齐 ⭐⭐⭐

当前 Rust 版 `SqliteOrderCache` 只有 `orders` 一张表，需补齐到完整 4 表：

```sql
-- 主订单表（已有，需补齐字段）
CREATE TABLE IF NOT EXISTS orders (
    order_id TEXT PRIMARY KEY,
    buyer_nickname TEXT NOT NULL DEFAULT '',
    normalized_nickname TEXT NOT NULL DEFAULT '',
    create_time INTEGER NOT NULL DEFAULT 0,
    confirm_receipt_time INTEGER NOT NULL DEFAULT 0,
    is_waybill_received INTEGER NOT NULL DEFAULT 0,
    waybill_received_time INTEGER NOT NULL DEFAULT 0,
    is_education_order INTEGER NOT NULL DEFAULT 0,
    order_status INTEGER NOT NULL DEFAULT 0,
    openid TEXT NOT NULL DEFAULT '',
    raw_source TEXT NOT NULL DEFAULT 'order_api',
    updated_at INTEGER NOT NULL DEFAULT 0
);

-- ⭐ 订单商品（需补齐）
CREATE TABLE IF NOT EXISTS order_products (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    order_id TEXT NOT NULL,
    product_id TEXT NOT NULL DEFAULT '',
    sku_id TEXT NOT NULL DEFAULT '',
    sale_param TEXT NOT NULL DEFAULT '',
    product_name TEXT NOT NULL DEFAULT '',
    thumb_img TEXT NOT NULL DEFAULT '',
    FOREIGN KEY(order_id) REFERENCES orders(order_id) ON DELETE CASCADE
);

-- ⭐ 同步状态（需补齐）
CREATE TABLE IF NOT EXISTS sync_state (
    scope TEXT PRIMARY KEY,
    coverage_start INTEGER NOT NULL DEFAULT 0,
    coverage_end INTEGER NOT NULL DEFAULT 0,
    last_incremental_start INTEGER NOT NULL DEFAULT 0,
    last_incremental_end INTEGER NOT NULL DEFAULT 0,
    last_success_at INTEGER NOT NULL DEFAULT 0,
    last_mode TEXT NOT NULL DEFAULT '',
    last_error TEXT NOT NULL DEFAULT ''
);

-- ⭐ 缓存段（需补齐）
CREATE TABLE IF NOT EXISTS cache_segments (
    scope TEXT NOT NULL,
    start_ts INTEGER NOT NULL,
    end_ts INTEGER NOT NULL,
    status TEXT NOT NULL DEFAULT 'complete',
    updated_at INTEGER NOT NULL DEFAULT 0,
    PRIMARY KEY (scope, start_ts, end_ts)
);

-- 索引
CREATE INDEX IF NOT EXISTS idx_orders_create_time ON orders(create_time DESC);
CREATE INDEX IF NOT EXISTS idx_products_order_id ON order_products(order_id);
CREATE INDEX IF NOT EXISTS idx_cache_segments_scope_start ON cache_segments(scope, start_ts, end_ts);

PRAGMA journal_mode=WAL;
```

### 6.3 OrderCacheRepository Rust 接口

```rust
// crates/desktop-services/src/order_cache_repository.rs
pub trait OrderCacheRepository: Send + Sync {
    fn initialize(&self) -> Result<()>;
    fn upsert_orders(&self, orders: &[OrderDoc], raw_source: &str) -> Result<usize>;
    fn get_state(&self, scope: &str) -> Result<Option<SyncState>>;
    fn save_state(&self, state: &SyncState) -> Result<()>;
    fn mark_segment_complete(&self, start_ts: i64, end_ts: i64, scope: &str) -> Result<()>;
    fn get_complete_segments(&self, start: i64, end: i64, scope: &str) -> Result<Vec<Segment>>;
    fn get_missing_segments(
        &self, start: i64, end: i64, scope: &str,
        merge_tolerance: i64, min_gap_width: i64,
    ) -> Result<Vec<(i64, i64)>>;
    fn has_dirty_sale_param(&self) -> Result<bool>;
    fn clear_all(&self) -> Result<()>;
    fn delete_older_than(&self, cutoff: i64) -> Result<usize>;
    fn fetch_orders_in_range(&self, start: i64, end: i64) -> Result<Vec<OrderDoc>>;
}
```

### 6.4 功能 #8：缺口补齐算法 ⭐⭐

#### 6.4.1 算法规范

```rust
pub fn get_missing_segments(
    &self,
    start_timestamp: i64,
    end_timestamp: i64,
    scope: &str,
    merge_tolerance: i64,    // 默认 120 秒
    min_gap_width: i64,      // 默认 300 秒
) -> Result<Vec<(i64, i64)>> {
    if start_timestamp <= 0 || end_timestamp <= 0 || start_timestamp > end_timestamp {
        return Ok(vec![]);
    }
    
    // 1. 取出范围内所有已完成的段
    let segments: Vec<(i64, i64)> = self.get_complete_segments(start_timestamp, end_timestamp, scope)?
        .into_iter()
        .map(|s| (s.start_ts.max(start_timestamp), s.end_ts.min(end_timestamp)))
        .filter(|(s, e)| s <= e)
        .collect();
    
    if segments.is_empty() {
        return Ok(vec![(start_timestamp, end_timestamp)]);
    }
    
    // 2. 合并相邻段（容差 merge_tolerance）
    let mut merged: Vec<[i64; 2]> = vec![];
    for (s, e) in segments {
        if merged.is_empty() || s > merged.last().unwrap()[1] + merge_tolerance {
            merged.push([s, e]);
        } else {
            merged.last_mut().unwrap()[1] = merged.last().unwrap()[1].max(e);
        }
    }
    
    // 3. 找出剩余缺口
    let mut missing = vec![];
    let mut cursor = start_timestamp;
    for [s, e] in &merged {
        if cursor < *s && s - cursor >= min_gap_width {
            missing.push((cursor, *s - 1));
        }
        cursor = cursor.max(e + 1);
    }
    if cursor <= end_timestamp && end_timestamp - cursor + 1 >= min_gap_width {
        missing.push((cursor, end_timestamp));
    }
    
    Ok(missing)
}
```

#### 6.4.2 验收标准

- ✅ 三段完整覆盖 → 返回空
- ✅ 中间有 500 秒缺口 → 返回该缺口
- ✅ 中间有 200 秒缺口（小于 `min_gap_width`）→ 返回空
- ✅ 两段间隔 60 秒（小于 `merge_tolerance`）→ 合并为一段
- ✅ 单元测试覆盖 10+ 边界场景

---

### 6.5 功能 #9：Dirty 数据检测与修复 ⭐

```rust
impl OrderCacheRepository {
    pub fn has_dirty_sale_param(&self) -> Result<bool> {
        let conn = self.conn.lock().unwrap();
        let count: i64 = conn.query_row(
            "SELECT COUNT(*) FROM order_products WHERE sale_param LIKE '[%'",
            [],
            |row| row.get(0),
        )?;
        Ok(count > 0)
    }
}

// 在 OrderSyncService::ensure_recent_cache 中
if self.repository.get_state(ORDER_CACHE_SCOPE)?.is_some()
    && self.repository.has_dirty_sale_param()? 
{
    on_progress("[缓存] 检测到历史数据格式异常，自动清空并重建缓存。");
    return self.rebuild_cache(on_progress);
}
```

---

### 6.6 功能 #10：全量扫描模式 ⭐⭐

```rust
pub async fn fetch_full_scan_orders(
    &self,
    earliest_time: i64,
    on_progress: ProgressCallback,
) -> Result<(Vec<OrderDoc>, Vec<String>)> {
    let (_, warnings, recent_start, recent_end) = 
        self.ensure_recent_cache(&on_progress).await?;
    
    let recent_orders = self.repository.fetch_orders_in_range(
        earliest_time.max(recent_start),
        recent_end,
    )?;
    
    if earliest_time >= recent_start {
        on_progress(format!(
            "[缓存] 完整补查命中最近 30 天缓存 {} 个订单。",
            recent_orders.len()
        ));
        return Ok((recent_orders, warnings));
    }
    
    let temporary_end = recent_start - 1;
    on_progress("[缓存] 开始补查 30 天前的临时订单（本次使用，不写入长期缓存）。".to_string());
    
    let (temporary_orders, temp_warnings) = self.finder.get_orders_for_cache(
        earliest_time,
        earliest_time,
        temporary_end,
        &on_progress,
        None,   // 临时数据不持久化
    ).await?;
    
    let combined = deduplicate_orders_by_id(
        temporary_orders.into_iter().chain(recent_orders.into_iter()).collect()
    );
    
    let mut all_warnings = warnings;
    all_warnings.extend(temp_warnings);
    
    Ok((combined, all_warnings))
}
```

---

### 6.7 功能 #11：旧缓存迁移 ⭐

**场景**：用户从 Python 4.3.0 升级到 Rust 5.1.0，需要自动迁移缓存。

```rust
fn migrate_legacy_cache_if_needed(&self) -> Result<()> {
    if self.db_path.exists() {
        return Ok(());
    }
    
    // Python 版的候选位置
    let legacy_candidates = vec![
        dirs::home_dir().unwrap().join(".tls-shipinhao").join("order_cache.sqlite3"),
        get_app_runtime_dir().join("cache").join("order_cache.sqlite3"),
    ];
    
    for legacy_path in &legacy_candidates {
        if legacy_path.exists() && legacy_path != &self.db_path {
            tracing::info!("检测到旧版缓存，正在迁移: {} -> {}",
                legacy_path.display(), self.db_path.display());
            std::fs::create_dir_all(self.db_path.parent().unwrap())?;
            std::fs::rename(legacy_path, &self.db_path)?;
            
            // 迁移 WAL / SHM 边车文件
            for suffix in &["-wal", "-shm"] {
                let legacy_sidecar = legacy_path.with_extension(
                    format!("{}{}", legacy_path.extension().unwrap().to_str().unwrap(), suffix)
                );
                let target_sidecar = self.db_path.with_extension(
                    format!("{}{}", self.db_path.extension().unwrap().to_str().unwrap(), suffix)
                );
                if legacy_sidecar.exists() && !target_sidecar.exists() {
                    std::fs::rename(&legacy_sidecar, &target_sidecar)?;
                }
            }
            break;
        }
    }
    Ok(())
}
```

---

## 七、评价匹配模块 PRD

### 7.1 功能 #12：反风控抓取 ⭐⭐⭐ 最紧急

#### 7.1.1 核心设计

```rust
// crates/desktop-services/src/order_fetcher.rs
use tokio::sync::Mutex;
use std::sync::Arc;

pub struct OrderFetcher {
    http_source: Arc<dyn OrderSearchSource>,
    stop_flag: Arc<AtomicBool>,
}

impl OrderFetcher {
    /// 正常模式：3 worker 并发，0.3 秒间隔
    pub async fn fetch_by_page_normal(
        &self,
        earliest_time: i64,
        on_progress: ProgressCallback,
        on_batch_completed: BatchCallback,
    ) -> Result<Vec<OrderDoc>, FetchError> {
        self.fetch_by_page(
            earliest_time,
            ORDER_WINDOW_WORKERS,          // 3
            FETCH_PAGE_INTERVAL_SECONDS,   // 0.3
            on_progress,
            on_batch_completed,
        ).await
    }
    
    /// 风控降级模式：1 worker，2.0 秒间隔
    pub async fn fetch_by_page_risk_mode(
        &self,
        earliest_time: i64,
        on_progress: ProgressCallback,
        on_batch_completed: BatchCallback,
    ) -> Result<Vec<OrderDoc>, FetchError> {
        self.fetch_by_page(
            earliest_time,
            ORDER_RISK_WINDOW_WORKERS,       // 1
            ORDER_RISK_PAGE_INTERVAL_SECONDS,// 2.0
            on_progress,
            on_batch_completed,
        ).await
    }
    
    async fn fetch_by_page(
        &self,
        earliest_time: i64,
        num_workers: usize,
        page_interval_secs: f64,
        on_progress: ProgressCallback,
        on_batch_completed: BatchCallback,
    ) -> Result<Vec<OrderDoc>, FetchError> {
        let shared_page = Arc::new(AtomicU32::new(1));
        let stop_event = Arc::new(AtomicBool::new(false));
        let collected = Arc::new(Mutex::new(Vec::new()));
        let pending = Arc::new(Mutex::new(Vec::new()));
        let fatal_errors = Arc::new(Mutex::new(Vec::new()));
        let risk_messages = Arc::new(Mutex::new(Vec::new()));
        let batch_counter = Arc::new(AtomicU32::new(0));
        
        let mut handles = vec![];
        for worker_id in 1..=num_workers {
            let handle = tokio::spawn(self.clone().worker_loop(
                worker_id,
                shared_page.clone(),
                stop_event.clone(),
                collected.clone(),
                pending.clone(),
                fatal_errors.clone(),
                risk_messages.clone(),
                batch_counter.clone(),
                earliest_time,
                page_interval_secs,
                on_progress.clone(),
                on_batch_completed.clone(),
            ));
            handles.push(handle);
        }
        
        for handle in handles {
            handle.await?;
        }
        
        // 处理剩余 pending
        // ...
        
        // 风控检测
        let risk_msgs = risk_messages.lock().await;
        if !risk_msgs.is_empty() {
            return Err(FetchError::RiskControl {
                message: risk_msgs[0].clone(),
                partial_orders: collected.lock().await.clone(),
            });
        }
        
        let errors = fatal_errors.lock().await;
        if !errors.is_empty() {
            return Err(FetchError::Fatal(errors.join("; ")));
        }
        
        Ok(deduplicate_orders_by_id(collected.lock().await.clone()))
    }
}
```

#### 7.1.2 限流处理（HTTP 429）

```rust
async fn retry_order_search_on_limit(
    &self,
    data: &OrderSearchRequest,
    headers: &HeaderMap,
    page_index: u32,
    api_level: bool,
    on_progress: &ProgressCallback,
) -> Result<serde_json::Value, FetchError> {
    for retry in 0..RATE_LIMIT_RETRY_COUNT {
        let wait_secs = 2u64.pow((retry + 1) as u32);
        let limit_type = if api_level { "(API)" } else { "" };
        on_progress(format!(
            "第 {} 页触发频率限制{}，等待 {} 秒后重试...",
            page_index, limit_type, wait_secs
        ));
        tokio::time::sleep(Duration::from_secs(wait_secs)).await;
        
        let response = self.http_source.post_json(data, headers).await?;
        if response.status() != StatusCode::TOO_MANY_REQUESTS {
            return Ok(response.json().await?);
        }
    }
    Err(FetchError::RateLimitExhausted)
}
```

#### 7.1.3 风控检测

```rust
fn is_risk_control_result(result: &serde_json::Value) -> bool {
    let code = result.get("code").and_then(|v| v.as_i64()).unwrap_or(0);
    let resp_status = result.get("respStatusCode").and_then(|v| v.as_i64()).unwrap_or(0);
    let message = result.get("msg").and_then(|v| v.as_str()).unwrap_or("");
    
    code == 430 || resp_status == 430 
        || message.contains("异常行为") 
        || message.contains("拒绝访问")
}
```

#### 7.1.4 自动降级到极速模式

```rust
pub async fn get_orders_for_cache(
    &self,
    earliest_time: i64,
    on_progress: ProgressCallback,
    on_batch_completed: BatchCallback,
) -> Result<(Vec<OrderDoc>, Vec<String>), FetchError> {
    match self.fetch_by_page_normal(
        earliest_time, on_progress.clone(), on_batch_completed.clone()
    ).await {
        Ok(orders) => Ok((orders, vec![])),
        Err(FetchError::RiskControl { partial_orders, .. }) => {
            const COOLDOWN_SECS: u64 = 60;
            on_progress(format!(
                "⚠️ 检测到平台风控，等待 {} 秒冷却后切换到极速模式（单线程 + 更慢间隔）。",
                COOLDOWN_SECS
            ));
            
            for remaining in (10..=COOLDOWN_SECS).rev().step_by(10) {
                if self.stop_flag.load(Ordering::Relaxed) {
                    break;
                }
                on_progress(format!("[风控冷却] 还剩 {} 秒...", remaining));
                tokio::time::sleep(Duration::from_secs(10)).await;
            }
            
            let risk_warning = "本次抓取触发平台风控，已自动降级到极速模式".to_string();
            match self.fetch_by_page_risk_mode(
                earliest_time, on_progress, on_batch_completed
            ).await {
                Ok(risk_orders) => {
                    let merged = deduplicate_orders_by_id(
                        partial_orders.into_iter().chain(risk_orders).collect()
                    );
                    Ok((merged, vec![risk_warning]))
                }
                Err(_) => {
                    if !partial_orders.is_empty() {
                        Ok((partial_orders, vec![
                            risk_warning,
                            "仍有部分窗口未完成，结果可能不完整".to_string(),
                        ]))
                    } else {
                        Err(FetchError::Fatal("平台风控持续触发，请稍后重试".to_string()))
                    }
                }
            }
        }
        Err(e) => Err(e),
    }
}
```

#### 7.1.5 验收标准

- ✅ 单元测试模拟 429 响应 → 验证指数退避（2s、4s、8s）
- ✅ 单元测试模拟 code=430 → 验证进入风控降级
- ✅ 风控冷却期间能被用户停止
- ✅ 降级后再次风控 → 返回已有数据 + 警告
- ✅ 集成测试：真实打压测试，命中风控后自动恢复

---

### 7.2 功能 #13：智能昵称匹配 ⭐

完整还原 Python 的 `similarity_percent` 逻辑，特别是改名场景：

```rust
// crates/desktop-services/src/matching/nickname.rs
use regex::Regex;

const TRAILING_DIGIT_REGEX: &str = r"[0-9０-９⁰¹²³⁴⁵⁶⁷⁸⁹₀₁₂₃₄₅₆₇₈₉\s]+$";

pub fn similarity_percent(left: Option<&str>, right: Option<&str>) -> u32 {
    let left_text = left.unwrap_or("");
    let right_text = right.unwrap_or("");
    
    if left_text == right_text { return 100; }
    if left_text.is_empty() || right_text.is_empty() { return 0; }
    
    let left_trimmed = left_text.trim();
    let right_trimmed = right_text.trim();
    if !left_trimmed.is_empty() && left_trimmed == right_trimmed {
        return 95;
    }
    
    if let Some(score) = nickname_similarity_by_rename_patterns(left_trimmed, right_trimmed) {
        return score;
    }
    
    sequence_similarity(left_text, right_text)
}

fn nickname_similarity_by_rename_patterns(left: &str, right: &str) -> Option<u32> {
    let left_core = strip_trailing_digit_tail(left);
    let right_core = strip_trailing_digit_tail(right);
    
    if !left_core.is_empty() && left_core == right_core && left != right {
        return Some(if left_core.chars().count() >= 2 { 95 } else { 80 });
    }
    
    let (shorter, longer) = if left.chars().count() <= right.chars().count() {
        (left, right)
    } else {
        (right, left)
    };
    
    if !shorter.is_empty() && longer.contains(shorter) {
        let len = shorter.chars().count();
        return Some(match len {
            n if n >= 3 => 90,
            2 => 80,
            _ => single_char_containment_similarity(longer),
        });
    }
    
    let (shorter_core, longer_core) = if left_core.chars().count() <= right_core.chars().count() {
        (left_core.as_str(), right_core.as_str())
    } else {
        (right_core.as_str(), left_core.as_str())
    };
    
    if !shorter_core.is_empty() && longer_core.contains(shorter_core) {
        let len = shorter_core.chars().count();
        return Some(match len {
            n if n >= 3 => 90,
            2 => 80,
            _ => single_char_containment_similarity(longer_core),
        });
    }
    
    if let Some(s) = subsequence_similarity_by_length(shorter) {
        if is_subsequence(shorter, longer) {
            return Some(s);
        }
    }
    
    if let Some(s) = subsequence_similarity_by_length(shorter_core) {
        if is_subsequence(shorter_core, longer_core) {
            return Some(s);
        }
    }
    
    None
}

fn subsequence_similarity_by_length(text: &str) -> Option<u32> {
    match text.chars().count() {
        n if n >= 4 => Some(85),
        3 => Some(80),
        2 => Some(70),
        _ => None,
    }
}

fn is_subsequence(shorter: &str, longer: &str) -> bool {
    if shorter.is_empty() { return false; }
    let mut iter = shorter.chars().peekable();
    for ch in longer.chars() {
        if iter.peek() == Some(&ch) {
            iter.next();
            if iter.peek().is_none() { return true; }
        }
    }
    false
}

fn single_char_containment_similarity(longer: &str) -> u32 {
    let normalized_length = longer.chars().count().max(3);
    (100 / normalized_length as u32).min(100)
}
```

### 7.3 功能 #14：通用昵称过滤 ⭐

```rust
const GENERIC_NICKNAME_PREFIXES: &[&str] = &["匿名", "微信用户", "默认昵称"];

pub fn is_generic_nickname(name: &str) -> bool {
    if name.is_empty() { return true; }
    GENERIC_NICKNAME_PREFIXES.iter().any(|prefix| name.starts_with(prefix))
}
```

### 7.4 功能 #15：匹配策略分级 ⭐

```rust
#[derive(Debug, Clone, Copy, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum MatchStrategy {
    ExactMatch,       // >= 100
    HighConfidence,   // >= AUTO_FILL_SCORE_THRESHOLD (100)
    ProbableMatch,    // >= MATCH_MIN_SCORE (50)
    Fallback,
}

pub fn match_strategy_by_score(score: i32) -> MatchStrategy {
    if score >= 100 { MatchStrategy::ExactMatch }
    else if score >= AUTO_FILL_SCORE_THRESHOLD { MatchStrategy::HighConfidence }
    else if score >= MATCH_MIN_SCORE { MatchStrategy::ProbableMatch }
    else { MatchStrategy::Fallback }
}
```

**前端展示**：

```typescript
const STRATEGY_LABEL: Record<MatchStrategy, {text: string, color: string}> = {
  exact_match: { text: '精确匹配', color: 'green' },
  high_confidence: { text: '高置信', color: 'blue' },
  probable_match: { text: '可能匹配', color: 'orange' },
  fallback: { text: '仅供参考', color: 'gray' },
};
```

### 7.5 功能 #16：差评可回复期检测 ⭐

```rust
fn is_evaluation_replyable(operation_info: &serde_json::Value) -> bool {
    let can_reply_expire_time = operation_info
        .get("canReplyExpireTime")
        .and_then(|v| v.as_i64())
        .unwrap_or(0);
    
    if can_reply_expire_time == 0 { return true; }  // 保守：未知则认为可回复
    
    let expire_date = chrono::DateTime::<chrono::Utc>::from_timestamp(can_reply_expire_time, 0)
        .unwrap_or_else(chrono::Utc::now);
    let now = chrono::Utc::now();
    let days_until_expire = (expire_date - now).num_days();
    
    days_until_expire >= -30  // 与 Python 版一致
}
```

---

## 八、发货管理模块 PRD

### 8.1 功能 #17：快递公司自动降级 ⭐⭐

```rust
// apps/desktop/src/adapters/http_delivery_gateway.rs

const DELIVERY_MISMATCH_MARKERS: &[&str] = &[
    "快递单号与所选物流商不匹配",
    "快递单号有误",
];

fn is_delivery_mismatch_error(error_msg: &str) -> bool {
    DELIVERY_MISMATCH_MARKERS.iter().any(|marker| error_msg.contains(marker))
}

pub async fn update_single_order(
    &self,
    order_id: &str,
    tracking_number: &str,
    session: &Session,
) -> Result<String, DeliveryError> {
    let context = self.fetch_current_delivery_context(order_id, session).await?;
    let old_waybill = context.snapshot.waybill_id.clone();
    
    // 第 1 次尝试：沿用原 deliveryId
    match self.update_delivery_info(order_id, tracking_number, &context.raw, None, session).await {
        Ok(_) => return Ok(old_waybill),
        Err(e) if !is_delivery_mismatch_error(&e.to_string()) => return Err(e),
        _ => {} // 快递公司不匹配，降级重试
    }
    
    // 第 2 次尝试：用单号前缀推断 deliveryId
    let tracking_prefix = tracking_number.trim().chars().take(2).collect::<String>();
    let current_delivery_id = &context.raw.delivery_id;
    
    if !tracking_prefix.is_empty() && tracking_prefix != *current_delivery_id {
        let override_info = DeliveryOverride {
            delivery_id: tracking_prefix,
            delivery_name: String::new(),
        };
        self.update_delivery_info(order_id, tracking_number, &context.raw, Some(override_info), session).await?;
        return Ok(old_waybill);
    }
    
    Err(DeliveryError::MismatchNoMapping)
}
```

### 8.2 功能 #18：物流快照保留 ⭐

```rust
pub fn build_update_delivery_payload(
    order_id: &str,
    tracking_number: &str,
    old_delivery_product_info: &DeliveryProductInfo,
    override_info: Option<&DeliveryOverride>,
) -> UpdateDeliveryPayload {
    let old_info = old_delivery_product_info.clone();
    let mut new_info = old_delivery_product_info.clone();
    
    // 关键：只修改 waybillId
    new_info.waybill_id = tracking_number.trim().to_string();
    
    if let Some(override_info) = override_info {
        if !override_info.delivery_id.is_empty() {
            new_info.delivery_id = override_info.delivery_id.clone();
        }
        if !override_info.delivery_name.is_empty() {
            new_info.delivery_name = override_info.delivery_name.clone();
        }
    }
    
    UpdateDeliveryPayload {
        order_id: order_id.trim().to_string(),
        change_info: vec![ChangeInfo { old: old_info, new: new_info }],
    }
}
```

### 8.3 功能 #19：initShipData → orderDetail 回退 ⭐

```rust
pub async fn fetch_current_delivery_context(
    &self,
    order_id: &str,
    session: &Session,
) -> Result<DeliveryContext, DeliveryError> {
    let init_error: Result<DeliveryContext, DeliveryError>;
    
    // 优先尝试 initShipData
    match self.fetch_init_ship_data_payload(order_id, session).await {
        Ok(payload) => {
            match self.extract_raw_delivery_product_info_from_init_ship_data(&payload) {
                Ok(raw_info) => return Ok(build_delivery_context(raw_info)),
                Err(e) => init_error = Err(e),
            }
        }
        Err(e) => init_error = Err(e),
    }
    
    // 回退到 orderDetail
    match self.fetch_order_detail_payload(order_id, session).await {
        Ok(payload) => {
            let raw_info = self.extract_raw_delivery_product_info_from_order_detail(&payload)?;
            Ok(build_delivery_context(raw_info))
        }
        Err(detail_err) => {
            // 两者都因缺少数据而失败 → 合并错误信息
            if let Err(init_err) = init_error {
                if is_missing_snapshot_error(&init_err) && is_missing_snapshot_error(&detail_err) {
                    return Err(DeliveryError::Missing("订单详情中没有可更新的物流信息".into()));
                }
            }
            Err(detail_err)
        }
    }
}
```

---

## 九、Cookie 与配置模块 PRD

### 9.1 功能 #20：配置目录多候选查找 ⭐

```rust
// apps/desktop/src/app_settings.rs
const CONFIG_DIR_NAME: &str = ".tls-shipinhao";
const USER_CONFIG_POINTER: &str = "selected_config_dir.txt";
const COOKIE_FILE_NAME: &str = "cookie.txt";

pub fn get_home_config_dir() -> PathBuf {
    dirs::home_dir().unwrap().join(CONFIG_DIR_NAME)
}

pub fn get_saved_user_config_dir() -> Option<PathBuf> {
    let pointer = get_home_config_dir().join(USER_CONFIG_POINTER);
    if !pointer.exists() { return None; }
    std::fs::read_to_string(&pointer).ok()
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty())
        .map(PathBuf::from)
}

pub fn save_user_config_dir(config_dir: &Path) -> Result<PathBuf> {
    let target = config_dir.canonicalize()?;
    std::fs::create_dir_all(&target)?;
    let home_dir = get_home_config_dir();
    std::fs::create_dir_all(&home_dir)?;
    std::fs::write(
        home_dir.join(USER_CONFIG_POINTER),
        target.to_str().unwrap(),
    )?;
    Ok(target)
}

pub fn resolve_config_dir() -> Result<PathBuf, ConfigNotFoundError> {
    let mut candidates = vec![];
    if let Some(saved) = get_saved_user_config_dir() {
        candidates.push(saved);
    }
    candidates.push(get_home_config_dir());
    
    let mut searched = vec![];
    for cfg_dir in &candidates {
        searched.push(cfg_dir.clone());
        let cookie_file = cfg_dir.join(COOKIE_FILE_NAME);
        if cookie_file.exists() {
            return Ok(cfg_dir.canonicalize()?);
        }
    }
    Err(ConfigNotFoundError { searched })
}
```

### 9.2 功能 #21：biz_magic 自动提取 ✅（已有）

保持与 Python 一致：

```rust
pub fn extract_biz_magic_from_cookie(cookie: &Cookie) -> String {
    if let Some(map) = cookie.as_map() {
        if let Some(value) = map.get("biz_magic").or_else(|| map.get("magic")) {
            return value.trim().to_string();
        }
    }
    
    let raw = serialize_cookie_data(cookie);
    for pattern in &[r"biz_magic=([^;\s]+)", r"magic=([^;\s]+)"] {
        let re = Regex::new(pattern).unwrap();
        if let Some(captures) = re.captures(&raw) {
            if let Some(m) = captures.get(1) {
                return m.as_str().trim().to_string();
            }
        }
    }
    String::new()
}
```

---

## 十、在线更新模块 PRD

### 10.1 功能 #22：更新检查 ⭐

```rust
// crates/desktop-services/src/update_service.rs
const UPDATE_VERSION_URL: &str = "https://gitee.com/tuolingshe/tuoling-shipinhao/raw/master/version.json";
const UPDATE_CHECK_DELAY_MS: u64 = 1200;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UpdateInfo {
    pub app: String,
    pub version: String,
    pub build: u32,
    pub mandatory: bool,
    pub platform: String,
    pub download_url: String,
    pub tutorial_url: String,
    pub notes: Vec<String>,
    pub has_update: bool,
    pub raw_payload: serde_json::Value,
}

pub async fn fetch_latest_version_info(
    current_version: Option<&str>,
) -> Result<UpdateInfo, UpdateError> {
    let client = reqwest::Client::new();
    let response = client.get(UPDATE_VERSION_URL)
        .timeout(Duration::from_secs(REQUEST_TIMEOUT))
        .send().await?;
    
    if !response.status().is_success() {
        return Err(UpdateError::HttpError(response.status()));
    }
    
    let payload: serde_json::Value = response.json().await?;
    let platform = detect_platform();
    let latest_version = payload.get("version")
        .and_then(|v| v.as_str()).unwrap_or("").to_string();
    let resolved_current = current_version.unwrap_or(APP_VERSION);
    let has_update = !latest_version.is_empty() 
        && is_newer_version(resolved_current, &latest_version);
    
    Ok(UpdateInfo {
        app: payload.get("app").and_then(|v| v.as_str()).unwrap_or("TLS-shipinhao").to_string(),
        version: latest_version,
        build: payload.get("build").and_then(|v| v.as_u64()).unwrap_or(0) as u32,
        mandatory: payload.get("mandatory").and_then(|v| v.as_bool()).unwrap_or(false),
        platform,
        download_url: payload.get("download_url").and_then(|v| v.as_str()).unwrap_or("").to_string(),
        tutorial_url: payload.get("tutorial_url").and_then(|v| v.as_str()).unwrap_or("").to_string(),
        notes: normalize_notes(&payload),
        has_update,
        raw_payload: payload,
    })
}

fn detect_platform() -> String {
    match std::env::consts::OS {
        "macos" => "mac".to_string(),
        "windows" => "windows".to_string(),
        _ => "unknown".to_string(),
    }
}

pub fn parse_version(version: &str) -> (u32, u32, u32) {
    let parts: Vec<u32> = version.trim().split('.')
        .filter_map(|s| s.parse().ok())
        .collect();
    let mut padded = parts;
    padded.resize(3, 0);
    (padded[0], padded[1], padded[2])
}

pub fn is_newer_version(current: &str, latest: &str) -> bool {
    parse_version(latest) > parse_version(current)
}
```

### 10.2 前端展示

```vue
<!-- ui/src/components/layout/UpdateBanner.vue -->
<template>
  <div v-if="updateInfo?.has_update" class="update-banner">
    <div class="flex items-center gap-3">
      <span class="text-lg">🎉</span>
      <div>
        <div class="font-semibold text-emerald-900">
          发现新版本 {{ updateInfo.version }}
          <span v-if="updateInfo.mandatory" class="text-red-500">（强制更新）</span>
        </div>
        <ul class="text-sm text-slate-700">
          <li v-for="(note, i) in updateInfo.notes" :key="i">• {{ note }}</li>
        </ul>
      </div>
    </div>
    <div class="flex gap-2">
      <a :href="updateInfo.download_url" target="_blank" class="btn-primary">下载</a>
      <a v-if="updateInfo.tutorial_url" :href="updateInfo.tutorial_url" target="_blank" class="btn-secondary">教程</a>
      <button v-if="!updateInfo.mandatory" @click="dismiss" class="btn-ghost">稍后</button>
    </div>
  </div>
</template>
```

---

## 十一、UI 规范

### 11.1 功能 #23：翠绿主题还原 ⭐⭐

完整还原 Python 版的翠绿主题到 Tailwind CSS 4。

```css
/* ui/src/assets/styles/main.css */
@import "tailwindcss";

:root {
  color-scheme: light;
  
  /* 背景与表面 */
  --color-window-base: #3A3D38;
  --color-bg: #ECFDF5;
  --color-surface: #FFFFFF;
  --color-surface-soft: #F0FDF4;
  
  /* 边框 */
  --color-border: #A7F3D0;
  --color-border-strong: #6EE7B7;
  --color-input-border: #A7F3D0;
  --color-input-border-focus: #059669;
  
  /* 文字 */
  --color-text: #064E3B;
  --color-heading: #022C22;
  --color-muted: #047857;
  --color-muted-soft: #059669;
  
  /* 品牌主色（翠绿） */
  --color-brand: #059669;
  --color-brand-deep: #047857;
  --color-brand-soft: #D1FAE5;
  --color-brand-tint: #A7F3D0;
  
  /* 强调色（橙） */
  --color-accent: #F97316;
  --color-accent-deep: #EA580C;
  
  /* 语义色 */
  --color-success: #059669;
  --color-success-soft: #D1FAE5;
  --color-warning: #F97316;
  --color-danger: #B91C1C;
  --color-danger-soft: #FEE2E2;
  
  /* 中性 */
  --color-neutral-bg: #F1F5F9;
  --color-neutral-text: #64748B;
  --color-neutral-border: #CBD5E1;
  
  /* 布局常量 */
  --radius-card: 16px;
  --radius-hero: 20px;
  --radius-badge: 12px;
}

/* 组件样式 */
.surface-panel {
  background: var(--color-surface);
  border: 1px solid var(--color-border);
  border-radius: var(--radius-card);
  padding: 16px;
}

.hero-panel {
  background: linear-gradient(135deg, var(--color-brand-soft), var(--color-surface));
  border: 1px solid var(--color-brand-tint);
  border-radius: var(--radius-hero);
  padding: 24px 26px;
}

.action-btn {
  height: 40px;
  padding: 0 20px;
  border-radius: 12px;
  font-size: 13px;
  font-weight: 600;
  background: var(--color-brand);
  color: white;
  transition: all 0.2s ease;
}
.action-btn:hover { background: var(--color-brand-deep); }
.action-btn:disabled { opacity: 0.5; cursor: not-allowed; }

/* 状态徽章 */
.status-badge-active { background: var(--color-success-soft); color: var(--color-success); }
.status-badge-warning { background: #FEF3C7; color: #D97706; }
.status-badge-danger { background: var(--color-danger-soft); color: var(--color-danger); }
```

### 11.2 功能 #24：UI 缩放支持 ⭐

```typescript
// ui/src/composables/useUiScale.ts
import { ref, computed, watch } from 'vue';

const MIN_UI_SCALE = 0.82;
const MAX_UI_SCALE = 1.0;
const STORAGE_KEY = 'ui_scale';

const rawScale = ref<number>(
  parseFloat(localStorage.getItem(STORAGE_KEY) || '1.0')
);

export function useUiScale() {
  const clampedScale = computed(() => 
    Math.max(MIN_UI_SCALE, Math.min(MAX_UI_SCALE, rawScale.value))
  );
  
  watch(clampedScale, (value) => {
    document.documentElement.style.setProperty('--ui-scale', String(value));
    document.documentElement.style.fontSize = `${14 * value}px`;
    localStorage.setItem(STORAGE_KEY, String(value));
  }, { immediate: true });
  
  function setScale(value: number) {
    rawScale.value = value;
  }
  
  function increment() { setScale(rawScale.value + 0.02); }
  function decrement() { setScale(rawScale.value - 0.02); }
  
  return { scale: clampedScale, setScale, increment, decrement };
}
```

### 11.3 功能 #25：多布局适配 ⭐

```typescript
// ui/src/composables/useLayout.ts
import { ref, computed, onMounted, onUnmounted } from 'vue';

const WIDE_LAYOUT_MIN_WIDTH = 1320;
const WIDE_LAYOUT_MIN_HEIGHT = 780;
const COMPACT_LAYOUT_MIN_WIDTH = 860;
const HIGH_DPI_COMPACT_THRESHOLD = 120;
const VERY_HIGH_DPI_COMPACT_THRESHOLD = 140;

export type LayoutMode = 'wide' | 'normal' | 'compact' | 'high_dpi_compact';

export function useLayout() {
  const width = ref(window.innerWidth);
  const height = ref(window.innerHeight);
  const dpi = ref(window.devicePixelRatio * 96);
  
  const onResize = () => {
    width.value = window.innerWidth;
    height.value = window.innerHeight;
    dpi.value = window.devicePixelRatio * 96;
  };
  
  onMounted(() => window.addEventListener('resize', onResize));
  onUnmounted(() => window.removeEventListener('resize', onResize));
  
  const mode = computed<LayoutMode>(() => {
    if (dpi.value >= VERY_HIGH_DPI_COMPACT_THRESHOLD) return 'high_dpi_compact';
    if (width.value >= WIDE_LAYOUT_MIN_WIDTH && height.value >= WIDE_LAYOUT_MIN_HEIGHT) return 'wide';
    if (width.value < COMPACT_LAYOUT_MIN_WIDTH || dpi.value >= HIGH_DPI_COMPACT_THRESHOLD) return 'compact';
    return 'normal';
  });
  
  return { mode, width, height, dpi };
}
```

---

## 十二、Tauri IPC 契约

### 12.1 完整命令清单（补齐后）

| 命令名 | 当前状态 | 补齐要求 |
|---|---|---|
| `activate_license` | ✅ 存在 | 补多域名容灾 + 签名校验 |
| `verify_license` | ✅ 存在 | 补 Lease 机制 |
| `get_license_status` | ✅ 存在 | 返回完整 RuntimeState |
| **`refresh_lease_if_due`** | ❌ | **新增** |
| **`validate_runtime_continuity`** | ❌ | **新增** |
| **`authorize_task`** | ❌ | **新增**（内部调用） |
| `sync_recent_order_cache` | ✅ 存在 | 补缺口算法 + dirty 检测 |
| `load_order_cache` | ✅ 存在 | 返回完整字段 |
| `get_order_cache_status` | ✅ 存在 | 返回 coverage_complete 等 |
| **`rebuild_order_cache`** | ❌ | **新增** |
| **`fetch_full_scan_orders`** | ❌ | **新增** |
| `find_reviews` | ✅ 存在 | 补风控降级 + 策略分级 |
| `find_quality_refund_orders` | ✅ 存在 | 补原因字段 |
| `update_delivery` | ✅ 存在 | 补快递降级 + 快照保留 |
| `batch_delivery` | ✅ 存在 | 补逐条明细 + carrier_code |
| `set_cookie` | ✅ 存在 | — |
| `get_cookie_status` | ✅ 存在 | — |
| `pick_cookie_save_dir` | ✅ 存在 | — |
| `open_cookie_login` | ✅ 存在 | — |
| `extract_cookie_from_login` | ✅ 存在 | — |
| `get_app_info` | ✅ 存在 | 返回 AUTHOR_WECHAT |
| **`check_for_update`** | ❌ | **新增** |
| **`get_ui_scale`** | ❌ | **新增** |
| **`set_ui_scale`** | ❌ | **新增** |

### 12.2 新增命令契约

```rust
// apps/desktop/src/commands/license.rs
#[tauri::command]
pub async fn refresh_lease_if_due(
    state: State<'_, AppState>,
) -> Result<Option<RuntimeState>, AppError> {
    state.license_service.refresh_lease_if_due().await
}

#[tauri::command]
pub async fn validate_runtime_continuity(
    state: State<'_, AppState>,
) -> Result<bool, AppError> {
    state.integrity_validator.validate()
}

// apps/desktop/src/commands/order.rs
#[tauri::command]
pub async fn rebuild_order_cache(
    app: tauri::AppHandle,
    state: State<'_, AppState>,
) -> Result<OrderCacheStatus, AppError> {
    let progress_emitter = create_progress_emitter(&app, "manual");
    state.order_sync_service
        .rebuild_cache(progress_emitter)
        .await?;
    state.order_sync_service.get_status().await
}

#[tauri::command]
pub async fn fetch_full_scan_orders(
    app: tauri::AppHandle,
    state: State<'_, AppState>,
    earliest_time: i64,
) -> Result<FullScanResult, AppError> {
    let progress_emitter = create_progress_emitter(&app, "full_scan");
    state.order_sync_service
        .fetch_full_scan_orders(earliest_time, progress_emitter)
        .await
}

// apps/desktop/src/commands/system.rs
#[tauri::command]
pub async fn check_for_update() -> Result<UpdateInfo, AppError> {
    let info = fetch_latest_version_info(None).await?;
    Ok(info)
}
```

### 12.3 事件推送

```rust
// 前端监听的事件
app.emit("order-sync-progress", OrderSyncProgress { ... })?;    // 已有
app.emit("license-state-changed", RuntimeState { ... })?;       // 新增
app.emit("integrity-compromised", IntegrityError { ... })?;     // 新增
app.emit("update-available", UpdateInfo { ... })?;              // 新增
app.emit("risk-control-cooldown", CooldownEvent { ... })?;      // 新增
```

---

## 十三、数据模型

### 13.1 OrderDoc（完整版）

```rust
// crates/domain-core/src/order.rs
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OrderDoc {
    pub common_info: CommonInfo,
    pub buyer_info: BuyerInfo,
    pub accept_info: AcceptInfo,
    pub order_status: OrderStatusInfo,
    pub order_product_info: Vec<OrderProduct>,
    pub quality_refund_info: Option<QualityRefundInfo>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CommonInfo {
    pub order_id: String,
    pub create_time: i64,
    pub status: i32,
    pub openid: String,
    pub is_education_order: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BuyerInfo {
    pub nick_name: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AcceptInfo {
    pub confirm_receipt_time: String,    // 原版是字符串形式秒级时间戳
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OrderStatusInfo {
    pub auto_confirm_info: AutoConfirmInfo,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AutoConfirmInfo {
    pub is_waybill_received: bool,
    pub waybill_received_time: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OrderProduct {
    pub product_id: String,
    pub sku_id: String,
    pub sale_param: String,
    pub title: String,
    pub thumb_img: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct QualityRefundInfo {
    pub reason: String,
    pub source: String,  // "quality_refund_api"
}
```

### 13.2 前端类型对齐

```typescript
// ui/src/types/order.ts
export interface OrderCacheEntry {
  order_id: string;
  buyer_name: string;
  create_time: number;
  confirm_receipt_time: number;
  is_waybill_received: boolean;
  waybill_received_time: number;
  is_education_order: boolean;              // 新增
  order_status: number;
  openid: string;                            // 新增
  products: OrderProduct[];                  // 新增
  quality_refund_info?: QualityRefundInfo;   // 新增
}

export interface OrderProduct {
  product_id: string;
  sku_id: string;
  sale_param: string;
  title: string;
  thumb_img: string;                         // 新增
}

export interface OrderCacheStatus {
  total_count: number;
  last_sync_at: number;
  coverage_start: number;
  coverage_end: number;
  coverage_complete: boolean;                // 新增
  missing_segment_count: number;             // 新增
  last_mode: string;                         // 新增
  last_error: string;                        // 新增
}
```

---

## 十四、反风控策略

### 14.1 策略矩阵

| 场景 | 检测信号 | 响应策略 |
|---|---|---|
| HTTP 429 | response.status == 429 | 指数退避 2^n 秒，重试 3 次 |
| API code 429 | result.code == 429 | 同上 |
| 风控 code 430 | result.code == 430 | 冷却 60 秒 + 极速模式 |
| 风控消息 | msg 含"异常行为"/"拒绝访问" | 同上 |
| 连续风控 | 极速模式下再次风控 | 返回已有数据 + 警告 |
| 空页/末尾 | result.code == 10003 或 empty | 早停，不视为失败 |

### 14.2 平台化 User-Agent ⭐

```rust
// crates/security-core/src/http_headers.rs
const WINDOWS_UA: &str = 
    "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 \
     (KHTML, like Gecko) Chrome/144.0.0.0 Safari/537.36";

const MACOS_UA: &str = 
    "Mozilla/5.0 (Macintosh; Intel Mac OS X 10_15_7) \
     AppleWebKit/537.36 (KHTML, like Gecko) Chrome/144.0.0.0 Safari/537.36";

pub fn get_user_agent() -> &'static str {
    #[cfg(target_os = "macos")]
    return MACOS_UA;
    WINDOWS_UA
}

pub fn get_sec_ch_ua_platform() -> &'static str {
    #[cfg(target_os = "macos")]
    return r#""macOS""#;
    r#""Windows""#
}
```

---

## 十五、迁移方案

### 15.1 从 Python 4.3.0 迁移到 Rust 5.1.0

```rust
// apps/desktop/src/migration/legacy_python.rs

pub struct LegacyPythonMigrator {
    home_dir: PathBuf,
    user_data_dir: PathBuf,
}

impl LegacyPythonMigrator {
    pub fn run(&self) -> Result<MigrationReport, MigrationError> {
        let mut report = MigrationReport::default();
        
        // 1. 迁移订单缓存
        if let Err(e) = self.migrate_order_cache() {
            report.errors.push(format!("缓存迁移失败: {}", e));
        } else {
            report.cache_migrated = true;
        }
        
        // 2. 迁移 Cookie
        if let Err(e) = self.migrate_cookie() {
            report.errors.push(format!("Cookie 迁移失败: {}", e));
        } else {
            report.cookie_migrated = true;
        }
        
        // 3. 迁移授权（如果原版 license.json 存在）
        if let Err(e) = self.migrate_license() {
            report.errors.push(format!("授权迁移失败: {}", e));
        } else {
            report.license_migrated = true;
        }
        
        // 4. 迁移用户配置目录指针
        if let Err(e) = self.migrate_config_pointer() {
            report.errors.push(format!("配置指针迁移失败: {}", e));
        }
        
        // 5. 备份原数据到 ~/.tls-shipinhao/legacy_backup/
        self.backup_legacy_data()?;
        
        Ok(report)
    }
    
    fn migrate_order_cache(&self) -> Result<()> { /* ... */ }
    fn migrate_cookie(&self) -> Result<()> { /* ... */ }
    fn migrate_license(&self) -> Result<()> { /* ... */ }
    fn migrate_config_pointer(&self) -> Result<()> { /* ... */ }
    fn backup_legacy_data(&self) -> Result<()> { /* ... */ }
}

pub struct MigrationReport {
    pub cache_migrated: bool,
    pub cookie_migrated: bool,
    pub license_migrated: bool,
    pub errors: Vec<String>,
}
```

### 15.2 首次启动引导

```
App 启动
  ↓
检测到 Python 版遗留数据
  ↓
显示对话框：
  "检测到您已安装旧版 4.3.0，是否自动迁移数据？
  
   将迁移：
   ✅ 订单缓存（xxx 条订单）
   ✅ Cookie 配置
   ✅ 卡密授权
   
   [自动迁移] [手动配置] [稍后提醒]"
  ↓
用户选择"自动迁移"
  ↓
执行 LegacyPythonMigrator.run()
  ↓
显示迁移报告 + 继续流程
```

---

## 十六、验收标准

### 16.1 功能验收检查清单

#### 授权模块（12 项）

- [ ] 1. 主域名可访问 → 激活成功
- [ ] 2. 主域名超时 → 自动切换备用域名
- [ ] 3. 4 个域名都超时 → 显示网络错误
- [ ] 4. 激活后收到完整 Lease（Ed25519 签名）
- [ ] 5. 24h 内无网络可正常使用
- [ ] 6. 24h 后自动续约
- [ ] 7. 72h 后要求重新激活
- [ ] 8. 设备指纹正确获取（三平台测试）
- [ ] 9. Lease 存入 Keychain（macOS）/ Credential Manager（Windows）
- [ ] 10. 任务级授权：差评查询前先调 authorize_task
- [ ] 11. 完整性校验：启动时验证所有关键文件哈希
- [ ] 12. 篡改文件后再次启动 → 显示"程序被篡改"警告

#### 反风控模块（5 项）

- [ ] 13. HTTP 429 触发 2-4-8 秒指数退避
- [ ] 14. 风控 code=430 触发 60 秒冷却
- [ ] 15. 冷却后自动进入极速模式（1 worker + 2.0s 间隔）
- [ ] 16. 极速模式再次风控 → 返回已有数据
- [ ] 17. UA 根据系统自动选择（macOS/Windows）

#### 订单同步（8 项）

- [ ] 18. 首次同步完成后建立 cache_segments 记录
- [ ] 19. 再次同步时通过 cache_segments 跳过已完成窗口
- [ ] 20. 有 500 秒缺口时自动补齐
- [ ] 21. 200 秒以下缺口直接忽略
- [ ] 22. dirty sale_param 检测 → 自动重建
- [ ] 23. fetch_full_scan_orders 支持查询 60 天前评价
- [ ] 24. 缓存包含 order_products、is_education_order 等完整字段
- [ ] 25. 从 Python 版升级自动迁移本地缓存

#### 评价匹配（6 项）

- [ ] 26. 买家改名加数字尾巴 → 识别为 95 分
- [ ] 27. 长昵称包含短昵称 → 90 分
- [ ] 28. 匿名/微信用户/默认昵称 → 直接 0 分
- [ ] 29. 差评可回复期检测正确（-30 天阈值）
- [ ] 30. 匹配结果包含 strategy 字段（exact_match/high_confidence/...）
- [ ] 31. 品退订单包含 reason 字段

#### 发货管理（3 项）

- [ ] 32. 粘贴 SF 开头快递单号 + 选择中通 → 自动降级为 SF
- [ ] 33. 物流更新后仅 waybillId 改变，其他字段不变
- [ ] 34. initShipData 失败 → 自动回退 orderDetail

#### UI/UX（6 项）

- [ ] 35. 窗口标题显示"驼铃·视频小店差评处理 5.1.0"
- [ ] 36. 主题色为翠绿（#059669）
- [ ] 37. 支持 UI 缩放（0.82-1.0，Ctrl + / -）
- [ ] 38. 小于 860px 宽度自动进入紧凑布局
- [ ] 39. 启动时自动检查更新
- [ ] 40. 有新版本时顶部显示横幅 + notes

### 16.2 性能验收

| 指标 | 目标 |
|---|---|
| 冷启动时间 | < 2 秒 |
| 订单同步 1000 条 | < 60 秒（正常模式） |
| 评价匹配 100 条 | < 5 秒 |
| 单条发货 | < 3 秒 |
| 批量发货 100 条 | < 300 秒 |
| 安装包体积 | < 30 MB |
| 运行内存 | < 200 MB |

### 16.3 兼容性验收

| 维度 | 要求 |
|---|---|
| 操作系统 | macOS 11+, Windows 10+ |
| 微信小店后台 | 接口契约与 4.3.0 一致 |
| 本地数据格式 | 兼容 Python 版 SQLite |
| 授权卡密 | 4.3.0 激活的卡密可直接使用 |

---

## 十七、工作分解与排期

### 17.1 WBS（工作分解结构）

```
5.1.0 补齐项目（18 周）
├── M1 反风控（3 周）
│   ├── 平台化 UA（2 天）
│   ├── 429 限流重试（3 天）
│   ├── 风控检测（2 天）
│   ├── 冷却与降级模式（4 天）
│   └── 集成测试（4 天）
├── M2 授权安全（4 周）
│   ├── 多域名容灾（3 天）
│   ├── Lease 数据结构（3 天）
│   ├── Ed25519 签名校验（4 天）
│   ├── 设备指纹三平台（5 天）
│   ├── Keychain 集成（3 天）
│   ├── 任务级授权（3 天）
│   ├── 完整性校验（4 天）
│   ├── Worker 端 API 对接（5 天）
│   └── 集成测试（4 天）
├── M3 数据兼容（3 周）
│   ├── Schema 补齐 4 表（3 天）
│   ├── 缺口补齐算法（3 天）
│   ├── dirty 检测与修复（2 天）
│   ├── 缓存迁移（4 天）
│   ├── 迁移 UI（2 天）
│   └── 回归测试（4 天）
├── M4 业务细节（3 周）
│   ├── 智能昵称匹配（4 天）
│   ├── 匹配策略分级（2 天）
│   ├── 通用昵称过滤（1 天）
│   ├── 可回复期检测（2 天）
│   ├── 品退字段补齐（2 天）
│   ├── 快递公司降级（3 天）
│   ├── 物流快照保留（2 天）
│   └── 全量扫描模式（2 天）
├── M5 UI 还原（3 周）
│   ├── 翠绿主题（4 天）
│   ├── UI 缩放（2 天）
│   ├── 多布局适配（4 天）
│   ├── 品牌还原（1 天）
│   ├── 在线更新（3 天）
│   ├── 更新横幅（2 天）
│   └── UX 走查（4 天）
└── M6 回归测试（2 周）
    ├── 功能用例（5 天）
    ├── 真实数据对比测试（3 天）
    ├── 性能压测（2 天）
    └── 打包发布（4 天）
```

### 17.2 人力估算

| 角色 | 人数 | 投入 |
|---|---|---|
| Rust 工程师 | 2 | 4 人月 |
| 前端工程师 | 1 | 2 人月 |
| Cloudflare Worker | 1 | 0.5 人月 |
| QA | 1 | 1.5 人月 |
| PM | 1 | 1 人月 |
| **合计** | — | **~9 人月** |

### 17.3 关键路径

```
M2 授权安全 → M3 数据兼容 → M6 回归测试
     ↘        ↗
      M1 反风控
          ↓
     M4 业务细节
          ↓
     M5 UI 还原
```

---

## 十八、风险与假设

### 18.1 技术风险

| 风险 | 可能性 | 影响 | 缓解 |
|---|---|---|---|
| Lease Ed25519 私钥泄露 | 低 | 致命 | HSM 存储 + 定期轮换 |
| 微信小店接口结构变更 | 中 | 严重 | 抓包监控 + 动态适配 |
| Keychain 不可用（CI/Linux） | 中 | 中 | 加密文件后备方案 |
| macOS 沙箱影响 ioreg | 低 | 中 | 降级到 `hostname` + `sysctl` |
| Tauri 2 API 破坏性变更 | 低 | 中 | 锁定版本 + 升级测试 |
| Worker D1 配额限制 | 中 | 严重 | 成本评估 + 请求去重 |

### 18.2 业务风险

| 风险 | 可能性 | 影响 | 缓解 |
|---|---|---|---|
| 用户数据迁移失败 | 中 | 严重 | 备份原数据 + 回滚机制 |
| 新版不支持旧卡密 | 低 | 致命 | 契约兼容性测试 |
| 真实用户反馈匹配率下降 | 中 | 严重 | 用真实数据回归测试 |
| 反风控策略失效 | 中 | 致命 | 灰度发布 + 快速热修 |

### 18.3 假设前提

1. **Cloudflare Worker D1 容量足够**：预计当前卡密数量不会超过 D1 免费额度
2. **Ed25519 私钥由运维保管**：客户端只验签
3. **用户不会大规模并发**：桌面端每用户 QPS < 5
4. **微信接口基本稳定**：1 个月内主要参数不会变更

---

## 附录 A：Python 到 Rust 映射表

| Python 函数 | Rust 对应 | 文件 |
|---|---|---|
| `activate_license(key)` | `LicenseService::activate` | `crates/license-service/src/lib.rs` |
| `authorize_task(task)` | `LicenseService::authorize_task` | 同上 |
| `check_stored_license()` | `LicenseService::verify_with_refresh` | 同上 |
| `check_stored_license_local()` | `LicenseService::verify_local` | 同上 |
| `get_device_id()` | `security_core::device_id::get_device_id` | `crates/security-core/` |
| `verify_signed_lease()` | `LeaseVerifier::verify` | 同上 |
| `validate_runtime_continuity()` | `IntegrityValidator::validate` | 同上 |
| `OrderCacheRepository` | `SqliteOrderCacheRepository` | `apps/desktop/src/adapters/` |
| `OrderSyncService` | `OrderSyncService` | `crates/desktop-services/` |
| `BadReviewOrderFinder` | `ReviewMatcher` | 同上 |
| `compute_match_score()` | `score::compute` | `crates/desktop-services/src/matching/` |
| `similarity_percent()` | `nickname::similarity_percent` | 同上 |
| `update_single_order()` | `DeliveryGateway::update_single` | `apps/desktop/src/adapters/` |
| `fetch_latest_version_info()` | `UpdateService::check` | `crates/desktop-services/` |

## 附录 B：PRD 完成度自检

- [x] 覆盖了所有 Python 原版识别的功能点
- [x] 明确了验收标准（40 项）
- [x] 提供了技术实现规范（Rust 代码骨架）
- [x] 定义了数据模型
- [x] 排期与人力估算
- [x] 风险评估
- [x] 迁移方案
- [x] API 契约（Tauri IPC）
- [x] UI 规范（主题、布局、缩放）
- [x] 兼容性要求

---

## 附录 C：参考文档

- 原版深度分析：`/Users/zxr/Downloads/source-code/TLS-shipinhao/docs/原版Python项目深度分析报告.md`
- 当前状态分析：`/Users/zxr/Downloads/source-code/TLS-shipinhao/docs/项目深度分析报告.md`
- 产品优化建议：`/Users/zxr/Downloads/source-code/TLS-shipinhao/docs/产品优化建议与功能完整性检查.md`

---

*本 PRD 是让 Rust 5.1.0 与 Python 4.3.0 功能完全对齐的完整规范。所有条款均来自真实源代码对比，可直接用于开发与验收。*

*PRD 作者：Claude（产品经理视角）*  
*版本：v1.0*  
*生效日期：2026-04-16*
