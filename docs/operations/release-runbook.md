# TLS-shipinhao 发布 / 灰度运行手册

## 1. 目标

按 **内测 → 灰度 10% → 全量** 节奏发布 `TLS-shipinhao 5.1.0`，并保留可快速回滚的操作路径。

## 2. 发布前门槛

<<<<<<<< Updated upstream:docs/release-runbook.md
- `M6-01`：`docs/regression-matrix.md` 已落地，40 条用例均有归属
- `M6-02`：`cargo run -p xtask -- bench-match ...` 已产出 `summary.md` / `diff.csv`
- `M6-03`：`cargo run -p xtask -- perf docs/perf-report-2026-04-17.md` 已执行
========
- `M6-01`：`docs/reports/regression-matrix.md` 已落地，40 条用例均有归属
- `M6-02`：`cargo run -p xtask -- bench-match ...` 已产出 `summary.md` / `diff.csv`
- `M6-03`：`cargo run -p xtask -- perf docs/reports/perf-report-2026-04-17.md` 已执行
>>>>>>>> Stashed changes:docs/operations/release-runbook.md
- GitHub Actions `release.yml` 可手动触发
- 签名 / notarization / Windows 代码签名材料已就绪

## 3. 标准发布流程

### 3.1 生成 release 元数据

```bash
cargo run -p xtask -- release 5.1.0 backend/dist/release
```

产物：

- `backend/dist/release/version.json`
- `rolling.percentage = 10`
- `mandatory = false`

### 3.2 触发 CI 打包

GitHub Actions：`Release Pipeline`

- 可选输入：`version`
- 构建平台：`macos-latest` / `windows-latest`
- 产物：
  - macOS `.dmg`
  - Windows `.exe`
  - `version.json`
  - `integrity_manifest.json`

### 3.3 内测 / 灰度 / 全量

1. **内测**
   - 仅内部发包，不改线上 `version.json`
   - 验证安装、启动、更新提示、授权、发货链路
2. **灰度 10%**
   - 发布 `version.json`
   - 保持：`rolling.percentage = 10`
   - 客户端按设备标识哈希命中灰度范围
3. **全量**
   - 将 `rolling.percentage` 调整为 `100`
   - 保持 `mandatory = false`

## 4. 回滚预案

### 4.1 软回滚

适用：仅新版本有问题，但旧版本仍可继续使用。

操作：

- 将 `version.json` 的 `version` 回退到上一稳定版本
- 保持 `mandatory = false`
- 重新上传 `version.json`

### 4.2 强制回滚 / 强制更新

适用：发现严重缺陷，需要强制用户离开问题版本。

操作：

- `version` 指向稳定版本
- `mandatory = true`
- `rolling.percentage = 100`
- `notes` 中明确写明“紧急回滚 / 强制更新原因”

## 5. 发布后观察项

- 更新横幅展示率
- 下载转化率
- 激活失败率
- 评价匹配异常率
- 发货失败率
- 崩溃 / 完整性告警数量

## 6. 演练建议

每次正式发版前至少演练一次：

1. 上传新 `version.json`
2. 灰度 10%
3. 本地验证命中 / 未命中两类设备
4. 执行一次回滚
5. 确认客户端恢复到旧版本提示逻辑
