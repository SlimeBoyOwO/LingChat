# 永久记忆（MemoryBank）运行时实现说明

> 阅读对象：要改、要调或要 debug 永久记忆的人。
> 代码基准：`dev` 分支。行号均为 `dev` 工作树行号（`persistent_memory_system.rs` 比 `origin/main` 少 5 行，差的是被删掉的 `history_revision_for_test`）。
> 配套图（黑白 SVG，直接浏览器打开）：[01 结构图](01-structure.html) · [02 触发判定流程](02-trigger-flow.html) · [03 一次压缩周期的时序](03-sequence.html) · [04 上下文组装数据流](04-context-flow.html)

---

## 1. 它解决什么问题

LLM 的上下文窗口有限。永久记忆做的是：**把超出窗口的对话，用 LLM 压缩成四段摘要文本长期保存，然后在每一轮对话里重新注入**。

四段（`GameMemoryBankData`，`ai_service/types.rs:265-281`）：

| 字段         | 提示词角色                             | 注入位置                                      |
| ------------ | -------------------------------------- | --------------------------------------------- |
| `short_term` | 【短期上下文摘要】承接话题，100–200 字 | 作为第一条 user 消息的前缀（`【近期回顾】…`） |
| `long_term`  | 【角色经历编年史】里程碑事件           | system 消息里的 `【长期经历】`                |
| `user_info`  | 【taの画像】玩家的姓名/职业/喜好/雷点  | system 消息里的 `【taの信息】`                |
| `promises`   | 【待办与契约清单】约定与完成核销       | system 消息里的 `【重要约定】`                |

外加两个元数据（`GameMemoryBankMeta`，`types.rs:257-262`）：

- `last_processed_global_idx`：**压缩指针**——已归档到哪一条台词（`line_list` 的全局下标）。
- `updated_at`：最后一次更新时间。

压缩是**触发式**的，不是定时的：每追加一条台词检查一次，攒够 `update_interval`（默认 250）条"该角色可见台词"就在后台压一次。

---

## 2. 所有权：唯一真源在哪

这是理解这套代码的第一前提（见 [01 结构图](01-structure.html)）：

```
PersistentMemorySystem.memory_bank   ← 唯一真源（Arc<Mutex<GameMemoryBank>>）
        │  sync_to_role() 非阻塞复制
        ▼
GameRole.memory_bank                 ← 副本，供存档落库 / 其它读取方
GameRole.memory: Vec<LlmMessage>     ← 组装好、待发给 LLM 的消息
```

- 每个**角色**一个 `PersistentMemorySystem` 实例，由 `GameRoleManager.memory_bank_systems: HashMap<i32, PersistentMemorySystem>` 持有（`role_manager.rs:30` 附近）。
- 压缩结果不回写 `line_list`，只更新 bank 与指针。
- `has_pending` 是"有未同步结果"的标志；下一次 `sync_memories` 会调 `sync_to_role(role)`（`persistent_memory_system.rs:324-332`）把真源复制进副本。

所有人都通过 `role_manager` 拿这些能力，`persistent_memory_system` 自己不做任何 DB 读写。

---

## 3. 一次 `add_line` 触发的完整调用链

```
GameStatus::add_line            game_status.rs:104-115
  ├─ line_list.push(GameLine)         （perceived_role_ids = 当前在场角色快照）
  └─ refresh_memories()
       └─ GameRoleManager::sync_memories(db, &line_list, None)      role_manager.rs:261
            ├─ 收集 involved_ids：本句的 sender + 全部 perceived_role_ids（跳过 0=玩家）
            └─ 对每个角色 rid：
                 ① ensure_memory_bank_system(rid, …)                role_manager.rs:300 / 382
                 ② sync_to_role(role)        ← 取上次后台结果      role_manager.rs:317
                 ③ check_and_trigger_auto_update(source_lines)     role_manager.rs:319  【同步】
                 ④ get_slice_start_index / get_system_memory_text / get_short_term_user_text
                 ⑤ 裁剪窗口 + 人设兜底                              role_manager.rs:331-346
                 ⑥ MemoryBuilder::build → merge_memory_bank_into_context
                 ⑦ 写入 role.memory
```

几个必须知道的事实：

