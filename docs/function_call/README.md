# AI 工具调用（Function Calling）

> 本文档面向维护者与贡献者，描述 **PR #523（`Part 0：llm调用底层架构大修改，全面适配工具循环调用`，由 `shadow01a` 提交，分支 `tools`）** 的实现方式。
>
> 核心内容：工具函数的注册与执行机制、工具循环（tool loop）调用、工具权限管理、与系统台词表（`line` 表）的联合、AI 记忆的存储 / 提取 / 构建。
>
> 合入方式：以 `b5590040` 并入 `tauri-refactor`，后续经 `302139fb`（合并 tauri-refactor（工具循环架构）并解决冲突）与思考链、TTS 等改动合流。PR 共 6 个 commit，1774 增 / 121 删 / 30 个文件。

## 背景：为什么需要它

PR #523 之前，LingChat 的工具调用能力「只有 God Agent 一条窄路」：

- **没有通用的工具注册表**：只有上帝 Agent 的 `select_next_speaker` 一个专用工具（走 `complete_with_tools` 非流式路径），普通聊天 / 剧本 / 主动对话完全不能调用工具；
- **没有权限控制**：谁在什么场景下能用哪些工具，无从谈起；
- **工具调用不可见**：工具请求与返回结果没有进入台词表，AI 记忆里也没有它们 —— 下一次生成时模型对「自己刚才查过什么」毫无印象。

PR #523 的目标：

1. 给聊天 / 剧本 / 主动对话一条**统一的工具循环**（`stream_with_tool_loop`），支持 LLM 连续多次调用工具、把每轮结果回填进上下文再继续生成；
2. 提供**应用级工具注册表**（`ToolRegistry`）与统一的 `Tool` trait，新工具只需实现两个方法即可接入；
3. 提供**「场景组 × 角色组」二维权限矩阵**（`tool_permissions.toml`），按调用来源 + 角色名决定下发给 LLM 的工具集；
4. 把工具调用的**过程与结果持久化到系统台词表**（`line` 表的 `tool_call` 列 + `tool` 属性），并让 `MemoryBuilder` 在构建 AI 记忆时把它们**原样还原**成 OpenAI 格式的 tool 消息 —— 记忆闭环。

> ⚠️ 本 PR 是「Part 0」：只交付底层架构（注册表 + 执行器 + 循环 + 权限 + 台词表联合）。**没有前端工具管理界面** —— 权限配置是纯 Rust + TOML 文件；工具列表也暂无 UI 开关。后续 PR 会在其上加管理界面与更多工具。

## 技术方案一句话

**「一次流式请求 = 多轮工具调用」：`stream_with_tool_loop` 把流式请求包装成最多 3 轮的工具闭环，每轮把 LLM 请求的工具执行后以 `tool` 消息回填，直到模型不再请求工具；执行前后由权限矩阵裁剪工具集，整个调用历史写入台词表并被记忆构建器还原。**

- 后端：`src-tauri/src/ai_service/tools/`（Rust），分层 `registry / executor / tool_loop / permissions`；
- LLM 层：`LlmChunk::ToolCalls` 新变体；`LlmProvider` trait 新增 `supports_streaming_tools` / `complete_stream_with_tools` / `complete_with_tools` 三组方法；
- 台词表：`line` 表新增 `tool_call` 列（迁移 `m20260727_add_line_tool_call`），`LineAttribute` 新增 `tool`；
- 记忆：`MemoryBuilder` 把带 `tool_call` 的 assistant 行与 `tool` 行还原为 `LlmMessage`，参与每个角色的 `role.memory`。

## 文档索引

| 文档 | 内容 |
|---|---|
| [architecture.md](architecture.md) | **实现思路**：tools 模块分层、核心类型、工具循环机制、LLM Provider 层的工具支持、接入点（`GeneratorSource` / `GeneratorDeps` / `run_pipeline`） |
| [extension.md](extension.md) | **如何扩展新的工具函数**：`Tool` trait、实现步骤、注册、权限配置、错误与参数规范、测试 |
| [permission.md](permission.md) | **工具权限管理**：「场景组 × 角色组」二维矩阵、`tool_permissions.toml`、默认配置、`allowed_tools` 计算逻辑 |
| [memory.md](memory.md) | **记忆的存储与提取、AI 记忆构建**：台词表联合、`MemoryBuilder` 还原、`role.memory` 生命周期、完整闭环 |

## 图表（SVG + HTML）

每张图为独立 HTML 文件，浏览器直接打开即可查看（内联 SVG，无需外部依赖）。

