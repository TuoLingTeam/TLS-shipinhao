# TLS-shipinhao 原版 Python 项目深度分析报告

> 分析对象：`/Users/zxr/Downloads/source-code/TLS-shipinhao/_legacy/app`  
> 生成时间：2026-04-16  
> 原版产品名：**驼铃·视频小店差评处理**  
> 原版版本：`4.3.0`  
> 作者微信：`TLS-801`  
> 更新源：`https://gitee.com/tuolingshe/tuoling-shipinhao/raw/master/version.json`

---

## 执行摘要（TL;DR）

**原版 Python 实现是一款成熟完整的商业桌面工具**，经过多版本迭代到 4.3.0，功能完整度远超当前 Rust 重构版（5.0.0）。重构过程中**流失了 20+ 项核心功能**，主要集中在：

- **授权与安全**（多域名容灾、Lease 租约、任务级授权、设备指纹、完整性校验、Keychain）
- **微信反风控**（429 限流重试、风控检测、自动降级、冷却机制）
- **订单同步**（缓存段管理、缺口补齐、全量扫描、dirty 数据检测）
- **评价匹配**（昵称智能识别、可回复期检测、品退合并、匹配策略分级）
- **发货策略**（快递公司自动降级、物流快照保留）
- **用户体验**（在线更新、UI 缩放、多布局、主题配色）

---

## 一、项目基本信息

### 1.1 代码规模

| 模块 | 文件数 | 总行数 |
|---|---|---|
| 入口 | 3（`main.py` / `bootstrap.py` / `__init__.py`） | 57 |
| 配置 | 1（`settings.py`） | 425 |
| Core 核心层 | 5 | 1,294 |
| Services 服务层 | 9 | 3,003 |
| UI 层（源码已删除，仅 `.pyc`） | 9 模块 | 估计 2,000+ |
| **合计（源码）** | **18** | **4,681** |

### 1.2 文件结构

```
_legacy/app/
├── __init__.py
├── main.py                      # 兼容入口（委托 Rust desktop）
├── bootstrap.py                 # 启动脚本（调用 cargo run -p desktop）
├── settings.py                  # 全局配置（425 行，含 UI 主题/API/授权常量）
├── assets/
│   └── favicon.png
├── core/                        # 核心能力
│   ├── __init__.py
│   ├── day_window.py            # 自然日时间窗口（31 行）
│   ├── http_utils.py            # HTTP 工具 + 平台 UA（138 行）
│   ├── license.py               # 授权兼容层（37 行，委托 security_runtime）
│   └── security_runtime.py      # 安全运行时 + Lease 租约（1087 行）
└── services/                    # 业务服务
    ├── __init__.py
    ├── delivery_api.py          # 发货 API + 快递公司降级（352 行）
    ├── order_cache.py           # 订单 SQLite 缓存（537 行）
    ├── order_field_utils.py     # 订单字段规范化（57 行）
    ├── order_match_scoring.py   # 匹配评分算法（285 行）
    ├── order_sync.py            # 订单同步服务（237 行）
    ├── review_matcher.py        # 评价匹配核心 + 风控降级（1354 行，最大文件）
    ├── update_service.py        # 在线更新检查（68 行）
    └── versioning.py            # 版本号比较（14 行）
```

### 1.3 UI 层（源码已删除，但 `.pyc` 残留揭示原有结构）

通过 `app/ui/__pycache__/` 可推断原 UI 模块：

| Pyc 文件 | 推断职责 |
|---|---|
| `window.pyc` | 主窗口（QMainWindow） |
| `window_view.pyc` | 主窗口视图 |
| `window_dialogs.pyc` | 对话框集合 |
| `widgets.pyc` | 自定义组件（卡片、徽章、进度等） |
| `cookie_dialog.pyc` | Cookie 配置对话框 |
| `review_worker.pyc` | 评价查询后台线程 |
| `batch_worker.pyc` | 批量发货后台线程 |
| `update_worker.pyc` | 在线更新检查后台线程 |

**技术推测**：基于 Python 3.14、`cryptography`、窗口级架构，UI 框架为 **PySide6/PyQt6**（最主流的 Python 桌面框架）。

---

## 二、代码语言与技术栈

### 2.1 语言与框架

| 维度 | 技术 |
|---|---|
| **语言** | Python 3.14（`.cpython-314.pyc`） |
| **UI 框架（推断）** | PySide6 / PyQt6 |
| **HTTP 客户端** | `requests` |
| **加密** | `cryptography`（Ed25519 签名校验） |
| **数据库** | SQLite3（stdlib） |
| **并发** | `threading` + `concurrent.futures.ThreadPoolExecutor` |
| **操作系统集成** | `ioreg`（macOS）/ `wmic`/`powershell`（Windows）/ `/etc/machine-id`（Linux） |
| **原生加速（可选）** | `security_core` 动态库（ctypes 绑定） |

### 2.2 外部依赖（推断）

```text
# 运行时依赖
requests                # HTTP 请求
cryptography            # Ed25519 签名验证
PySide6 或 PyQt6        # 桌面 UI（推断）

# 标准库
sqlite3, threading, concurrent.futures
hashlib, base64, json, logging
subprocess, platform
pathlib, dataclasses, datetime, typing
```

### 2.3 打包方式

- 支持 **PyInstaller 冻结**（`getattr(sys, "frozen", False)` 检测）
- 目标平台：**macOS**（优先）、**Windows**
- 配置目录：
  - macOS: `~/Library/Application Support/TLS-shipinhao/`
  - Windows: `%LOCALAPPDATA%\TLS-shipinhao\`
  - Linux: `~/.local/share/TLS-shipinhao/`
- 用户配置：`~/.tls-shipinhao/`

---

## 三、架构设计

### 3.1 整体架构：经典分层

```
┌─────────────────────────────────────────────┐
│           UI 层（PyQt/PySide，已删）          │
│  window / widgets / dialogs / workers       │
└───────────────────┬─────────────────────────┘
                    │ 函数调用 + 工作线程
                    ▼
┌─────────────────────────────────────────────┐
│                Services 层                   │
│  review_matcher / order_sync / order_cache  │
│  delivery_api / update_service              │
└───────────────────┬─────────────────────────┘
                    │
                    ▼
┌─────────────────────────────────────────────┐
│                  Core 层                     │
│  security_runtime / http_utils / license    │
│  day_window                                 │
└───────────────────┬─────────────────────────┘
                    │
                    ▼