1. **`recent_n` 是死参数**：唯一调用点 `game_status.rs:111-115` 恒传 `None`，所以 `source_lines` == 整个 `line_list`（`role_manager.rs:267-270`）。
2. **每条台词都会跑一遍**（不只是新角色），所以 `check_and_trigger_auto_update` 是热路径——这也是它所有日志都必须是 `debug` 级的原因（默认过滤器是 `warn,ling_chat_lib=info`，见 `app/logging.rs:29-30`）。
3. **触发判定在持有 `GameStatus` 锁的主线程里同步执行**，只有四段 LLM 调用被甩到后台。
4. **同一个角色可能因为"被感知"而进入 `involved_ids`**，即使它从没说过话（`perceived_role_ids` 来自 `add_line` 那一刻的在场角色快照）。

---

## 4. 触发判定：`check_and_trigger_auto_update`

完整流程图见 **[02 触发判定流程](02-trigger-flow.html)**。它是一条纯判定链，共 **8 个提前 return 点**，全部在 `persistent_memory_system.rs:349-423`：

| #   | 行号       | 条件                                 | 不满足时的行为                          | 当前日志 |
| --- | ---------- | ------------------------------------ | --------------------------------------- | -------- |
| A   | `:350`     | `enabled`                            | return                                  | 无       |
| B   | `:354`     | `!is_updating`                       | return                                  | 无       |
| C   | `:360-365` | 不在 60s 失败冷却内                  | return                                  | 无       |
| D   | `:373`     | `memory_bank.try_lock()` 成功        | return                                  | 无       |
| E   | `:378-384` | 指针未越界                           | **写回指针 = 0**，继续                  | 无       |
| F   | `:394`     | `visible_count >= update_interval`   | return                                  | 无       |
| G   | `:398-406` | `chat_text` 非空                     | **把指针推进到 `target_idx`** 后 return | 无       |
| H   | `:415-421` | CAS `is_updating: false → true` 成功 | return                                  | 无       |

唯一的正向信号是 `:408` 的 `info!("…触发自动压缩…")`——注意它打在 **CAS 之前**，所以理论上存在"日志说触发了、实际被 CAS 挡回"的误导窗口（单线程调用下极难发生，但它是第 9 个静默点）。

### 4.1 阈值与可见性口径（最容易误解的地方）

```rust
fn line_visible_to_role(line, role_id) -> bool {          // :667-673
    attribute != System
      && content.trim() != ""
      && (sender_role_id == Some(role_id) || perceived_role_ids.contains(&role_id))
}
```

- `visible_count` 统计的是**该角色可见、非 system、内容非空**的台词条数（`:631-634`）。
- **单位不是"消息条数"**：别的角色的台词、`System` 行、该角色感知不到的台词都不计数。设置项文案是「触发摘要的可见台词数（1–10000，默认 250）」，用户按"250 条消息"理解时体感会差很多（多角色剧本尤其明显）。
- `recent_window`（默认 30，「压缩后保留的角色可见台词数」）**用的是同一套可见性口径**——`get_slice_start_index`（`:265-290`）也是从指针处往回数"可见台词"。

### 4.2 指针语义

- 指针是 `line_list` 的**全局下标**，不是"已处理条数"。
- 撤回/读档会让 `line_list` 变短 ⇒ 指针越界 ⇒ 判定 E 把它写回 0（并置 `has_pending`，让副本跟着刷新）。若不重置，`get_slice_start_index` 会一直返回过期大下标，窗口无限膨胀。

### 4.3 判定 G 的特殊性

只有它**既不压缩、又推进指针**：当"可见行够 250 条"但 `MemoryBuilder` 产出的正文全为空时（例如窗口里只剩 System 行与孤儿 tool 行、正文被 `sanitize_tool_pairing` 剪光），整个窗口会被静默跳过。原注释说是"区间对该角色完全不可见，直接移动指针避免无限触发"——出发点是防重复触发，但**它没有任何日志**，是整条链上最隐蔽的一条。

---

## 5. 后台压缩任务

`spawn_background_update`（`:427-576`），见 [03 时序图](03-sequence.html)：

```
tokio::spawn:
  0. UpdatingGuard 构造（Drop ⇒ is_updating = false）
  1. slot_snapshot(&llm)  取当前客户端（支持热切换）——为空 ⇒ warn + record_failure
  2. tokio::join! 四段并发 update_section(short/long/user/promises)
       · 每段 = 一段提示词 + 【旧内容】(按 section_limits 截断) + 【新增对话】
       · LLM 返回空串 ⇒ 视为失败（`:616-620`），避免"写回空内容 + 推进指针"静默丢对话
  3. 任一段 Err ⇒ warn + record_failure，指针不动，整批下轮重试
  4. 复核 history_revision 未变（`:540-546`）——变了就丢弃结果（不写回、不推进指针、**也不记失败**）
  5. commit_update_if_current（`:552-568`，在 commit_gate 内再复核一次）
  6. 成功 ⇒ 写回四段 + 指针 = target_idx + 清失败状态 + has_pending = true
```

