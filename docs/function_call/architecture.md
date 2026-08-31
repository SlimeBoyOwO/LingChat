# 实现思路（architecture）

> 总体数据流见 [diagrams/architecture.html](diagrams/architecture.html)，工具循环时序见 [diagrams/tool-loop.html](diagrams/tool-loop.html)。

## 1. 分层结构

PR #523 在 `src-tauri/src/ai_service/tools/` 下新增完整工具子系统，并横向改造了 LLM 层、生成管线、台词表与记忆构建器。整体结构：

```
生成触发层    chat.rs(UserChat) · game.rs(EntryGreeting) · script事件(AiDialogue/FreeDialogue) · 主动系统(Proactive)
   │  构造 GeneratorDeps{ source, tool_registry, ... }
生成管线层    MessageGenerator.process_message
   │          → get_current_context() 读 role.memory（工具历史已还原）
   ▼
工具循环层    tools/tool_loop.rs  stream_with_tool_loop
   ├─ registry.allowed_tools(source, role_name) → 权限裁剪
   ├─ llm.complete_stream_with_tools(带 tools 的流式请求)
   ├─ ToolExecutor.execute(每个 tool call) → ToolResult
   └─ 回填 assistant(tool_calls) + tool(result) 消息
   ▼
LLM 层       LlmClient → LlmProvider
   ├─ genai_provider（流式工具：capture_tool_calls → 流尾 yield ToolCalls）
   └─ kimi_code（非流式 complete_with_tools，供 God Agent 专用）
   ▼
写入层       generator.rs：tool_messages → line 表（assistant 带 tool_call / tool 行）
   │          → gs.refresh_memories() → MemoryBuilder 重建 role.memory
   ▼
磁盘         data/game_database.db · line 表（save_repo 持久化 tool_call）
```

**各文件职责：**

| 文件 | 职责 |
|---|---|
| `tools/mod.rs` | 模块入口；`built_in_registry()`：注册内置工具 → 加载/创建权限配置 → 把角色初始化进 `default` 角色组 |
| `tools/registry.rs` | `ToolRegistry`：`register`（重名拒绝）/ `get` / `definitions` / `definitions_for_allowed` / `definitions_for` / `allowed_tools` |
| `tools/executor.rs` | `Tool` trait、`ToolContext`、`ToolExecutor` |
| `tools/tool_loop.rs` | `stream_with_tool_loop` 工具闭环 |
| `tools/permissions.rs` | `ToolPermissionConfig` 权限矩阵 |
| `tools/clock.rs` | 内置示例工具 `get_current_time` |

## 2. 核心类型（types.rs / executor.rs）

### 工具的「定义」与「调用」分离

`ToolDefinition` 是**给 LLM 看**的 JSON Schema（OpenAI function calling 格式）；`ToolCall` 是**LLM 返回**的调用请求；`ToolResult` 是**工具执行**的返回 JSON。

```rust
// 给 LLM 看的定义
ToolDefinition { type_: "function", function: FunctionSchema { name, description, parameters } }

// LLM 返回的调用请求
ToolCall { id, type_: "function", function: FunctionCall { name, arguments /* JSON 字符串 */ } }

// 工具执行返回
type ToolResult = serde_json::Value;
```

### 统一的 LLM 消息结构（LlmMessage）

`LlmMessage` 新增三个可选字段，一套结构同时表达「普通消息」与「工具轮次消息」：

```rust
LlmMessage {
    role: String,                       // "system" | "user" | "assistant" | "tool"
    content: String,
    tool_calls: Option<Vec<ToolCall>>,  // assistant 请求调用工具时携带
    tool_call_id: Option<String>,       // tool 消息回填时携带
}
// 构造助手：
LlmMessage::tool(tool_calls)          // role=assistant, content=""
LlmMessage::tool_result(id, content)  // role=tool, tool_call_id=id
```

### 流式 chunk（LlmChunk）

`LlmChunk` 新增 `ToolCalls` 变体 —— **工具调用不是流式中间件逐字吐的，而是在一轮流结束后由 Provider 一次性给出**（各家的流式 API 都是「结束事件里带捕获到的完整 tool call」）：

```rust
pub enum LlmChunk {
    Content(String),                    // 正式回复内容
    Reasoning(String),                  // 思考链内容
    ToolCalls(Vec<ToolCall>),           // 一轮流结束后得到的完整工具调用
}
```

## 3. 工具注册（ToolRegistry）

```rust
pub trait Tool: Send + Sync {
    fn definition(&self) -> ToolDefinition;                                        // 给 LLM 的定义
    async fn execute(&self, context: &ToolContext, arguments: Value)
        -> Result<ToolResult, ToolError>;                                          // 执行
}
```

