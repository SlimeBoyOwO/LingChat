# 记忆的存储与提取、AI 记忆构建（memory）

> 记忆构建数据流见 [diagrams/memory.html](diagrams/memory.html)。

## 1. 一句话

**工具调用不是临时数据，而是正式写入系统台词表（`line` 表）的一类台词；每次生成前，`MemoryBuilder` 把台词表按目标角色重建成 OpenAI 格式的 `LlmMessage` 列表（含工具轮次），存入该角色的 `role.memory`。** 存储靠台词表，提取靠 `MemoryBuilder`，构建产物就是给 LLM 的上下文。

## 2. 存储（Storage）：工具过程进入系统台词表

### 台词表是唯一的「历史真相源」

PR #523 没有给工具调用开独立存储，而是复用系统台词表（`line` 表）—— 它与玩家台词、角色台词同表同序，天然获得「追加 / 截断还原 / 持久化」全套语义（剧本试玩的 `line_len` 截断、存档读写都自动覆盖工具行）。

### 数据库结构

- **迁移** `m20260727_add_line_tool_call`：给 `line` 表加一列 `tool_call`（TEXT，可空）；
- **实体** `db/entities/line.rs`：
  - `LineAttribute` 枚举新增 `Tool`（DB 字符串值 `"tool"`）—— 与 `user / system / assistant` 并列；
  - `Model` 新增 `tool_call: Option<String>`；
- **业务结构** `ai_service/types.rs`：`LineBase` 新增 `tool_call: Option<String>`。

### 落盘时机：`run_pipeline` 第 ③ 步

工具循环返回 `ToolLoopResult { stream, tool_messages }` 后，`generator.rs` 把 `tool_messages`（一个 `Vec<LlmMessage>`，交替的 assistant 请求行 + tool 结果行）按序写入当前 `line_list` 末尾：

```rust
for msg in tool_loop_result.tool_messages.iter().rev() {
    let (attribute, content, tool_call) = match msg.role.as_str() {
        "assistant" => (
            LineAttribute::Assistant,
            msg.content.clone(),                       // 本轮说的人类可读文本
            msg.tool_calls.map(|calls| serde_json::to_string(calls).unwrap_or_default()),
        ),
        "tool" => (
            LineAttribute::Tool,
            serde_json::to_string(&serde_json::json!({
                "tool_call_id": msg.tool_call_id,
                "result": 解析后的 msg.content,          // JSON 值
            })).unwrap_or_default(),
            None,
        ),
        _ => continue,
    };
    let line = LineBase { content, tool_call, attribute, sender_role_id: None, ..Default::default() };
    gs.line_list.insert(insert_pos, GameLine::from_base(line, perceived.clone()));
}
gs.refresh_memories(&self.deps.db).await?;          // 关键：写台词表后立刻重建记忆
```

所以 `line` 表里同一轮工具调用长这样（按时间序）：

| attribute   | content                                                                                                      | tool_call                                                                                     |
| ----------- | ------------------------------------------------------------------------------------------------------------ | --------------------------------------------------------------------------------------------- |
| `assistant` | 「好的，我来查一下现在几点。」                                                                               | `[{"id":"call_1","type":"function","function":{"name":"get_current_time","arguments":"{}"}}]` |
| `tool`      | `{"tool_call_id":"call_1","result":{"local_time":"2026-08-02T...","timezone":"local","unix_timestamp":...}}` | `NULL`                                                                                        |
| `assistant` | 「现在是 2026-08-02 14:30。」                                                                                | `NULL`（最终回复，走 `add_assistant_line`）                                                   |

要点：

- **assistant 行**：`content` 存工具间隙说的话，`tool_call` 存整段调用请求 JSON；
- **tool 行**：`content` 存结构化的 `{tool_call_id, result}`，与「角色台词」同表同构，但 `sender_role_id: None`（不是任何角色的发言）；
- **持久化**：`save_repo.rs` 读写存档时同步读写 `tool_call` 列（含弱匹配去重逻辑），工具历史随存档走；
- **对玩家不可见**：前端展示历史时过滤 `attribute === 'tool'` 的行（`stores/modules/game/actions.ts`），玩家只看到 assistant 行的自然语言。

## 3. 提取（Extraction）：`MemoryBuilder` 还原

`MemoryBuilder::build(lines)` 把台词表序列按目标角色翻译成 LLM 消息。工具相关的还原分两类：

### assistant 工具请求行的还原

```rust
if matches!(line.attribute(), LineAttribute::Assistant) {
    // 新版：tool_call 字段存 JSON
    if 解析 line.base.tool_call 为 Vec<ToolCall> 成功 {
        memory.push(LlmMessage {
            role: "assistant",
            content: line.base.content.clone(),     // 原样保留工具间隙的话
            tool_calls: Some(tool_calls),            // 还原成 tool_calls
            tool_call_id: None,
        });
        continue;                                    // 不再走普通 assistant 缓冲
    }
    // 旧版兼容：content = "tool_calls_json\n\n文本"
    else if 从 content 里 \n\n 前段解析出 Vec<ToolCall> 成功 {
        memory.push(LlmMessage { content: 后段文本, tool_calls: Some(...), ... });
        continue;
    }
}
```

