# L4-2 异步 trait 边界决策

> **状态**：已决议（2026-04-29）
> **作用域**：`crates/desktop-services/` 与 `apps/desktop/` 的 HTTP / SQLite 适配边界
> **后续触发**：需要满足下文「后续启动条件」中至少一项才考虑重启第四期

## 1. 背景

L4-2 系列目标是**消除桌面端 Rust 后端的「同步阻塞 + 异步反应器」混血路径**：

| 期次 | 范围 | 状态 |
|---|---|---|
| L4-2 第一期 | 品退 / `HttpQualityRefundSource` HTTP 路径改纯 async，去掉 `block_on` 线程套娃 | ✅ |
| L4-2 第二期 | `delivery` / `review` / `order` HTTP 路径改纯 async，去掉双层线程包装；`HttpOrderCacheFinder::get_orders_for_cache` 改用 `Handle::current().block_on(...)` 替代每次新建 `Runtime` | ✅ |
| L4-2 第三期 | `ReviewSource` / `DeliveryGateway` 两个 trait 改 `#[async_trait]`，HTTP 适配器同步 trait 薄壳删除 | ✅ |
| **L4-2 第四期** | **3 个剩余 sync trait 改 `#[async_trait]`**：`BatchDeliveryGateway` / `OrderCacheStore` / `CacheOrderFinder` | **决议保留 sync** |

## 2. 第四期范围拆解

如果继续推进，需要联动改造的全部位点：

| Trait | 唯一调用点 | 直接 impl | 上层链路 |
|---|---|---|---|
| `BatchDeliveryGateway` | `desktop_services::delivery::batch_runner::run_batch_delivery_with_hooks` | `apps/desktop/src/adapters/delivery.rs::HttpDeliveryGateway` | `commands::delivery::batch_delivery` 顶层 `spawn_blocking` |
| `OrderCacheStore` | `commands::order::load_order_cache` | `apps/desktop/src/adapters/order_cache.rs::SqliteOrderCache` | 命令层 `spawn_blocking` 包裹 SQLite |
| `CacheOrderFinder` | `desktop_services::order::sync_service::OrderSyncService` 11+ method | `apps/desktop/src/adapters/order/mod.rs::HttpOrderCacheFinder` | `commands::order::sync_recent_order_cache` / `commands::review::find_reviews` 顶层 `spawn_blocking` |

直接改面：`±300 行`；连带 `#[tokio::test]` 与 mock 改造：`±500 行`；新增 `tokio::task::spawn_blocking` 包装点：`8` 处。

## 3. 收益分析

### 3.1 理论收益

**唯一真收益**：删除 HTTP 适配器内部的 `Handle::current().block_on(self.<async_method>())` 桥接代码（约 60 行）。

### 3.2 「错觉收益」

| 看似收益 | 实际情况 |
|---|---|
| 命令层去掉 `spawn_blocking` | 改面下移到 service 内部 SQLite 处仍要 `spawn_blocking`，只是位置不同 |
| HTTP / SQLite trait 接口一致 | SQLite 仍是单连接串行（`rusqlite`），改 async 仅外观一致，运行时无并发收益 |
| async 上下文取消传播更快 | 已有 `batch_delivery_cancel: AtomicBool` + `should_cancel: Fn() -> bool` 的取消信号链，新增 `.await` 不带额外可取消点 |
| 性能提升 | 业务路径都是顺序串行 IO（一次激活 / 一次发货 / 一次评价匹配），无并发场景，吞吐量与延迟无变化 |

### 3.3 行为不变性

L4-2 第四期**理论上**不改变任何业务可观察行为：

- HTTP 报文（UA / headers / body / URL）零改动
- `#[tauri::command]` JSON 输入输出契约保持
- 取消信号检查时机一致
- 进度事件 emit 顺序一致
- SQLite 串行调用语义保持
- license guard / task grant 流程保持

## 4. 风险分析

虽然行为不变性可保证，**重构本身仍有 regression 风险**：