`ToolRegistry` 用 `HashMap<String, Arc<dyn Tool>>` + 有序 `Vec<String>` 维护：

- `register()`：**重名直接报错**（`RegistryError::DuplicateName`），从根上杜绝「两个工具同名导致定义列表打架」；
- `definitions()`：按**注册顺序**返回全部定义（顺序稳定，LLM 收到的工具列表顺序恒定）；
- `definitions_for_allowed(allowed)`：按预计算的允许集合过滤，避免调用方重复算权限；
- `definitions_for(source, role_name)` / `allowed_tools(source, role_name)`：一次调用完成「权限计算 + 过滤」，给执行层同时返回工具集，供二次校验。

应用启动时在 `lib.rs::setup` 里创建一次（单例放进 `AppState`）：

```rust
let role_names = RoleRepo::get_all_tool_role_names(&db)?;   // (role.name, ai_name) 列表
let tool_registry = Arc::new(tools::built_in_registry(role_names)?);
```

`built_in_registry` 的三件事：

1. 建空注册表，注册全部内置工具（当前内置 `CurrentTimeTool`，名称 `get_current_time`）；
2. `ToolPermissionConfig::load_or_create(data_dir, 全部工具名)` —— 首次启动生成 `tool_permissions.toml`；
3. `initialize_characters(data_dir, ai_name 列表)` —— 把每个已知角色加入 `default` 角色组（默认**无任何工具权限**，见 [permission.md](permission.md)）。

权限查重用的是 `ai_name`（角色显示名）：生成管线在 `run_pipeline` 里读 `current_role.display_name`，而 `display_name` 即 `settings.ai_name`，两处对齐。

## 4. 工具执行（ToolExecutor）

`ToolExecutor` 是「查找 + 解析 + 超时 + 错误编码」的统一封装，**任何工具错误都不会向上抛**，而是编码成稳定的 JSON 回填给 LLM（LLM 会读到自己调用的工具出错了，自行决定下一步）：

```text
execute(name, arguments, context)
  ① context.allows(name) 不通过        → {ok:false, error:{code:"tool_not_allowed"}}
  ② registry.get(name) 找不到          → {ok:false, error:{code:"unknown_tool"}}
  ③ arguments 不是合法 JSON            → {ok:false, error:{code:"invalid_json"}}
  ④ arguments 不是 JSON object         → {ok:false, error:{code:"invalid_arguments"}}
  ⑤ tool.execute() 返回 Err            → {ok:false, error:{code:"tool_error", message}}
  ⑥ 超过 2 秒                          → {ok:false, error:{code:"timeout"}}
  ⑦ 结果序列化失败                     → {ok:false, error:{code:"serialization_error"}}
  ⑧ 成功                               → 结果的 JSON 字符串
```

要点：

- **`ToolContext` 只读**：持有一个 `allowed_tools: HashSet<String>`（本轮由权限矩阵预计算），`Tool` 实现里可自行 `context.allows(name)` 二次校验；
- **2 秒超时**：`tokio::time::timeout`，慢工具直接返回 `timeout` 错误结果，不卡生成管线；
- **错误可回填**：错误 JSON 走 `LlmMessage::tool_result` 回填给模型 —— 模型会看到「工具调用了、但报错了」，这是与「工具抛异常导致整轮崩溃」的本质区别。

## 5. 工具循环（stream_with_tool_loop）

见 [diagrams/tool-loop.html](diagrams/tool-loop.html)。核心逻辑（`tool_loop.rs`）：

```text
stream_with_tool_loop(llm, registry, messages, source, role_name)
  ├─ allowed = registry.allowed_tools(source, role_name)
  ├─ definitions = registry.definitions_for_allowed(&allowed)
  ├─ 若 definitions 为空 或 provider 不支持流式工具：
  │      → 返回「普通流式」（presentation_stream 过滤杂散的 ToolCalls），tool_messages 为空
  ├─ 建 (content_tx, content_rx) 无界 channel —— 把每轮 Content chunk 实时透传出去
  │
  for round in 0..=MAX_TOOL_ROUNDS:            // MAX_TOOL_ROUNDS = 3
  │   response_stream = provider.stream_with_tools(messages, definitions)
  │   逐 chunk：
  │     ToolCalls(calls)  → 收集进 tool_calls
  │     Content(text)     → round_text += text；透传 content_tx
  │     Reasoning/其他    → 透传 content_tx
  │   若 tool_calls 为空：
  │     → 关闭 content_tx，返回 ToolLoopResult {
  │          stream: Box::pin(content_rx 流),      // 所有轮次内容的合并流
  │          tool_messages,                         // 本循环里产出的 assistant/tool 消息
  │        }
  │   若 round == MAX_TOOL_ROUNDS：
  │     → 报错「工具调用超过最大轮次 3」
  │   （校验 call.id 非空且不重复）
  │   assistant_message = { role:"assistant", content: round_text, tool_calls: Some(calls) }
  │   messages.push(assistant_message)；tool_messages.push(assistant_message)
  │   for call in calls:
  │       result = executor.execute(call.function.name, call.function.arguments, context)
  │       tool_message = LlmMessage::tool_result(call.id, result)
  │       messages.push(tool_message)；tool_messages.push(tool_message)
```

