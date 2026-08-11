# AxonHub Manager

一个使用 Tauri 2、Rust、React 和 TypeScript 编写的 AxonHub 本地桌面管理器。

它负责 AxonHub 二进制的安装、SHA-256 校验、升级、回滚、进程控制、健康检查、配置编辑与日志查看；渠道、模型、API Key、请求追踪等业务管理继续使用 AxonHub 自带的 Web 管理后台。

## 功能

- 查询 `looplj/axonhub` 最新 GitHub Release，并自动匹配当前平台资产
- 下载后验证 GitHub Release 提供的 SHA-256 digest
- 保留上一安装版本，支持停止状态下一键回滚
- 启动、停止并监控 AxonHub，本地健康检查地址为 `/health`
- 编辑 `config.yml`，执行 YAML 基础检查与 `axonhub config validate`
- 聚合管理器事件和 AxonHub 标准输出日志
- 深色/浅色主题、键盘焦点、减少动态效果等可访问性支持

默认数据目录由 Tauri 的应用数据目录决定，其中包含：

```text
config.yml
axonhub.db
current.json
previous.json
versions/
logs/axonhub.log
```

## 开发

```bash
npm install
npm run tauri -- dev
```

只构建前端：

```bash
npm run build
```

检查 Rust：

```bash
cargo fmt --manifest-path src-tauri/Cargo.toml -- --check
cargo check --manifest-path src-tauri/Cargo.toml
```

## 设计原则

界面采用高信息密度的开发者工具布局，以状态、版本、安全边界和恢复路径为首要层级。设计规范保存在 `design-system/axonhub-manager/MASTER.md`。

## 上游项目

- [AxonHub](https://github.com/looplj/axonhub)
- 功能边界参考 [CPA-Manager-native](https://github.com/gcdd1993/CPA-Manager-native)