四段**必须全部成功**才提交。失败会记 `last_failure_at_ms` 与 `fail_count`，60s 冷却（判定 C）内不再触发。

> 时区提示：`update_section` 的 `_ai_name` 形参（`:586`）目前未被使用——压缩提示词里**没有告诉模型角色的名字**，但提示词规则却要求"用（本AI角色的名字）第三人称"。摘要张冠李戴是"越推越错"的一个合理来源。

---

## 6. 上下文组装

完整数据流见 **[04 上下文组装数据流](04-context-flow.html)**。要点：

1. **窗口裁剪**：`sliced = source_lines[slice_start..]`，`slice_start` 由指针 + `recent_window` 决定。
2. **人设兜底**（`role_manager.rs:337-346`）：人设只是 `line_list` 里一条 `System` 行（`sender_role_id == 本角色`）。若切片里没有，就从**全量历史**里找回来插到首位；**两者都找不到时只打一条 warn 就继续**——此时 `MemoryBuilder` 产出的消息里没有 system 项，`merge_memory_bank_into_context`（`:615-628`）会新插一条**纯记忆库**的 system，于是上下文里"只剩长期记忆内容"。
   - 会产生"历史里根本没有这一行"的路径：剧本 NPC 的 `system_prompt` 为空（`script_manager.rs:425` 只在非空时写行，这是**官方认可的合法配置**，见 `validate.rs:739-747`）、只被 `perceived_role_ids` 牵进来的角色、撤回时 `truncate(idx)` 切掉中途加入角色的人设行（`api/chat.rs:310-330`）、读档 `load_lines` 整体覆盖历史。
3. **`MemoryBuilder::build`**（`memory_builder.rs:87-292`）：按角色视角重建——System 行只取本角色第一条、自己的 assistant 行与他人台词分块交替、`assistant(tool_calls)`/`tool` 行原样转消息、感知不到的行跳过。**无状态**，每次重建。
4. **`normalize_window_head`**（`:302-331`）：裁到首条 `user`；窗口内没有 user（工具轮挤满窗口、剧本回合）时裁到首条 assistant，并按 `memory_inject_continue_user` 决定是否补一条 user「继续」（Gemini 要求首条为 user）。**`cut` 之前只保留 system 消息**，其余前导消息丢弃；`cut` 之后全量保留（包括 system）。
5. **`sanitize_tool_pairing`**（`:340-386`，上游 #774）：剪掉没有结果回应的 `tool_calls`、以及没有声明者的孤儿 `tool` 消息——窗口裁剪可能把 `assistant(tool_calls) → tool` 拦腰截断，provider 会直接 400。
6. **`merge_memory_bank_into_context`**（`role_manager.rs:608-660`）：`system_addendum` 追加到首条 system 末尾（首条不是 system 就新插一条）；`short_term_prefix` 前拼到首条 user；最后把相邻 system 合并。注意 `get_system_memory_text()`（`:294-307`）**恒返回非空**（空库也带 `====== 记忆库 ======` 边框），所以 `use_mb` 几乎恒为真。

---

## 7. 并发与一致性：四个原语

| 原语               | 类型                        | 作用                                                  | 谁写                                                     |
| ------------------ | --------------------------- | ----------------------------------------------------- | -------------------------------------------------------- |
| `is_updating`      | `Arc<AtomicBool>`           | 单飞：同一角色同时只有一个压缩任务                    | CAS 置 true（`:415`）；`UpdatingGuard::drop` 置 false    |
| `history_revision` | `Arc<AtomicU64>`            | 历史版本号；后台任务提交前必须仍匹配                  | `invalidate_history()`（`:253-259`）                     |
| `commit_gate`      | `Arc<std::sync::Mutex<()>>` | 把"版本校验 + 提交/记失败"放进同一临界区，封掉 TOCTOU | `commit_update_if_current` / `record_failure_if_current` |
| `has_pending`      | `Arc<AtomicBool>`           | "有未同步结果"，驱动 `sync_to_role`                   | 提交成功 / 指针修正 / 判定 G                             |

