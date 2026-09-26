# 上帝 Agent · 实现说明（architecture）

> 配套图：[组件结构图](diagrams/architecture.html) · [发言者选择工作流](diagrams/speaker-flow.html) · [上下文组装数据流](diagrams/context-assembly.html) · [好感度评估时序图](diagrams/affection-flow.html)

---

## 1. 它解决什么问题

单角色对话里，"谁来说话"是恒定的。一旦舞台上同时站着两个以上 NPC，管线就必须回答三个新问题：

1. **用户说完话，谁先接？** —— 传统实现要么固定第一个在场角色，要么让所有角色抢着说。
2. **一个 NPC 说完，还有没有下一个？** —— 若不管，NPC 会互相刷屏，把玩家晾在一边。
3. **这段对话让谁的感情动了？** —— 感情状态不能靠角色自己一边演一边改（它正在扮演，判断不客观），需要一个"局外人"来评估。

上帝 Agent 就是把这三个问题收进一个**独立的、上帝视角的、不生成台词的**小 LLM 调用里。它复用同一个 `GameStatus`，但走自己的 LLM 槽位（可单独配模型）、自己的 prompt、自己的工具。

### 它明确不做的事

- **不生成台词**：产物只有 `role_id` 和一个理由字符串，台词仍由被选中的角色自己的对话管线产出（走该角色的 `memory`）。
- **不写角色记忆**：决策 prompt 即用即弃，不入 `line_list`、不入 MemoryBank。
- **不在剧本里工作**：`should_activate` 第一条件就是 `script_status.is_none()`。剧本模式有自己的事件机（`AiDialogue` / `FreeDialogue`），编排权归剧本。

---

## 2. 激活条件

唯一的判据是 `GodAgentCore::should_activate`（`core.rs`）：

```rust
gs.script_status.is_none() && gs.present_role_ids.len() > 1
```

即 **自由对话模式（没有剧本在跑）且在场角色多于一人**。注意 `present_role_ids` **包含玩家（role_id = 0）**，所以"玩家 + 1 个 NPC"（长度 2）也满足激活条件。

但激活不等于"每次都会问 LLM"。`decide_next_speaker` 里另有一道短路（见 [diagrams/speaker-flow.html](diagrams/speaker-flow.html)）：

```rust
let npc_ids: Vec<i32> = gs.present_role_ids.iter().filter(|&&id| id != 0)...;
if npc_ids.len() <= 1 {
    return Ok((npc_ids.first().copied().unwrap_or(0), "single_npc".into()));
}
```

**只有 1 个 NPC 时直接返回它，不调 LLM**（省一次请求，也避免模型在没得选时乱选）。这条短路对好感度评估**不适用**——`evaluate_affection` 单 NPC 时照样真调 LLM，因为情感评估本来就需要模型判断。

### 装配与热切换

| 环节   | 位置                                                                 | 说明                                                                                                                                                    |
| ------ | -------------------------------------------------------------------- | ------------------------------------------------------------------------------------------------------------------------------------------------------- |
| 构建   | `app/setup/build.rs`                                                 | `resolve_god_agent_provider(&app).map(\|llm\| Arc::new(GodAgentCore::new(slot, config)))`，`None` 时 `state.god_agent = None`，多人对话静默降级为单角色 |
| 槽位   | `GodAgentCore.llm: LlmSlot`（`Arc<RwLock<Option<Arc<LlmClient>>>>`） | 与聊天/翻译同款槽位，`slot_snapshot()` 读取                                                                                                             |
| 热切换 | `api/settings.rs::switch_llm`                                        | 重建三个槽位，其中第 3 步就是 `god.llm.write().await = new_god`，无需重启                                                                               |

### LLM 选择的 4 级兜底

`config.rs::resolve_god_agent_provider` 按序尝试，命中即返回：

1. `llm.god_agent_provider_id`（显式指定）
2. `role_assignment.god_agent_provider_id`（LLM 设置页里给"上帝Agent"角色分配的）
3. `role_assignment.chat_provider_id`（聊天主 LLM）
4. 任意第一个 `is_usable()` 的 provider

四级全空 → `tracing::warn!("上帝Agent 未找到可用 LLM，多人对话功能将禁用")` 并返回 `None`。

---

## 3. 上下文管理

这是上帝 Agent 最容易被误解的部分，先给结论：

