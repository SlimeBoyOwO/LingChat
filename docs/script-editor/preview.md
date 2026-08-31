# 预览（试玩）逻辑（preview）

> 试玩隔离时序图见 [diagrams/preview-isolation.html](diagrams/preview-isolation.html)，相关类 UML 见 [diagrams/preview-class.html](diagrams/preview-class.html)。

## 1. 总览：复用真引擎，而不是另写一套预览解释器

`editor_start_preview` 内部先 `rescan`（作者刚存的改动才能生效），然后用**引擎的真实执行路径**跑 —— 语义与正式游玩完全一致，这是当初选「复用真引擎」而不是另写一套预览解释器的理由。

与正式游玩的两点区别：

1. `on_script_end` 传 `completed = false` —— 试玩永远不记通关；
2. 不调用 `handle_adventure_completion` —— 不解锁后续羁绊冒险、不发成就。

因此试玩刻意不用 `execute_script`，而是自己组合它内部那三个 pub 步骤：

```
editor_start_preview
  ├─ editor_rescan_scripts()              // 磁盘状态先进引擎
  ├─ resolve_preview_main_role()          // MAIN 是谁，定不下来就报错
  ├─ PreviewSession::begin()              // 快照 + 按「刚进游戏」搭场子
  ├─ tokio::spawn {
  │    init_script(&script, &ctx)          // 引擎真实路径
  │    run_script(&mut ctx)                // 事件逐条执行
  │    on_script_end(..., completed=false) // 不记通关
  │    apply_pending_restore()             // 自然结束时还原
  │  }
  └─ 返回 { generation }                   // 本轮试玩代号
```

试玩会**真调 LLM**（按 token 计费）。LLM 未配置时，遇到 AI 事件（`ai_dialogue` / `free_dialogue`）**直接终止剧本**而不是静默跳过 —— 这是对「静默失败」的一类修正：自由对话没有 LLM 会陷入「玩家反复输入、永远收不了尾」的死循环。

## 2. 为什么需要三层隔离

试玩是**内嵌在编辑器里**、跑在玩家正在用的**同一个 `GameStatus`** 上的。历史上直接在 `GameStatus` 上跑，两个方向都出问题：

- **往里看**：试玩场次是残缺的。正式游玩前必然走过 `init_game_status()` 的三件事（清空台词表、写入主角的人设 SYSTEM 台词、把主角 `onstage`）。编辑器是独立路由，这三件一件都没做，后果是：立绘不出来（没人在台上）、日志刷「人设丢失」（没有人设台词）、AI 对话在没有人设的上下文里生成。
- **往外看**：试玩会往真实会话里漏东西。剧本跑出来的每一句台词都进了玩家的 `line_list`；背景、音乐、在场角色、`script_status` 全留在原地。退出编辑器回自由对话，看到的就是试玩残留。

所以现在的做法是：**进来时整体备份、按新会话搭好场子、走的时候整体还原**。试玩期间引擎爱怎么改怎么改，出去之后玩家的会话一个字节都没变。三层隔离分别解决三个层面的泄漏：

| 层 | 解决什么 | 手段 |
|---|---|---|
| 后端会话快照 | `GameStatus` 被试玩改写 | `PreviewSession` 快照 / 还原 |
| 代号守卫 | 中止任务的游离写入污染会话 | `preview_generation` 捕获 / 比对 |
| 前端双快照 + 队列 | 浏览器侧独立状态被改写、事件残留 | `PreviewStage` 存 / 还，`eventQueue.clear()` |
| （路由守卫） | MainChat 挂载竞态 | `onBeforeRouteLeave` 阻塞等还原完成 |

## 3. 后端：`PreviewSession` 快照 / 还原

### begin（进入试玩）

```text
① resolve_preview_main_role
   - 羁绊冒险：按 adventure.bound_character_folder 在角色库找
   - 独立剧本：沿用 game_status.main_role_id
   - 两者都拿不到 → 直接报错（避免对着不动的画面猜）

② gs.preview_generation += 1        // 本轮代号
③ 快照（PreviewSession 字段）：
   - line_len: usize                 // 台词表长度（引擎只追加，截回即还原）
   - scene: to_snapshot()            // 背景/音乐/特效/在场角色/全局变量…
   - main_role_id / current_role_id
   - script_status
   - user_name / user_subtitle
④ 按「刚进游戏」搭场子（对齐 init_game_status 三件事）：
   - 清空 present_role_ids / onstage_role_ids → 只留主角 onstage（立绘出现）
   - main_role_id = current_role_id = main_id
   - 玩家名 ← 绑定角色卡 settings.user_name（%player% 替换用）
   - 写主角人设 SYSTEM 台词（缺了 AI 对话没有性格上下文）
⑤ 失败时把快照套回去再报错，不留半场
```

