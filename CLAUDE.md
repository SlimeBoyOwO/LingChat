## 项目概览
LingChat 是一个 AI Galgame 引擎，一个桌面 AI 聊天伴侣 / 桌宠应用，基于Tauri 2。核心功能：LLM 驱动的聊天，内置情绪分类器、TTS 语音、屏幕感知、基于剧本的多角色故事、AI 宠物/桌面伴侣模式、Python 插件系统、局域网同步，以及存档/成就。
代码、注释、提交信息和文档主要使用**中文**编写。编写注释时，请与周围语言保持一致，仓库为SlimeBoyOwO/LingChat
## 常用命令
包管理器是pnpm
-pnpm tauri dev：以开发模式运行完整桌面应用
-pnpm dev：仅启动 Vite 开发服务器，针对已在运行的 Rust 后端进行纯前端迭代
-pnpm build：前端类型检查和生产构建（必须使用此命令检查前端，不得加tail/head等）
-pnpm tauri build：完整桌面打包，生成 NSIS / dmg / deb / AppImage + 更新器产物
-pnpm format/pnpm format:check：prettier和cargo fmt
-pnpm check:rs：cargo check --workspace
-测试 — `cargo test --workspace`，没有前端测试框架
-pnpm init生成应用图标，准备桌面资源，下载情绪 ONNX 模型。
-Android：pnpm android:prepare、pnpm android:dev、pnpm android:build（aarch64，apk）、pnpm android:check（cargo ndk check）
-iOS：pnpm ios:init，pnpm ios:build（`docs/ios-build.md`；`.npmrc` 记录了 Xcode 下 pnpm-11 的限制）。
## 高层架构
### 前端和后端 IPC 通信
前端通过 `@tauri-apps/api/core` 的 `invoke()` 调用 Rust 命令。`src/api/services/*` 中每个域都有一个服务模块，但许多组件直接 `invoke`。**所有**命令都在一个地方注册：`src-tauri/src/lib.rs` 中的 `invoke_handler!` 宏
**自定义应用命令不受 ACL 门控。** 项目已移除应用命令的 ACL 门控。将 `#[tauri::command]` 添加到 `lib.rs` 的 `generate_handler!` 就足够了。`src-tauri/capabilities/*.json` 只门控核心/插件命令（updater、fs、dialog、screenshots 等）。编辑 capabilities 时，`src-tauri/gen/schemas/` 下生成的 schema 会通过 `build.rs` 重新生成；如果更改未生效，可用 `touch src-tauri/build.rs` 强制触发
### 对话事件管线（核心运行时）
需要理解的最重要流程：
1. **Rust**：`src-tauri/src/ai_service/message_system/generator.rs` 将对话作为 Tauri 事件流式发送——`ai:reply`、`ai:thinking` 等。载荷是 `ScriptEventType` 对象
2. **前端**：`src/api/tauri-events.ts` 监听并将它们送入 `EventQueue`（`src/core/events/event-queue.ts`）
3. 队列通过 `src/core/events/processors/*.ts` 中按类型划分的处理器逐个处理事件（dialogue、narration、background、music、sound、thinking 等），这些处理器由 `src/core/events/index.ts` 通过 `import.meta.glob` 自动注册
4. 推进语义来自 `duration`：`-1` = 等待用户点击继续，`0` = 立即继续，`>0` = 等待 N 秒。`isFinal` 标记回合结束。`dialogue-merge.ts` 实现同一角色短连续回复的内联合并
中心游戏状态是 Pinia 的 `game` store（`src/stores/modules/game/`），尤其是 `currentStatus`（`input` / `responding`）。`script-editor` store 有自己的预览事件流，并会丢弃过期回复（`tauri-events.ts` 中的 `isStalePreviewReply`）
### 后端布局（`src-tauri/src/`）
-lib.rs：应用引导 + 庞大的 `invoke_handler!`。`AppState` 将 `InnerAppState` 包装在 `OnceLock` 中：先 `manage()` 一个空壳（Android 在 `setup` 完成前就创建 webview，因此命令可能在初始化完成前触发），然后用真实状态 `fill()`。桌面端在初始化前访问会 panic；Android 则自旋等待
-api：每个命令域一个文件（character、chat、game、save、scene、settings、script、script_editor、plugins、pet、asr 等）。`api::data_dir()` 解析数据目录
-init：启动序列（`initialize()`）：播种数据目录 → 应用 LAN 同步暂存（必须在数据库初始化之前）→ 打开数据库 → 从文件夹同步角色 → 迁移 LLM 配置 → 构建 LLM 槽位 → 构造 `AIService`、情绪分类器等
-ai_service：整个 AI 栈：
  -llm：基于 genai 的客户端。`LlmSlot` 是可热插拔的 `RwLock`。提供商预设位于前端 `src/constants/llm-presets.ts`；见 `docs/llm-provider-presets.md`——添加新的 `provider` 类型还需要在 `ai_service/llm/provider_config.rs` 中添加分支。
  -message_system：聊天处理 + `ai:reply` 流式发送
  -game_system：剧本引擎 + 事件、自动存档、角色管理器、持久记忆
  -asr：流式 ASR（VAD 分段、可插拔提供商、WebSocket）
  -tts：本地进程内 TTS（SBV2 / onnxruntime、DeBerta、设备选择）+ 云端 CosyVoice
  -emotion：ONNX 情绪分类器（19-emo 模型位于 `data/third_party/emotion_model_19emo/`）。
  -screen_analyzer.rs`、`proactive_system/`、`god_agent/`、`skill_agent/`、`translator.rs`、`tools/`（ToolRegistry + 内置工具定义）
-db：sea-orm + SQLite；`entities/` + `managers/`（仓库）。迁移位于 `migration/`
-plugins：— 插件管理器（见 `docs/plugin-dev-guide.md`）
-lan_sync：axum HTTP+WebSocket 服务器 + mDNS 对等发现，manifest 差异推送/拉取
-cast：投屏（screen-cast）：第二个窗口，镜像主窗口的对话（`cast:mirror`）
- `resource_sync/` + `manifest/` — 安装器种子 / 数据版本同步。
- `achievements/`、`adventures/` — 成就触发器 + 按角色的冒险 / 羁绊系统。

### 前端布局（`src/`）
-components按功能划分的视图：game、pet、settings、script-editor、schedule、pomodoro、effects，以及根视图（`MainMenu`、`CompanionMode`、`PetMode`、`CastWindow`、`LogWindow`、`ScriptEditor`、`WorkshopPage`）。
-stores/modules：Pinia stores（game、settings、ui、user、agent、script-editor、adventure、asr），带自定义持久化插件（`stores/plugins/persist`）。
-core/events：事件队列 + 处理器
-api：服务模块+tauri-events.ts；遗留的 axios `http.ts` 层（基本未使用）。
-locales：用于 `en`、`ja`、`zh-CN`、`zh-HK` 的 vue-i18n（+ `schema-i18n.ts`）。
-路由（src/router/index.ts）：/MainMenu、`/chat`、`/pet`、`/second`、`/credit`、`/log-window`、`/cast`、`/script-editor`、`/workshop`。
### 数据目录模型
`data/` 是运行时数据目录（`api::data_dir()` / `init/static_copy.rs`）。在桌面 **dev** 模式下是仓库根目录的 `data/`（可实时编辑的游戏数据）；发布版中是与 exe 相邻的 `data/`；移动端则从打包的 `data.7z` 解压。
-`game_data/` — characters/、scripts/、backgrounds/、musics/、ambients/、schedules.json。
-`data_manifest.json` — 数据版本 + SHA-256 文件列表（`manifest/` 模块）。
-`.official/` — 桌面安装包将资源捆绑在此处；首次启动会将其播种到 `data/`，然后删除 `.official/`；应用更新会重新创建它，用户通过 `ResourceSyncDialog`（`resource_sync/`）同步。`third_party/`（ONNX 模型）直接随包发布，并在更新时覆盖。
-`plugins/` — 用户/导入的插件。
-存档、`settings.json`（tauri-plugin-store）、日志（`utils/file_logger`）。
## 关键注意事项
-pnpm tauri dev会先运行格式化
-`src-tauri/build.rs` 和 `.cargo/config.toml` 包含平台链接变通方案，不要移除它们
-`[lib] crate-type` 为 `["cdylib", "rlib"]`：省略 `staticlib`，因为否则桌面构建会在每次增量构建时重新归档约 1.4 GB 的 `ling_chat_lib.lib`。**手动 iOS 构建必须把 `"staticlib"` 加回来**——见 `docs/ios-build.md`
-移动端构建需要 `custom-protocol` Cargo feature（在 `[features]` 下声明）
## 参考文档
各功能的权威文档位于 `docs/`：插件（`plugin-dev-guide.md`）、脚本编辑器（`script-editor/`）、Live2D 创作（`live2d/`）、函数调用工具（`function_call/`）、LLM 预设（`llm-provider-presets.md`）、i18n（`i18n.md`）、本地 TTS API（`local-tts-api.md`）、推理设备（`inference-devices.md`）、Android（`android/`）、iOS（`ios-build.md`）、更新逻辑（`自动更新逻辑.md`）、Rust 构建耗时优化（`build-performance.md`）
## 关键约定
-与用户交流的时候，以专业的软件工程师的口吻交流，避免频繁提及函数名与内部实现细节，注重交流整体软件结构和功能。发言不要 AI 化严重。重点是让用户理解软件目前架构和情况，方便开发者定位问题。
-进行代码更改的时候，保证最小化破坏更改，代码上仅保留必要的注释，注释中禁止出现md语法。
-遵循 MVP 原则，不要过度设计，不要过度实现，不要过度优化，不要过度封装，开发中能先用简洁的方式实现就不要用过于复杂的方法，随着开发和需求动态调整代码结构设计。
-Rust 格式化字符串优先内联捕获变量：tracing等宏里用 `{role_id}` 直接捕获同名变量，优先于位置参数写法（可读性更高）。复杂表达式（字段访问 / 方法调用）不能内联，先绑定局部变量或继续用 `{}` 位置参数。新写的代码一律用这种风格；顺手改旧代码时也可以替换，但不要为此单独发起纯风格重构。
-代码注释、commit message、变量命名倾向中文。commit 用cc前缀 + 中文：`feat：xxx`、`fix：xxx`、`chore: xxx`、`build：xxx`、`refactor: xxx`（注意前缀中文冒号用全角）。**commit 用单行标题**，不加正文、**不要加 `Co-Authored-By:` 尾注**。避免在无关改动里顺手重构，仓库维护者明确不欢迎无上下文的风格性重构。
-格式化：用户明确不要求，格式检查不作为验收依据，格式化在提交时由 husky + lint-staged 处理
-换行/编码：`.gitattributes` 强制文本文件 LF + UTF-8（Windows 脚本 CRLF）。媒体/大文件走 Git LFS，不要提交大的二进制
-样式优先用 Tailwind CSS v4，而不是传统css：项目用 `@tailwindcss/vite` 4.x，CSS-first 配置，v4 语法与 v3 有差异，不确定时可查网络或 context7 文档，官方库 ID：`/websites/tailwindcss`（无需再 resolve-library-id）
-provider 预设：LLM 服务商快捷配置在 `src/constants/llm-presets.ts`，改这个数组即可，勿动组件。
