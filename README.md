# TLS-shipinhao工具

## 项目说明

这是一个用于更新微信小商店订单物流信息的桌面应用程序。

## 目录结构

```text
TLS-shipinhao/
├── app/                      # 客户端应用
│   ├── main.py               # 入口，启动 src.app.main
│   ├── requirements.txt      # Python 依赖
│   ├── src/
│   │   ├── __init__.py
│   │   ├── app.py            # 程序入口（main 函数）
│   │   ├── constants.py      # 全局常量（窗口尺寸、颜色、URL、配置名等）
│   │   ├── config.py         # 配置文件管理（cookie/biz_magic 读取、目录解析）
│   │   ├── api.py            # 微信小商店 API 交互（订单查询、物流更新）
│   │   ├── worker.py         # BatchWorker 后台批量任务执行器
│   │   ├── widgets.py        # 自定义控件（BatchInputEdit、LicenseDialog、字体工具）
│   │   ├── window.py         # MainWindow 主窗口（UI 构建、事件处理、授权管理）
│   │   └── license.py        # 在线授权管理（卡密激活、设备绑定）
│   └── scripts/
│       └── build.py          # 统一构建入口
├── backend/                  # Cloudflare Workers 后端（卡密验证 + 管理页面）
├── .github/workflows/        # CI/CD 构建流水线
├── dist/                     # 打包产物
└── README.md
```

### 模块依赖关系

```text
constants ← config ← api ← worker
                                ↘
widgets + worker ← window ← app（入口）
```

## 功能特性

- 通过订单号查询订单详情
- 更新订单的物流单号
- 图形化界面操作，简单易用

## 环境要求

- Python 3.10+
- PySide6

## 依赖安装

```bash
pip install -r app/requirements.txt
```

## 注意事项

- 确保 `cookie.txt` 文件存在且内容有效（包含 biz_magic 值）
- Cookie 信息需要定期更新
- 建议在虚拟环境内运行和打包


执行混淆
python3 app/scripts/obfuscate.py

用混淆文件目录构建包
python3 app/scripts/build.py --dist

部署到 Cloudflare
cd /Users/zxr/Downloads/source-code/TLS-shipinhao/backend && npx wrangler deploy 2>&1
