# rust-desktop-client 测试计划

## 测试组 1：迁移闭环验证

关联需求：[[spec.md#requirement-1-桌面客户端必须收敛为-rust-主实现]]

- 验证 Rust 桌面工程可以独立构建
- 验证旧 Python 客户端下线后正式产物仍可运行
- 验证桌面产物不再依赖 Python 解释器 / PyInstaller / Cython

## 测试组 2：功能等价验证

关联需求：[[spec.md#requirement-2-桌面端必须保留现有业务能力]]

- 中差评查找结果与旧版本一致性对比
- 完整补查结果一致性对比
- 批量发货成功/失败路径一致性对比
- SQLite 缓存读写与旧版本兼容验证

## 测试组 3：UI 路线收敛验证

关联需求：[[spec.md#requirement-3-ui-框架必须统一为单一-rust-方案]]

- 验证正式发布分支只存在一套可发布 UI
- 验证旧 PySide6 代码不再承载新增正式功能
- 验证 UI 构建与发布命令统一到 Rust 工具链

## Traceability

### Forward Links
- [[spec.md#requirement-1-桌面客户端必须收敛为-rust-主实现]]
- [[spec.md#requirement-2-桌面端必须保留现有业务能力]]
- [[spec.md#requirement-3-ui-框架必须统一为单一-rust-方案]]


## Task Links
- [[tasks.md#1-建立-rust-workspace-与目标目录骨架]]
- [[tasks.md#4-将桌面业务能力迁移到-rust-services]]
- [[tasks.md#5-重建桌面-ui-为-rust-唯一路线]]
- [[tasks.md#7-执行割接与旧实现退役]]