| 风险 | 描述 |
|---|---|
| `OrderSyncService` self ownership | `sync_range` / `ensure_recent_cache` 等内部多次调用 `self.repository`，改 async + `spawn_blocking` 需 `Arc::clone` move 进闭包，self 生命周期复杂化 |
| `Fn` closure 类型迁移 | `run_batch_delivery_with_hooks` 的 `on_step` / `should_cancel` closure 在 async 上下文中可能需 `Send + 'static` 约束加严 |
| 单测 mock 全面改写 | `FakeGateway` / `FakeFinder` / `FakeStore` 等 sync mock 都要重写为 async impl，10+ 测试用例要改 `#[tokio::test]` |
| panic 行为微变 | 原 sync 路径 panic 直接传到命令层；改 async + `spawn_blocking` 后 panic 转为 `JoinError`，错误文案从 `"foo failed: ..."` 变 `"task panicked"` |
| `#[tokio::test]` flavor | 默认 single-thread，部分测试可能需 `#[tokio::test(flavor = "multi_thread")]` |
| 真壳验证不可省 | 5 条端到端路径必须人工跑：激活 / cookie 同步 / 评价查询 / 批量发货 + 取消 / 订单缓存同步 |

## 5. 决议

**保留 3 个剩余 sync trait 不动**。

### 5.1 工程理由

- HTTP 路径已完成 async 化（一/二/三期），剩余 sync trait 是**业务流程边界**而非「待清理的技术债」
- SQLite 仍是单连接同步，改 async 不带来运行时收益
- ROI 倒挂：改面 `±500 行` × 真壳验证 5 路径 vs 删除 `60 行` 桥接 + 接口外观一致

### 5.2 设计语义

把这个边界明确化：

```text
            ┌─────────────────────────┐
            │  #[tauri::command]       │  全 async（前端 IPC 入口）
            └────────┬─────────────────┘
                     │
            ┌────────▼─────────────────┐
            │  sync trait + spawn_blocking 边界  │
            │  ─ BatchDeliveryGateway           │   ← 业务流程同步语义
            │  ─ OrderCacheStore                │   ← SQLite 单连接串行
            │  ─ CacheOrderFinder               │   ← HTTP 已 async，trait 仅薄壳
            └────────┬─────────────────┘
                     │
            ┌────────▼─────────────────┐
            │  HTTP adapter / SQLite repo │  HTTP 全 async；SQLite 同步
            └─────────────────────────┘
```

边界的目的是**让上层流程（OrderSyncService / run_batch_delivery_with_hooks）保持顺序、可读、可测试的同步控制流**，下层 IO 各自走自己最自然的执行模型。

## 6. 后续启动条件

满足以下任一条件时再考虑启动第四期：

1. **SQLite 换异步驱动**（如 `tokio-rusqlite` / `sqlx`）——届时 repository trait 必然 async，sync trait 不再有意义
2. **同步流程引入并发分支**——如批量发货改为「同时跑多个店铺」，sync `for` 循环阻碍并发，必须改 async
3. **trait 扩展明显异步语义**——如新增 `subscribe_progress(&self) -> impl Stream<...>` 这类纯异步方法
4. **运行时取消信号需要 await 点**——如新增「在 HTTP 请求中途取消」需求，sync `for` 循环无法插入取消点

否则保持现状，不被「接口一致性」洁癖驱动重构。

## 7. 关联实现位点

| 文件 | 角色 |
|---|---|
| `crates/desktop-services/src/lib.rs` | `OrderCacheStore` trait 定义 |
| `crates/desktop-services/src/delivery/batch_runner.rs` | `BatchDeliveryGateway` + `run_batch_delivery_with_hooks` |
| `crates/desktop-services/src/order/sync_service/mod.rs` | `CacheOrderFinder` + `OrderSyncService` |
| `crates/desktop-services/src/common/http_client.rs` | 模块顶部注释——本决议的工程口径出处 |
| `apps/desktop/src/commands/{review,order,delivery}.rs` | 顶层 `spawn_blocking` 包裹 |
| `apps/desktop/src/adapters/{delivery,order}/*.rs` | sync trait 实现 |

## 8. 历史变更

| 日期 | 变更 |
|---|---|
| 2026-04-28 | L4-2 第三期完成，`ReviewSource` / `DeliveryGateway` 改 async_trait |
| 2026-04-29 | 本决议形成；`http_client.rs:41-44` 注释从「未排期」改为「边界决议」语气；本文件归档 |