### tool 结果行的还原

```rust
if matches!(line.attribute(), LineAttribute::Tool) {
    let (tool_call_id, result) = 解析 content 里的 {tool_call_id, result};
    memory.push(LlmMessage {
        role: "tool",
        content: result,                              // 结果值
        tool_calls: None,
        tool_call_id,
    });
    continue;
}
```

### 兼容性设计

- **新版为主、旧版兜底**：`tool_call` 字段缺失时，回退解析老格式 —— 老存档（工具调用写在 `content` 里，`\n\n` 分隔 JSON 与正文）也能被还原；
- **`tool` 行一定 `flush` 掉上一个缓冲**：工具行不属于任何「角色对话缓冲」，必须独立成消息，避免被并进旁边的人话；
- 工具行被还原成 `LlmMessage` 后，**不带情绪 / TTS / 动作富化**（`format_content_with_extras` / `format_context_line` 只用于普通台词）—— 工具消息是纯协议消息。

## 4. 构建（Construction）：`role.memory` 生命周期

每个角色在 `GameRoleManager` 里持有一份 `memory: Vec<LlmMessage>`，它就是「给这个角色看的 AI 记忆」。生命周期：

```text
① 建角色时（首次进入场景 / 加载存档）
     角色.memory = MemoryBuilder{ target_role_id }.build(line_list)
② 每次 line_list 变化（加台词、加工具行、加旁白、截断还原）
     GameStatus::add_line → ... → role_manager.sync_memories()
     sync_memories 收集 line_list 里涉及到的角色（sender 或 perceived）
       → 对每个角色用 MemoryBuilder 整体重建 memory
③ 生成时
     generator.get_current_context() = 当前角色.memory.clone()
     → 交给 stream_with_tool_loop 作为 messages 起点
④ 生成后
     run_pipeline 把工具行写回 line_list → 再 refresh_memories() → 回到 ②
```

`sync_memories` 是**全量重建**而非增量追加（参数 `recent_n` 可选，用于只重建最近 N 条）：每次 `refresh_memories` 都从 `line_list` 重新 build 一遍，保证记忆与台词表严格一致 —— 代价是每轮生成前有一次 O(行数) 的重建，换来「永不漂移」。

## 5. 完整闭环（工具记忆如何参与下一轮生成）

见 [diagrams/memory.html](diagrams/memory.html) 的数据流。以一个「玩家问时间」为例：

```text
① 玩家：现在几点了？
② get_current_context() → role.memory（此前记忆，无工具历史）
③ stream_with_tool_loop(messages, definitions=[get_current_time])
     LLM 返回 assistant(tool_calls=[get_current_time])
     → 执行 → tool({local_time, timezone, unix_timestamp})
     → 回填 messages（assistant 请求行 + tool 结果行）
     → 再请求一轮 → LLM 据此组织回复正文
④ tool_messages 写回 line 表（assistant 带 tool_call + tool 行）
   → refresh_memories → MemoryBuilder 重建角色.memory（含工具行）
⑤ 最终流 → producer/consumer → add_assistant_line（最终回复行也入表）
⑥ 下次玩家问任何事，role.memory 里已经有：
     assistant(tool_calls=get_current_time) / tool(结果) / assistant(正文)
   —— 模型「记得自己查过时间」，甚至能自然接「我刚才看过了，现在是……」
```

### God Agent 多轮下的特殊价值

`process_message` 在 God Agent 模式下会循环多次（每个候选角色各生成一轮）。因为 ④ 在每轮 `run_pipeline` 里都会执行，**后一轮角色读到的记忆已包含前一轮角色的工具调用过程** —— 工具历史在跨轮、跨角色之间都是连贯的，这正是「先落台词表再刷新记忆」这一顺序的意义。

## 6. 与既有记忆体系的关系

| 层                                            | 内容                                          | 说明                                    |
| --------------------------------------------- | --------------------------------------------- | --------------------------------------- |
| `role.memory`                                 | 本轮会话的 LLM 上下文（`MemoryBuilder` 重建） | 含工具行还原逻辑                        |
| 台词表 `line` 表                              | 会话历史真相源（`save_repo` 持久化）          | `tool_call` 列 + `Tool` 属性            |
| `memory_archive_{role_id}`（`kind='memory'`） | 跨会话的长期记忆：工具写入、RAG 自动检索      | `MemoryBuilder` 不直接读它，见 §7       |
| `MemoryBank`（旧持久记忆摘要）                | **已废弃**：表保留、链路不再读写              | 同步时只清不导（`lan_sync/db_sync.rs`） |

长期记忆由模型主动调工具写入（`memory_store`）—— 没有后台抽取；
检索则在**每条用户消息生成前**自动完成（RAG）：宿主用最近几轮对话当 query 做混合检索，
把命中结果落成一条 `Memory` 属性的台词行插在该用户行之前，经 `MemoryBuilder` 重建进入当轮上下文；
下一条用户消息会替换掉上一轮的检索行，历史里最多只保留最近一轮的结果。检索没有对应工具。

