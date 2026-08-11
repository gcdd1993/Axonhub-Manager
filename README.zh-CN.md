# AxonHub Manager

[English](README.md) | [简体中文](README.zh-CN.md)

![Version](https://img.shields.io/badge/version-0.0.1-blue)
![Platform](https://img.shields.io/badge/platform-Windows-0078D4)
![Tauri](https://img.shields.io/badge/Tauri-2-24C8DB)
![Rust](https://img.shields.io/badge/Rust-2021-000000)
![React](https://img.shields.io/badge/React-19-61DAFB)

AxonHub Manager 是面向 Windows 的 [AxonHub](https://github.com/looplj/axonhub) 桌面生命周期管理器。它负责安装和校验 AxonHub Release、托管 AxonHub 进程，并提供配置、健康状态、日志、升级和回滚界面。

渠道、模型、API Key 和请求追踪等业务管理仍由 AxonHub 自带的 Web 管理后台负责。

## 主要功能

- 查询 `looplj/axonhub` 最新 Release，并自动选择 Windows x64 资产。
- 上游 Release 提供 SHA-256 digest 时，对下载内容进行校验。
- 保留上一个已安装版本，支持一键回滚。
- Manager 启动时自动启动已安装的 AxonHub。
- Manager 退出前停止并等待 AxonHub 退出，避免遗留托管进程。
- 支持系统托盘后台运行；窗口关闭按钮只隐藏窗口，不会退出程序。
- 监控 AxonHub 进程和 `/health` 健康检查接口。
- 编辑 `config.yml`，执行 YAML 检查和 `axonhub config validate`。
- 配置保存、升级或回滚后自动恢复托管进程。
- 在同一日志界面查看 Manager 事件和 AxonHub 标准输出。
- 支持深色/浅色主题、键盘操作和减少动态效果等可访问性能力。

## 安装

从 [GitHub Releases](https://github.com/gcdd1993/Axonhub-Manager/releases) 下载最新的 Windows x64 安装程序。

当前安装包尚未进行代码签名，因此 Windows SmartScreen 可能显示警告。运行前请确认安装包来自本仓库。

## 运行方式

AxonHub 的进程生命周期完全由 Manager 托管：

1. 已安装 AxonHub 时，启动 Manager 会自动启动 AxonHub。
2. 点击窗口右上角关闭按钮只会隐藏到系统托盘，AxonHub 继续运行。
3. 左键单击托盘图标可恢复窗口。
4. 从托盘菜单选择 **退出** 后，Manager 会先停止并等待 AxonHub 退出，再关闭自身。

应用托管的数据保存在操作系统分配的应用数据目录中：

```text
config.yml
axonhub.db
current.json
previous.json
versions/
logs/axonhub.log
```

## 环境要求

- Windows 10 或更高版本，x64
- Microsoft Edge WebView2 Runtime
- 安装或检查 AxonHub Release 时能够访问 GitHub

## 开发

安装依赖并启动 Tauri 应用：

```powershell
npm ci
npm run tauri -- dev
```

执行与 CI 一致的检查：

```powershell
npm run build
cargo fmt --manifest-path src-tauri/Cargo.toml -- --check
cargo check --manifest-path src-tauri/Cargo.toml
cargo test --manifest-path src-tauri/Cargo.toml --lib
```

构建 Windows NSIS 安装程序：

```powershell
npm run tauri -- build --bundles nsis
```

安装包输出到 `src-tauri/target/release/bundle/nsis/`。

## 发布流程

Pull Request 和推送到 `master` 会执行前端与 Rust 校验。推送匹配 `v*` 的标签后，流水线会构建 Windows x64 NSIS 安装包并发布到 GitHub Releases。手动运行流水线会创建草稿 Release。

发布标签前：

1. 将 `VERSION`、`package.json`、`package-lock.json`、`src-tauri/Cargo.toml`、`src-tauri/Cargo.lock` 和 `src-tauri/tauri.conf.json` 更新为相同版本。
2. 将 `CHANGELOG.md` 中 **Unreleased** 的内容整理到带日期的版本章节。
3. 提交修改并推送匹配的标签，例如 `v0.0.1`。

流水线会拒绝版本文件不一致或与版本不一致的标签。

## 技术栈

- Tauri 2
- Rust 2021
- React 19 与 TypeScript
- Vite 6

## 上游与项目边界

- [AxonHub](https://github.com/looplj/axonhub) 是本程序管理的上游服务。
- AxonHub Manager 是独立桌面辅助工具，并非 AxonHub 官方发行版。
- 产品功能边界参考了 [CPA-Manager-native](https://github.com/gcdd1993/CPA-Manager-native)。

## 许可证

当前仓库尚未包含许可证文件；添加许可证前，各部分权利归其各自所有者。
