# TLS-shipinhao

微信小商店中差评订单查找与物流更新工具，包含桌面客户端和卡密授权后端。

## 目录结构

```text
TLS-shipinhao/
├── app/                          # 兼容期 Python 启动壳（委托 Rust desktop-app）
├── crates/                       # Rust 核心 crates（domain / services / security / app）
│   ├── main.py                   # 入口，调用 bootstrap.main()
│   ├── bootstrap.py              # 程序启动入口（QApplication 初始化）
│   ├── settings.py               # 全局配置与常量（URL、窗口尺寸、抓取参数、匹配权重等）
│   ├── requirements.txt          # Python 依赖
│   ├── assets/                   # 静态资源
│   │   └── favicon.png
│   ├── core/                     # 核心基础层
│   │   ├── day_window.py         # 自然日时间窗口工具
│   │   ├── http_utils.py         # HTTP 工具（请求头构建、响应解析）
│   │   └── license.py            # 在线授权（卡密激活、设备绑定、离线校验）
│   ├── services/                 # 业务服务层
│   │   ├── delivery_api.py       # 发货/物流接口
│   │   ├── order_cache.py        # 订单本地缓存（SQLite 持久化）
│   │   ├── order_match_scoring.py# 订单匹配评分
│   │   ├── order_sync.py         # 订单增量同步（缓存+远程协调）
│   │   └── review_matcher.py     # 中差评订单查找（抓取、匹配、汇总）
│   └── ui/                       # 界面层
│       ├── batch_worker.py       # 批量任务后台执行器
│       ├── cookie_dialog.py      # Cookie 浏览器弹窗
│       ├── review_worker.py      # 中差评/品退任务后台执行器
│       ├── widgets.py            # 自定义控件与通用 UI 组件
│       └── window.py             # MainWindow 主窗口
├── apps/                         # Rust 应用壳（desktop / license-worker）
├── backend/                      # 兼容期 Cloudflare 目录（待完全退役）
│   ├── src/
│   │   ├── index.js              # Worker 入口（API 路由、卡密生成/校验）
│   │   └── admin.html            # 管理后台页面
│   ├── schema.sql                # D1 数据库建表语句
│   ├── wrangler.toml             # Workers 部署配置
│   └── README.md                 # 后端详细文档
├── xtask/                        # Rust 构建、manifest 与发布命令
└── README.md
```

## 模块依赖关系

```text
settings ← core ← services/review_matcher
               ← services/order_cache ← services/order_sync
               ← services/delivery_api
               ← core/http_utils
               ← core/license

旧 PySide6 UI 已冻结，`app/main.py` / `app/bootstrap.py` 仅作为兼容启动壳委托 Rust desktop-app
```

## 功能特性

- 中差评订单自动查找（多线程并行抓取 + 智能评价匹配）
- 订单本地 SQLite 缓存（增量同步，避免重复拉取）
- 批量更新订单物流单号
- Cookie 浏览器自动采集登录态
- 在线卡密授权（设备绑定 + 离线回退校验）
- 图形化桌面界面（Rust + Slint）

## 环境要求

- Rust stable
- Node.js 18+（仅兼容期 Cloudflare 目录/部署辅助）

## 快速开始

### 桌面客户端

```bash
# 运行 Slint 桌面客户端
cargo run -p desktop-app
```

### 构建发布包

```bash
# 构建 release 二进制
cargo run -p xtask -- desktop-build --release

# 生成完整性清单
cargo run -p xtask -- manifest target/release dist/integrity-manifest.json desktop-app
```

### 后端部署

```bash
cd backend
npm install
npm run deploy
```

或直接使用完整路径一键部署：

```bash
cd /Users/zxr/Downloads/source-code/TLS-shipinhao/backend && npx wrangler deploy 2>&1
```

详见 [backend/README.md](backend/README.md)。

## 注意事项

- 确保 `cookie.txt` 文件存在且内容有效（包含 biz_magic 值）
- Cookie 信息需要定期更新，或通过内置浏览器重新登录采集
- 正式构建发布链路已切换到 Rust workspace + xtask
