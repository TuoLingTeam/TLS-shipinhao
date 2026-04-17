# E2E / 回归脚本目录约定

本目录用于承接 `docs/regression-matrix.md` 中标记为“待补自动化”的回归脚本。

## 推荐命名

- `ac_13_http_429_backoff.rs`
- `ac_20_gap_fill_500s.rs`
- `ac_32_delivery_carrier_downgrade.rs`
- `ac_39_update_check_startup.rs`

## 约定

- 文件名优先与验收编号对齐，避免后续回归对不上号。
- 每个脚本/测试文件头部注明：
  - 对应验收编号
  - 前置条件
  - 可复现步骤
  - 断言点
- 若短期无法自动化，也必须先在 `docs/regression-matrix.md` 中登记归属与原因。
