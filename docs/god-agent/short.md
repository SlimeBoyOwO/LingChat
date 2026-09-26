# 上帝 Agent · 速览

上帝 Agent 是**多人自由对话的调度器**。它不生成台词 —— 只回答两个问题：**下一个谁说话**，以及**这段对话让谁的感情动了**。

代码在 `src-tauri/src/ai_service/god_agent/`，四个文件、700 行。

---

## 1. 模块职责

| 文件        | 职责                                                              | 不负责                 |
| ----------- | ----------------------------------------------------------------- | ---------------------- |
| `mod.rs`    | 模块门面，只 re-export `GodAgentCore`                             | —                      |
| `config.rs` | 运行参数 `GodAgentConfig` + LLM 选择 `resolve_god_agent_provider` | 不持有状态             |
| `core.rs`   | 决策逻辑：激活判断、prompt 构建、解析后的结果校验                 | 不执行工具、不写数据库 |
| `tools.rs`  | 两个工具的 JSON Schema 定义 + 从 `ToolCall` 里容错抠参数          | **没有执行体**（见下） |

> `tools.rs` 和 `ai_service/tools/` 那套通用工具系统不是一回事。通用工具是"LLM 请求 → 我们执行 → 结果回填再问 LLM"的闭环；上帝 Agent 的工具**只解析不执行**，模型给出的参数就是最终答案。它走的是非流式 `LlmClient::complete_with_tools`，一次调用拿结果。

---

## 2. 对外接口

### `GodAgentCore`（core.rs）

```rust
pub struct GodAgentCore {
    pub llm: LlmSlot,          // Arc<RwLock<Option<Arc<LlmClient>>>>，支持热切换
    pub config: GodAgentConfig,
}

impl GodAgentCore {
    pub fn new(llm: LlmSlot, config: GodAgentConfig) -> Self;

    /// 要不要干活：自由对话 + 在场角色 > 1（present_role_ids 含玩家 0）
    pub fn should_activate(&self, gs: &GameStatus) -> bool;

    /// 选下一个发言者。返回 (role_id, reason)，role_id=0 表示交还玩家。
    pub async fn decide_next_speaker(
        &self,
        gs: &GameStatus,
        current_speaker: Option<i32>,
    ) -> Result<(i32, String)>;

    /// 评估好感度变化，返回**未应用**的调整列表。
    pub async fn evaluate_affection(
        &self,
        lines: &[GameLine],
        npcs: &[NpcAffectionView],
    ) -> Result<Vec<tools::AffectionAdjustment>>;
}
```

`decide_next_speaker` 的 Err 都是"本轮编排放弃"：没返回 tool_calls、解析失败、或选了个不在场的角色。**不重试**。

`evaluate_affection` 收 `NpcAffectionView`（`role_id` / `name` / `subtitle` / `info` / `current` / `negative`），不是 `GameStatus` —— 因为调用方要在锁内快照、锁外调 LLM，不能持锁跨 await。

### `config.rs`

```rust
pub struct GodAgentConfig {
    pub provider_id: Option<String>,        // 专用 provider，None = 跟聊天主 LLM
    pub max_consecutive_npc: usize,         // 默认 3，下限 1
    pub recent_window: usize,               // 默认 20，下限 5
    pub affection_eval_interval: usize,     // 默认 5，下限 1
    pub affection_enabled: bool,            // 默认 true
}

impl GodAgentConfig {
    pub fn load(app: &AppHandle) -> Self;   // 从 tauri-plugin-store 读
}

/// 4 级兜底选 LLM，全落空返回 None（多人对话静默降级为单角色）
pub fn resolve_god_agent_provider(app: &AppHandle) -> Option<LlmClient>;
```

兜底顺序：`llm.god_agent_provider_id` → 角色分配里的 `god_agent_provider_id` → 聊天主 LLM → 第一个可用的 provider。

⚠️ `load()` 用 `.as_str().and_then(parse)` 读值，**store 里必须写字符串**。写成 JSON 数字会被当成没配置，静默回落默认值。三个 `god_agent.*` 参数没有 UI，只能手改 `settings.json`；`affection.enabled` 有 UI。

### `tools.rs`

```rust
pub fn select_next_speaker_tool() -> ToolDefinition;
pub fn parse_speaker_selection(&ToolCall) -> Option<(i32, String)>;

pub fn update_affection_tool() -> ToolDefinition;
pub struct AffectionAdjustment {
    pub role_id: i32,
    pub deltas: Vec<(String, i32)>,           // 好感维度增量，已 clamp ±5
    pub negative_deltas: Vec<(String, i32)>,  // 负面维度增量，同上
    pub reason: String,
}
pub fn parse_affection_update(&ToolCall) -> Option<AffectionAdjustment>;
```

