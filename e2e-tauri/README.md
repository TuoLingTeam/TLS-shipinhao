# Tauri 真壳 E2E（L4-4）

WebdriverIO + `tauri-driver` 驱动 Tauri 真壳的端到端测试骨架，与
`apps/ui/e2e/`（Playwright + vite preview，仅前端 / IPC mock）互为补充。

> 本目录**故意脱离根 `pnpm-workspace.yaml`**：避免 wdio 链上百兆依赖污染主前端
> `node_modules`，也方便桌面 / Worker / 前端各自独立的开发者按需 opt-in。

## 平台支持矩阵

| 平台 | 状态 | 驱动 |
|------|------|------|
| Linux（Ubuntu 22.04+） | ✅ 一等支持 | `webkit2gtk-driver` + `tauri-driver` |
| Windows 10/11 | ⚙️ 待营位 | `msedgedriver`（与 WebView2 配套） + `tauri-driver` |
| macOS | ❌ 不支持 | Apple WKWebView 没有官方 WebDriver；macOS 上请用 `pnpm test:e2e:web` |

## 本地运行（Linux）

### 1. 一次性安装系统依赖

```bash
# Ubuntu / Debian
sudo apt-get update
sudo apt-get install -y webkit2gtk-driver xvfb libwebkit2gtk-4.1-dev

# 安装 tauri-driver（Rust crate）
cargo install --locked tauri-driver
```

### 2. 安装本目录 npm 依赖

```bash
cd e2e-tauri
npm install
```

> 用 `npm` 而非 `pnpm`：本目录没接入根 workspace，独立安装更直观。

### 3. 构建桌面 release 产物

```bash
# 在仓库根执行
pnpm tauri:build
```

`wdio.conf.ts` 默认使用 `<repo-root>/target/release/desktop`（Linux/macOS）或
`desktop.exe`（Windows）作为被测应用。

### 4. 跑测试

```bash
# 桌面环境（已有 X server）：
npm test

# 无头服务器（CI 或 SSH）：
npm run test:ci   # 等价于 xvfb-run -a wdio run wdio.conf.ts
```

> 也可以从仓库根使用统一入口：
> - `pnpm test:e2e:tauri`：自动判定平台。macOS 会打印降级提示并 exit 0；
>   Linux/Windows 自动 `cd e2e-tauri && npm install && npm test`。
> - `pnpm test:e2e:tauri:headless`：等价于上面但通过 `xvfb-run` 走无头链路。
> - 入口脚本：`scripts/run-tauri-e2e.mjs`。

`wdio.conf.ts` 的 `onPrepare` 会自动 spawn `tauri-driver` 子进程，结束时
`onComplete` 收尾，无需手动管理。如需对接外部已运行的 driver，设置
`SKIP_TAURI_DRIVER=1` 即可禁用自动 spawn。

## CI 接入

`.github/workflows/tauri-e2e.yml` 仅在 `workflow_dispatch` 手动触发，避免
每次 PR 都跑（cargo build release 在 GitHub Actions 上耗时较长）。触发后会：

1. apt 安装 `webkit2gtk-driver` / `xvfb` / `libwebkit2gtk-4.1-dev`
2. `cargo install --locked tauri-driver`
3. `pnpm install --frozen-lockfile && pnpm --filter tls-shipinhao-ui build`
4. `cargo tauri build --no-bundle`（仅产 binary，省 dmg/AppImage）
5. `cd e2e-tauri && npm install`
6. `xvfb-run -a npm test`

如需在 PR 上自动跑，把 workflow 的 `on:` 块加上 `pull_request`，但建议同时
设置路径过滤（`paths: [apps/**, crates/**, e2e-tauri/**, Cargo.toml]`）以
规避非桌面相关变更触发昂贵任务。

## 测试覆盖范围

当前仅 `specs/smoke.spec.ts`：

- Vue 已挂载到 `#app` 且容器有子节点
- 窗口标题包含产品名

不在本骨架做业务断言（订单匹配、授权状态机、批量发货）——那些用 vitest
单测和 Playwright + IPC mock 已能覆盖；真壳 E2E 的核心价值是验证
**「打包后的二进制能不能在真实 WebView 里启动并响应基础交互」**，对应
回归点是工具链 / Tauri capability / IPC 适配等"集成层"。

后续可扩展的 spec 方向（按性价比排序）：

1. 设置页 → 模拟 Cookie 流程能不能弹出登录窗口（不实际登录）
2. 评价管理页能不能渲染 + 分页交互不挂
3. 批量发货按钮可见性 + 取消按钮的事件绑定

## 常见问题

### `tauri-driver: command not found`

未 `cargo install`；Tauri 官方目前**不**提供预编译 binary，需要从源码编译。

### Linux 上启动后窗口空白 / `webkit2gtk-driver` 报 socket 错

通常是 `xvfb-run` 没起到作用或显示编号冲突。试 `xvfb-run --auto-servernum`
或显式指定 `Xvfb :99 -screen 0 1920x1080x24 &` 然后 `DISPLAY=:99 npm test`。

### `target/release/desktop` 不存在

先在仓库根跑 `pnpm tauri:build` 或 `cd apps/desktop && cargo build --release`。

### macOS 用户尝试运行

会得到 `tauri-driver` install 失败或运行时挂起。请退回 web 路径：
```bash
pnpm test:e2e:web
```