**什么时候会 `invalidate_history()`**（`role_manager.rs:373-377` 遍历所有运行时）——全部是"历史被重写"的场景：

- `api/chat.rs:326` 撤回玩家消息（`truncate`）
- `service.rs:180` 清空游戏状态、`service.rs:201` 读档替换历史
- `generator.rs:297` 就地改写 user 行（temp 段清理）、`generator.rs:575` 工具消息回填（中间插入）
- `api/script_editor/commands.rs:1568/1627` 剧本试玩

**普通追加（`add_line`）不会改版本号**——所以"聊天时压缩被新台词作废、指针永不前进"是不成立的（这一点在排查"压缩不触发"时经常被误判，特此写明）。

`UpdatingGuard`（`:158-165`）覆盖正常返回、`?` 提前返回、panic 展开、以及被 drop 的 future。唯一空白在 `is_updating` 的 CAS（`:415`）与 guard 构造（`:447`，在 spawn 出来的 future 首次被 poll 时）之间的窗口：若 spawn 失败或 future 未被 poll 即被丢弃，标志会永久停留在 `true`。

---

## 8. 失败与重试

- 失败 = 槽位为空 / 任一段 LLM 报错 / 任一段返回空串。
- 失败**不推进指针**（下轮重试同一批），但记 60s 冷却（`RETRY_COOLDOWN_MS`，`:15`），避免 LLM 故障期间每轮白打 4 个请求。
- 过期的失败**不污染**新会话的冷却状态（`record_failure_if_current` 会先复核 revision，`:167-183`）。
- 结果过期（revision 变了）**不算失败**：不写回、不推进指针、不进冷却。

---

## 9. 观测面（debug 时先看这里）

| 想确认                           | 手段                                                                                                                                                                                                                                                                                                 |
| -------------------------------- | ---------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| 压缩到底有没有被触发             | 找 `MemoryBank: role_id=N 累积未归档可见台词 X 条 (阈值 250)，触发自动压缩...`（`:408`，info）。**它从不出现 ⇒ 断点在 A~G 之间**                                                                                                                                                                     |
| 压缩有没有成功落库               | `记忆库更新完成! 指针已移动至 N`（`:570`，info）                                                                                                                                                                                                                                                     |
| 失败与冷却                       | `分段压缩失败 (key=…)`（`:528`，warn）、`LLM 槽位为空`（`:468`，warn）                                                                                                                                                                                                                               |
| 结果被作废                       | `历史版本已变化，丢弃过期压缩结果`（`:541`/`:563`，info）                                                                                                                                                                                                                                            |
| 人设有没有丢                     | `没有找到 SYSTEM 属性的台词，可能人设丢失`（`role_manager.rs:344`，warn）                                                                                                                                                                                                                            |
| 真正发给 LLM 的上下文            | 打开 `log.llm_request_body`（`config/keys.rs:140`），每次请求落盘为 `data/log/llm/{时间戳}_{provider}_{序号}.json`（`utils/llm_request_logger.rs`）——**排查人设/记忆注入问题最直接的证据**                                                                                                           |
| 补日志                           | `RUST_LOG=ling_chat_lib=debug`                                                                                                                                                                                                                                                                       |
| **界面里直接看**（推荐先看这个） | **高级设置 → 菜单 → 永久记忆调试**：选角色即可看到存储真源 / 注入视图 / 真实上下文 / 触发进度（指针、累积可见台词数 vs 阈值、是否压缩中、冷却剩余、失败次数）。它是只读的——不触发压缩、不写回记忆库。实现见 `src-tauri/src/api/memory.rs` 与 `src/components/settings/pages/SettingsMemoryDebug.vue` |

**当前的观测缺口**：判定 A–H 全部无日志（或仅 debug），所以"阈值 250 却不触发"在日志上与"卡在 `is_updating`"、"卡在冷却"、"没攒够可见台词"完全无法区分。排查时建议先在 `:350 / :354 / :363 / :394` 四处临时加 `debug!`（打印 `visible_count`、`update_interval`、`last_idx`、`enabled`、剩余冷却），跑一次就能把断点锁定。

---

## 10. 已知边界与坑