设计要点：

- **透传而非缓冲**：每一轮的 Content chunk 通过 channel 实时转发给调用方，调用方拿到的是一个「所有轮次内容拼接」的流 —— 前端的打字机效果在工具调用间隙不中断（模型边说边调工具）；
- **工具调用发生在流结束后**：流式 API 必须等整轮流结束（`ChatStreamEvent::End`）才能拿到 `captured_into_tool_calls()` 的完整参数，所以参数「已经由 provider 合并完整」；
- **最多 3 轮**：防止模型陷入「反复调同一个工具」的无限循环；超过即硬报错；
- **非流式工具 provider 直接跳过闭环**：`supports_streaming_tools() == false` 时连工具定义都不下发，回到普通单次流式 —— 避免退回「非流式预检」这类笨办法。

### LlmMessage 与 LlmChunk 的类型命名歧义说明

同一段代码里有 `ToolCall`（types.rs）与 `LlmChunk::ToolCalls`（llm/mod.rs），前者是「调用请求结构」，后者是「流式 chunk 变体」，是两层概念。

## 6. LLM Provider 层的工具支持

`LlmProvider` trait 新增三组方法（[diagrams/architecture.html](diagrams/architecture.html) 的 LLM 层）：

```rust
// 是否支持「原生流式」工具调用
fn supports_streaming_tools(&self) -> bool { false }            // 默认不支持

// 流式 + tools：仅 supports_streaming_tools=true 时使用
async fn complete_stream_with_tools(&self, http, messages, tools, tool_choice)
    -> Result<ChunkStream> { self.complete_stream(http, messages).await }   // 默认回退

// 非流式 + tools：供 God Agent 这类「先决定、再生成」的编排器使用
async fn complete_with_tools(&self, http, messages, tools, tool_choice)
    -> Result<LlmResponseWithTools> { /* 默认回退 complete */ }
```

`LlmClient` 薄封装两个入口（都先做 `cfg.is_usable()` 校验）：

- `complete_stream_with_tools(messages, tools, tool_choice)` —— 聊天工具闭环用的流式入口；
- `complete_with_tools(messages, tools, tool_choice)` —— 非流式，一次拿完整 `LlmResponseWithTools { content, tool_calls }`。

### 当前两个 Provider 的差异

| Provider | 流式工具 | 说明 |
|---|---|---|
| `genai_provider` | ✅ `supports_streaming_tools() = true` | `build_chat_options` 里 `with_capture_tool_calls(true)`，流结束后 `end.captured_into_tool_calls()` → `yield LlmChunk::ToolCalls`；`tool_choice` 映射 `auto / none / required` |
| `kimi_code` | ❌（默认 false） | 聊天路径不参与工具循环；实现非流式 `complete_with_tools`，按 Anthropic `tool_use` / `tool_result` 块格式收发，**专供 God Agent** 决定「下一句话谁说」 |

> 也就是说：**聊天 / 剧本 / 主动对话的工具循环只在 genai 系 Provider 下生效**；Kimi Code 虽不能流式工具，但其 God Agent 的非流式工具早已独立工作。

## 7. 接入点：GeneratorSource / GeneratorDeps / run_pipeline

### GeneratorSource

新增枚举，标识「本轮生成是谁发起的」—— 这是权限矩阵里「场景组」的键来源：

```rust
pub enum GeneratorSource { UserChat, Proactive, ScriptAiDialogue, ScriptFreeDialogue, EntryGreeting }
```

### GeneratorDeps 新增两个字段

```rust
pub struct GeneratorDeps {
    pub source: GeneratorSource,          // 本轮业务来源
    pub tool_registry: Arc<ToolRegistry>, // 共享注册表
    // ... 原有 app / db / game_status / processor / translator / llm / concurrency
}
```

每个触发点构造 deps 时各填各的 source：

| 触发点 | 文件 | source |
|---|---|---|
| 玩家发消息 | `api/chat.rs:86` | `UserChat` |
| 主动对话 | `api/chat.rs:339` / `proactive_system/mod.rs:247` | `Proactive` |
| 剧本 AI 对话事件 | `script_engine/events/ai_dialogue_event.rs:95` | `ScriptAiDialogue` |
| 剧本自由对话事件 | `script_engine/events/free_dialogue_event.rs:121` | `ScriptFreeDialogue` |
| 入场问候 | `api/game.rs:908` | `EntryGreeting` |

