# DLC 集成

DLC 所需的宿主代码随 LingChat 一起维护：Rust 接口和运行时分别位于 `crates/ling-chat-dlc-contract/`、`crates/ling-chat-dlc-runtime/`，前端组件位于 `packages/ling-chat-dlc-ui/`。构建主程序不依赖个人仓库的 SDK 或发布包。

## 内容来源

DLC 内容包与宿主源码分开分发。新安装默认不预设内容仓库；用户可在「设置 → 插件 / DLC → DLC」填写 GitHub 仓库的 `owner/repository` 或 `https://github.com/owner/repository`，再同步目录，也可直接导入本地压缩包。

同步会读取所选仓库的 GitHub Releases，按 `<id>@<version>.manifest.toml` 和 `<id>-<version>.zip` 配对发现内容。只有非草稿、非预发布且带有效 SHA-256 的资源会被接受；下载地址必须属于所选仓库。下载、校验失败时保留之前的目录缓存和已安装内容。

升级时，已有缓存中的 GitHub Releases 地址和旧版目录地址会转换为 `owner/repository`，并在使用前重新校验。新安装没有默认来源，也不会自动访问个人内容仓库。

## 安装和隔离

内容包复用现有插件清单、资源管理与启停流程。安装时会校验包的大小、SHA-256、清单身份、路径和资源类型；游戏 WebView 与宿主 IPC 隔离。更新或卸载 DLC 不会删除游戏存档。

SHA-256 用于校验下载内容与 Release 提供的摘要一致，不代表内容由官方认证；用户应自行确认所选仓库和内容包的可信度。
