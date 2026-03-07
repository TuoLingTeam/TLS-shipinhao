# TLS-shipinhao工具

## 项目说明

这是一个用于更新微信小商店订单物流信息的桌面应用程序。

## 目录结构

```text
TLS-shipinhao/
├── main.py               # 根入口，仅负责启动 src.app.main
├── src/
│   ├── __init__.py
│   └── app.py            # 主要界面与业务逻辑
├── scripts/
│   └── build.py          # 统一构建入口
├── build_mac.sh          # macOS 打包脚本
├── build_windows.bat     # Windows 打包脚本
├── requirements.txt
├── README.md
├── dist/                 # 打包产物
└── logs/                 # 运行日志
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