┌─────────────────────────────────────────────┐
│           External / Storage                 │
│  微信小店 API / Cloudflare Worker / SQLite    │
│  Keychain / 设备硬件 / Ed25519 公钥            │
└─────────────────────────────────────────────┘
```

### 3.2 关键设计模式

| 模式 | 应用场景 |
|---|---|
| **分层架构** | UI → Services → Core → 外部 |
| **Repository 模式** | `OrderCacheRepository`（SQLite 数据访问封装） |
| **工作线程** | `review_worker`、`batch_worker`、`update_worker`（避免 UI 卡顿） |
| **Dataclass** | `UpdateInfo`、`RuntimeState`、`RuntimeGrant` |
| **兼容层** | `core/license.py` 对外保留旧 API，内部委托 `security_runtime` |
| **回调通知** | `on_progress(message)`（进度回调）、`on_window_completed`（持久化回调） |
| **异常降级** | `OrderRiskControlError` 触发极速模式 |

---

## 四、功能逻辑详细分析

### 4.1 settings.py：全局配置中心（425 行）

**核心配置块**：

#### 4.1.1 API 端点

```python
ORDER_DETAIL_URL           = "https://store.weixin.qq.com/shop-faas/mmchannelstradeorder/detail/cgi/orderDetail"
ORDER_INIT_SHIP_DATA_URL   = "https://store.weixin.qq.com/shop-faas/mmchannelstradeorder/ship/cgi/initShipData"
ORDER_DELIVERY_UPDATE_URL  = "https://store.weixin.qq.com/shop-faas/mmchannelstradeorder/ship/cgi/updateDeliveryInfo"
EVALUATION_SEARCH_URL      = "https://store.weixin.qq.com/shop-faas/mmchannelstradeevaluation/cgi/search"
ORDER_SEARCH_URL           = "https://store.weixin.qq.com/shop-faas/mmchannelstradeorder/list/cgi/orderSearch"
QUALITY_REFUND_ORDER_URL   = "https://store.weixin.qq.com/shop-faas/statistic/dsr/product/refund/order"
```

#### 4.1.2 授权后端（多域名容灾）⭐ Rust 版缺失

```python
LICENSE_API_BASE_URLS = [
    "https://sphapi.199908.top",
    "https://sphapi.tuoling.ccwu.cc",
    "https://sphapi.tuoling.us.ci",
    "https://sphapi.tuoling.eu.cc",
]
```

**设计意图**：单一域名被墙/被封时可自动切换到备用域名，保证授权可用性。

#### 4.1.3 授权协议参数

| 参数 | 值 | 含义 |
|---|---|---|
| `LICENSE_PROTOCOL_VERSION` | 3 | 协议版本 |
| `LICENSE_API_TIMEOUT` | 10 秒 | API 超时 |
| `LICENSE_STATUS_CACHE_TTL_SECONDS` | 60 秒 | 本地缓存 TTL |
| `LICENSE_LEASE_RENEWAL_HOURS` | 24 小时 | Lease 刷新窗口 |
| `LICENSE_LEASE_HARD_EXPIRY_HOURS` | 72 小时 | Lease 硬过期 |
| `LICENSE_RUNTIME_GRANT_MINUTES` | 30 分钟 | 任务级授权凭证 |
| `LICENSE_REQUIRE_ONLINE_FOR_TASKS` | True | 任务必须在线授权 |
| `LICENSE_PUBLIC_KEY` | Ed25519 公钥 | 服务端签名校验 |

#### 4.1.4 任务级授权策略 ⭐ Rust 版缺失

```python
LICENSE_TASK_REVIEW_FIND       = "review_find"        # 差评查询
LICENSE_TASK_REVIEW_FULL_SCAN  = "review_full_scan"   # 全量评价扫描
LICENSE_TASK_QUALITY_REFUND    = "quality_refund"     # 品退查询
LICENSE_TASK_BATCH_DELIVERY    = "batch_delivery"     # 批量发货
LICENSE_TASK_CACHE_MANAGE      = "cache_manage"       # 缓存管理
```

**原理**：每次执行任务前先向服务端申请"任务令牌"（30 分钟有效），实现**精细化权限控制**。

#### 4.1.5 订单缓存策略

| 参数 | 值 | 含义 |
|---|---|---|
| `ORDER_CACHE_COVERAGE_DAYS` | 30 | 缓存覆盖范围 |
| `ORDER_CACHE_INCREMENTAL_DAYS` | 3 | 增量刷新范围 |
| `ORDER_CACHE_INCREMENTAL_OVERLAP_DAYS` | 1 | 增量重叠天数（防漏） |
| `ORDER_CACHE_SCOPE` | `"orders_30d"` | 缓存域标识 |
| `ORDER_CACHE_DB_NAME` | `order_cache.sqlite3` | DB 文件名 |

#### 4.1.6 网络抓取策略

| 参数 | 值 |
|---|---|
| `ORDER_PAGE_SIZE` | 100 |
| `FETCH_PAGE_INTERVAL_SECONDS` | 0.3 |
| `ORDER_WINDOW_WORKERS` | 3（正常模式并发） |
| `ORDER_RISK_WINDOW_WORKERS` | 1（降级模式并发） |
| `ORDER_RISK_PAGE_INTERVAL_SECONDS` | 2.0（降级模式间隔） |
| `RATE_LIMIT_RETRY_COUNT` | 3（限流重试次数） |

#### 4.1.7 评价/匹配参数

| 参数 | 值 | 说明 |
|---|---|---|
| `EVALUATION_PAGE_SIZE` | 10 | 评价每页条数 |
| `EVALUATION_MAX_PAGES` | 10 | 最大页数 |
| `EVALUATION_MAX_DAYS` | 30 | 评价查询最大天数 |
| `EDUCATION_ORDER_MAX_DAYS` | 60 | 教育订单回溯天数 |
| `MATCH_MIN_SCORE` | 50 | 最低匹配分数 |
| `AUTO_FILL_SCORE_THRESHOLD` | 100 | 自动预填阈值 |
| `DEFAULT_REVIEW_DAYS` | 30 | 默认查询天数 |

#### 4.1.8 UI 布局与主题 ⭐ Rust 版缺失

```python
# 窗口
APP_VERSION             = "4.3.0"
WINDOW_TITLE            = f"驼铃·视频小店差评处理 {APP_VERSION}"
AUTHOR_WECHAT           = "TLS-801"
DEFAULT_WINDOW_WIDTH    = 880
DEFAULT_WINDOW_HEIGHT   = 830
MIN_WINDOW_WIDTH        = 800
MIN_WINDOW_HEIGHT       = 700
MIN_UI_SCALE            = 0.82
MAX_UI_SCALE            = 1.0

