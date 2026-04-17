# TLS-shipinhao 5.1.0 回归验收用例矩阵

> 对应 `docs/功能补齐PRD_与原版对齐.md` §16.1 的 40 项验收清单。  
> 用途：作为 M6 发版前的统一验收基线、CI/手工回归执行清单、问题追踪索引。

## 1. 使用说明

- `用例编号` 与 PRD 验收编号保持一致，不回收、不重排。
- `执行方式`
  - `自动化`：优先纳入 Rust 单测 / 集成测试 / 前端构建校验 / 脚本化回归
  - `手工`：需要真实设备、系统能力、Keychain、UI 视觉或外部环境参与
- `状态`
  - `待补自动化`：矩阵已落表，但测试代码/脚本仍需在 M6-02 / M6-03 或后续 QA 中补齐
  - `可直接执行`：已有明确命令或既有测试承载
  - `手工执行`：保留为人工回归项
- 所有失败项必须在测试报告中记录：`编号 + 现象 + 复现步骤 + 影响范围 + 结论`。

## 2. 覆盖统计

| 分类 | 数量 |
|---|---:|
| 总用例数 | 40 |
| 自动化/脚本化 | 21 |
| 手工执行 | 19 |
| 其中高风险（授权/数据/发货） | 23 |

> 满足卡片要求：**自动化用例 ≥ 20 条**。

## 3. 执行矩阵