> **上帝 Agent 没有任何跨轮次的状态。每一次决策都是一次性的、无历史的单次 LLM 调用，prompt 现场从 `GameStatus` 重新拼装。**

它不维护消息列表，不做摘要，不做压缩，不写回任何东西。这带来两个直接后果，都是**有意为之**的取舍：

- ✅ 上下文开销是 **O(1) 常数**，不随对话长度增长，永远只有 2 条消息（1 system + 1 user）。长跑对话不会把它撑爆，也不会产生"越聊越贵"的曲线。
- ⚠️ **窗口之外的信息对它永久不可见**。超过 `recent_window`（默认 20 行）之前的对话，上帝 Agent 完全不知道发生过。它靠的是"最近发生了什么"，而不是"故事讲到了哪"。

### 3.1 一次决策的上下文由三块拼成

`build_decision_prompt`（`core.rs`）产出的 `Vec<LlmMessage>` **恒为长度 2**：

| #   | 角色     | 内容                                                                   | 来源         |
| --- | -------- | ---------------------------------------------------------------------- | ------------ |
| 0   | `system` | 固定任务说明（"你是一个多人对话的导演…调用 select_next_speaker 工具"） | 硬编码字符串 |
| 1   | `user`   | 角色信息块 + 最近对话块 + 当前发言者提示                               | 现场拼装     |

`user` 消息的三块（格式见 [diagrams/context-assembly.html](diagrams/context-assembly.html)）：

**① 角色信息块** —— 遍历 `npc_ids`（`present_role_ids` 去掉 0），对每个角色取：

```
- role_id=6: 林雪
  简介: <settings.ai_subtitle>      ← 空则填「无」
  设定: <settings.info>             ← 空则填「无」
```

来源是 `gs.role_manager.get_loaded(rid)`。**只有这三个字段**：显示名、副标题、设定文本。没有记忆、没有当前好感度、没有立绘状态。

**② 最近对话块** —— `gs.line_list` 的**尾部滑动窗口**：

```rust
gs.line_list.iter().rev().take(window).cloned().collect::<Vec<_>>()
    .into_iter().rev().collect()          // 再翻回来，保持由旧到新
```

每行渲染为 `[role_id=N] 名字: 【情绪】内容`。注意：

- 窗口**不区分说话人**，玩家、NPC、旁白、系统行一视同仁进同一个窗口；靠行首的 `role_id` 区分（玩家行是 `role_id=0`）。
- 带 `original_emotion`（情绪标签），这是 `select_next_speaker` 独有、`update_affection` 没有的信息。
- **旁白行也在窗口里**。好感度变化写回的旁白台词（见 §5）因此会自然出现在下一轮决策的上下文中。

**③ 当前发言者提示** —— `current_speaker` 的三种分支：

| 值          | 文本                                                                                            |
| ----------- | ----------------------------------------------------------------------------------------------- |
| `Some(0)`   | "当前发言者是「玩家」。请选择下一个发言的 NPC 角色。"                                           |
| `Some(rid)` | "当前发言者是「X」(role_id=rid)，刚刚说完话。请判断：…"（引导它在"继续"与"交还玩家"之间二选一） |
| `None`      | 空字符串                                                                                        |

### 3.2 为什么 system 只放任务说明

`core.rs` 有一处刻意的拆分，注释写得很直白：

```rust
// system 只放任务说明，数据载荷放 user——只发单条 system 消息时，
// Codex（Responses API）会转换出空 input 被 400 拒绝
// （"One of input or ... must be provided"）。
```

Codex 的 Responses API 在只有 system 消息时会生成空 `input` 而被 400 拒绝。所以**任务说明与数据载荷必须分居 system / user**。改动 prompt 时不要图省事把数据塞回 system。好感度评估的 prompt 出于同样原因遵守该约定。

### 3.3 它和周遭上下文的关系