# 多布局适配
WIDE_LAYOUT_MIN_WIDTH       = 1320  # 宽屏布局
WIDE_LAYOUT_MIN_HEIGHT      = 780
COMPACT_LAYOUT_MIN_WIDTH    = 860   # 紧凑布局
HIGH_DPI_COMPACT_THRESHOLD  = 120   # 高 DPI
VERY_HIGH_DPI_COMPACT_THRESHOLD = 140

# 主题配色（翠绿系）
APP_COLORS = {
    "window_base":  "#3A3D38",
    "bg":           "#ECFDF5",   # 柔绿背景
    "surface":      "#FFFFFF",
    "border":       "#A7F3D0",
    "text":         "#064E3B",
    "blue":         "#059669",   # 主色
    "orange":       "#F97316",   # 强调色
    "red":          "#B91C1C",   # 警告
    # ... 完整 20+ 色彩变量
}
```

### 4.2 core/security_runtime.py：安全运行时（1087 行）⭐ 核心亮点

**这是原版项目最核心的文件，Rust 版丢失了大量细节**。

#### 4.2.1 核心数据类

```python
@dataclass
class RuntimeState:
    license_key: str                    # 卡密
    device_id: str                      # 设备 ID
    reason: str                         # 状态（ok/expired/revoked/...）
    status_hint: str                    # 状态提示
    license_expires_at: str             # 授权到期
    lease_expires_at: str               # 租约到期
    renew_after: str                    # 刷新时间点
    last_verify_at: str                 # 最后校验
    risk_level: str                     # 风险等级（low/medium/high）
    task_policy: list[str]              # 允许的任务列表
    compromised: bool                   # 是否被破解
    runtime_backend: str                # 运行时后端（python/native）

@dataclass
class RuntimeGrant:
    task_type: str                      # 任务类型
    granted: bool                       # 是否授权
    grant_id: str                       # 授权 ID
    valid_until: str                    # 有效期
    risk_level: str                     # 风险等级
    degraded_reason: str                # 降级原因
    state: RuntimeState | None          # 状态快照
```

#### 4.2.2 授权状态机（完整版）⭐ Rust 版大幅简化

| 状态常量 | Python | Rust 版 |
|---|---|---|
| `_REASON_OK` | ✅ | ✅ |
| `_REASON_NOT_FOUND` | ✅ | ⚠️ 部分 |
| `_REASON_INVALID` | ✅ | ✅ |
| `_REASON_EXPIRED` | ✅ | ✅ |
| `_REASON_DEVICE_MISMATCH` | ✅ | ✅ |
| `_REASON_REACTIVATION_REQUIRED` | ✅ | ❌ |
| `_REASON_REVOKED` | ✅ | ✅ |
| `_REASON_ONLINE_REFRESH_REQUIRED` | ✅ | ❌ |
| `_REASON_RENEWAL_DUE` | ✅ | ✅ |
| `_REASON_COMPROMISED` | ✅ | ✅ |

#### 4.2.3 设备指纹机制 ⭐ Rust 版缺失

**三平台适配**：

- **macOS**：`ioreg -rd1 -c IOPlatformExpertDevice` 提取 `IOPlatformSerialNumber`
- **Windows**：`wmic csproduct get UUID` + PowerShell 回退
- **Linux**：读 `/etc/machine-id` / `/var/lib/dbus/machine-id`
- **Fallback**：`platform.node() + platform.machine() + platform.system()` 组合后 SHA256 截取 16 位

#### 4.2.4 Keychain / Credential Manager 集成 ⭐ Rust 版缺失

```python
_KEYCHAIN_SERVICE = "com.tuoling.tls-shipinhao.runtime"
_KEYCHAIN_ACCOUNT = "runtime_bundle"
```

**原理**：将敏感的 `runtime_bundle.json`（含 Lease token）存储到 OS 级安全存储：
- macOS: **Keychain**
- Windows: **Credential Manager**
- 后备：文件系统（加密）

#### 4.2.5 原生加速绑定 ⭐ Rust 版缺失

```python
_NATIVE_CORE_BINDINGS = None
SECURITY_CORE_LIBRARY_BASENAME = "security_core"
```

通过 `ctypes` 动态加载 `security_core.dylib`/`.dll`/`.so`，提供：
- 设备 ID 原生读取（`_native_get_device_id`）
- Lease 签名验证原生实现（`_native_verify_signed_lease`）
- 抗逆向、反调试加固

#### 4.2.6 Lease 租约机制 ⭐ Rust 版缺失

**流程**：

```
用户激活（activate_license）
  ↓
服务端签发 Lease token（Ed25519 签名）
  {
    "kind": "license_lease",
    "device_id": "xxx",
    "exp": 1730000000,
    "renew_after": 1729000000,
    ...
  }
  ↓
本地存 Keychain / 文件
  ↓
每次启动/使用前 validate_runtime_continuity
  ↓
到达 renew_after 时间点 → refresh_lease_if_due（静默续约）
  ↓
