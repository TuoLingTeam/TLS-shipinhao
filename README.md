# TLS-shipinhao

微信小商店中差评订单查找与物流更新工具，包含桌面客户端和卡密授权后端。

## 目录结构

```text
TLS-shipinhao/
├── app/                          # 桌面客户端（Python + PySide6）
│   ├── main.py                   # 入口，启动 src.app.main
│   ├── requirements.txt          # Python 依赖
│   ├── src/
│   │   ├── __init__.py
│   │   ├── app.py                # 程序入口（main 函数、QApplication 初始化）
│   │   ├── constants.py          # 全局常量（URL、窗口尺寸、抓取参数、匹配权重等）
│   │   ├── config.py             # 配置管理（cookie/biz_magic 读写、昵称标准化）
│   │   ├── core/                 # 核心基础层
│   │   │   ├── api.py            # 微信小商店 API 交互（订单查询、物流更新）
│   │   │   ├── http_utils.py     # HTTP 工具（请求头构建、响应解析）
│   │   │   ├── cookie_browser.py # Cookie 浏览器弹窗（WebEngine 登录采集）
│   │   │   └── license.py        # 在线授权（卡密激活、设备绑定、离线校验）
│   │   ├── services/             # 业务服务层
│   │   │   ├── review_matcher.py # 中差评订单查找（多线程抓取、评价匹配）
│   │   │   ├── order_cache.py    # 订单本地缓存（SQLite 持久化）
│   │   │   └── order_sync.py     # 订单增量同步（缓存+远程协调）
│   │   └── ui/                   # 界面层
│   │       ├── window.py         # MainWindow 主窗口（UI 布局、事件处理）
│   │       ├── widgets.py        # 自定义控件（批量输入框、授权弹窗、字体）
│   │       ├── worker.py         # BatchWorker 后台批量任务执行器
│   │       └── review_worker.py  # ReviewWorker 中差评查找后台线程
│   └── scripts/
│       ├── obfuscate.py          # Cython 混淆编译脚本
│       └── build.py              # PyInstaller 打包构建脚本
├── backend/                      # 卡密授权后端（Cloudflare Workers + D1）
│   ├── src/
│   │   ├── index.js              # Worker 入口（API 路由、卡密生成/校验）
│   │   └── admin.html            # 管理后台页面
│   ├── schema.sql                # D1 数据库建表语句
│   ├── wrangler.toml             # Workers 部署配置
│   └── README.md                 # 后端详细文档
└── README.md
```

## 模块依赖关系

```text
constants ← config ← core/api ← services/review_matcher
                    ← core/http_utils      ↓
                    ← core/license    services/order_cache ← services/order_sync
                                           ↓
              ui/widgets + ui/worker ← ui/window ← app（入口）
                        ui/review_worker ↗
```

## 功能特性

- 中差评订单自动查找（多线程并行抓取 + 智能评价匹配）
- 订单本地 SQLite 缓存（增量同步，避免重复拉取）
- 批量更新订单物流单号
- Cookie 浏览器自动采集登录态
- 在线卡密授权（设备绑定 + 离线回退校验）
- 图形化桌面界面（PySide6）

## 环境要求

- Python 3.10+
- Node.js 18+（后端部署）

## 快速开始

### 客户端

```bash
# 安装依赖
pip install -r app/requirements.txt

#激活虚拟环境
source .venv/bin/activate
#激退出虚拟环境
deactivate .venv/bin/activate

# 运行
python app/main.py
```

### 构建发布包

```bash
# Cython 混淆编译
python app/scripts/obfuscate.py

# PyInstaller 打包（使用混淆后的代码）
python app/scripts/build.py --dist
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
- 建议在虚拟环境内运行和打包
