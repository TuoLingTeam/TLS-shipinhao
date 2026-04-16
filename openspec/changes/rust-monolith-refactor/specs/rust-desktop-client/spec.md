# rust-desktop-client 规范

## 概述

该能力定义将现有 Python + PySide6 客户端整体迁移为 Rust 桌面客户端的范围、边界与阶段性行为。

## Requirement 1: 桌面客户端必须收敛为 Rust 主实现

系统必须以 Rust 作为桌面客户端的唯一核心实现语言，并最终移除 Python 客户端运行依赖。  
Traceability: [[design.md#phase-3桌面业务层迁移到-rust]] [[design.md#phase-4桌面-ui-重建]] [[specs/rust-desktop-client/test-plan.md#测试组-1迁移闭环验证]]

### Scenario: 业务层先 Rust 化
- GIVEN 当前项目仍在使用旧 UI
- WHEN 开始整体 Rust 重构
- THEN 必须先将查评、缓存、发货、授权调用等核心业务迁移到 Rust
- AND 不允许直接先重做 UI 而保留大量 Python 业务逻辑不动

### Scenario: 最终移除 Python 运行时依赖
- GIVEN Rust 桌面客户端已经完成业务与 UI 迁移
- WHEN 发布正式切换版本
- THEN 最终发布产物不得依赖 Python 解释器、Cython、PyInstaller

## Requirement 2: 桌面端必须保留现有业务能力

Rust 桌面客户端必须保留当前业务功能，不得因重构而缩减关键能力。  
Traceability: [[design.md#目标架构]] [[design.md#phase-3桌面业务层迁移到-rust]] [[specs/rust-desktop-client/test-plan.md#测试组-2功能等价验证]]

### Scenario: 中差评查找能力保留
- GIVEN 用户使用新桌面客户端
- WHEN 执行中差评订单查找或完整补查
- THEN 新客户端必须提供与现有版本等价的任务入口、执行结果和状态反馈

### Scenario: 缓存与批量发货能力保留
- GIVEN 用户使用新桌面客户端
- WHEN 执行订单缓存同步或批量物流更新
- THEN 新客户端必须提供等价能力，并保持稳定的错误提示与中断恢复逻辑

## Requirement 3: UI 框架必须统一为单一 Rust 方案

系统必须只保留一套桌面 UI 实现路线，不允许长期共存 PySide6 与 Rust GUI 双实现。  
Traceability: [[design.md#ui-技术选型]] [[design.md#决策-2保留阶段性双轨但最终只保留-rust]] [[specs/rust-desktop-client/test-plan.md#测试组-3ui-路线收敛验证]]

### Scenario: 选型固定后不再引入第二套正式 UI
- GIVEN 已确认使用 Tauri 或其他 Rust UI 方案
- WHEN 进入正式实现阶段
- THEN 旧 PySide6 UI 只允许作为迁移过渡
- AND 不允许继续给旧 UI 添加长期新功能

## Traceability

### Forward Links
- [[design.md#phase-3桌面业务层迁移到-rust]]
- [[design.md#phase-4桌面-ui-重建]]
- [[specs/rust-desktop-client/test-plan.md#测试组-1迁移闭环验证]]
- [[specs/rust-desktop-client/test-plan.md#测试组-2功能等价验证]]
- [[specs/rust-desktop-client/test-plan.md#测试组-3ui-路线收敛验证]]


## Task Links
- [[tasks.md#1-建立-rust-workspace-与目标目录骨架]]
- [[tasks.md#4-将桌面业务能力迁移到-rust-services]]
- [[tasks.md#5-重建桌面-ui-为-rust-唯一路线]]
- [[tasks.md#7-执行割接与旧实现退役]]