## 7. 长期记忆的生命周期：去重 / 删除 / 摘要 / 检索注入 / 多端同步

入口统一是 `ai_service/memory/`（`MemoryService`），配置在启动时快照。

- **嵌入模型（可选）**：远程优先 —— 配置了 `embedding.*`（base_url / api_key / model）时用远程 OpenAI 兼容 embeddings 接口；留空且随应用打包的内置本地模型齐备时，自动使用 **bge-small-zh-v1.5 中文量化版**（`data/third_party/bge_small_zh_v1_5/`，约 11MB，完全离线，无需联网与任何 API）。内置模型为 int4 Transformer + int2 词嵌入表、512 维 CLS 池化，加载是懒加载（首次嵌入时才读表建 ONNX Session），推理在阻塞线程池执行；换模型后旧向量不参与检索（按 `model` 列过滤）。**两者都不可用时不影响记忆功能**：检索与写入自动退化为 **字符 n-gram 词法模式**（非嵌入式 RAG）——写入不再要求向量（`embedding` / `dim` / `model` 落 NULL），检索按「中文 bigram + 单字、ASCII 整词」的 IDF 加权覆盖率打分，并在多个查询片段（换行 / 句末标点切分）间取最大值；纯词法模式下最低分阈值按 0.5 系数放宽。
- **写入（store）**：三级去重 —— 余弦 > 0.92 拒收；0.70–0.92 交给 LLM 判 merge/skip/new；LLM 失败降级为直接存储。无嵌入模型时改用词法覆盖率（拒收 0.85 / LLM 判定下界 0.45）。
- **删除**：`memory_delete` 按 `memory_id` 精确删或按相似度删（余弦阈值 0.5，词法模式下同系数放宽）；删角色时 `RoleRepo::delete_main_role` 连同 `memory_archive_profiles` 目录行与 `memory_archive_{role_id}` 数据表一并清理，并兜底清掉旧 `memory_embeddings` 残留行。
- **摘要（summarize）**：`MemoryService::summarize(role_id, keep_recent)` 把较早的一批记忆交给 LLM 合成一条 `summary` 记忆，**先插摘要、再删源条目**（插入失败不丢数据）。三条触发路径共用同一条链路：
  1. 模型主动调 `memory_summarize` 工具；
  2. 写入后条数达到 `AUTO_SUMMARIZE_THRESHOLD`（当前 64）时后台自动压缩（不阻塞本轮工具）；
  3. 插件 `compress_context()` 显式触发（只处理达到阈值的角色，返回 `{ ok, triggered }`）。
- **检索注入（RAG）**：`MessageGenerator::inject_rag_memories`（仅用户新消息轮）以最近几轮真实对话为 query 调 `MemoryService::search`（hybrid，命中取 `min(memory.top_k, 6)` 条），结果写成 `LineAttribute::Memory` 台词行插在该用户行之前，并替换掉本角色上一轮的检索行。记忆功能关闭时只清旧行不检索；**未配置嵌入模型时改用纯词法检索**（非嵌入式 RAG），不再整段跳过；检索失败只记警告、不影响本轮生成。该行不参与 LAN 同步（对端下条消息自行重建），也从前端历史、上帝 Agent 决策视图与 `chat_message_count` 中排除。
- **多端同步**：`memory_embeddings` 随 LAN 记录级同步整表镜像（导出时 `embedding` 转 base64，避免 JSON 数字数组膨胀）；旧对端发来的记录没有该字段时保留本地记忆、不做镜像。
- **调试**：聊天框输入 `/查看记忆`，后端日志会打印角色短期记忆、记忆服务配置（`memory::describe`）与该角色的长期记忆条数。
- **浏览 / 删除（UI）**：设置 → 高级设置 → 记忆系统 → 「查看记忆」面板可按角色浏览全部长期记忆（正文、时间、标签，分块已聚合），并逐条删除（`memory_list_roles` / `memory_list` / `memory_delete` 命令，按 `role_id` 作用域删）。只读写库表，不依赖 `memory_enabled` 开关与嵌入配置——功能关闭时也能查看和清理。

## 8. 常见问题

- **为什么工具行塞进 `line_list` 而不是单独数组？** —— 复用台词表的「追加 / 截断 / 持久化 / 感知者」语义：剧本试玩的 `line_len` 截断能把整段工具历史一起带走；存档读写天然覆盖；且 `MemoryBuilder` 按行序重建时工具行和台词顺序天然一致。
- **工具行的 `perceived_role_ids` 有意义吗？** —— 落盘时取了当时的 `present_role_ids`，但工具行还原时是「无条件还原」（`attribute == Tool` 直接 push），不参与 `is_target` 感知过滤 —— 工具过程对所有角色记忆都可见，符合「工具是系统行为」的定位。
- **旧存档还能加载吗？** —— 能。`tool_call` 列可空；旧格式靠 `\n\n` 分隔符兼容解析。