### restore（退出 / 被中止 / 报错）

```text
① gs.preview_generation += 1        // 让上一场游离任务持有的旧代号立即过期
② line_list.truncate(line_len)      // 截掉试玩追加的台词
③ apply_snapshot(scene)             // 场景状态还原
④ 还原 main_role_id / current_role_id / script_status / 玩家名/副标题
⑤ refresh_memories(db)              // 台词表变短了，记忆要按新列表重建
```

### 托管与幂等

`PreviewSession` 不存命令栈上，而是交给 `AppState` 的两个槽：

```
pending_preview_restore: Mutex<Option<PreviewSession>>   // 待还原快照
preview_task:            Mutex<Option<JoinHandle<()>>>    // 试玩任务句柄
```

试玩任务自然结束时调用 `apply_pending_restore`，`editor_stop_preview` 兜底再调一次。`apply_pending_restore` 用 `Option::take()` 实现幂等：**先到者拿走 `Option` 执行还原，后到者拿到 `None` 直接跳过**。这样无论「跑完 / 报错 / 被中止」哪条路，共享 `GameStatus` 都能回到试玩前，不会污染玩家自由对话的上下文。

## 4. 代号守卫：`preview_generation`

这是把「中止后迟到写入」挡在**写入点**的关键。

- `GameStatus.preview_generation: u64` —— 每次试玩「进来备份 / 走时还原」都会递增；自由对话本身不递增，恒等比对，行为不受影响。
- 生成管线（`GeneratorDeps`）在**每次生成前**捕获当前代号：`generation: gs.preview_generation`，并带上 `is_preview: bool`（来自 `ScriptContext`）。
- `add_assistant_line()`（把 assistant 台词写入 `GameStatus` 的入口）先做比对：

```rust
if gs.preview_generation != deps.generation {
    // 本轮生成已过期（试玩被中止后游离任务仍在写），丢弃整条，含记忆同步
    return Ok(());
}
```

典型场景：试玩任务被 `abort()` 后，其内部仍在排空的游离 consumer 任务会带着旧代号继续生成句子。此时 `GameStatus` 可能已还原回自由对话 —— 没有守卫的话，这些试玩台词就会漏进自由对话的上下文与历史。守卫让它们**写不进去**。

## 5. 事件标记：`preview_gen`

后端把写入点守住之后，还有一条**已经 emit 到前端**的路需要拦：被中止任务的流式 `ai:reply` 可能经 IPC 到达前端时，试玩已结束、甚至新一轮试玩已开始。

- `ReplyResponse.preview_gen: Option<u64>` —— 仅 `is_preview` 时 `Some(generation)`，自由对话 / 正式剧本为 `None`（不序列化）。
- 前端 `tauri-events.ts` 的 `isStalePreviewReply`：

```ts
// 判定规则：事件带 previewGen（试玩专用字段）时，
// 仅当「当前在试玩 且 代号与本轮一致」才收；
// 不带该字段的是自由对话/正式剧本回复，永远放行。
function isStalePreviewReply(payload) {
  const gen = payload.previewGen
  if (typeof gen !== 'number') return false
  return !store.previewing || store.previewGeneration !== gen
}
```

## 6. 前端：`PreviewStage` 双份快照

后端已经把 `GameStatus` 整个备份还原了，但**前端这份是独立的一套状态**：立绘在场名单、对话历史、剧情模式标记都只存在于浏览器里，引擎 emit 的事件经 `eventQueue` 直接改它。不管的话，退出编辑器回自由对话，看到的还是试玩留下的立绘和台词（包括「AI 已关闭」那几条占位）。

`PreviewStage` 复用了真实的游戏渲染层（`GameBackground` / `GameRolesStage` / `GameExtraUI` / `GameDialog`）—— 这就是「复用真引擎 + 真渲染层」的兑现点：这四个组件读的是同一份 store，引擎 emit 的事件进来后，表现与正式游玩逐帧一致。

**进入试玩（`previewing` 变 true 的 watch）：**

