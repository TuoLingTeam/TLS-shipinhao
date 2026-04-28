# 文档目录约定

本目录收纳设计、运营、安全相关文档。**所有新增 / 迁入文档必须放进对应子目录**，
根级仅保留本 README，避免再次出现根级与子目录同名漂流副本。

## 子目录职责

| 子目录 | 收纳 | 命名风格 |
|--------|------|---------|
| `architecture/` | 架构设计、模块划分、技术选型分析 | 中文主题名，例如 `项目深度分析报告.md` |
| `backend/` | 后端（Worker / 授权服务）相关设计、迁移计划 | 中文主题名 |
| `operations/` | 发版流程、运维手册、混淆/打包策略 | 短小英文 kebab-case，例如 `release-runbook.md` |
| `product/` | PRD、产品优化建议、任务卡片 | 中文主题名 |
| `reports/` | 性能/回归/全链路测试报告 + 模板 | 报告类带日期 `*-YYYY-MM-DD.md`；模板 `*-template.md` |
| `security/` | 安全审查、授权链路审计、职责图 | 中文主题名 |
| `archive/` | 已归档的历史快照（仅供回溯，**不保证现状一致**） | 见下方说明 |

## archive/ 历史快照

- `archive/1.md` / `archive/2.md`：Tauri 重构早期版本的架构全景与任务计划，
  提到的目录布局（`infra/tooling/xtask`、`shared/api-contracts` 等）已在
  正式落地时调整为 `crates/*` 命名，仅作历史参考。
- `archive/Tauri-Vue-架构全景.md`：重构落地后的架构全景图，反映 `crates/*`
  与 `apps/desktop`、`apps/ui` 的当前布局；如再次发生大规模目录重构，
  建议在该文件之上写新版而非修改本快照。

## 编辑约定

- 文档语言：**中文**为主；技术专有名词（API、命令名、路径）保留英文。
- 不要在根级新建 `.md`；新增内容请落到对应子目录。
- 报告类文档命名带日期前/后缀；模板与实例同时存在时，模板用 `*-template.md`。
- `regression-report-{date}.md` 这类**字面占位** stub 不应进入仓库，请用
  `reports/regression-report-template.md` 拷贝后再改名为带真实日期的实例。

## 与 `.gitignore` 的关系

根目录 `.gitignore` 中 `/docs/` + `!docs/` 的组合表示「默认忽略，但已跟踪
内容继续受版本管理」。新增文档同样需要 `git add` 才会进入仓库；如发现
`git status` 没显示某个新文档，多半是 `.gitignore` 子规则把它排除了，
请检查后将文件归入合适子目录再提交。
