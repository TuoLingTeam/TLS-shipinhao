# rust-build-release 规范

## 概述

该能力定义将当前 Python 构建链路统一迁移为 Rust 工具链的范围与目标。

## Requirement 1: 桌面构建链路必须统一为 Rust 工具链

系统必须以 Rust 工具链承担正式构建、打包与发布，不再依赖 Cython/PyInstaller 作为正式桌面构建链路。  
Traceability: [[design.md#phase-5构建链路统一]] [[specs/rust-build-release/test-plan.md#测试组-1构建链路替换验证]]

### Scenario: 正式发布命令统一
- GIVEN 项目进入 Rust 单语言阶段
- WHEN 执行正式构建
- THEN 应通过 `cargo` / `cargo tauri` / `cargo xtask` 等 Rust 工具完成

## Requirement 2: 签名与完整性清单必须保留并融入 Rust 发布流程

系统必须保留当前租约、manifest 与安全构件的签名能力，并将其整合进 Rust 发布流程。  
Traceability: [[design.md#phase-5构建链路统一]] [[design.md#决策-4安全与业务模型分离]] [[specs/rust-build-release/test-plan.md#测试组-2安全发布验证]]

### Scenario: 发布产物带签名清单
- GIVEN 执行正式发布
- WHEN 构建桌面产物
- THEN 产物必须包含完整性清单与签名信息
- AND 构建流程必须验证必要密钥与发布参数

## Requirement 3: 旧构建脚本必须可删除

当 Rust 发布链路稳定后，旧的 Python 构建脚本必须可完全退役。  
Traceability: [[design.md#phase-5构建链路统一]] [[specs/rust-build-release/test-plan.md#测试组-3旧链路退役验证]]

### Scenario: 删除旧链路不影响发布
- GIVEN Rust 发布链路已通过全部验证
- WHEN 移除旧脚本
- THEN 桌面构建、版本注入、发布上传仍可正常完成

## Traceability

### Forward Links
- [[design.md#phase-5构建链路统一]]
- [[specs/rust-build-release/test-plan.md#测试组-1构建链路替换验证]]
- [[specs/rust-build-release/test-plan.md#测试组-2安全发布验证]]
- [[specs/rust-build-release/test-plan.md#测试组-3旧链路退役验证]]


## Task Links
- [[tasks.md#1-建立-rust-workspace-与目标目录骨架]]
- [[tasks.md#6-统一构建签名与发布流程]]
- [[tasks.md#7-执行割接与旧实现退役]]