（`4cc9b1b0 fix：补全入场问候工具注册` 就是给入场问候补上 `tool_registry` 的提交 —— 该触发点早期漏传导致编译失败。）

### run_pipeline 的接入

`run_pipeline` 是生成管线的统一出口，工具循环就挂在这里：

```rust
async fn run_pipeline(&self, context: Vec<LlmMessage>, ...) -> Result<String> {
    // ① 取当前角色 display_name（= ai_name），作为权限查重的角色名
    let role_name = /* gs.current_role_id → role.display_name */;

    // ② 工具循环
    let tool_loop_result = stream_with_tool_loop(
        &self.deps.llm, &self.deps.tool_registry,
        context, self.deps.source, role_name,
    ).await?;

    // ③ 工具轮次消息落台词表（见 §8 / [memory.md](memory.md)）
    if !tool_loop_result.tool_messages.is_empty() {
        // 按顺序把 assistant(tool_calls) / tool(result) 写成 Line 行插进 line_list，
        // 然后 gs.refresh_memories() 重建所有角色记忆
    }

    // ④ 用合并流继续原有 producer/consumer 管线
    let llm_stream = tool_loop_result.stream;
    ...
}
```

一个关键的先后关系：**工具历史先落台词表并刷新记忆，然后最终回复的流才开跑**。这样即使 God Agent 模式下同一轮会有多次 `process_message` 循环（每个角色各生成一次），后一次生成读到的 `role.memory` 已包含前一次的工具调用过程。

## 8. 与系统台词表的联合

工具调用不是「调用完就丢」的临时数据，而是**正式进入系统台词表**（`line` 表），理由见 [memory.md](memory.md)。落盘发生在 `run_pipeline` 的 ③：

- **assistant（带工具请求）行**：`LineAttribute::Assistant`，`content` 存本轮 `round_text`（工具间隙说的话），`tool_call` 存 `serde_json::to_string(tool_calls)` —— 即整段 `[{id, type, function}]` JSON；
- **tool（工具返回）行**：`LineAttribute::Tool`（新增属性），`content` 存结构化 JSON：

```json
{ "tool_call_id": "<call.id>", "result": { ... } }
```

这两类行都 `sender_role_id: None`（不是角色台词），插入位置是当时 `line_list` 的末尾，`perceived_role_ids` 取当时的在场角色。随后 `gs.refresh_memories()` 让记忆与台词表一致。

持久化：`save_repo.rs` 读写存档时同步读写 `tool_call` 列；`m20260727_add_line_tool_call` 迁移给 `line` 表加列；前端展示历史时按 `attribute === 'tool'` 过滤（`game/actions.ts`）—— **工具过程对玩家不可见，只活在台词表与记忆里**。

## 9. 与既有 God Agent 工具的关系（两条路径并存）

PR #523 引入的是「通用聊天工具」路径；God Agent 的「选说话人」是**另一条独立的专用路径**：

| | 通用聊天工具（PR #523） | God Agent 选说话人（既有） |
|---|---|---|
| 注册 | `ToolRegistry` | God Agent 私有 `tools::select_next_speaker_tool()` |
| 入口 | `complete_stream_with_tools`（流式循环） | `complete_with_tools`（非流式单次） |
| 权限 | 「场景组 × 角色组」矩阵 | 无（硬编码） |
| 结果去向 | 台词表 + 记忆 | 仅用于选说话者，不进台词表 |

## 10. 设计取舍

1. **工具调用是「写入口」级别的持久化**：没有把工具调用做成独立表，而是复用 `line` 表 + 属性列。理由：台词表本来就有「追加 + 截断还原」语义（试玩快照按 `line_len` 截断即可完整带走工具行），且 `MemoryBuilder` 可以按行序无缝还原成 LLM 消息；
2. **权限在「下发定义」与「执行」两处都拦**：`allowed_tools` 既用来裁剪 `definitions_for_allowed`（LLM 根本看不到不允许的工具），又放进 `ToolContext` 供 `ToolExecutor` 二次校验（防 `run_pipeline` 之外的调用路径绕过）；但 ToolExecutor 的二次校验实际主要防的是「内部误传」而非对抗恶意 —— 单进程内没有恶意边界；
3. **工具错误永不抛出**：统一编码成 `{ok:false,...}` 回填，LLM 自主决定下一步 —— 让「工具失败」成为对话语义的一部分而不是管线崩溃；
4. **流式工具是硬性门槛**：不支持流式工具的 Provider 宁可整个跳过工具闭环，也不做非流式预检 —— 避免用户可感知的延迟抖动。