1. `eventQueue.clear()` —— 清掉上一轮残留事件（如 `show_character`）；
2. `snapshot = captureGameState()` —— 存 `gameStore` 的 `runningScript / presentRoleIds / currentInteractRoleId / mainRoleId / userName / userSubtitle / currentScene / currentLine / currentStatus / dialogHistory / command / gameRoles / initialized`。只存这几个字段而不是整个 `$state`：其余部分试玩不会碰，整份深拷贝反而可能把别处刚改好的东西覆盖回去。`gameRoles` 必须 JSON 深拷贝（含嵌套的 clothes/bodyPart），浅拷贝 `{...role}` 仍会共用嵌套对象；
3. `sceneSnapshot = captureSceneState()` —— 存 `settingsStore.display`（背景、粒子特效，**persist 到 localStorage 的**，不还原会跨会话长期泄漏）+ `uiStore`（过渡时长、BGM 轨与速度、插图、音效、环境音轨、角色标题副标题、台词情绪动作文本）；
4. 清空舞台、注入主角身份（`readiness.mainRoleId`）、预载主角立绘（`getOrCreateGameRole`）、`eventQueue.resume()`。

**退出试玩：**

1. `eventQueue.clear()`（内部把 paused 置回 true）；
2. `restoreGameState(snapshot)` + `restoreSceneState(sceneSnapshot)`；
3. 两个「只清不存」字段：`currentSoundEffect` / `currentAvatarAudio` 是**值变化即播放**的一次性触发型字段，还原成试玩前的路径会误重播，直接清成 `'None'`。

## 7. 停止试玩：`editor_stop_preview`

```text
① 释放 script_channels：choice_tx / input_tx 各 send 空串解除阻塞，choice_allow_free = false
② is_running.store(false)
③ 取 preview_task，abort() 立即中止（不等待 —— 任务可能正阻塞在 LLM 流上）
④ apply_pending_restore() —— 会话立即还原
```

中止后仍在排空的游离流式任务（publisher/consumer）写不进已还原的会话：`restore` 已递增 `preview_generation`，`add_assistant_line` 的守卫丢弃它们的迟到写入；它们 emit 的 `ai:reply` 也带旧 `preview_gen` 代号，前端比对不中即丢弃，不会串进自由对话或下一轮试玩。

前端 `stopPreview` 还有一步：**必须在后端返回之后再 `eventQueue.clear()` 一次**。`PreviewStage` 的 watch 在 `previewing=false` 时已 clear 过一次，但那次早于后端试玩任务收尾 —— 任务在 await 期间还会继续 emit 晚到的占位/旁白事件，它们入队（队列已暂停不处理），等下次进自由对话 resume 时被消费，就串到正常对话的首句。

## 8. 路由守卫：`onBeforeRouteLeave`

离开编辑器前的统一清理由**路由守卫**完成（不再用 `onUnmounted`）：

```text
onBeforeRouteLeave(cleanupBeforeExit)
  ├─ await store.stopPreview()      // 后端还原完成
  ├─ eventQueue.clear()             // 兜底排空迟到事件
  ├─ await store.flushPendingSave() // 待写入改动落盘
  └─ await store.syncEngine()       // rescan，让主菜单立即能看到改动
```

关键点：

- **必须 await 完成才放行导航**。此前清理放在 `onUnmounted`（异步、路由不等待），MainChat 会先挂载并 resume 事件队列 / 读取尚未还原的 line_list，试玩内容就串进自由对话（历史显示 + AI 上下文）。路由守卫能阻塞导航，从根上消除这个竞态；
- 幂等：模块级 `exitCleaned` 标志避免与 ✕ leave() / onUnmounted 重复执行；
- 顺序不能反：先落盘再同步，引擎重扫的是磁盘，没写完就同步等于同步了旧内容。

## 9. 试玩可行性预检

`editor_preview_readiness` 在打开剧本时就算好 `MAIN` 是谁（前端提前显示常驻横幅，而不是等作者点了试玩才报错）：

- `main_role_id` / `main_role_name` —— 前端据此载入立绘、设 `mainRoleId`（不设的话玩家气泡空名、立绘不出现）；
- `user_name` —— 绑定角色卡里写的玩家名，前端显示玩家身份、后端替换 `%player%`；
- `ok = false` 时给出 `reason`（如「剧本绑定的角色不在角色库里」「还没选定主角」）。

`startPreview`（store action）在真正启动前还拦一道：先 `flushPendingSave` + `runValidation`，`errorCount > 0` 就跳校验页拒绝试玩 —— **跑一个已知跑不通的剧本只会浪费作者时间**。另外若上一轮试玩已结束但 `previewing` 仍是 true（试玩界面停留在终场），会先 `stopPreview()` 再开新场，让 `PreviewStage` 的 watch 完整走一遍「快照 / 清理 / 还原」。