到达 exp 时间点 → 强制重新 activate
```

**优点**：
- 减少对服务端 API 的依赖（72 小时离线可用）
- 支持吊销（revoke）
- 防止本地时间篡改（签名保护）

#### 4.2.7 完整性校验清单 ⭐ Rust 版缺失

```python
INTEGRITY_MANIFEST_FILE_NAME = "integrity_manifest.json"
INTEGRITY_MANIFEST_PUBLIC_KEY = LICENSE_PUBLIC_KEY
```

**原理**：
1. 打包时生成 `integrity_manifest.json`，记录所有关键文件的 SHA256
2. 服务端用 Ed25519 私钥签名该 manifest
3. 启动时客户端用公钥校验签名，然后比对所有文件哈希
4. 发现篡改 → 标记 `compromised = true` → 禁用功能

**防御场景**：防止用户替换代码、注入恶意 dll、破解授权。

---

### 4.3 services/order_sync.py：订单同步服务（237 行）

#### 4.3.1 三种同步模式

| 方法 | 场景 | 说明 |
|---|---|---|
| `rebuild_cache` | 首次使用 / 强制重建 | 清空后全量拉取 30 天 |
| `refresh_cache` | 日常使用 | 增量刷新最近 3 天（含 1 天重叠） |
| `ensure_orders` | 匹配前确保缓存 | 自动补齐缺口 + 触发刷新 |
| `fetch_full_scan_orders` | 超长历史查询 | 30 天用缓存 + 更早临时抓取 |

#### 4.3.2 缓存段（Cache Segments）管理 ⭐ Rust 版不完整

**核心设计**：用 `cache_segments` 表记录每个时间窗口是否已完整抓取。

```sql
CREATE TABLE cache_segments (
    scope TEXT NOT NULL,
    start_ts INTEGER NOT NULL,
    end_ts INTEGER NOT NULL,
    status TEXT NOT NULL DEFAULT 'complete',
    updated_at INTEGER NOT NULL DEFAULT 0,
    PRIMARY KEY (scope, start_ts, end_ts)
);
```

**缺口计算算法**（`get_missing_segments`）：
1. 查询指定范围内所有 `complete` 段
2. 合并相邻段（`merge_tolerance = 120 秒`）
3. 计算剩余缺口
4. 丢弃太小的缺口（`min_gap_width = 300 秒`）

#### 4.3.3 Dirty 数据检测与自动修复 ⭐ Rust 版缺失

```python
def has_dirty_sale_param(self) -> bool:
    """检测是否存在 str(list) 污染的 sale_param 历史数据。"""
    row = connection.execute(
        "SELECT COUNT(*) AS cnt FROM order_products WHERE sale_param LIKE '[%'"
    ).fetchone()
    return (row["cnt"] if row else 0) > 0

# 在 _ensure_recent_cache 中：
if state and self.repository.has_dirty_sale_param():
    self._progress(on_progress, "[缓存] 检测到历史数据格式异常，自动清空并重建缓存。")
    need_rebuild = True
```

**背景**：历史版本曾误将 Python list 用 `str()` 存入数据库（如 `"['大码', '红色']"`），新版本检测到后自动修复。

#### 4.3.4 缓存迁移

```python
def _migrate_legacy_cache_if_needed(self) -> None:
    # 自动把旧版本的 cache 文件（~/.tls-shipinhao/order_cache.sqlite3）
    # 迁移到新版本位置（~/Library/.../TLS-shipinhao/cache/order_cache.sqlite3）
    # 包括 wal/shm 边车文件
```

---

### 4.4 services/review_matcher.py：评价匹配核心（1354 行）⭐ 最复杂模块

#### 4.4.1 订单抓取：页码并行 + 风控降级 ⭐ Rust 版关键缺失

**策略背景**：微信小店 `orderSearch` API **忽略 createTimeStart/End**，时间窗口无法在服务端过滤，只能用**页码偏移 + 客户端早停**实现并行。

**正常模式**（`_fetch_orders_by_page`）：

```
3 个 worker 共享自增页码计数器（page_lock）
  ↓
每个 worker 抢一个页码 → 拉取 → 客户端过滤 earliest_time
  ↓
每 1000 条订单批量触发 on_batch_completed 回调（持久化）
  ↓
遇到 code=10003（末尾）或空页 → 早停
  ↓
0.3 秒间隔（避免触发限流）
```

**限流处理**（`_retry_order_search_on_limit`）：

```
HTTP 429 或 code=429
  ↓
指数退避：2^(retry+1) 秒（即 2s、4s、8s）
  ↓
重试 3 次后仍失败 → 抛异常
```

**风控降级**（`OrderRiskControlError`）：

```
检测：code=430 或 msg 含"异常行为"/"拒绝访问"
  ↓
触发 OrderRiskControlError
  ↓
冷却 60 秒（每 10 秒打一次进度）
  ↓
切换到"极速模式"：1 个 worker + 2.0 秒间隔
  ↓
合并已抓取 + 新抓取的订单
  ↓
即使再次风控，仍返回已有部分订单 + 警告
```

**Rust 版对比**：
- ✅ 已有：基础并发抓取
- ❌ 缺失：限流 429 重试、风控检测、自动降级、冷却机制

#### 4.4.2 差评获取（`get_bad_evaluations`）

- 分页拉取（每页 10 条，最多 10 页）
- 过滤 `attitudeName == "不够好"` 的评价
- 检查 `canReplyExpireTime` 是否在可回复期内（-30 天内）
- 自动根据 `totalCnt` 计算实际页数

#### 4.4.3 匹配算法（完整还原）

**索引构建（`_build_product_sku_index`）**：

```python
product_sku_index = {}
for order in orders:
    for product in order.orderProductInfo:
        # ID 键：id::productId::skuId
        id_key = f"id::{product_id}::{sku_id}"
        # 值键：value::normalized_name::normalized_sku
        value_key = f"value::{name_norm}::{sku_norm}"
        # 两个键都加入索引
```

**候选订单收集**：对每条评价，根据产品 ID 和名称两个维度从索引中找候选。

**评分（`compute_match_score`）**：

```
买家昵称匹配：
  - 完全一致 → 100
  - 去尾部数字后一致 → 95
  - 包含关系（shorter in longer, 长度 >= 3）→ 90
  - 子序列（长度 >= 4）→ 85
  - 其他 → SequenceMatcher 比值

商品匹配：
  - productId 一致：+40 权重
  - skuId 一致：+40 权重
  - 标题相似度：+20 权重

综合计分：
  - 双方都完全一致 → 100
  - 一边完全一致，另一边相似 → 100 - 另一边扣分
  - 两边都仅相似 → 100 - 两边平均扣分
  - 下限 50
```

**匹配策略分级** ⭐ Rust 版缺失：

```python
def _match_strategy_by_score(score):
    if score >= 100: return "exact_match"
    if score >= AUTO_FILL_SCORE_THRESHOLD (100): return "high_confidence"
    if score >= MATCH_MIN_SCORE (50): return "probable_match"
    return "fallback"
```

#### 4.4.4 智能昵称识别 ⭐ Rust 版缺失

**通用昵称过滤**：

```python
_GENERIC_NICKNAME_PREFIXES = ("匿名", "微信用户", "默认昵称")

def _is_generic_nickname(cls, name):
    if not name: return True
    return any(name.startswith(prefix) for prefix in cls._GENERIC_NICKNAME_PREFIXES)