| 上下文                                                                       | 上帝 Agent 用不用  | 说明                                                        |
| ---------------------------------------------------------------------------- | ------------------ | ----------------------------------------------------------- |
| `GameStatus.line_list`                                                       | ✅ 用（尾部 N 行） | 唯一的"历史"来源                                            |
| `gs.role_manager.get_loaded(rid)` 的 `display_name` / `ai_subtitle` / `info` | ✅ 用              | 角色身份描述                                                |
| `gs.current_role_id`                                                         | ✅ 用              | 作为 `current_speaker` 提示                                 |
| `gs.present_role_ids`                                                        | ✅ 用              | 候选集 + 返回值合法性校验                                   |
| `GameRole.affection` / `negative`                                            | ✅ 好感度评估用    | 作为"当前情感状态"喂进去                                    |
| **`GameRole.memory`（MemoryBank 注入的 `Vec<LlmMessage>`）**                 | ❌ **完全不用**    | 这是角色侧长程记忆，与上帝 Agent 无关                       |
| `role.memory` 里的工具调用历史                                               | ❌ 不用            | 上帝 Agent 的调用不入 `line_list`，也就不会被记忆构建器还原 |
| 剧本上下文（`script_status`）                                                | ❌ 不用            | 剧本模式下它根本不激活                                      |
| 场景 / 立绘 / 音乐 / BGM                                                     | ❌ 不用            | prompt 里没有任何场景信息                                   |

### 3.4 锁的边界

`GameStatus` 是一把 `Arc<Mutex<…>>`。上帝 Agent 的所有 LLM 调用都在**锁外**进行，锁内只做快照：

- `decide_next_speaker(&gs, …)` 直接在 `MutexGuard` 下被 `.await`（`generator.rs` 里 `let gs = self.deps.game_status.lock().await; god.decide_next_speaker(&gs, current_speaker).await?`）——**这是现存的一处粗粒度持锁异步**，一轮决策期间会阻塞其它想拿 `GameStatus` 的任务。
- `evaluate_affection` 则走了正确的那条路：先在锁内把 `NpcAffectionView` 与 `lines` **快照**出来，释放锁，再 `spawn` 后台任务调 LLM。`NpcAffectionView` 存在的唯一理由就是"评估前从 GameStatus 快照，避免持锁跨 await"（`core.rs` 原注释）。

改动这部分时以 `evaluate_affection` 的写法为准。

---

## 4. 流水线一：发言者选择

入口在 `message_system/generator.rs::MessageGenerator::process_message`。上帝 Agent 插入的位置分三处：

```
process_message(user_message)
 ├─ 1.   handle_user_message          写 User 行
 ├─ 1.5 detect_scene_change           场景切换旁白
 ├─ 2.   god_agent_pre_select   ★     仅当 user_message.is_some()
 ├─ 3.   loop {                        ★ 可能多轮
 │         get_current_context         读 current_role 的 memory
 │         execute_pipeline            流式生成 → ai:reply
 │         cleanup_temp_message        仅首轮
 │         consecutive_npc_rounds += 1
 │         god_agent_post_select ★     决定 continue / break
 │       }
 └─ 4.   maybe_evaluate_affection ★   后台，不阻塞
```

### pre-select：用户说话后谁先接

`god_agent_pre_select` 在**用户发消息时**触发一次（`user_message.is_some()` 才走）。它：

1. 锁内读 `(should_activate, gs.current_role_id)`，不激活就返回；
2. 调 `decide_next_speaker`；
3. `selected_role_id == 0` → 什么都不做（"玩家自己接着说"，保持现状）；
4. 否则写 `gs.current_role_id = Some(selected)`，取显示名，`emit_character_switch`。

### 生成循环：NPC 能不能接着说

`god_agent_post_select(consecutive_npc_rounds)` 的判定顺序，**从便宜到贵**：

1. `god_agent` 为 `None` → `(false, 0)`，退出；
2. `consecutive_npc_rounds >= max_consecutive_npc` → 打日志"连续 N 轮 NPC 发言，强制返回玩家"，`(false, 0)`；
3. `!should_activate` → `(false, 0)`；
4. 调 `decide_next_speaker`；
5. `role_id == 0` → `(false, 0)`；
6. 否则写 `gs.current_role_id`、`emit_character_switch`、返回 `(true, role_id)` → 循环继续。

`consecutive_npc_rounds` 在**循环体内自增，每轮用户消息重置为 0**。默认 `max_consecutive_npc = 3` 意味着：**一条用户消息最多换来 3 轮 NPC 回复**，第 3 轮后无条件把发言权还给玩家。这是防"玩家被边缘化"的硬闸，不受模型判断影响。

> `god_agent_post_select` 返回的 `(bool, i32)` 里第二个值目前调用方**丢弃不用**（`let (should_continue, _next_role) = …`），角色切换的副作用已由函数自身完成。

### 决策的返回值校验