| 图 | 说明 |
|---|---|
| [diagrams/architecture.html](diagrams/architecture.html) | 总体架构数据流图：生成管线 → 工具循环 → Provider → 执行器 → 台词表 → 记忆 → 下次生成 |
| [diagrams/tool-loop.html](diagrams/tool-loop.html) | 工具循环时序图：`stream_with_tool_loop` 的多轮「流式请求 → 工具调用 → 执行 → 回填」 |
| [diagrams/permission.html](diagrams/permission.html) | 权限模型图：「场景组 × 角色组」矩阵、`tool_permissions.toml` 结构、`allowed_tools` 决策流程 |
| [diagrams/memory.html](diagrams/memory.html) | 记忆构建数据流图：台词表 → MemoryBuilder → role.memory → LLM 上下文 → 写回台词表 |
| [diagrams/extend.html](diagrams/extend.html) | 扩展新工具步骤流程图：实现 trait → 注册 → 配置权限 → 测试 |

## 关键代码位置

**后端（`src-tauri/src/ai_service/`）**

| 文件 | 职责 |
|---|---|
| `tools/mod.rs` | 模块入口；`built_in_registry()` 创建注册表 + 加载权限配置 + 初始化角色归属 |
| `tools/registry.rs` | `ToolRegistry`：注册 / 查找 / 按权限裁剪工具定义 |
| `tools/executor.rs` | `Tool` trait、`ToolContext`、`ToolExecutor`（权限校验 → 查找 → 解析 → 2s 超时 → 稳定错误编码） |
| `tools/tool_loop.rs` | `stream_with_tool_loop`：流式工具闭环（最多 3 轮） |
| `tools/permissions.rs` | `ToolPermissionConfig`：「场景组 × 角色组」权限矩阵、`tool_permissions.toml` 读写 |
| `tools/clock.rs` | 内置示例工具 `CurrentTimeTool`（`get_current_time`） |
| `llm/mod.rs` | `LlmChunk`（含 `ToolCalls`）、`LlmClient`（`complete_stream_with_tools` / `complete_with_tools`） |
| `llm/provider.rs` | `LlmProvider` trait：三组工具方法，默认 fallback 行为 |
| `llm/providers/genai_provider.rs` | genai（OpenAI 兼容）流式工具调用：`with_capture_tool_calls`、流结束时 yield `ToolCalls` |
| `llm/providers/kimi_code.rs` | Kimi Code 非流式 `complete_with_tools`（Anthropic `tool_use` 格式，供 God Agent 用） |
| `message_system/generator.rs` | `GeneratorSource` / `GeneratorDeps` / `run_pipeline`：调用工具循环、把 `tool_messages` 写入台词表 |
| `game_system/memory_builder.rs` | `MemoryBuilder`：把工具行还原成 OpenAI 格式 `LlmMessage`（含旧版兼容） |
| `game_system/role_manager.rs` | `sync_memories`：按台词表重建每个角色的 `role.memory` |

**数据 / 配置**

| 位置 | 说明 |
|---|---|
| `src-tauri/src/migration/m20260727_000002_add_line_tool_call.rs` | `line` 表加 `tool_call` TEXT 列 |
| `src-tauri/src/db/entities/line.rs` | `LineAttribute::Tool` 变体、`Model.tool_call` |
| `src-tauri/src/db/managers/save_repo.rs` | 存档读写同时持久化 `tool_call` |
| `<data_dir>/tool_permissions.toml` | 权限配置文件（首次启动自动生成，原子写） |

**前端（`src/`）**

| 文件 | 说明 |
|---|---|
| `stores/modules/game/actions.ts` | 展示历史时过滤 `attribute === 'tool'` 的行（工具过程对玩家不可见） |

## 工具调用闭环（速览）

```
用户/剧本/主动对话触发生成
   → GeneratorDeps{ source, tool_registry, ... }
   → run_pipeline → stream_with_tool_loop(llm, registry, context, source, role_name)
        ├─ 权限矩阵算出本轮 allowed 工具集
        ├─ 若 Provider 不支持流式工具 → 直接普通流式返回
        ├─ 每轮：流式请求带 tools → 收集 ToolCalls + 透传 Content
        │     · 无工具调用 → 返回合并流（内容已逐字透传）
        │     · 有工具调用 → 执行器逐个执行（2s 超时）
        │                  → assistant(tool_calls) + tool(结果) 回填 messages
        │                  → 最多 MAX_TOOL_ROUNDS=3 轮
   → tool_messages 写入 line 表（assistant 带 tool_call JSON / tool 带 {tool_call_id,result}）
   → refresh_memories → MemoryBuilder 按台词表重建 role.memory（工具历史已入记忆）
   → 最终 LLM 流 → producer/consumer → add_assistant_line（试玩代号守卫）
   → 下次生成时 get_current_context 读 role.memory，工具历史完整参与上下文
```