```

**改名场景识别**（`_nickname_similarity_by_rename_patterns`）：
- 去除尾部数字（如 `小明123` → `小明`）后比对
- 包含关系（短昵称包含在长昵称中）
- 子序列（保持顺序的字符序列）

**单字符保守处理**（`_single_char_containment_similarity`）：避免 `"a"` 出现在任何 10 字昵称中就判定相似。

#### 4.4.5 品退订单合并 ⭐ Rust 版存在但简化

```python
def merge_quality_refund_orders(self, orders, earliest_time, on_progress):
    base_orders = self.deduplicate_orders_by_id(orders)
    quality_refund_orders = self.get_quality_refund_orders(earliest_time, on_progress)
    merged_orders = self.deduplicate_orders_by_id(base_orders + quality_refund_orders)
    # 品退订单转换为统一订单结构（_build_quality_refund_order_stub）
```

**特殊字段**：品退订单带 `qualityRefundInfo.reason`（退款原因），可用于产品质量分析。

---

### 4.5 services/delivery_api.py：发货 API（352 行）

#### 4.5.1 核心流程

```
update_single_order(order_id, tracking_number, session)
  ↓
fetch_current_delivery_context：
  1. 优先调 initShipData 接口获取物流信息
  2. 失败后回退 orderDetail 接口
  3. 提取 deliveryId、deliveryName、waybillId、productInfos
  ↓
build_update_delivery_payload：
  - old_info：原物流对象
  - new_info：仅修改 waybillId
  - changeInfo: [{old, new}]
  ↓
POST updateDeliveryInfo
  ↓
如果失败且错误包含"快递单号与所选物流商不匹配"：
  - 用 tracking_number[:2] 作为 deliveryId 重试
  - 例如 SF0001 → deliveryId="SF"
  ↓
返回 old_waybill（供审计）
```

#### 4.5.2 快递公司自动降级 ⭐ Rust 版缺失

```python
def _is_delivery_mismatch_error(exc):
    return any(marker in str(exc) for marker in (
        DELIVERY_MISMATCH_MESSAGE,   # "快递单号与所选物流商不匹配"
        "快递单号有误",
    ))

# 重试逻辑：
try:
    update_delivery_info(order_id, tracking_number, delivery_product_info, session)
except RuntimeError as exc:
    if _is_delivery_mismatch_error(exc):
        tracking_prefix = str(tracking_number).strip()[:2]
        if tracking_prefix and tracking_prefix != current_delivery_id:
            # 用单号前缀作为 deliveryId 重试
            override = {"deliveryId": tracking_prefix, "deliveryName": ""}
            update_delivery_info(..., override)
```

**产品价值**：用户不需要手动选择快递公司，只要粘贴单号，系统自动推断。

#### 4.5.3 旧物流快照

**设计意图**：

- 保留原有 `deliveryProductInfo`（完整复制）
- 只修改 `waybillId`
- **与小店手动操作行为一致** — 不改变其他字段，减少被风控的风险

---

### 4.6 core/http_utils.py：HTTP 工具（138 行）

#### 4.6.1 平台化 User-Agent ⭐ Rust 版可能缺失

```python
_BASE_USER_AGENT = (
    "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 "
    "(KHTML, like Gecko) Chrome/144.0.0.0 Safari/537.36"
)
_MACOS_USER_AGENT = (
    "Mozilla/5.0 (Macintosh; Intel Mac OS X 10_15_7) "
    "AppleWebKit/537.36 (KHTML, like Gecko) Chrome/144.0.0.0 Safari/537.36"
)
```

根据 `platform.system()` 动态选择 UA，**模拟真实浏览器**，降低被风控概率。

#### 4.6.2 统一请求头

```python
{
    "Accept": "application/json, text/plain, */*",
    "Content-Type": "application/json",
    "Origin": "https://store.weixin.qq.com",
    "biz_magic": magic,              # 从 Cookie 提取
    "potter-scene": "weixinShop",
    "sec-ch-ua": '"Not(A:Brand";v="8", "Chromium";v="144", "Google Chrome";v="144"',
    "sec-ch-ua-mobile": "?0",
    "sec-ch-ua-platform": '"Windows"' | '"macOS"',  # 平台化
    # ... 完整的 Sec-Fetch-* 头
}
```

#### 4.6.3 统一错误提取

```python
_PAYLOAD_MESSAGE_KEYS = ("errmsg", "message", "msg")
_PAYLOAD_CODE_KEYS = ("code", "errcode", "ret")

# get_response_error / get_payload_error
# 自动适配多种错误字段格式
```

---

### 4.7 services/update_service.py：在线更新（68 行）⭐ Rust 版缺失

```python
@dataclass(frozen=True)
class UpdateInfo:
    app: str              # 应用名
    version: str          # 最新版本
    build: int            # 构建号
    mandatory: bool       # 是否强制更新
    platform: str         # 目标平台（mac/windows/unknown）
    download_url: str     # 下载地址
    tutorial_url: str     # 教程链接
    notes: list[str]      # 更新说明
    has_update: bool      # 是否有更新
    raw_payload: dict     # 原始响应
```

**version.json 示例**（推测）：

```json
{
  "app": "TLS-shipinhao",
  "version": "4.3.0",
  "build": 430,
  "mandatory": false,
  "download_url": "https://gitee.com/tuolingshe/xxx/releases/4.3.0",
  "tutorial_url": "https://xxx.com/tutorial",
  "notes": ["优化匹配算法", "修复若干 Bug"]
}
```

---

## 五、数据库 Schema

完整的 SQLite 表结构（来自 `order_cache.py`）：

```sql
-- 主订单表
CREATE TABLE orders (
    order_id TEXT PRIMARY KEY,
    buyer_nickname TEXT NOT NULL DEFAULT '',
    normalized_nickname TEXT NOT NULL DEFAULT '',
    create_time INTEGER NOT NULL DEFAULT 0,
    confirm_receipt_time INTEGER NOT NULL DEFAULT 0,   -- 确认收货时间
    is_waybill_received INTEGER NOT NULL DEFAULT 0,    -- 运单是否签收
    waybill_received_time INTEGER NOT NULL DEFAULT 0,
    is_education_order INTEGER NOT NULL DEFAULT 0,     -- 教育订单标识
    order_status INTEGER NOT NULL DEFAULT 0,
    openid TEXT NOT NULL DEFAULT '',
    raw_source TEXT NOT NULL DEFAULT 'order_api',      -- 数据来源
    updated_at INTEGER NOT NULL DEFAULT 0
);

-- 订单商品
CREATE TABLE order_products (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    order_id TEXT NOT NULL,
    product_id TEXT NOT NULL DEFAULT '',
    sku_id TEXT NOT NULL DEFAULT '',
    sale_param TEXT NOT NULL DEFAULT '',               -- 规格参数（SKU 描述）
    product_name TEXT NOT NULL DEFAULT '',
    thumb_img TEXT NOT NULL DEFAULT '',                -- 商品缩略图 URL
    FOREIGN KEY(order_id) REFERENCES orders(order_id) ON DELETE CASCADE
);

