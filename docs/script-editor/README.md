# 剧本编辑器（Script Editor）

> 本文档面向维护者与贡献者，描述 **PR #540（`feat: 新增剧本编辑器`，由 `FlameTN7/pr2/script-editor` 合入）** 的实现方式、剧本编辑器逻辑、预览（试玩）逻辑与剧本自动错误检测机制。
>
> 合入 commit：`50863976`（Merge pull request #540）。分支当前为 `tauri-refactor`。

## 背景：为什么需要它

在 PR #540 之前，剧本从前端视角**完全只读**：

- `api/script.rs` 只有 5 个只读命令，没有写入 / 校验 / 重扫的能力；
- Tauri `fs` 插件的 scope 覆盖不到 `<data_dir>/game_data/scripts`；
- 引擎只启动时扫一次剧本目录，作者存完剧本必须**重启整个应用**才能试玩；
- 引擎里大量失败是**静默的**（素材缺失、条件写错、字段拼错），作者只能对着不动的画面猜；
- 存在一个原型编辑器，但它与引擎的 schema 脱节（`set_variable` / `chapter_end` 跑不通），写盘是「打开即截断」的非原子写。

PR #540 的目标：

1. 给剧本一个完整、schema 驱动的可视化编辑器（新建 / 编辑 / 素材 / 角色 / 校验）；
2. 在编辑器内**内嵌试玩**，且与自由对话**严格隔离**；
3. 把引擎的静默失败变成作者看得见的**三级诊断**（error / warn / info）。

## 技术方案一句话

**前端只见 JSON，YAML 语义只存在于 Rust 一侧；所有写入原子化并留 `.bak`；任何来自前端的路径都必须过 `paths` 模块的校验。**

- 后端：`src-tauri/src/api/script_editor/`（Rust），分层 `paths / io / schema / validate / commands`；
- 前端：`src/components/script-editor/*` + `src/stores/modules/script-editor/*`（Vue 3 + Pinia），路由 `/script-editor` 懒加载；
- 试玩：复用真引擎执行路径（`init_script → run_script → on_script_end`），三层隔离（后端会话快照、代号守卫、前端双快照）。

## 文档索引

| 文档 | 内容 |
|---|---|
| [architecture.md](architecture.md) | **实现方式**：模块分层、设计约束、命令清单、三种磁盘布局、schema 单一真相源、原子写与路径安全 |
| [editor.md](editor.md) | **编辑器逻辑**：schema 驱动表单、编辑操作与撤销栈、防抖自动保存、事件折叠、章节流程图、素材 / 角色管理 |
| [preview.md](preview.md) | **预览（试玩）逻辑**：内嵌真渲染层、后端会话快照 / 还原、`preview_generation` 孤儿写入守卫、前端迟到事件丢弃、路由守卫清理 |
| [validation.md](validation.md) | **自动错误检测机制**：校验流水线、三级诊断、事件级 / 章节图 / 变量分析、稳定诊断码 |

## 图表（SVG + HTML）

每张图为独立 HTML 文件，浏览器直接打开即可查看（内联 SVG，无需外部依赖）。

| 图 | 说明 |
|---|---|
| [diagrams/architecture.html](diagrams/architecture.html) | 总体架构数据流图：前端三层 → Tauri IPC → 后端命令层 → 四个子模块 → 磁盘 / 引擎 |
| [diagrams/editor-flow.html](diagrams/editor-flow.html) | 编辑流程数据流图：编辑操作 → 撤销栈 → 防抖自动保存 → 原子写；同步触发校验 → 诊断回填 |
| [diagrams/event-schema.html](diagrams/event-schema.html) | 事件 Schema UML 类图：ScriptSchema / EventSpec / FieldSpec / FieldKind / ActionSpec |
| [diagrams/preview-isolation.html](diagrams/preview-isolation.html) | 试玩隔离时序图：启动快照 → 试玩运行 → 中止还原 → 迟到事件丢弃 → 路由守卫 |
| [diagrams/preview-class.html](diagrams/preview-class.html) | 试玩与会话隔离 UML 类图：PreviewSession / GameStatus / GeneratorDeps / ReplyResponse |
| [diagrams/validation-flow.html](diagrams/validation-flow.html) | 校验流程图：story_config → 逐章节 → 逐事件 → 章节图 / 变量 → 报告 |

## 关键代码位置

**后端（`src-tauri/`）**

| 文件 | 职责 |
|---|---|
| `src/api/script_editor/mod.rs` | 模块入口与分层说明 |
| `src/api/script_editor/schema.rs` | 16 种事件字段的单一真相源，导出给前端 |
| `src/api/script_editor/paths.rs` | 剧本 key ⇄ 磁盘路径、三种布局、穿越防护、名称合法性 |
| `src/api/script_editor/io.rs` | YAML ⇄ JSON、原子写、`.bak` 备份、章节文档归一 |
| `src/api/script_editor/validate.rs` | 校验器：把引擎静默失败变成诊断 |
| `src/api/script_editor/commands.rs` | Tauri 命令层（`editor_*` 前缀） |

**前端（`src/`）**

| 文件 | 职责 |
|---|---|
| `api/services/script-editor.ts` | 纯 invoke 封装 + 类型 |
| `stores/modules/script-editor/{state,getters,actions,index}.ts` | setup 风格 Pinia store |
| `components/views/ScriptEditor.vue` | 主视图（五个页签 / 试玩 / 弹窗 / 快捷键） |
| `components/script-editor/*.vue` | ChapterFlow / ChapterTimeline / EventPropertyPanel / FieldRow / CompositeField / EventRow / PreviewStage |
| `composables/useEventFolding.ts` | 转场与 AI 互动轮次的折叠逻辑 |
| `api/tauri-events.ts` | `isStalePreviewReply` 迟到事件丢弃 |

## 试玩与会话隔离要点（速览）

1. **后端**：`editor_start_preview` 先 `rescan`，再用 `PreviewSession::begin` 快照共享 `GameStatus`（台词表长度、场景快照、三个未覆盖字段、玩家名/副标题），并按「刚进游戏」搭场子；结束后 `restore` 整体还原。
2. **代号守卫**：`GameStatus.preview_generation` 每次进出试玩递增；生成管线捕获代号，`add_assistant_line` 比对不一致即丢弃整条写入 —— 拦下被中止任务的游离写入。
3. **前端**：`PreviewStage` 对 `gameStore` 与场景渲染态各存一份快照，进出各还一次；`eventQueue.clear()` 清空残留。
4. **迟到事件**：`ai:reply` 带 `preview_gen`，前端 `isStalePreviewReply` 只在「当前在试玩且代号一致」时放行。
5. **路由守卫**：`onBeforeRouteLeave` 阻塞导航，await 完成 stopPreview → clear 队列 → flush 保存 → syncEngine，保证 MainChat 挂载时后端已还原、队列干净。
