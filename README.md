# TLS-shipinhao工具

## 项目说明

这是一个用于更新微信小商店订单物流信息的桌面应用程序。

## 目录结构

```text
TLS-shipinhao/
├── main.py               # 根入口，仅负责启动 src.app.main
├── src/
│   ├── __init__.py
│   ├── app.py            # 程序入口（main 函数）
│   ├── constants.py      # 全局常量（窗口尺寸、颜色、URL、配置名等）
│   ├── config.py         # 配置文件管理（cookie/biz_magic 读取、目录解析）
│   ├── api.py            # 微信小商店 API 交互（订单查询、物流更新）
│   ├── worker.py         # BatchWorker 后台批量任务执行器
│   ├── widgets.py        # 自定义控件（BatchInputEdit、LicenseDialog、字体工具）
│   ├── window.py         # MainWindow 主窗口（UI 构建、事件处理、授权管理）
│   └── license.py        # 离线授权管理（卡密激活、设备绑定）
├── scripts/
│   └── build.py          # 统一构建入口
├── build_mac.sh          # macOS 打包脚本
├── build_windows.bat     # Windows 打包脚本
├── requirements.txt
├── README.md
├── dist/                 # 打包产物
└── logs/                 # 运行日志
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
pip install -r requirements.txt
```

## 配置文件

运行前需要准备两个配置文件：

1. `cookie.txt` - 存放微信小商店的 Cookie 信息
2. `biz_magic.txt` - 存放 biz_magic 认证信息

开发模式默认从项目根目录读取这两个文件；打包后默认从 `.app` 或 `exe` 同级目录读取。

## 运行方式

```bash
python main.py
```

## 使用说明

1. 启动程序后，在左侧输入订单号
2. 在右侧输入新的物流单号
3. 点击“点击开始批量处理”
4. 在下方查看执行日志和结果提示

## 注意事项

- 确保 `cookie.txt` 和 `biz_magic.txt` 文件存在且内容有效
- Cookie 信息需要定期更新
- 建议在虚拟环境内运行和打包

## 打包为可执行文件

### 统一入口

```bash
python scripts/build.py
```

该脚本会根据当前系统自动调用：
- macOS: `build_mac.sh`
- Windows: `build_windows.bat`

### Windows (exe)

请在 Windows 环境下执行：

```bat
build_windows.bat
```

输出目录为 `dist/TLS-shipinhao/`，主程序为 `dist/TLS-shipinhao/TLS-shipinhao.exe`。

### macOS (.app)

在项目目录下执行：

```bash
chmod +x build_mac.sh
./build_mac.sh
```

应用输出在 `dist/TLS-shipinhao.app`。使用前将 `cookie.txt` 和 `biz_magic.txt` 放在 `dist/` 目录，与 `.app` 同级即可。
