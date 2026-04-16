# rust-license-service 测试计划

## 测试组 1：API 兼容验证

关联需求：[[spec.md#requirement-1-授权服务必须迁移为-rust-实现]]

- 对比 Rust 服务与旧服务 `/api/activate` `/api/verify` 返回结构
- 验证客户端无需立即修改协议即可接入
- 验证错误码与状态字段兼容

## 测试组 2：授权状态机验证

关联需求：[[spec.md#requirement-2-租约设备绑定和审计必须统一在-rust-服务中]]

- 未激活 -> 激活 -> 校验 -> 续签
- 激活后设备更换 -> 拒绝 / 重绑
- 吊销 -> 下一次校验失效
- 过期 -> 返回一致状态
- 审计日志完整性验证

## 测试组 3：部署与运行验证

关联需求：[[spec.md#requirement-3-部署方式必须与语言统一目标一致]]

- 验证 Rust 服务独立运行
- 验证构建、发布、配置不再以 Node.js 为核心
- 验证灰度环境与正式环境可切换

## Traceability

### Forward Links
- [[spec.md#requirement-1-授权服务必须迁移为-rust-实现]]
- [[spec.md#requirement-2-租约设备绑定和审计必须统一在-rust-服务中]]
- [[spec.md#requirement-3-部署方式必须与语言统一目标一致]]


## Task Links
- [[tasks.md#2-抽离共享协议与领域模型]]
- [[tasks.md#3-将授权服务整体迁移到-rust]]
- [[tasks.md#7-执行割接与旧实现退役]]
