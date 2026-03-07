# 订单物流信息更新工具

## 项目说明

这是一个用于更新微信小商店订单物流信息的桌面应用程序。

## 功能特性

- 通过订单号查询订单详情
- 更新订单的物流单号
- 图形化界面操作，简单易用

## 环境要求

- Python 3.6+
- tkinter（Python 自带）

## 依赖安装

```bash
pip install -r requirements.txt
```

## 配置文件

运行前需要准备两个配置文件：

1. `cookie.txt` - 存放微信小商店的 Cookie 信息
2. `biz_magic.txt` - 存放 biz_magic 认证信息

## 运行方式

```bash
python main.py
```

## 使用说明

1. 启动程序后，在界面中输入订单号
2. 输入新的物流单号
3. 点击"更新物流信息"按钮
4. 等待系统提示操作结果

## 注意事项

- 确保 cookie.txt 和 biz_magic.txt 文件存在且内容有效
- Cookie 信息需要定期更新
- 程序会验证授权有效期

## 打包为可执行文件

### Windows (exe)

请在 Windows 环境下执行：

```bat
build_windows.bat
```

Windows 打包使用 `cx_Freeze`，输出目录为 `dist/订单物流信息更新/`，主程序为 `dist/订单物流信息更新/订单物流信息更新.exe`。

说明：
- Windows 打包必须在 Windows 系统上执行
- 脚本会自动清理 `build/`、`订单物流信息更新.spec`、旧的 `dist/订单物流信息更新.exe` 和 `dist/订单物流信息更新`
- 如果项目根目录存在 `cookie.txt` 和 `biz_magic.txt`，脚本会自动复制到 exe 同目录
- `cx_Freeze` 默认生成目录分发包，而不是单文件 exe

### macOS (.app)

在项目目录下执行：

```bash
pip install pyinstaller
chmod +x build_mac.sh
./build_mac.sh
```

或直接使用 PyInstaller：

```bash
pip install pyinstaller
pyinstaller --onefile --windowed --name "订单物流信息更新" main.py
```

应用输出在 `dist/订单物流信息更新.app`。使用前将 `cookie.txt` 和 `biz_magic.txt` 放入 .app 包内：右键「订单物流信息更新.app」→ 显示包内容，把两个文件放在包根目录（与 `Contents` 同级）。