`decide_next_speaker` 对模型输出做三层校验，任一层不过就返回 `Err` 向上传播：

| 情况                                     | 处理                                                       |
| ---------------------------------------- | ---------------------------------------------------------- |
| `tool_calls` 为空 / 没有元素             | `Err("上帝Agent 未调用工具…")`，附上模型返回的文本便于排查 |
| `parse_speaker_selection` 解析失败       | `Err("解析 tool_call 失败…")`                              |
| `role_id` 不在 `present_role_ids` 且非 0 | `warn` + `Err("上帝Agent 选择了不在场的角色 N")`           |

注意**没有重试**。一次失败就放弃本轮编排（用户消息仍然已入 `line_list`，只是没人被显式点名）。

---

## 5. 流水线二：好感度定期评估

与发言者选择完全独立，`maybe_evaluate_affection` 在生成循环**之后**调用。它的作用是让"关系"这个慢变量有一个不干扰对话节奏的更新通道。

### 触发条件（全部满足才评估）

1. `deps.god_agent` 存在；
2. `config.affection_enabled` 为真（总开关）；
3. `gs.script_status.is_none()`（剧本对话不进评估，函数内锁内判断）；
4. `gs.line_list` 里 **真实对话** 的段数 ≥ `affection_eval_cursor + affection_eval_interval`；
5. 快照出的 `npcs` 非空。

"真实对话"由 `auto_save::is_real_dialogue` 定义：`Assistant` 属性且带 `sender_role_id`，或 `User` 属性且 `sender_role_id == 0` 且**内容不以旁白标记开头**。也就是说系统旁白（包括好感度自己写回的那条）不计入节流计数——否则会自我触发。

### 游标先推进

```rust
gs.affection_eval_cursor = real_count;   // 在 spawn 之前
```

游标在**发起评估前**就推进。因此即使后面的 LLM 调用失败、解析失败、或进程被杀，也不会在下一轮重复评估同一段对话——**不形成重试风暴**。代价是这段时间的对话永久丢失评估机会。

### 快照 → 后台 → 回写

```
锁内：snapshot Vec<NpcAffectionView> + Vec<GameLine>（尾部 recent_window 行）→ 释放锁
 ↓  tauri::async_runtime::spawn（不阻塞本轮回复的呈现）
锁外：god.evaluate_affection(&lines, &npcs)
       → build_affection_prompt → complete_with_tools([update_affection], "auto")
       → 逐条 parse_affection_update，按 npcs 过滤掉不在场的 role_id
 ↓
锁内：对每条 adjustment
       ① role_manager.adjust_affection(role_id, deltas, negative_deltas) → (AffectionVector, NegativeVector)
       ② gs.set_variable("affection.{role_id}", state_to_value(...))    ← 持久化进存档全局变量
       ③ app.emit("affection:changed", payload)                         ← 前端刷新
       ④ describe_change_for_line(...) → add_line(旁白)                  ← 进 line_list，随记忆构建入上下文
```

四点值得注意：

- **持久化位置是存档全局变量**（`global_variables["affection.{role_id}"]`），不是角色文件、也不进 `line` 表。读旧档即回到旧档时的感情状态。`GameStatus::get_role` 每次取角色时会用全局变量覆盖角色加载值（`affection/mod.rs` 顶部注释）。
- **`total` 是派生字段**，等于好感六维平均，读写时自动同步；手改它不会生效。
- **旁白台词以 `LineAttribute::User` + `sender_role_id: Some(0)` 写入，不是 `System`**。代码里有一条明确警告：`System` 属性会被记忆构建器**去重丢弃**，切勿使用。文案由 `describe_change_for_line` 生成，**只出现程度词、不出现数值**（`tier_label` / `negative_tier_label`），且以角色名作主语、玩家名作宾语——因为共享台词历史由在场多名角色共读，"你"会指代不明。
- **不做每轮注入**。好感度不塞进 `role.memory`，只以旁白形式躺在 `line_list` 里，靠记忆构建自然进入后续上下文。这样每次思维链不会都背着情感状态。

### 评估 prompt 的差异

`build_affection_prompt` 与决策 prompt 同构（system 任务说明 + user 数据），但：

