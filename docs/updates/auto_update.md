kUpdatesOnStartup()

// 等待 pywebview API 准备就绪
window.addEventListener('pywebviewready', () => {
window.addEventListener('keydown', handleKeyDown)

data_manifest.json + data_files.zip（默认游戏资源）

# 2. 为 Windows / macOS / Linux 构建 Tauri 应用

# 3. 创建 GitHub Release 草稿，上传所有产物

#

# 需要的 GitHub Secrets:

es. Let me fix the Rust check path.

Thought for 3s, ran 3 shell commands

Both Rust and TypeScript compile clean. Let me reset the working directory and summarize.

Thought for 1s, ran 1 shell command

All implementation is complete and verified. Here's a summary of everything that was done:
All implementation is complete and verified. Here's a summary of everything that was done:

---

完成总结

修改的文件 (9个)

┌─────────────────────────────────────┬────────────────────────────────────────────────────────────────────────────┐
│ 文件 │ 改动 │
├─────────────────────────────────────┼────────────────────────────────────────────────────────────────────────────┤
│ .gitignore │ 添加 data_manifest.json 到忽略列表 │
├─────────────────────────────────────┼────────────────────────────────────────────────────────────────────────────┤
│ package.json │ 添加 @tauri-apps/plugin-updater, @tauri-apps/plugin-process │
├─────────────────────────────────────┼───────────────────────────────────────────────┤
├─────────────────────────────────────┼────────────────────────────────────────────────────────────────────────────┤
│ package.json │ 添加 @tauri-apps/plugin-updater, @tauri-apps/plugin-process │
├─────────────────────────────────────┼───────────────────────────────────────────────┤
│ src-tauri/Cargo.toml │ 添加 tauri-plugin-updater, tauri-plugin-process, zip, sha2, tempfile │
├─────────────────────────────────────┼────────────────────────────────────────────────────────────────────────────┤
│ src-tauri/tauri.conf.json │ 添加 plugins.updater 配置 (GitHub Releases endpoint + pubkey │
│ │ 占位符)；bundle.createUpdaterArtifacts: true │
├─────────────────────────────────────┼────────────────────────────────────────────────────────────────────────────┤
│ src-tauri/capabilities/default.json │ 添加ow-restart 权限 │
├─────────────────────────────────────┼────────────────────────────────────────────────────────────────────────────┤
│ src-tauri/src/lib.rs │ 注册 tauri_plugin_updater + tauri_plugin_process；注册 │
│ │ Dataupdate / apply_data_update 命令 │
├─────────────────────────────────────┼────────────────────────────────────────────────────────────────────────────┤
│ src/App.vue │ 集成检查更新 │
└─────────────────────────────────────┴────────────────────────────────────────────────────────────────────────────┘

新增的文件 (5个)

┌───────────────────────────────────────┬────────────────────────────────────────────┐
│ 文件 │ 用途 │
├───────────────────────────────────────┼─────────────────────────────────────────────────────────────────────────┤
│ scripts/generate-data-manifest.js │ 扫manifest.json（含 SHA-256 校验和） │

Let me verify the frontend compiles, then create the GitHub Actions workflow.

Thought for 15s, ran 2 shell commands

Frontend compiles clean. Now let me create the GitHub Actions release workflow.

Thought for 5s, ran 2 shell commands

● Write(D:\NoiProjects\GitHub\LingChat_Re\LingChat\.github\workflows\release.yml)
Wrote 213 lines to ..\.github\workflows\release.yml

# LingChat 发布工作流

#

# 当推送版本标签 (v\*) 时:

# 1. 生成 data_manifest.json + data_files.zip（默认游戏资源）

# 2. 为 Windows / macOS / Linux 构建

# 3. 创建 GitHub Release 草稿，上传所有产物

#

# 需要的 GitHub Secrets:

# - TAURI_SIGNING_PRIVATE_KEY (tauri signer generate 生成的私钥)

# - TAURI_SIGNING_PRIVATE_KEY_PASSWORD (私钥密码)

… +204 lines

Thought for 1s

Thought for 2s, read 1 file

All tasks complete. Let me do a final verifiend.

Thought for 5s, read 1 file, ran 1 shell command

Let me run final verification — both Rust and frontend type checks.

Thought for 2s, ran 3 shell commands

TypeScript passes. Let me fix the Rust check path.

Thought for 3s, ran 3 shell commands

Both Rust and TypeScript compile clean. Let y and summarize.

Thought for 1s, ran 1 shell command

All implementation is complete and verified. Here's a summary of everything that was done:

┌─────────────────────────────────────┬────────────────────────────────────────────────────────────────────────────┐
│ 文件 │ 改动 │
├─────────────────────────────────────┼────────────────────────────────────────────────────────────────────────────┤
│ .gitignore │ 添加 data_manifest.json 到忽略列表 │
├─────────────────────────────────────┼────────────────────────────────────────────────────────────────────────────┤
│ package.json │ 添加 @tauri-apps/plugin-updater, @tauri-apps/plugin-process │
├─────────────────────────────────────┼────────────────────────────────────────────────────────────────────────────┤
│ src-tauri/Cargo.toml │ 添加 tauri-plugin-updater, tauri-plugin-process, zip, sha2, tempfile │
├─────────────────────────────────────┼────────────────────────────────────────────────────────────────────────────┤
│ src-tauri/tauri.conf.json │ 添加 plugins.updater 配置 (GitHub Releases endpoint + pubkey │
│ │ 占位符)；bundle.createUpdaterArtifacts: true │
├─────────────────────────────────────┼────────────────────────────────────────────────────────────────────────────┤
│ src-tauri/capabilities/default.json │ 添加 updater:default, process:allow-restart 权限 │
├─────────────────────────────────────┼────────────────────────────────────────────────────────────────────────────┤
│ src-tauri/src/lib.rs │ 注册\_plugin_process；注册 │
│ │ DataUpdateState；注册 check_data_update / apply_data_update 命令 │
├─────────────────────────────────────┼────────────────────────────────────────────────────────────────────────────┤
│ src/App.vue │ 集成 UpdateDialog 组件，启动3秒后检查更新 │
└─────────────────────────────────────┴───────────────────────────────────────────────┘

新增的文件 (5个)

┌───────────────────────────────────────┬─────────────────────────────────────────────────────────────────────────┐
│ 文件 │ 用途 │
├───────────────────────────────────────┼─────────────────────────────────────────────────────────────────────────┤
│ scripts/generate-data-manifest.js │ 扫描 data/game_data/ 生成 data_manifest.json（含 SHA-256 校验和） │
├───────────────────────────────────────┼─────────────────────────────────────────────────────────────────────────┤
│ src-tauri/src/data_update/mod.rs │ 数据更新模块入口 — Tauri commands: check_data_update, apply_data_update │
├───────────────────────────────────────┼─────────────────────────────────────────────────────────────────────────┤
│ src-tauri/src/data_update/manifest.rs │ 数据结构: DataManifest, DataUpdateInfo, DataUpdateResult │
├───────────────────────────────────────┼─────────────────────────────────────────────────────────────────────────┤
│ src-tauri/src/data_update/sync.rs │ 核心逻辑: 清单比较、zip下载、解压、原子文件合并 │
├───────────────────────────────────────┼─────────────────────────────────────────────────────────────────────────┤
│ src/composables/useUpdater.ts │ 前ta 更新检查与应用逻辑 │
├───────────────────────────────────────┼─────────────────────────────────────────────────────────────────────────┤
│ src/components/UpdateDialog.vue │ 更新对话框 UI — 进度条、变更摘要、一键更新 │
├───────────────────────────────────────┼─────────────────────────────────────────────────────────────────────────┤
│ .github/workflows/release.yml │ CI │
└───────────────────────────────────────┴─────────────────────────────────────────────────────────────────────────┘

还需要你手动做的事

1. 生成签名密钥: 在项目根运行:
   pnpm tauri signer generate -w ./src-tauri/
1. 输入密码保护私钥。然后把：

- 公钥 填入 src-tauri/tauri.conf.json 替换 PLACEHOLDER_RUN_tauri_signer_generate
- 私钥 设为 GitHub Secret TAURI_SIGNING_PRIVATE_KEY
- 密码 设为 GitHub Secret TAURI_SIGNING_PRIVATE_KEY_PASSWORD

2. 首次运行时生成基线清单: 运行 node scriptsdata-version 1 生成data_manifest.json，放置到 data/ 目录
3. 发布流程: 推送 v\* 标签 → GitHub Actions 审核后发布

node scripts/generate-data-manifest.js --data-version 1
