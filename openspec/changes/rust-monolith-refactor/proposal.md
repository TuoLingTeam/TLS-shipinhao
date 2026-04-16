# Rust 单语言整体重构提案

## 背景

当前仓库同时包含：

- Python + PySide6 桌面客户端
- Cloudflare Workers JavaScript 授权后端
- Python/Cython/PyInstaller 构建链路
- 新引入的 Rust `security-core`

这种结构虽然可以逐步加固，但会带来明显复杂度：

- 客户端、后端、构建、加固逻辑跨 3 种语言维护
- 测试、打包、发布、依赖管理各自独立
- 授权、安全、构建策略分散，长期会增加出错面与维护成本

用户已经明确接受“整体 Rust 化”的方向，并将“单语言、单结构、长期可维护”放在高优先级。

## 为什么现在做

1. 当前项目功能边界已相对稳定，适合做一次架构收敛。
2. GitHub 已有完整源码备份，允许进行结构性重构。
3. 已经开始引入 Rust 安全核，说明基础技术路线成立。
4. 继续在 Python/JS/Rust 三套结构上叠加能力，会让长期维护成本高于一次有计划的统一重构。

## 目标

将 TLS-shipinhao 重构为以 Rust 为核心的单仓单语言项目：

- 桌面端核心逻辑使用 Rust 实现
- 授权服务使用 Rust 实现
- 构建、打包、版本、发布流程统一围绕 Rust 工具链
- 最终移除 Python 客户端、JavaScript Worker、Cython/PyInstaller 构建依赖

## 非目标

- 本轮不追求一次性“全部推倒重写后再发布”
- 不要求第一阶段立即替换现有 UI 交互全部细节
- 不要求 Linux 平台同步支持
- 不在本提案内处理云端代理化业务架构调整

## 能力拆分

### 新能力
- `rust-desktop-client`
- `rust-license-service`
- `rust-build-release`

### 修改能力
- 现有桌面客户端授权流程
- 现有后端签发与校验模型
- 现有构建与发布流程

## 成功标准

- 可以在单一 Rust workspace 中同时构建桌面端与授权服务
- 桌面端核心业务与授权逻辑不再依赖 Python
- 授权后端不再依赖 JavaScript Worker 运行时
- 构建发布产物不再依赖 PyInstaller/Cython
- 迁移期间每个阶段都有可发布、可回滚、可验证的中间状态