| 编号 | 模块 | 验收点 | 执行方式 | 当前归属 / 命令 / 证据入口 | 状态 | 失败记录要求 |
|---|---|---|---|---|---|---|
| AC-01 | 授权 | 主域名可访问 → 激活成功 | 手工 | `activate_license`，需联通真实授权服务 | 手工执行 | 记录主域名、卡密尾号、返回状态 |
| AC-02 | 授权 | 主域名超时 → 自动切换备用域名 | 手工 | `apps/desktop/src/adapters/http_license_client.rs`，需网络注入 | 手工执行 | 记录超时域名、切换顺序、最终命中域名 |
| AC-03 | 授权 | 4 个域名都超时 → 显示网络错误 | 手工 | 真实/模拟断网环境执行激活 | 手工执行 | 记录错误文案、耗时、是否阻塞 UI |
| AC-04 | 授权 | 激活后收到完整 Lease（Ed25519 签名） | 手工 | `crates/license-service/**` + 激活链路抓包/日志 | 手工执行 | 记录 lease 字段完整性与签名校验结果 |
| AC-05 | 授权 | 24h 内无网络可正常使用 | 手工 | 已激活后断网验证核心功能可用 | 手工执行 | 记录断网时长、可用功能、授权状态 |
| AC-06 | 授权 | 24h 后自动续约 | 手工 | 调整时钟/模拟 lease 时间窗口 | 手工执行 | 记录续约时机、续约后到期时间 |
| AC-07 | 授权 | 72h 后要求重新激活 | 手工 | 调整时钟/模拟硬过期 | 手工执行 | 记录提示文案、阻断点 |
| AC-08 | 授权 | 设备指纹正确获取（三平台测试） | 手工 | `security-core` + macOS/Windows/其他环境 | 手工执行 | 记录平台、指纹来源、稳定性 |
| AC-09 | 授权 | Lease 存入 Keychain / Credential Manager | 手工 | 系统级存储检查 | 手工执行 | 记录存储位置、读写结果 |
| AC-10 | 授权 | 任务级授权：差评查询前先调 authorize_task | 手工 | 评价查询前抓日志/请求顺序 | 手工执行 | 记录授权调用先后顺序 |
| AC-11 | 授权 | 完整性校验：启动时验证关键文件哈希 | 手工 | 启动流程验证 | 手工执行 | 记录校验范围与启动表现 |
| AC-12 | 授权 | 篡改文件后再次启动 → 显示警告 | 手工 | 篡改测试样本后重启应用 | 手工执行 | 记录被改文件、告警文案、阻断行为 |
| AC-13 | 反风控 | HTTP 429 触发 2-4-8 秒指数退避 | 自动化 | `cargo test -p desktop-services anti_risk_pipeline -- --nocapture` | 可直接执行 | 记录重试次数与退避间隔 |
| AC-14 | 反风控 | 风控 code=430 触发 60 秒冷却 | 自动化 | `crates/desktop-services/tests/anti_risk_pipeline.rs` | 可直接执行 | 记录冷却开始时间与恢复条件 |
| AC-15 | 反风控 | 冷却后自动进入极速模式 | 自动化 | `crates/desktop-services/tests/anti_risk_pipeline.rs` | 可直接执行 | 记录 worker 数、节流间隔 |
| AC-16 | 反风控 | 极速模式再次风控 → 返回已有数据 | 自动化 | `crates/desktop-services/tests/anti_risk_pipeline.rs` | 可直接执行 | 记录回退数据来源与条数 |
| AC-17 | 反风控 | UA 根据系统自动选择 | 自动化 | `tests/test_rust_desktop_services.py` + Rust 单测 | 待补自动化 | 记录平台、UA 字符串 |
| AC-18 | 订单同步 | 首次同步完成后建立 `cache_segments` 记录 | 自动化 | `cargo test -p desktop-services order_cache_storage -- --nocapture` | 待补自动化 | 记录 scope / coverage 区间 |
| AC-19 | 订单同步 | 再次同步时通过 `cache_segments` 跳过已完成窗口 | 自动化 | `order_gap_planner` / `order_sync_service` 单测 | 待补自动化 | 记录跳过窗口与请求次数 |
| AC-20 | 订单同步 | 有 500 秒缺口时自动补齐 | 自动化 | `cargo test -p desktop-services get_missing_segments -- --nocapture` | 待补自动化 | 记录缺口输入与补齐输出 |
| AC-21 | 订单同步 | 200 秒以下缺口直接忽略 | 自动化 | 同上 | 待补自动化 | 记录 gap 宽度与忽略判断 |
| AC-22 | 订单同步 | dirty `sale_param` 检测 → 自动重建 | 自动化 | `order_cache_storage` / `order_sync_service` 单测 | 待补自动化 | 记录 dirty 样本与 rebuild 行为 |
| AC-23 | 订单同步 | `fetch_full_scan_orders` 支持查询 60 天前评价 | 自动化 | `xtask`/后续 e2e 脚本 | 待补自动化 | 记录 earliest_time 与结果条数 |
| AC-24 | 订单同步 | 缓存包含 `order_products` / `is_education_order` 等完整字段 | 自动化 | `tests/test_rust_domain_models.py` + Rust 仓储单测 | 待补自动化 | 记录字段快照 |
| AC-25 | 订单同步 | 从 Python 版升级自动迁移本地缓存 | 自动化 | `cargo test -p desktop start_legacy_migration -- --nocapture` | 待补自动化 | 记录迁移前后文件与数据量 |
| AC-26 | 评价匹配 | 买家改名加数字尾巴 → 识别为 95 分 | 自动化 | `cargo test -p desktop-services nickname -- --nocapture` | 待补自动化 | 记录昵称输入与得分 |
| AC-27 | 评价匹配 | 长昵称包含短昵称 → 90 分 | 自动化 | 同上 | 待补自动化 | 记录昵称输入与得分 |
| AC-28 | 评价匹配 | 匿名/微信用户/默认昵称 → 直接 0 分 | 自动化 | 同上 | 待补自动化 | 记录过滤命中原因 |
| AC-29 | 评价匹配 | 差评可回复期检测正确（-30 天阈值） | 自动化 | `cargo test -p desktop-services reply_window -- --nocapture` | 待补自动化 | 记录时间边界与结果 |
| AC-30 | 评价匹配 | 匹配结果包含 `strategy` 字段 | 自动化 | `tests/test_rust_desktop_services.py` / Rust 流程单测 | 待补自动化 | 记录 strategy 分档 |
| AC-31 | 评价匹配 | 品退订单包含 `reason` 字段 | 自动化 | `tests/test_rust_desktop_services.py` / 前后端类型对齐校验 | 待补自动化 | 记录 reason 实际值 |
| AC-32 | 发货 | SF 单号 + 选择中通 → 自动降级为 SF | 自动化 | `cargo test -p desktop delivery -- --nocapture` | 待补自动化 | 记录输入 carrier 与最终 carrier |
| AC-33 | 发货 | 物流更新后仅 `waybillId` 改变 | 自动化 | `crates/desktop-services/src/delivery_update.rs` 单测 | 待补自动化 | 记录 old/new diff |
| AC-34 | 发货 | `initShipData` 失败 → 自动回退 `orderDetail` | 自动化 | `apps/desktop/src/adapters/http_delivery_gateway.rs` 相关单测 | 待补自动化 | 记录主链路失败与回退成功证据 |
| AC-35 | UI/UX | 窗口标题显示“驼铃·视频小店差评处理 5.1.0” | 自动化 | `cargo test -p domain-core` + `get_app_info_returns_brand_fields` | 可直接执行 | 记录标题来源与版本号 |
| AC-36 | UI/UX | 主题色为翠绿（#059669） | 手工 | `ui/src/assets/styles/main.css` + 人眼走查 | 手工执行 | 截图留档，标注主按钮/横幅/侧边栏 |
| AC-37 | UI/UX | 支持 UI 缩放（0.82-1.0，Ctrl +/-/0） | 自动化 | `cargo test -p desktop set_ui_scale_clamps_to_supported_range -- --nocapture` + 手工快捷键回归 | 可直接执行 | 记录缩放值、快捷键、重启持久化 |
| AC-38 | UI/UX | 小于 860px 宽度自动进入紧凑布局 | 手工 | 浏览器窗口 / Tauri 窗口缩放走查 | 手工执行 | 记录窗口尺寸、布局模式、截图 |
| AC-39 | UI/UX | 启动时自动检查更新 | 自动化 | `crates/desktop-services/src/update_service.rs` + `apps/desktop/src/main.rs` | 可直接执行 | 记录启动后 1.2s 检查行为 |
| AC-40 | UI/UX | 有新版本时顶部显示横幅 + notes | 手工 | `ui/src/components/layout/UpdateBanner.vue` + mock 更新 payload | 手工执行 | 记录版本号、notes、按钮、稍后隐藏 |

