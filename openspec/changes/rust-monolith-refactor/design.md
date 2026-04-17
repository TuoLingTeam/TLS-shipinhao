# Rust 单语言整体重构设计

## 设计目标

- 单语言：核心代码、服务端、构建能力统一为 Rust
- 单结构：一个 Rust workspace 管理多个 crate
- 分阶段迁移：每一阶段都可以验证、回归、回滚
- 先保证业务与授权正确性，再逐步收敛 UI 与发布链路

## 目标架构

```text
workspace/
├── backend/
│   ├── crates/
│   ├── domain-core           # 订单、评价、缓存、授权共享模型
│   ├── security-core         # 设备绑定、验签、完整性、风险检测
│   ├── desktop-app           # 桌面客户端入口与 UI 壳层
│   ├── desktop-services      # 本地业务编排（查评、发货、缓存同步）
│   ├── license-service       # 授权 HTTP 服务
│   ├── build-tools           # manifest、版本、发布辅助工具
│   └── api-contracts         # 客户端/服务端共享 DTO
├── apps/
│   ├── desktop               # 桌面应用启动配置/资源
│   └── license-server        # 服务部署入口
│   ├── tests/
│   └── xtask/
    ├── integration
    └── e2e
```

## UI 技术选型

推荐顺序：

1. **Tauri + Rust backend + 前端 Web UI**（推荐）
2. `slint`（如果追求更纯粹的原生 Rust UI）
3. `egui`（如果追求开发速度，但界面可塑性需重新评估）

### 推荐：Tauri

原因：
- Windows/macOS 桌面打包成熟
- Rust 负责核心能力与命令处理
- UI 可以通过 Web 技术重建，替代 PySide6 成本比纯原生 Rust UI 更低
- 更容易实现 Cookie 采集、页面登录态桥接与跨平台 UI 适配

## 分阶段迁移策略

### Phase 0：建 Rust workspace 骨架

输出：
- 顶层 `Cargo.toml` workspace
- `domain-core / api-contracts / security-core / license-service / desktop-services / desktop-app / build-tools`
- 基础 lint/test/build 命令统一

### Phase 1：先抽共享域模型

将以下内容从 Python/JS 提炼为 Rust 共享模型：
- license lease / runtime grant / manifest / risk report
- 订单缓存实体、订单匹配结果、批量发货请求/响应模型
- 公共错误码、状态机与时间窗口模型

目的：
- 先冻结“数据和边界”
- 避免后续 UI/后端同时迁移时协议漂移

### Phase 2：授权服务迁移到 Rust

将 `backend/src/worker/index.js` 迁移为 Rust `license-service`：
- 保持 `/api/activate`、`/api/verify` 对外协议兼容
- 数据库可继续用 Cloudflare D1，或切换为更适配 Rust 的 SQLite/Postgres
- 签发逻辑、设备绑定、租约续签、审计全部进入 Rust

推荐先做“服务逻辑 Rust 化”，部署位置后移：
- 可先作为独立 HTTP 服务运行
- 稳定后再决定是否继续部署在 Cloudflare 兼容层、Fly.io、Railway、Render 等

### Phase 3：桌面业务层迁移到 Rust

先迁移非 UI 业务：
- 查找评价
- 订单同步
- 批量物流更新
- 本地 SQLite 缓存
- Cookie 数据解析与校验

此阶段保留旧 UI 壳层也可以，但所有业务调用改为 Rust 服务/命令。

### Phase 4：桌面 UI 重建

选择 Tauri 后：
- 旧 PySide6 `window.py / widgets.py / workers` 全部退役
- 用前端页面重建 UI
- 所有命令调用走 Tauri command -> Rust services

### Phase 5：构建链路统一

移除：
- `scripts/obfuscate.py`
- `scripts/build.py` 中 Cython / PyInstaller 路径

新增：
- `cargo build --workspace`
- `cargo tauri build`
- `cargo xtask release`
- manifest 生成、签名、打包、版本注入统一由 Rust 工具完成

## 技术决策

### 决策 1：先服务和业务，后 UI

原因：
- 授权与业务逻辑是复杂度最高、最值得统一的部分
- UI 技术选型可后置，不阻塞域模型与服务收敛

### 决策 2：保留阶段性双轨，但最终只保留 Rust

原因：
- 一次性替换风险过高
- 但目标必须明确：每一阶段结束都要删除一部分旧实现，而不是长期共存

### 决策 3：共享协议单独 crate 管理

原因：
- 客户端/服务端的数据结构必须统一
- 降低“客户端和后端各自演进”的风险

### 决策 4：安全与业务模型分离

原因：
- `security-core` 聚焦验签、设备绑定、完整性、风险
- `desktop-services` 聚焦业务编排
- 避免后续形成新的“大而杂安全模块”

## 风险与缓解

### 风险 1：UI 全量重建周期长

缓解：
- 先迁移业务与服务
- UI 最后替换
- 允许阶段性用旧客户端调用 Rust 二进制/FFI

### 风险 2：部署栈变化影响后端稳定性

缓解：
- 保持现有 API 合约不变
- 先做 shadow service / staging 验证
- 采用蓝绿切换

### 风险 3：Cookie 采集流程迁移复杂

缓解：
- 单独视为桌面 UI 子能力
- 先保留旧采集路径或 WebView 临时方案
- 迁移时单独做兼容层

### 风险 4：阶段性双轨时间过长

缓解：
- 每个阶段定义“必须删除的旧模块”
- 不允许无限期保留 Python 与 JS 对应实现

## 回滚策略

- 每个阶段单独发布
- 服务端 API 保持兼容至少一个版本周期
- 客户端迁移先并行发布 Beta，再替换正式版
- 任一阶段回滚时，仍能恢复到 Python 客户端 + JS 授权服务的稳定版本
