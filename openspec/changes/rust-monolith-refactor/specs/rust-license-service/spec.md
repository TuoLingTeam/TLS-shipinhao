# rust-license-service 规范

## 概述

该能力定义把当前 JavaScript/Cloudflare Workers 授权后端迁移为 Rust 授权服务的范围与约束。

## Requirement 1: 授权服务必须迁移为 Rust 实现

系统必须以 Rust 作为授权服务的主实现语言，并最终移除 `backend/src/worker/index.js` 作为正式生产实现。  
Traceability: [[design.md#phase-2授权服务迁移到-rust]] [[specs/rust-license-service/test-plan.md#测试组-1-api-兼容验证]]

### Scenario: 保持 API 兼容
- GIVEN 现有客户端依赖 `/api/activate` 与 `/api/verify`
- WHEN 授权服务迁移到 Rust
- THEN 新服务必须保持现有 API 合约兼容至少一个迁移周期

## Requirement 2: 租约、设备绑定和审计必须统一在 Rust 服务中

Rust 授权服务必须统一管理租约签发、设备绑定、吊销与审计。  
Traceability: [[design.md#phase-2授权服务迁移到-rust]] [[design.md#决策-3共享协议单独-crate-管理]] [[specs/rust-license-service/test-plan.md#测试组-2授权状态机验证]]

### Scenario: 设备绑定状态机一致
- GIVEN 用户已激活授权
- WHEN 在新服务上进行再次校验或更换设备
- THEN 必须与原有绑定规则等价
- AND 吊销、过期、迁移状态要有明确状态机与审计记录

## Requirement 3: 部署方式必须与语言统一目标一致

授权服务迁移后，正式维护形态必须以 Rust 项目和 Rust 构建链路为中心，不再依赖 Node.js 作为核心实现。  
Traceability: [[design.md#phase-2授权服务迁移到-rust]] [[specs/rust-license-service/test-plan.md#测试组-3部署与运行验证]]

### Scenario: 生产部署不再依赖 JavaScript 主实现
- GIVEN Rust 授权服务已经通过兼容验证
- WHEN 切换正式部署
- THEN 不应继续以 JS Worker 作为唯一正式主实现

## Traceability

### Forward Links
- [[design.md#phase-2授权服务迁移到-rust]]
- [[specs/rust-license-service/test-plan.md#测试组-1-api-兼容验证]]
- [[specs/rust-license-service/test-plan.md#测试组-2授权状态机验证]]
- [[specs/rust-license-service/test-plan.md#测试组-3部署与运行验证]]


## Task Links
- [[tasks.md#1-建立-rust-workspace-与目标目录骨架]]
- [[tasks.md#2-抽离共享协议与领域模型]]
- [[tasks.md#3-将授权服务整体迁移到-rust]]
- [[tasks.md#7-执行割接与旧实现退役]]