## 4. 推荐执行顺序

### 4.1 自动化回归批次

1. Rust 域模型与桌面服务
   - `cargo test -p domain-core`
   - `cargo test -p desktop-services -- --nocapture`
2. Desktop 命令与桌面适配层
   - `cargo test -p desktop -- --nocapture`
3. Python/Rust 桥接与仓库级冒烟
   - `python3 -m unittest discover -s tests/integration -p "test_*.py"`
4. 前端类型与构建
   - `pnpm --filter tls-shipinhao-ui lint`
   - `pnpm --filter tls-shipinhao-ui build`

### 4.2 手工回归批次

1. 授权与完整性
2. 桌面 UI / 更新横幅 / 紧凑布局
3. 真实发货与风控外部环境
4. 打包产物验收

## 5. 失败用例记录规范

每条失败用例必须最少包含以下信息：

- 用例编号：如 `AC-19`
- 版本：如 `5.1.0`
- 环境：macOS / Windows / 网络 / 测试账号
- 复现步骤：最少 3 步，可直接重跑
- 实际结果 vs 预期结果
- 日志/截图/命令输出路径
- 是否阻塞发版：`阻塞 / 非阻塞`
- 临时绕过方案（如有）

## 6. 发布门槛

- 40 条用例必须全部在本矩阵中有归属。
- 自动化用例跑完后才能进入手工回归。
- 任一 `阻塞发版` 的失败项未关闭前，不允许执行 `M6-04 打包 / 灰度发布`。
