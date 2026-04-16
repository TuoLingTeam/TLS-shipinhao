# rust-build-release 测试计划

## 测试组 1：构建链路替换验证

关联需求：[[spec.md#requirement-1-桌面构建链路必须统一为-rust-工具链]]

- 验证 workspace 可以统一 build
- 验证桌面端正式产物由 Rust 工具链生成
- 验证 macOS / Windows 两套产物输出一致

## 测试组 2：安全发布验证

关联需求：[[spec.md#requirement-2-签名与完整性清单必须保留并融入-rust-发布流程]]

- 验证 manifest 生成与签名
- 验证租约公钥/私钥配置检查
- 验证完整性校验在产物内可工作

## 测试组 3：旧链路退役验证

关联需求：[[spec.md#requirement-3-旧构建脚本必须可删除]]

- 删除 `scripts/obfuscate.py` 后仍可构建
- 删除 `scripts/build.py` 后仍可构建
- CI 不再依赖 Python 构建步骤

## Traceability

### Forward Links
- [[spec.md#requirement-1-桌面构建链路必须统一为-rust-工具链]]
- [[spec.md#requirement-2-签名与完整性清单必须保留并融入-rust-发布流程]]
- [[spec.md#requirement-3-旧构建脚本必须可删除]]


## Task Links
- [[tasks.md#1-建立-rust-workspace-与目标目录骨架]]
- [[tasks.md#6-统一构建签名与发布流程]]
- [[tasks.md#7-执行割接与旧实现退役]]