| 项          | 决策 prompt                      | 好感度 prompt                                                                        |
| ----------- | -------------------------------- | ------------------------------------------------------------------------------------ |
| 角色块字段  | 名 / 副标题 / 设定               | 名 / 副标题 / 设定 + **好感六维数值** + **负面六维数值**                             |
| 对话行格式  | `[role_id=N] 名字: 【情绪】内容` | `名字: 内容`（无 role_id、无情绪）                                                   |
| system 内容 | 导演任务                         | 情感观察员任务 + 12 个维度释义 + 评估原则（日常 ±1~2、明显 ±3、深刻 ±4~5、宁少勿多） |
| 工具        | `select_next_speaker`            | `update_affection`（每个有变化的角色调一次）                                         |

12 个维度定义在 `ai_service/types.rs` 的 `AffectionVector::DIMENSIONS`（好感：好感/信赖/亲密/默契/兴趣/思念）与 `NegativeVector::DIMENSIONS`（负面：愤怒/受伤/失望/冷漠/嫉妒/疏远）。

---

## 6. 工具与容错解析

`tools.rs` 是两个工具的定义 + 解析，没有执行体——**上帝 Agent 的工具从不"执行"，模型返回的 `tool_call` 参数就是最终答案**。这是它与 `tools/` 通用工具子系统的根本差别。

两家模型（尤其国产模型）在 function calling 上普遍不可靠，所以解析层做了大量归一化：

| 容错点                                           | 处理                                                                               |
| ------------------------------------------------ | ---------------------------------------------------------------------------------- |
| `arguments` 是嵌套的 `{"arguments": {...}}`      | `parse_tool_args`（`types.rs`）拆壳                                                |
| `arguments` 是被**双重编码**的 JSON 字符串       | 同上，二次解析                                                                     |
| `arguments` 是非法 JSON                          | 同上，尽力而为                                                                     |
| `role_id` 输出成字符串 `"6"` / `" 6 "` / `"6.0"` | `parse_role_id` 依次尝试 `as_i64` → `as_f64` → 字符串 parse i64 → 字符串 parse f64 |
| 维度增量输出成浮点或字符串                       | `parse_delta` 同上                                                                 |
| `deltas` 是双编码字符串                          | `parse_deltas_object` 检测非 object 时再 `parse_tool_args`                         |
| 维度名不认识 / 增量为 0                          | 静默丢弃（按 `DIMENSIONS` 白名单过滤）                                             |
| 增量超出 ±5                                      | `clamp(-5, 5)`                                                                     |
| `update_affection` 的 `role_id == 0`             | 直接返回 `None`（玩家没有好感度）                                                  |
| 两个增量对象都空                                 | 返回 `None`，不产生调整                                                            |
| 调整了不在场的角色                               | `warn` + 丢弃（发言者选择这条**是报错**，不是丢弃）                                |

解析失败一律降级为"这次什么都不做"，绝不 panic。

---

## 7. 谁接入了上帝 Agent

`GeneratorDeps.god_agent` 在 5 个调用点被填充，**只有 3 个传了 `Some`**：

| 调用点                                                      | 来源            | `god_agent` | 后果                                     |
| ----------------------------------------------------------- | --------------- | ----------- | ---------------------------------------- |
| `api/chat.rs::send_chat_message`                            | `UserChat`      | ✅ `Some`   | 主要战场：多人自由对话                   |
| `api/chat.rs::trigger_ai_response`                          | `Proactive`     | ✅ `Some`   | 主动对话也能编排                         |
| `api/game.rs::notify_player_entry`                          | `EntryGreeting` | ✅ `Some`   | 入场问候也走编排                         |
| `ai_service/proactive_system/mod.rs::deliver`               | `Proactive`     | ❌ `None`   | 定时主动搭话**不**编排                   |
| `script_engine/events/{ai_dialogue,free_dialogue}_event.rs` | 剧本            | ❌ `None`   | 剧本内自由对话由剧本自己的事件机控制轮次 |
| `tools/background_command.rs`                               | —               | ❌ `None`   | 后台命令行不编排                         |

剧本侧虽然传了 `None`，`should_activate` 里的 `script_status.is_none()` 仍是第二道保险。

---

## 8. 前端契约

两个事件，都是 `app.emit` 单向广播：

**`character:switch`** —— 由 `generator.rs::emit_character_switch` 发起，payload `{ type, roleId, characterName }`。