-- 同步状态
CREATE TABLE sync_state (
    scope TEXT PRIMARY KEY,                            -- "orders_30d"
    coverage_start INTEGER NOT NULL DEFAULT 0,         -- 覆盖开始
    coverage_end INTEGER NOT NULL DEFAULT 0,           -- 覆盖结束
    last_incremental_start INTEGER NOT NULL DEFAULT 0,
    last_incremental_end INTEGER NOT NULL DEFAULT 0,
    last_success_at INTEGER NOT NULL DEFAULT 0,
    last_mode TEXT NOT NULL DEFAULT '',                -- rebuild/incremental/gap_fill
    last_error TEXT NOT NULL DEFAULT ''
);

-- 缓存段（时间窗口完成状态）
CREATE TABLE cache_segments (
    scope TEXT NOT NULL,
    start_ts INTEGER NOT NULL,
    end_ts INTEGER NOT NULL,
    status TEXT NOT NULL DEFAULT 'complete',
    updated_at INTEGER NOT NULL DEFAULT 0,
    PRIMARY KEY (scope, start_ts, end_ts)
);

-- 索引
CREATE INDEX idx_orders_create_time ON orders(create_time DESC);
CREATE INDEX idx_products_order_id ON order_products(order_id);
CREATE INDEX idx_cache_segments_scope_start ON cache_segments(scope, start_ts, end_ts);

