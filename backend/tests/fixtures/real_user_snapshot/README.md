# real_user_snapshot 夹具说明

本目录用于承接 `M6-02 真实数据比对测试` 的脱敏快照。

## 文件约定

- `snapshot.json`：基准输入与 Python 结果
- `snapshot.template.json`：字段模板
- 运行命令：`cargo run -p xtask -- bench-match backend/tests/fixtures/real_user_snapshot backend/tests/artifacts/bench_match`

## 脱敏要求

以下字段必须先脱敏再入库：

- `openid`
- `buyer_nickname`
- `tracking_number`
- `receiver_name`
- 其他可识别个人/订单身份的信息

建议做法：

- 稳定哈希：`sha256(value + salt)`
- 同一字段保留稳定映射，确保 Python / Rust 两边输入一致
- 商品 ID / SKU ID / 时间戳 /评分结果等用于比对的关键字段不要改语义

## snapshot.json 结构

- `snapshot_name`：快照名
- `notes`：备注
- `orders`：Rust 候选订单输入（`CandidateOrder[]`）
- `evaluations`：评价输入（`EvaluationRecord[]`）
- `python_results`：Python 4.3.0 导出的基准结果

## 当前状态

仓库内默认放的是 **脱敏样板数据**，用于验证 `bench-match` 工作流可运行。
正式发版前请替换成真实用户脱敏快照，并保留生成出的 `summary.md` / `diff.csv` 作为发版证据。


## 通过门槛

- `python_results` 至少 **100 条评价样本**
- Rust / Python 不一致率需 **≤ 2%**
- 报告需输出 `summary.md` / `diff.csv` / `diff.json`