解析层做了大量容错，因为模型输出不可靠：拆 `{"arguments": {...}}` 套壳、双重编码的 JSON 字符串、`role_id` 输出成 `"6"` / `" 6 "` / `"6.0"`、增量输出成浮点、未知维度名静默丢弃、增量超范围 clamp 到 ±5。解析不了一律降级为"这次什么都不做"，不 panic。

---

## 3. 谁调用它

### 装配（进程启动一次）

| 位置                          | 做什么                                                                                                                    |
| ----------------------------- | ------------------------------------------------------------------------------------------------------------------------- |
| `app/setup/build.rs`          | `resolve_god_agent_provider()` → `GodAgentCore::new(slot, config)` → 存进 `AppState.god_agent: Option<Arc<GodAgentCore>>` |
| `api/settings.rs::switch_llm` | 用户切模型时重建 `god.llm` 槽位，热生效，不用重启                                                                         |

### 运行时（`message_system/generator.rs`）

`MessageGenerator` 通过 `GeneratorDeps.god_agent: Option<Arc<GodAgentCore>>` 拿到它，插在 `process_message` 的三个位置：

```
process_message(user_msg)
 ├─ handle_user_message / detect_scene_change
 ├─ god_agent_pre_select()        ← 只在 user_msg.is_some() 时；选谁先接话
 ├─ loop {                        ← 最多 max_consecutive_npc 轮
 │     get_current_context()          读当前角色的 memory
 │     execute_pipeline()             流式生成 → ai:reply
 │     god_agent_post_select(n)   ← 决定继续还是交还玩家
 │   }
 └─ maybe_evaluate_affection()    ← 循环之后，spawn 到后台，不阻塞回复
```

三个方法都是 `generator.rs` 的私有方法，它们内部才去碰 `GodAgentCore`：

- `god_agent_pre_select()` → `decide_next_speaker()`，选中后写 `gs.current_role_id` 并 emit `character:switch`。
- `god_agent_post_select(n)` → 先看硬闸 `n >= max_consecutive_npc`，再看激活条件，最后 `decide_next_speaker()`。返回 `(should_continue, role_id)`。
- `maybe_evaluate_affection()` → 锁内快照 + 推进游标，然后 `spawn` 后台任务跑 `evaluate_affection()`，结果写角色状态和存档全局变量。

### 哪些入口接了它

`GeneratorDeps.god_agent` 有 6 个填充点，**只有 3 个传 `Some`**：

| 调用点                                             | 来源            | 接了吗 |
| -------------------------------------------------- | --------------- | ------ |
| `api/chat.rs::send_chat_message`                   | `UserChat`      | ✅     |
| `api/chat.rs::trigger_ai_response`                 | `Proactive`     | ✅     |
| `api/game.rs::notify_player_entry`                 | `EntryGreeting` | ✅     |
| `ai_service/proactive_system/mod.rs::deliver`      | `Proactive`     | ❌     |
| `script_engine/events/{ai,free}_dialogue_event.rs` | 剧本            | ❌     |
| `ai_service/tools/background_command.rs`           | —               | ❌     |

剧本那两条虽然传了 `None`，`should_activate` 里的 `script_status.is_none()` 还是第二道保险。

### 前端拿到什么

两个 `app.emit` 单向广播，前端在 `src/api/tauri-events.ts` 里听：

- `character:switch` → `{ roleId, characterName }`。前端切 `currentInteractRoleId`、更新标题；角色不在 `presentRoleIds` 里时才替换舞台。
- `affection:changed` → `AffectionChangedPayload`（增量、调整后的六维、平均分、理由）。写回 `gameStore.gameRoles[rid]`，并记 `lastAffectionChange` 给好感度面板用。

---

## 4. 上下文（一句话版）

**没有跨轮状态。** 每次决策现场从 `GameStatus` 拼 prompt，恒为 2 条消息：system 放固定任务说明，user 放「在场 NPC 名单 + 最近 `recent_window` 行对话 + 当前发言者提示」。

`role.memory`（角色侧 MemoryBank 的长程记忆）**完全不读** —— 它只看最近发生了什么，不看故事讲到了哪。窗口之外永久不可见，换来的是上下文开销 O(1)。

细节见 [architecture.md](architecture.md)，图见 [diagrams/](diagrams/)。
