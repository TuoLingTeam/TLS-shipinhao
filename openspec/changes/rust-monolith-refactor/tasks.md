## 1. 建立 Rust workspace 与目标目录骨架

Depends on: none

Traceability:
- Requirements: [[specs/rust-desktop-client/spec.md#requirement-1-桌面客户端必须收敛为-rust-主实现]] [[specs/rust-license-service/spec.md#requirement-1-授权服务必须迁移为-rust-实现]] [[specs/rust-build-release/spec.md#requirement-1-桌面构建链路必须统一为-rust-工具链]]
- Tests: [[specs/rust-desktop-client/test-plan.md#测试组-1迁移闭环验证]] [[specs/rust-license-service/test-plan.md#测试组-3部署与运行验证]] [[specs/rust-build-release/test-plan.md#测试组-1构建链路替换验证]]

- [ ] 1.1 RED: 新增 workspace 结构验证测试，断言 crate 拓扑完整
- [ ] 1.2 RED: 运行目标测试并确认当前失败
- [ ] 1.3 GREEN: 创建顶层 Cargo workspace 与基础 crates
- [ ] 1.4 GREEN: 运行目标测试并确认通过
- [ ] 1.5 REFACTOR: 统一命名、目录与 crate 责任边界

## 2. 抽离共享协议与领域模型

Depends on: 1

Traceability:
- Requirements: [[specs/rust-desktop-client/spec.md#requirement-2-桌面端必须保留现有业务能力]] [[specs/rust-license-service/spec.md#requirement-2-租约设备绑定和审计必须统一在-rust-服务中]]
- Tests: [[specs/rust-desktop-client/test-plan.md#测试组-2功能等价验证]] [[specs/rust-license-service/test-plan.md#测试组-2授权状态机验证]]

- [ ] 2.1 RED: 为 lease/grant/manifest/order-cache DTO 编写失败测试 [[specs/rust-license-service/spec.md#requirement-2-租约设备绑定和审计必须统一在-rust-服务中]] [[specs/rust-license-service/test-plan.md#测试组-2授权状态机验证]]
- [ ] 2.2 RED: 运行协议模型测试并确认失败
- [ ] 2.3 GREEN: 新建 `domain-core` 与 `api-contracts`，迁移共享结构
- [ ] 2.4 GREEN: 运行协议模型测试并确认通过
- [ ] 2.5 REFACTOR: 清理重复结构与命名漂移

## 3. 将授权服务整体迁移到 Rust

Depends on: 2

Traceability:
- Requirements: [[specs/rust-license-service/spec.md#requirement-1-授权服务必须迁移为-rust-实现]] [[specs/rust-license-service/spec.md#requirement-2-租约设备绑定和审计必须统一在-rust-服务中]] [[specs/rust-license-service/spec.md#requirement-3-部署方式必须与语言统一目标一致]]
- Tests: [[specs/rust-license-service/test-plan.md#测试组-1-api-兼容验证]] [[specs/rust-license-service/test-plan.md#测试组-2授权状态机验证]] [[specs/rust-license-service/test-plan.md#测试组-3部署与运行验证]]

- [ ] 3.1 RED: 为 `/api/activate` `/api/verify` 等价响应增加兼容测试
- [ ] 3.2 RED: 运行授权服务测试并确认失败
- [ ] 3.3 GREEN: 实现 Rust `license-service`，接管租约签发/设备绑定/审计
- [ ] 3.4 GREEN: 运行授权服务测试并确认通过
- [ ] 3.5 REFACTOR: 清理 JS Worker 中重复授权逻辑，仅保留短期兼容层

## 4. 将桌面业务能力迁移到 Rust services

Depends on: 2, 3

Traceability:
- Requirements: [[specs/rust-desktop-client/spec.md#requirement-1-桌面客户端必须收敛为-rust-主实现]] [[specs/rust-desktop-client/spec.md#requirement-2-桌面端必须保留现有业务能力]]
- Tests: [[specs/rust-desktop-client/test-plan.md#测试组-1迁移闭环验证]] [[specs/rust-desktop-client/test-plan.md#测试组-2功能等价验证]]

- [ ] 4.1 RED: 为查评、缓存、批量发货建立 Rust 等价行为测试
- [ ] 4.2 RED: 运行桌面业务核心测试并确认失败
- [ ] 4.3 GREEN: 将 `review_matcher/order_sync/order_cache/delivery_api` 迁移至 Rust
- [ ] 4.4 GREEN: 运行桌面业务核心测试并确认通过
- [ ] 4.5 REFACTOR: 删除 Python 业务层重复实现或降级为临时适配层

## 5. 重建桌面 UI 为 Rust 唯一路线

Depends on: 4

Traceability:
- Requirements: [[specs/rust-desktop-client/spec.md#requirement-3-ui-框架必须统一为单一-rust-方案]] [[specs/rust-desktop-client/spec.md#requirement-2-桌面端必须保留现有业务能力]]
- Tests: [[specs/rust-desktop-client/test-plan.md#测试组-2功能等价验证]] [[specs/rust-desktop-client/test-plan.md#测试组-3ui-路线收敛验证]]

- [ ] 5.1 RED: 为新 UI 命令接口、核心页面流程与功能入口建立 e2e 测试
- [ ] 5.2 RED: 运行 UI 测试并确认失败
- [ ] 5.3 GREEN: 采用 Tauri（推荐）或最终确认的 Rust UI 路线重建桌面应用
- [ ] 5.4 GREEN: 运行 UI 测试并确认通过
- [ ] 5.5 REFACTOR: 移除 PySide6 主窗口/worker 正式职责，冻结旧 UI

## 6. 统一构建、签名与发布流程

Depends on: 3, 5

Traceability:
- Requirements: [[specs/rust-build-release/spec.md#requirement-1-桌面构建链路必须统一为-rust-工具链]] [[specs/rust-build-release/spec.md#requirement-2-签名与完整性清单必须保留并融入-rust-发布流程]] [[specs/rust-build-release/spec.md#requirement-3-旧构建脚本必须可删除]]
- Tests: [[specs/rust-build-release/test-plan.md#测试组-1构建链路替换验证]] [[specs/rust-build-release/test-plan.md#测试组-2安全发布验证]] [[specs/rust-build-release/test-plan.md#测试组-3旧链路退役验证]]

- [ ] 6.1 RED: 为 Rust 发布命令、签名清单与 CI 流程添加失败测试/验证脚本
- [ ] 6.2 RED: 运行发布链路验证并确认失败
- [ ] 6.3 GREEN: 实现 `cargo xtask`/`cargo tauri build`/发布签名工具链
- [ ] 6.4 GREEN: 运行发布链路验证并确认通过
- [ ] 6.5 REFACTOR: 删除 `scripts/obfuscate.py`、`scripts/build.py` 与旧 CI 构建依赖

## 7. 执行割接与旧实现退役

Depends on: 4, 5, 6

Traceability:
- Requirements: [[specs/rust-desktop-client/spec.md#requirement-1-桌面客户端必须收敛为-rust-主实现]] [[specs/rust-license-service/spec.md#requirement-3-部署方式必须与语言统一目标一致]] [[specs/rust-build-release/spec.md#requirement-3-旧构建脚本必须可删除]]
- Tests: [[specs/rust-desktop-client/test-plan.md#测试组-1迁移闭环验证]] [[specs/rust-license-service/test-plan.md#测试组-3部署与运行验证]] [[specs/rust-build-release/test-plan.md#测试组-3旧链路退役验证]]

- [ ] 7.1 RED: 为正式切换、回滚与兼容窗口建立发布验收清单
- [ ] 7.2 RED: 在 staging 环境执行完整彩排并记录失败点
- [ ] 7.3 GREEN: 正式切换到 Rust 客户端 + Rust 授权服务 + Rust 发布链路
- [ ] 7.4 GREEN: 执行回归验证并确认通过
- [ ] 7.5 REFACTOR: 删除 Python 客户端、JS 授权主实现与旧构建脚本残余代码


---

## 当前进度快照（2026-04-16）

已完成：
- [x] Workspace / crates 基础骨架
- [x] 共享协议与领域模型基础迁移
- [x] 授权服务核心状态机（activate/verify）
- [x] worker JSON 路由适配
- [x] Rust build-tools 的 manifest 生成/签名/校验
- [x] workspace `cargo check --workspace`
- [x] 桌面业务纯函数：订单字段归一化
- [x] 桌面业务核心算法：订单匹配评分
- [x] review matcher 公共逻辑：理由生成 / 候选择优
- [x] review matcher 候选订单评分流程

剩余待办（按执行顺序）：

## 8. review matcher 核心流程 Rust 化

Depends on: 4

- [ ] 8.1 RED: 为 `_match_single_evaluation` 建立 Rust 等价测试（空候选 / 多候选择优 / 无有效候选）
- [ ] 8.2 GREEN: 实现 `review_match_flow`，接管单条评价候选筛选与最佳匹配选择
- [ ] 8.3 GREEN: 将 `match_strategy_by_score`、结果组装结构迁移到 Rust
- [ ] 8.4 REFACTOR: 让 Python `review_matcher.py` 开始优先委托 Rust helper

## 9. review matcher 批处理总流程 Rust 化

Depends on: 8

- [ ] 9.1 RED: 为整批评价匹配流程建立 Rust 回归测试
- [ ] 9.2 GREEN: 实现批量 `match_orders_with_evaluations` 核心流程
- [ ] 9.3 GREEN: 迁移候选订单收集与 product index 相关逻辑
- [ ] 9.4 REFACTOR: 删除 Python 中已被 Rust 覆盖的匹配主路径

## 10. 订单缓存与同步 Rust 化

Depends on: 8

- [ ] 10.1 RED: 为 `order_cache.py` / `order_sync.py` 建立等价行为测试
- [ ] 10.2 GREEN: 迁移 SQLite 缓存读写与同步逻辑到 Rust
- [ ] 10.3 GREEN: 暴露 desktop-services 统一缓存接口
- [ ] 10.4 REFACTOR: 下线 Python `order_cache.py` / `order_sync.py` 正式职责

## 11. 批量发货链路 Rust 化

Depends on: 10

- [ ] 11.1 RED: 为 `delivery_api.py` 建立请求/错误路径等价测试
- [ ] 11.2 GREEN: 迁移物流更新请求构造、返回解析、批量失败处理
- [ ] 11.3 GREEN: 将批量发货入口接入 Rust services
- [ ] 11.4 REFACTOR: 下线 Python `delivery_api.py` 正式职责

## 12. desktop-app 真正接线

Depends on: 9, 10, 11

- [ ] 12.1 RED: 为 Slint 主界面命令流建立端到端/命令级测试
- [ ] 12.2 GREEN: 让 desktop-app 调用真实 desktop-services，而不是占位回调
- [ ] 12.3 GREEN: 接入授权状态展示、任务启动、日志输出、错误反馈
- [ ] 12.4 REFACTOR: 停止给 PySide6 UI 增加任何正式功能

## 13. 旧 Python UI / worker 退役前置

Depends on: 12

- [ ] 13.1 RED: 为关键 UI 流程建立迁移验收清单
- [ ] 13.2 GREEN: 移除 `app/ui/*.py` 的正式执行路径
- [ ] 13.3 GREEN: 移除 `app/bootstrap.py` / `app/main.py` 的正式职责
- [ ] 13.4 REFACTOR: 将旧 Python 客户端降级为仅兼容/迁移代码，或删除

## 14. Rust 构建发布主链路收口

Depends on: 12

- [ ] 14.1 RED: 为 `xtask` / release 流程补充失败测试与命令验收
- [ ] 14.2 GREEN: 让 Rust build-tools 接管版本注入、manifest、打包编排
- [ ] 14.3 GREEN: 更新 GitHub Actions 到 Rust-only 主链路
- [ ] 14.4 REFACTOR: 删除 `scripts/build.py` / `scripts/obfuscate.py` 正式链路

## 15. Cloudflare Rust 授权部署收口

Depends on: 3

- [ ] 15.1 RED: 为 license-worker 部署入口补充运行测试
- [ ] 15.2 GREEN: 让 Cloudflare 兼容层真正以 Rust 主逻辑运行
- [ ] 15.3 REFACTOR: 删除 `backend/src/worker/index.js` 主授权实现

## 16. 最终割接与清理

Depends on: 13, 14, 15

- [ ] 16.1 GREEN: 删除 `app/` 正式运行实现
- [ ] 16.2 GREEN: 删除旧 JS Worker 主实现与不再需要的 Node/Python 构建依赖
- [ ] 16.3 GREEN: 保留必要迁移文档，清理仓库残余
- [ ] 16.4 VERIFY: 全量回归、构建、授权流程验证
- [ ] 16.5 COMMIT: 最终割接提交