1. **配置不可热更新**：`enabled` / `update_interval` / `recent_window` / `section_limits` 只在 `PersistentMemorySystem::new` 时写入，构造后没有 setter，`GameRoleManager` 侧同样只在构造时取 `AppConfig`。前端文案与启动日志都写明"记忆设置需重启生效"，与代码一致。
2. **运行时可能一整个会话不存在**：`ensure_memory_bank_system`（`role_manager.rs:382-404`）在 **LLM 槽位为空时直接 return、不插入运行时**。`sync_memories` 每次都会重试，所以槽位就绪后能补上；但若槽位始终为空（未配置 LLM），该角色的记忆压缩与注入**全程不会发生**，只留一条 warn。
3. **"人设丢失"不是记忆库的错**：记忆库只是"没找到人设时占了 system 位"。根因在"历史里没有这条 System 行"（见 §6.2）。官方对空人设 NPC 的立场是"合法配置"（`validate.rs:739-747`），所以这条路径需要单独决策。
4. **指针推进与丢弃的关系**：判定 G 会推指针（哪怕没压缩）；失败不会。两者混在一起时，"指针到哪了"不等于"压缩到哪了"，排查时要同时看指针与 `updated_at`。
5. **压缩提示词不含角色名**：`update_section` 的 `_ai_name` 未使用（`:586`）。
6. **`section_limits` 截断会丢内容**：旧内容按上限截断后才喂给 LLM，超出上限的尾部会在本次压缩写回后**永久丢失**（超限时有 warn，`:598-605`）。0 = 不截断。

---

## 11. 测试现状

`persistent_memory_system.rs` 末尾有 `#[cfg(test)] mod tests`（`:683` 起，9 个用例），覆盖：

- `get_slice_start_index` 的窗口语义（可见性口径、历史过短回退、`recent_window = 0`）
- 指针越界修正会置 `has_pending`
- `invalidate_history` 在压缩中递增版本
- 过期结果既不提交也不污染冷却
- 正常结果原子提交四段 + 指针
- `UpdatingGuard` 总是释放标志

**空白**：没有任何用例驱动 `check_and_trigger_auto_update` 走到"达阈值 → spawn"，因为测试辅助 `system()`（`:702-716`）恒传 `llm = Arc::new(RwLock::new(None))` 且调用处传 `&[]`。要覆盖触发链，需要给 `LlmSlot` 装一个桩 provider（`LlmClient::new` 与 `LlmProvider` trait 都是 `pub`，可注入）。

另：`.github/workflows/*` 里没有任何 `cargo test` 入口，`cargo build` 也不编译 `#[cfg(test)]` 代码——**这些单测在 CI 里连编译门禁都没有**，只能本地 `cargo test --lib` 跑。

---

## 12. 图索引

| 图                                          | 内容                                                         |
| ------------------------------------------- | ------------------------------------------------------------ |
| [01 结构图](01-structure.html)              | 类/组件关系：谁持有谁、哪个是唯一真源                        |
| [02 触发判定流程](02-trigger-flow.html)     | `check_and_trigger_auto_update` 的 8 个判定点 + 后台任务分支 |
| [03 一次压缩周期的时序](03-sequence.html)   | 同步判定 / 异步压缩 / 期间新台词 / 两个作废分支              |
| [04 上下文组装数据流](04-context-flow.html) | 从 `line_list` 到 `Vec<LlmMessage>` 的六步变换               |

## 13. 代码索引

| 位置                                                               | 内容                                                   |
| ------------------------------------------------------------------ | ------------------------------------------------------ |
| `src-tauri/src/ai_service/game_system/persistent_memory_system.rs` | 运行时本体（859 行）：判定、窗口、压缩、并发原语、单测 |
| `src-tauri/src/ai_service/game_system/role_manager.rs:261-368`     | `sync_memories`：触发 → 裁剪 → 组装 → 注入             |
| `src-tauri/src/ai_service/game_system/role_manager.rs:382-490`     | 运行时惰性构造 / 读档恢复                              |
| `src-tauri/src/ai_service/game_system/memory_builder.rs`           | 角色视角消息重建、窗口头部规范化、工具配对清洗         |
| `src-tauri/src/ai_service/game_system/game_status.rs:104-115`      | `add_line` → `refresh_memories` 入口                   |
| `src-tauri/src/ai_service/types.rs:253-305`                        | `GameMemoryBank` / `Meta` / `Data` 定义与默认值        |
| `src-tauri/src/ai_service/service.rs:42-62`                        | 配置从 `AppConfig` 注入 `GameRoleManager`              |
| `src-tauri/src/config/app_config.rs:37-42, 111-119`                | 默认值 250 / 30 与设置项                               |