-- WAL 模式
PRAGMA journal_mode=WAL;
```

**Rust 版对比**：Rust 的 `sqlite_order_cache.rs` 仅有 orders 表，**缺失 `order_products`、`sync_state`、`cache_segments` 三张核心表**。

---

## 六、Rust 重构版 vs Python 原版 — 功能缺失清单

### 6.1 P0 - 重大业务能力丢失 🔴

| # | 功能 | Python 原版 | Rust 重构版 | 业务影响 |
|---|---|---|---|---|
| 1 | **微信 API 风控检测** | ✅ 检测 code=430、"异常行为"、"拒绝访问" | ❌ | 触发风控后继续盲目请求，可能导致**账号被封** |
| 2 | **HTTP 429 限流指数退避重试** | ✅ 2^n 秒退避，重试 3 次 | ❌ | 偶发限流即失败，用户体验差 |
| 3 | **风控自动降级模式** | ✅ 60 秒冷却 + 极速模式 | ❌ | 风控后无救场策略 |
| 4 | **授权多域名容灾** | ✅ 4 个 Base URL 自动切换 | ❌ | 主域名被墙/宕机 → 全部用户无法激活 |
| 5 | **Lease 租约机制** | ✅ 24h 刷新 + 72h 硬过期 | ❌ | 必须每次在线校验，离线不可用 |
| 6 | **任务级授权** | ✅ 5 种任务独立授权 | ❌ | 无精细化权限控制 |
| 7 | **设备指纹绑定** | ✅ ioreg/wmic/machine-id | ⚠️ 简化 | 反破解能力下降 |
| 8 | **Keychain 存储 Lease** | ✅ | ❌ | Lease token 存文件，可被拷贝 |
| 9 | **完整性校验 manifest** | ✅ Ed25519 签名 + 文件哈希 | ❌ | 无法检测被篡改 |
| 10 | **快递公司自动降级** | ✅ 单号前缀推断 deliveryId | ❌ | 用户必须手选，出错率高 |
| 11 | **物流快照保留** | ✅ 仅改 waybillId，保留其他字段 | ❌ | 可能修改不该改的字段 |
| 12 | **缓存段管理（cache_segments）** | ✅ 精确记录窗口完成状态 | ❌ | 缓存覆盖不可靠 |
| 13 | **缺口补齐算法** | ✅ merge_tolerance + min_gap_width | ❌ | 缓存可能有大量细微缝隙 |
| 14 | **全量扫描模式** | ✅ 30 天缓存 + 更早临时抓取 | ❌ | 无法查询超过 30 天的评价 |
| 15 | **在线更新检查** | ✅ version.json + 强制更新 | ❌ | 用户不知道有新版本 |

### 6.2 P1 - 体验与质量问题 🟡

| # | 功能 | Python 原版 | Rust 重构版 | 影响 |
|---|---|---|---|---|
| 16 | **智能昵称匹配（改名识别）** | ✅ 尾部数字/子序列/包含 | ⚠️ 简化 | 昵称修改后匹配失败率上升 |
| 17 | **通用昵称过滤** | ✅ 匿名/微信用户/默认昵称 | ❌ | 把无效昵称当有效匹配 |
| 18 | **匹配策略分级** | ✅ exact/high/probable/fallback | ⚠️ 仅分数 | 用户看不到信心度 |
| 19 | **差评可回复期检测** | ✅ -30 天阈值 | ❌ | 展示了已不能回复的差评 |
| 20 | **品退原因字段** | ✅ qualityRefundInfo.reason | ⚠️ 简化 | 无法做品质分析 |
| 21 | **订单缓存 dirty 检测** | ✅ 自动检测 + 重建 | ❌ | 历史污染数据无法修复 |
| 22 | **缓存迁移** | ✅ 自动迁移旧位置 | ❌ | 升级后用户需重新同步 |
| 23 | **订单完整字段（产品/图片/规格）** | ✅ order_products 完整表 | ❌ 简化 | 界面展示信息不足 |
| 24 | **教育订单标识** | ✅ isEducationOrder | ❌ | 无法区分教育订单 |
| 25 | **多布局适配（宽屏/紧凑/HiDPI）** | ✅ 4 种布局 | ❌ 单一 | 小屏幕体验差 |
| 26 | **UI 缩放支持** | ✅ 0.82-1.0 | ❌ | 不支持用户自定义缩放 |
| 27 | **主题配色（翠绿系）** | ✅ 20+ 色彩变量 | ⚠️ 风格变了 | 品牌一致性丢失 |
| 28 | **平台化 User-Agent** | ✅ macOS / Windows 自适应 | ⚠️ 固定 | 部分场景被识别为非浏览器 |
| 29 | **品牌信息** | ✅ "驼铃·视频小店差评处理" + TLS-801 | ❌ | 变为通用名"TLS-shipinhao" |

### 6.3 P2 - 附加功能丢失 🟢

| # | 功能 | Python 原版 | Rust 重构版 |
|---|---|---|---|
| 30 | 原生加速库（security_core） | ✅ ctypes 绑定 | ❌ |
| 31 | 页码并行持久化批处理（1000 条触发） | ✅ | ❌ |
| 32 | 会话复用（requests.Session） | ✅ | ⚠️ |
| 33 | 配置目录多候选查找 | ✅ | ❌ |
| 34 | 用户自定义配置目录记忆 | ✅ | ⚠️ |
| 35 | WAL 日志模式 | ✅ | 未知 |
| 36 | SQLite 边车文件（wal/shm）迁移 | ✅ | ❌ |

---

## 七、项目依赖对比

### 7.1 Python 原版依赖

| 依赖 | 用途 |
|---|---|
| `requests` | HTTP 客户端 |
| `cryptography` | Ed25519 公钥校验 |
| `PySide6`/`PyQt6`（推断） | 桌面 UI |
| `sqlite3`（stdlib） | 本地数据库 |
| `threading`/`concurrent.futures`（stdlib） | 并发 |
| `subprocess`（stdlib） | 调用系统命令（ioreg/wmic） |
| `ctypes`（stdlib） | 加载 security_core 动态库 |

### 7.2 Rust 重构版依赖

| 依赖 | 用途 |
|---|---|
| `tauri` | 桌面框架 |
| `reqwest` | HTTP 客户端 |
| `rusqlite` | SQLite |
| `serde/serde_json` | 序列化 |
| `tokio` | 异步运行时 |
| `chrono` | 时间 |
| `sha2` | 哈希 |
| `tracing` | 日志 |
| Vue 3 + Vite + TypeScript + Tailwind | 前端 |

---

## 八、迁移决策评估

### 8.1 Rust 重构的价值

| 优点 | 说明 |
|---|---|
| **分发体积小** | Tauri 产物 ~ 20MB（Python + Qt 通常 80+ MB） |
| **启动快** | Rust 原生 + Vite 优化 |
| **类型安全** | Rust + TypeScript 比 Python 更难出运行时错误 |
| **内存安全** | Rust 无 GC |
| **技术栈现代** | 吸引前端开发者加入 |
| **前端开发效率** | Vue 3 + Tailwind 优于 Qt Widgets |

### 8.2 Rust 重构的损失

| 代价 | 说明 |
|---|---|
| **业务功能大幅缩水** | 15 项 P0 功能缺失，用户实际可用性下降 |
| **反风控能力归零** | 原版多年积累的反风控策略全部丢失 |
| **安全能力降级** | Lease、设备指纹、Keychain、完整性校验都没了 |
| **测试覆盖率下降** | `tests/` 目录下的 Python 测试不再适用 |
| **UX 一致性丢失** | 品牌、配色、布局都变了 |
| **用户迁移成本** | 缓存格式不兼容，老用户数据丢失 |

### 8.3 综合建议

**Rust 重构版可以继续，但需要做三件事**：

1. **补齐 P0 功能**（15 项）— 从 Python 版逐个移植到 Rust，特别是反风控和授权 Lease
2. **重建 UI 细节**（多布局、UI 缩放、品牌）
3. **缓存格式兼容**（实现从旧 SQLite 到新 SQLite 的一次性迁移）

**预估工作量**：
- 核心功能补齐：**3-4 人月**
- UI/UX 对齐：**1-2 人月**
- 数据迁移：**0.5 人月**
- **合计：约 4-6 人月才能达到 Python 版的功能完整度**

---

## 九、完整功能清单映射表

按**业务模块**列出 Python 原版所有功能，标注 Rust 版状态：

### 授权管理

| Python 函数 | Rust 等价 | 状态 |
|---|---|---|
| `activate_license(key)` | `activate_license` command | ⚠️ Worker 占位 |
| `authorize_task(task_type)` | ❌ | ❌ 缺失 |
| `check_stored_license()` | `verify_license` | ⚠️ 简化 |
| `check_stored_license_local()` | ❌ | ❌ 缺失 |
| `deactivate_license()` | ❌ | ❌ 缺失 |
| `get_device_id()` | ⚠️ 简化 | ⚠️ 缺指纹 |
| `get_license_info()` | `get_license_status` | ✅ |
| `issue_or_refresh_session_token()` | ❌ | ❌ 缺失 |
| `load_runtime_state()` | ⚠️ 部分 | ⚠️ |
| `refresh_lease_if_due()` | ❌ | ❌ Lease 机制缺失 |
| `validate_runtime_continuity()` | ❌ | ❌ 完整性校验缺失 |
| `verify_signed_lease()` | ❌ | ❌ |

### 订单同步

| Python | Rust | 状态 |
|---|---|---|
| `OrderCacheRepository.initialize()` | `SqliteOrderCache::new` | ⚠️ Schema 简化 |
| `.upsert_orders()` | ✅ | ⚠️ 字段缺失 |
| `.get_state()` / `.save_state()` | ❌ | ❌ sync_state 表没了 |
| `.mark_segment_complete()` | ❌ | ❌ cache_segments 表没了 |
| `.get_missing_segments()` | ❌ | ❌ 缺口算法缺失 |
| `.has_dirty_sale_param()` | ❌ | ❌ dirty 检测缺失 |
| `.delete_older_than()` | ⚠️ 可能有 | ? |
| `.fetch_orders_in_range()` | ✅ | ✅ |
| `OrderSyncService.rebuild_cache()` | ⚠️ 部分 | ⚠️ |
| `.refresh_cache()` | ⚠️ 部分 | ⚠️ |
| `.ensure_orders()` | ✅ | ✅ |
| `.fetch_full_scan_orders()` | ❌ | ❌ 全量扫描缺失 |

### 评价匹配

| Python | Rust | 状态 |
|---|---|---|
| `BadReviewOrderFinder.get_bad_evaluations()` | ✅ | ⚠️ |
| `.get_quality_refund_orders()` | ✅ | ✅ |
| `.merge_quality_refund_orders()` | ⚠️ | ⚠️ |
| `._fetch_orders_by_page()` | ⚠️ | ⚠️ 无风控 |
| `._is_risk_control_result()` | ❌ | ❌ 风控检测缺失 |
| `._retry_order_search_on_limit()` | ❌ | ❌ 限流重试缺失 |
| `._is_generic_nickname()` | ❌ | ❌ 通用昵称过滤缺失 |
| `compute_match_score()` | ✅ | ⚠️ 简化 |
| `similarity_percent()` | ⚠️ | ⚠️ 改名场景不全 |
| `_match_strategy_by_score()` | ❌ | ❌ 策略分级缺失 |

### 发货管理

| Python | Rust | 状态 |
|---|---|---|
| `fetch_init_ship_data_payload()` | ✅ | ✅ |
| `fetch_order_detail_payload()` | ✅ | ✅ |
| `fetch_current_delivery_context()` | ⚠️ | ⚠️ 缺回退 |
| `build_update_delivery_payload()` | ✅ | ⚠️ 简化 |
| `update_delivery_info()` | ✅ | ✅ |
| `update_single_order()` | ✅ | ⚠️ 缺降级 |
| `_is_delivery_mismatch_error()` | ❌ | ❌ 快递降级缺失 |

### 配置与工具

| Python | Rust | 状态 |
|---|---|---|
| `get_home_config_dir()` | ⚠️ 平台不同 | ⚠️ |
| `get_user_data_dir()` | ⚠️ | ⚠️ |
| `resolve_config_dir()` | ❌ | ❌ 多候选查找缺失 |
| `save_user_config_dir()` | ❌ | ❌ 目录记忆缺失 |
| `parse_batch_input()` | ✅ | ✅ |
| `read_cookie_data()` | ✅ | ✅ |
| `save_cookie_data()` | ✅ | ✅ |
| `extract_biz_magic_from_cookie()` | ✅ | ✅ |

### 更新与版本

| Python | Rust | 状态 |
|---|---|---|
| `fetch_latest_version_info()` | ❌ | ❌ 更新检查完全缺失 |
| `is_newer_version()` | ❌ | ❌ |
| `parse_version()` | ❌ | ❌ |

### UI / UX

| Python | Rust | 状态 |
|---|---|---|
| UI 缩放（`set_ui_scale`/`scale_px`） | ❌ | ❌ |
| 多布局（wide/compact/HiDPI） | ❌ | ❌ 单一布局 |
| 翠绿主题 | ❌ | ❌ 换了主题 |
| `AppSidebar/AppHeader/AppLayout` | ⚠️ | ⚠️ 不同设计 |

---

## 十、结论与建议

### 10.1 核心结论

**Python 原版 `4.3.0` 是一款远比 Rust 重构版 `5.0.0` 成熟完整的商业桌面工具**。原版经过多轮迭代积累了大量**反风控经验**、**授权安全机制**和**业务细节优化**。

当前 Rust 版虽然技术栈先进，但**实际业务可用性远低于 Python 原版**，严格来说不能算"重构完成"，更像"重新造轮子但只造了一半"。

### 10.2 补齐路线图建议

**优先级从高到低**：

#### 阶段 1：反风控补齐（1-1.5 月）🔴 最紧急

1. HTTP 429 指数退避重试
2. 风控检测（code=430 / 异常行为文案）
3. 风控冷却 + 极速模式切换
4. 平台化 User-Agent（macOS/Windows 自适应）

#### 阶段 2：授权与安全（1-1.5 月）🔴

5. 补完 Worker `/api/activate` 和 `/api/verify` 对接 D1
6. Lease 租约机制（24h 刷新 + 72h 硬过期）
7. 设备指纹（ioreg/wmic/machine-id）
8. Keychain 存储 Lease
9. 完整性校验 manifest
10. 任务级授权（5 种任务）

#### 阶段 3：订单/缓存功能（1 月）🟡

11. `order_products`、`sync_state`、`cache_segments` 三表完整实现
12. 缺口补齐算法（merge_tolerance + min_gap_width）
13. Dirty 数据检测与自动修复
14. 缓存迁移（旧 Python 版 → 新 Rust 版）
15. 全量扫描模式

#### 阶段 4：评价与发货细节（0.5-1 月）🟡

16. 智能昵称匹配（改名识别、通用昵称过滤）
17. 匹配策略分级（exact/high/probable/fallback）
18. 差评可回复期检测
19. 快递公司自动降级
20. 物流快照保留（仅改 waybillId）

#### 阶段 5：UI 和更新（0.5-1 月）🟢

21. UI 缩放支持
22. 多布局适配
23. 翠绿主题还原
24. 在线更新检查（version.json）

### 10.3 风险提示

⚠️ **如果不补齐反风控能力，Rust 版上线后可能出现大规模账号封禁**，给用户和团队带来严重信誉损失。强烈建议在推广 5.0.0 之前优先完成阶段 1 和阶段 2。

### 10.4 替代方案

**备选方案 A**：让 Rust 版只做"前端 UI + 部分轻量业务"，**核心反风控和授权继续用 Python 实现**，通过 Tauri sidecar 或本地进程通信。

**备选方案 B**：**回退到 Python 版**，前端用 PySide6 + QtWebEngine 嵌入 Vue 3 UI，兼顾现代 UI 和业务完整性。

**备选方案 C**：接受当前 Rust 版功能缺失，但**在显眼位置提示用户"当前版本为 Beta，某些功能待补全"**，设置明确的补齐节奏。

---

## 附录：关键文件的精确路径

- 原版入口：`/Users/zxr/Downloads/source-code/TLS-shipinhao/_legacy/app/main.py`
- 全局配置：`/Users/zxr/Downloads/source-code/TLS-shipinhao/_legacy/app/settings.py`
- 安全运行时：`/Users/zxr/Downloads/source-code/TLS-shipinhao/_legacy/app/core/security_runtime.py`
- 评价匹配：`/Users/zxr/Downloads/source-code/TLS-shipinhao/_legacy/app/services/review_matcher.py`
- 订单缓存：`/Users/zxr/Downloads/source-code/TLS-shipinhao/_legacy/app/services/order_cache.py`
- 订单同步：`/Users/zxr/Downloads/source-code/TLS-shipinhao/_legacy/app/services/order_sync.py`
- 发货 API：`/Users/zxr/Downloads/source-code/TLS-shipinhao/_legacy/app/services/delivery_api.py`
- 匹配评分：`/Users/zxr/Downloads/source-code/TLS-shipinhao/_legacy/app/services/order_match_scoring.py`
- HTTP 工具：`/Users/zxr/Downloads/source-code/TLS-shipinhao/_legacy/app/core/http_utils.py`
- 更新服务：`/Users/zxr/Downloads/source-code/TLS-shipinhao/_legacy/app/services/update_service.py`

---

*本报告基于源码静态分析，所有功能细节均可在上述文件中复核。对比 Rust 重构版的结论来自已读取的 `apps/desktop/` 和 `crates/` 代码。*

*报告作者：Claude*
