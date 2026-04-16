# TLS-shipinhao

微信小商店中差评订单查找与物流更新工具，包含桌面客户端和卡密授权后端。

## 目录结构

```text
TLS-shipinhao/
├── app/                          # 兼容期 Python 启动壳（已不含 PySide6 UI）
├── crates/                       # Rust 核心 crates（domain / services / security / app）
│   ├── main.py                   # 入口，调用 bootstrap.main()
│   ├── bootstrap.py              # 程序启动入口（QApplication 初始化）
│   ├── settings.py               # 全局配置与常量（URL、窗口尺寸、抓取参数、匹配权重等）
│   ├── assets/                   # 静态资源
│   │   └── favicon.png
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

```

## 功能特性

- 桌面客户端主实现：Rust + Slint
- 授权服务主实现：Rust + Cloudflare Worker 兼容层
- 构建/完整性清单：Rust xtask + build-tools


## 环境要求

- Rust stable
- Node.js 18+（仅 wrangler/npx 本地部署辅助）

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
