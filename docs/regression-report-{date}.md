# TLS-shipinhao 回归测试报告（{date}）

- 版本：`5.1.0`
- 执行人：`{owner}`
- 环境：`{env}`
- 基线 PRD：`docs/功能补齐PRD_与原版对齐.md` §16.1 / §16.2
- 关联矩阵：`docs/regression-matrix.md`

## 1. 结论

- 总用例：40
- 通过：{passed}
- 失败：{failed}
- 阻塞项：{blocking_count}
- 是否允许进入下一阶段：`是 / 否`

## 2. 自动化执行记录

| 批次 | 命令 | 结果 | 备注 |
|---|---|---|---|
| Rust Core | `cargo test -p domain-core` | {pass_or_fail} | {note} |
| Desktop Services | `cargo test -p desktop-services -- --nocapture` | {pass_or_fail} | {note} |
| Desktop | `cargo test -p desktop -- --nocapture` | {pass_or_fail} | {note} |
| Python/Rust 冒烟 | `pytest tests/test_rust_*.py tests/test_security_runtime.py -q` | {pass_or_fail} | {note} |
| UI Lint | `pnpm --filter tls-shipinhao-ui lint` | {pass_or_fail} | {note} |
| UI Build | `pnpm --filter tls-shipinhao-ui build` | {pass_or_fail} | {note} |

## 3. 手工回归记录

| 用例编号 | 模块 | 环境 | 结果 | 备注 |
|---|---|---|---|---|
| AC-01 | 授权 | {env} | {pass_or_fail} | {note} |
| AC-02 | 授权 | {env} | {pass_or_fail} | {note} |
| ... | ... | ... | ... | ... |

## 4. 失败清单

| 用例编号 | 严重级别 | 是否阻塞 | 现象 | 复现步骤 | 证据 |
|---|---|---|---|---|---|
| AC-xx | 高 / 中 / 低 | 阻塞 / 非阻塞 | {actual} | 1. ... 2. ... 3. ... | `{path_or_link}` |

## 5. 兼容性与风险备注

- 操作系统：{os_summary}
- 授权兼容：{license_summary}
- 数据迁移兼容：{migration_summary}
- UI/UX 风险：{ux_summary}

## 6. 发版建议

- 建议：`直接发布 / 修复后复测 / 暂缓发布`
- 原因：{reason}