`src/api/tauri-events.ts` 中的处理：`getOrCreateGameRole(roleId)` 确保角色数据已加载 → 设 `gameStore.currentInteractRoleId` → **仅当该角色不在 `presentRoleIds` 时才把舞台替换为它**（多人场景下上帝 Agent 只会选在场角色，本分支不进；用替换而非 push 是为了避免标准模式舞台出现两个角色）→ 同步 `uiStore.showCharacterTitle` / `showCharacterSubtitle`。

**`affection:changed`** —— 由 `maybe_evaluate_affection` 后台任务发起，payload 是 `AffectionChangedPayload`（`affection/mod.rs`）：`role_id` / `deltas` / `negative_deltas` / `values` / `negative` / `average` / `reason`。

前端把 `values` / `negative` 写回 `gameStore.gameRoles[role_id]`，并记 `gameStore.lastAffectionChange`（含 `deltaSum`、维度明细、理由、时间戳）供好感度面板展示"最近一次评估的变化与理由"。面板本身在 `components/tools/AffectionPanel.vue`（日程式居中弹窗 + 六边形雷达图，数值允许溢出）。

> 好感度也有一个 `api/affection.rs` 命令用于初始化/兜底刷新，但**变更的唯一主动来源是事件**（`api/affection.rs` 顶部注释）。

---

## 9. 失败模式与已知取舍

| 现象                                                  | 原因                                                          | 影响                                         |
| ----------------------------------------------------- | ------------------------------------------------------------- | -------------------------------------------- |
| 日志 `上帝Agent 选择了不在场的角色 N`                 | 模型幻觉，或在窗口内看到了已离场角色的台词                    | 该轮无人被点名，编排放弃                     |
| 日志 `上帝Agent 未调用工具，返回文本: …`              | 模型不支持 / 忽略 function calling                            | 同上；把 provider 换成支持工具调用的模型     |
| Codex 返回 400 `One of input or ... must be provided` | prompt 退化成了单条 system 消息                               | 检查是否把 user 载荷挪进了 system（见 §3.2） |
| 多人对话完全不动                                      | `resolve_god_agent_provider` 返回了 `None`                    | 查启动日志 `上帝Agent 未找到可用 LLM`        |
| 好感度从不变化                                        | 总开关关 / 剧本模式 / 真实对话不足 interval 段 / NPC 快照为空 | 对照 §5 的 5 个条件                          |
| 好感度变了但角色语气没变                              | 旁白台词是异步写入的，且要等记忆构建把它带进上下文            | 正常延迟，非 bug                             |

已成文的取舍（不建议"顺手修"）：

- **窗口硬截断**，没有摘要补偿——有意保持常数上下文。若要长程意识，正确做法是复用角色的 MemoryBank，而不是把窗口调大。
- **无重试**——决策失败就放弃本轮，靠用户下一条消息自然恢复。加重试会放大延迟与 token 消耗。
- **`decide_next_speaker` 持 `GameStatus` 锁跨 await**——见 §3.4，是已知的粗粒度点，但改动会影响调用方的锁序，需谨慎。
- **`max_consecutive_npc` 等三个键没有 UI**——只有 `affection.enabled` 进了设置树（`config/tree.rs`）。要暴露它们，在 `tree.rs` 里补 `ConfigSetting` 即可，`load` 侧不用改。

---

## 10. 扩展点

**加一个新工具**（例如"判断是否需要切换 BGM"）：

1. `tools.rs` 里写 `xxx_tool() -> ToolDefinition`（`ToolDefinition::new(name, description, 参数 JSON Schema)`）；
2. 写一个 `parse_xxx(&ToolCall) -> Option<T>`，照抄 `parse_role_id` / `parse_delta` 的容错风格；
3. `core.rs` 里加一个 `build_xxx_prompt` + `pub async fn do_xxx(...)`，`complete_with_tools(&messages, &[xxx_tool()], Some("auto"))`；
4. 决定它在 `generator.rs` 里的挂载点（pre / post / 循环外后台）。

**加一个评估维度**：改 `types.rs` 的 `AffectionVector::DIMENSIONS` / `NegativeVector::DIMENSIONS`，并同步 `tools.rs::update_affection_tool` 的 JSON Schema properties 与 `core.rs::build_affection_prompt` 的维度释义文案。前端 `AffectionVector` 类型与雷达图维度也要跟上。

**换编排模型**：不用改代码，在 LLM 设置页给"上帝Agent"角色分配另一个 provider（或改 `llm.god_agent_provider_id`），`switch_llm` 会热切换槽位。
