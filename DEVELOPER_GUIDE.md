# LingChat开发者文档

> 本文档面向**开发者**，用于快速上手 LingChat develop 分支，并以**当前仓库源码为准**解释 0.4.0 的“存档/台词表/记忆构建/永久记忆持久化/剧情系统”。
>
> - **后端**：Python（FastAPI + WebSocket）  
> - **前端**：Vue3（Vite + Pinia）  
> - **运行时数据**：`data/`（开发态）+ `data/game_database.db`（SQLite，SQLModel）  
>

---

## 目录

- [1. 项目一眼看懂](#1-项目一眼看懂)
- [2. 快速启动与调试（Windows 优先）](#2-快速启动与调试windows-优先)
- [3. 代码结构速览（你该从哪读起）](#3-代码结构速览你该从哪读起)
- [4. 前后端协议：HTTP API 与 WebSocket](#4-前后端协议http-api-与-websocket)
- [5. 0.4.0 新系统：存档=完整状态容器（核心）](#5-040-新系统存档完整状态容器核心)
- [6. 0.4.0 新系统：台词表与记忆构建](#6-040-新系统台词表line_list与记忆构建memorybuilder)
- [7. 0.4.0 新系统：数据库/持久化（Save/Line/Role/MemoryBank/RunningScript）](#7-040-新系统数据库持久化savelinerolememorybankrunningscript)
- [8. 0.4.0 新系统：剧情系统（ScriptManager/事件系统）](#8-040-新系统剧情系统scriptmanager事件系统)
- [9. 开发常用断点与排错清单](#9-开发常用断点与排错清单)
- [10. 扩展开发指南（新增事件/新增存档状态/接入永久记忆）](#10-扩展开发指南新增事件新增存档状态接入永久记忆)

---

## 1. 项目一眼看懂

### 1.1 “你在跑什么”

- **后端入口**：`main.py` → `ling_chat/main.py`  
- **后端服务**：`ling_chat/api/app_server.py`（FastAPI + Uvicorn）  
- **WS 端点**：`/ws`（由路由统一注册）  
- **前端工程**：`frontend_vue/`（Vite 开发服务器；也可被后端静态托管）  

### 1.2 “0.4.0 的核心变化是什么”

0.4.0 的关键是把“对话历史”拆成更原子的**台词表**，并把“对 LLM 的记忆列表”变成从台词表按角色动态构建：

- **台词表 `GameStatus.line_list`**：保存每一句话的原子信息（说话者是谁、情绪、TTS 文本、音频文件、动作、显示名等），同时记录“被哪些角色感知到”（用于多角色/旁白/剧本介入）。
- **记忆构建 `MemoryBuilder`**：把“某个角色能看到的台词”压成标准的 OpenAI `[{role, content}, ...]` 格式，确保 LLM 输入稳定。
- **存档 `Save`**：不仅保存台词链，还预留/承载“背景、音乐、特效、剧本进度、永久记忆”等状态，让“载入存档=还原完整现场”成为可能。

---

## 2. 快速启动与调试（Windows 优先）

### 2.1 必要环境

- Python：建议 3.12
- Node.js + pnpm：用于 `frontend_vue/`
- TTS/情绪模型：部分功能需要额外模型文件（详见 `docs/develop/dev_guide.md`）

### 2.2 一键启动

- 根目录：`start.bat`
  - 启动后端与前端

### 2.3 手动启动

#### 后端

1. 复制环境文件：把 `.env.example` 复制为 `.env`（至少保证基本键存在）
2. 在仓库根目录运行：

```bash
python main.py
```

默认端口（参考现有文档与前端配置）：

- HTTP：`http://localhost:8765`
- WebSocket：`ws://localhost:8765/ws`

#### 前端

在 `frontend_vue/`：

```bash
pnpm i
pnpm dev
```

前端默认：

- Vite：`http://localhost:5173`
- 代理：`/api -> http://localhost:8765`，`/ws -> ws://localhost:8765`（见 `frontend_vue/vite.config.ts`）

---

## 3. 代码结构速览

### 3.1 顶层目录

- `ling_chat/`：后端 Python 包（API、游戏/对话/剧本引擎、数据库）
- `frontend_vue/`：Vue3 前端（WS 事件队列、Pinia store、各 UI 面板）
- `data/`：开发环境运行时数据目录（会自动生成）
- `docs/`：历史文档/指南（有用，但以源码为准）
- `memory-bank/`：本地上下文笔记（不进 Git）

### 3.2 后端关键入口

- **启动与静态资源复制**：`ling_chat/main.py`
  - 启动时会确保 `data/game_data/` 存在（从 `ling_chat/static/game_data/` 补齐拷贝）。
- **FastAPI app**：`ling_chat/api/app_server.py`
  - lifespan 中会初始化数据库、同步角色等（具体以代码为准）。
- **路由与 WS 注册**：`ling_chat/api/routes_manager.py`（见 `memory-bank/PROJECT_STRUCTURE.md` 的索引描述）
- **AIService 核心**：`ling_chat/core/ai_service/core.py`
  - 持有 `GameStatus`，对话生成 `MessageGenerator`，剧情系统 `ScriptManager`。

### 3.3 前端关键入口

- `frontend_vue/src/main.ts`
  - 建立 WS 连接、初始化事件处理器（EventQueue processors）。
- `frontend_vue/src/api/websocket/*`
  - WS 消息分发、`script-handler.ts` 注册各种 `type` 的处理器并入队。
- `frontend_vue/src/core/events/*`
  - `EventQueue` 串行处理，processor 将 WS 事件落到 UI/Store。

---

## 4. 前后端协议：HTTP API 与 WebSocket

### 4.1 HTTP API（开发常用）

后端路由前缀多为 `/api/v1/...`，前端 axios `baseURL='/api'`（开发态由 Vite proxy 转发）。

常用模块（建议从这些文件入手）：

- **初始化**：`GET /api/v1/chat/info/init`
  - 用于前端进入对话界面时，绑定 WS `client_id` 并取初始 UI/角色信息。
- **角色**：`/api/v1/chat/character/*`
  - 扫描角色、切换角色、获取角色资源等。
- **存档**：`/api/v1/chat/history/*`（见 `ling_chat/api/chat_history.py`）
  - `list/load/create/save/delete`
- **剧本**：`/api/v1/chat/script/*`
  - 获取剧本列表、初始化剧本 UI、加载剧本资源等。

> 更完整的接口清单可参考 `docs/前后端通讯api.md` 与 `memory-bank/API_HTTP.md`，但写代码时以 `ling_chat/api/*` 的真实实现为准。

### 4.2 WebSocket（对话/剧情事件的“主通道”）

- 端点：`ws://localhost:8765/ws`
- 连接建立后服务端会先发：

```json
{ "type": "connection_established", "client_id": "client_<uuid>" }
```

随后主要通信模式：

- Client → Server：`{"type":"message","content":"..."}`  
- Server → Client：`reply/narration/background/music/sound/modify_character/input/error/status_reset/...`

前端处理链路：

- `frontend_vue/src/api/websocket/index.ts`：按 `type` 分发
- `frontend_vue/src/api/websocket/handlers/script-handler.ts`：注册核心 handler，推入 `EventQueue`
- `frontend_vue/src/core/events/processors/*`：逐类处理事件并更新 Store/UI

> WS 字段细节参考 `memory-bank/API_WEBSOCKET.md`，新增/修改 WS type 时务必同步前后端类型定义与 processor。

---

## 5. 0.4.0 新系统：存档=完整状态容器（核心）

### 5.1 运行时状态容器：`GameStatus`

源码：`ling_chat/core/ai_service/game_system/game_status.py`

`GameStatus` 是运行时“共享状态”容器，关键字段（按实际代码）：

- `line_list: list[GameLine]`：台词表（原子历史）
- `role_manager: GameRoleManager`：运行时角色缓存 + 记忆同步
- `current_character: GameRole`：**当前要喂给 LLM 的角色**
- `present_roles: set[GameRole]`：**在场角色集合**（决定“谁能感知台词”）
- `main_role: GameRole`：主角（导入的游戏角色/剧本冒险主角）
- `background/background_music/background_effect`：场景状态（目前 DB 的 `Save.status` 预留持久化）
- `script_status: ScriptStatus | None`：剧本状态（运行剧本时才存在）

### 5.2 “无缝继续上次对话”的当前实现现状

当前的“继续”主要靠：

- 载入存档后把 `line_list` 还原到内存（`AIService.load_lines()`）
- 由 `GameRoleManager.sync_memories()` 立刻从 `line_list` 重建各角色记忆
- 选定主角为 `current_character`（现阶段默认如此，后续可由 `Save.status` 或 `RunningScript` 决定）

> 注意：存档中对“背景/音乐/特效/剧本变量”等的持久化还在逐步补齐（`chat_history.load/create/save` 里也有 TODO 注释）。

---

## 6. 0.4.0 新系统：台词表（line_list）与记忆构建（MemoryBuilder）

### 6.1 台词的“原子结构”：`LineBase / GameLine`

源码：`ling_chat/game_database/models.py`

- `LineBase`：台词基础字段（运行时与 DB 都复用的基类）
  - `content`：文本
  - `attribute`：`user/system/assistant`
  - `sender_role_id`：**统一说话人 role_id**（0.4.0 重点：不再使用 script_role_id）
  - `display_name`：显示名（剧本可同角色不同显示名）
  - `original_emotion/predicted_emotion/tts_content/action_content/audio_file`：运行中生成的附加信息（用于回放/重渲染）
- `GameLine(LineBase)`：运行时台词对象
  - `perceived_role_ids: list[int]`：**本句台词被哪些角色感知到**

### 6.2 “感知列表”的来源：`present_roles`

源码：`GameStatus.add_line()`

当后端把一条 `LineBase` 加入 `GameStatus` 时，会转换成 `GameLine` 并把：

```python
perceived_role_ids = [role.role_id for role in present_roles if role.role_id is not None]
```

写入该句台词。也就是说：

- **当下在场的角色**会“听到/看到”这句台词
- 后续构建记忆时，只有“说过/听到过”的台词才会进入该角色的上下文

### 6.3 从台词表构建“角色记忆”：`GameRoleManager.sync_memories()`

源码：`ling_chat/core/ai_service/game_system/role_manager.py`

同步流程（按实现）：

1. 选取数据源：`source_lines = lines[-recent_n:] if recent_n else lines`
2. 收集涉及角色：包含
   - `sender_role_id`
   - `perceived_role_ids`
3. 对每个涉及角色 `rid`：
   - 获取运行时 `GameRole`（不存在则创建空壳 `GameRole(role_id=rid)`）
   - 从最近台词回填 `display_name`
   - 用 `MemoryBuilder(target_role_id=rid).build(source_lines)` 生成该角色的 `role.memory`

### 6.4 记忆构建算法：`MemoryBuilder`

源码：`ling_chat/core/ai_service/game_system/memory_builder.py`

核心规则（按实现）：

- 一条台词对某角色可见，当且仅当：
  - 该角色是说话者 `sender_role_id == target_role_id`，或
  - 该角色在 `perceived_role_ids` 中
- `system` 台词：只有“可见”才会被写入 memory（`{"role":"system","content":...}`）
- 目标角色自己说的话：合并成 `assistant` 消息，并把附加信息拼进 `content`：
  - `【情绪】正文<TTS>（动作）`
- 其他人说的话（包括旁白/NPC/玩家）：会被归到 `user` 消息，但会把“非 user 的上下文行”包装进大括号 `{...}`：
  - 上下文行格式：`DisplayName: 【情绪】正文<TTS>（动作）`
  - 之后再拼接玩家真实 `user` 文本

这样做的效果是：

- LLM 输入仍是稳定的 `system/user/assistant` 三角色格式
- 但可以在 `user` 内容里携带多角色上下文，不会出现“一个 LLM 维护多个人设”的混乱

---

## 7. 0.4.0 新系统：数据库/持久化（Save/Line/Role/MemoryBank/RunningScript）

### 7.1 数据库位置与初始化

源码：`ling_chat/game_database/database.py`

- SQLite 文件：`data/game_database.db`（开发环境）
- 初始化：`init_db()` → `SQLModel.metadata.create_all(engine)`

> 旧文档中提到的 `chat_system.db` 与 `sqlite3` 原生实现属于历史方案；现在以 `game_database.db`（SQLModel）为准。

### 7.2 关键表（按 `models.py`）

- `Role`：统一角色表
  - `id`：**role_id（唯一标识）**
  - `script_key + script_role_key`：用于剧本角色映射（联合唯一约束）
  - `role_type`：`main/npc/system`
  - `resource_folder`：资源文件夹名（主角在 `game_data/characters`，NPC 在剧本目录）
- `Save`：存档表
  - `last_message_id`：指向 `Line.id` 的“链尾指针”（用于快速回溯）
  - `status`：JSON，预留存储背景/音乐/特效等
  - `running_script_id`：指向 `RunningScript`
  - `main_role_id`：主角 role_id
- `Line`：台词表（DB）
  - `parent_line_id`：链式父指针
  - `sender_role_id`：说话者 role_id
  - `perceived_by`：通过 `LinePerception` 关联“被哪些角色感知”
- `RunningScript`：剧本运行状态（按存档保存）
  - `script_folder`、`current_chapter`、`event_sequence`
  - `variable_info`：JSON（剧本变量）
- `MemoryBank`：永久记忆仓库（按存档+角色）
  - `save_id` + `role_id`
  - `info`：JSON

### 7.3 存档读写：`SaveManager`

源码：`ling_chat/game_database/managers/save_manager.py`

你会最常用到：

- `create_save(user_id, title)`：创建存档
- `sync_lines(save_id, input_lines: list[GameLine])`：把内存台词“智能同步”回 DB（支持分叉裁剪）
- `get_gameline_list(save_id)`：从 DB 读完整链并转为 `GameLine`（带 `perceived_role_ids`）
- `update_save_main_role(save_id, role_id)`：写入主角 role_id
- `update_running_script(save_id, script_data)`：写入/更新剧本进度（目前 API 层尚未完整接入）

### 7.4 永久记忆持久化：`MemoryManager`（重要但“接入 LLM”仍待完善）

源码：`ling_chat/game_database/managers/memory_manager.py`

提供对 `MemoryBank` 表的 CRUD：

- `add_memory(save_id, info, role_id)`
- `get_memories(save_id, role_id=None)`
- `update_memory(memory_id, new_info=None, new_role_id=None)`
- `delete_memory(memory_id)`
- `delete_memories_by_role(save_id, role_id)`（安全：未传 role_id 不会删）

当前实现现状（请以代码为准）：

- `GameRole` 数据结构里已预留 `memory_bank: dict`
- `MessageGenerator.process_message_stream()` 里明确写了 TODO：永久记忆与数据库结合等待重构
- **也就是说：永久记忆（MemoryBank）目前主要是“可持久化的存储能力已具备”，但“如何插入到 LLM 的上下文”还需要你在后续版本按设计接入。**

---

## 8. 0.4.0 新系统：剧情系统（ScriptManager/事件系统）

### 8.1 剧本入口：`ScriptManager`

源码：`ling_chat/core/ai_service/script_engine/script_manager.py`

- 剧本目录：`data/game_data/scripts/`
- 扫描规则：每个剧本文件夹需要 `story_config.yaml`
- `start_script(script_name)`：
  - 写入 `game_status.script_status`
  - 注册/创建剧本角色到 DB（`RoleManager.get_role_by_script_keys()` / `create_role()`）
  - 为每个剧本角色追加一条 `system` 台词（角色设定 prompt）
  - 进入章节循环：`Chapter.run()` → 返回下一章名，直到 `"end"`

### 8.2 章节执行：`Chapter`

源码：`ling_chat/core/ai_service/script_engine/chapter.py`

- 一个章节包含：
  - `events_data: list[dict]`：事件列表（按顺序）
  - `ends_data: dict`：章节结束逻辑（现阶段仍保留 EndsHandler）
- `run()`：
  - 依次执行事件直到完成
  - 调用 `EndsHandler.process_end()` 得到下一章节 key

### 8.3 事件系统：`EventsHandler + EventHandlerLoader + BaseEvent`

源码：

- `ling_chat/core/ai_service/script_engine/events_handler.py`
- `ling_chat/core/ai_service/script_engine/events/events_handler_loader.py`
- `ling_chat/core/ai_service/script_engine/events/base_event.py`

机制：

- `EventHandlerLoader` 会扫描 `events/` 目录下的 `.py`，找出 `BaseEvent` 子类并注册
- 每个事件类通过 `can_handle(event_type)` 声明自己处理的 `type`
- 事件执行时会持有：
  - `game_status`（可修改 `current_character/present_roles/line_list/...`）
  - `script_status`（剧本变量与进度）
  - `client_id`（WS 推送目标客户端）

### 8.4 present_roles / current_character 在剧情里的意义

你需要牢记两点（否则会出现“角色听不到台词/记忆乱”）：

- **`present_roles`**：决定台词的 `perceived_role_ids`，从而决定哪些角色能把这句台词纳入自己的记忆。
  - 典型由 `ModifyCharacterEvent` 控制 `show_character/hide_character`。
- **`current_character`**：决定下一次“要喂给 LLM 的是谁”。
  - 典型由 `DialogueEvent` / `FreeDialogueEvent`（或你自定义事件）在执行前设置。

---

## 9. 开发常用断点与排错清单

### 9.1 后端（推荐断点）

- WS 收消息入口：`ling_chat/api/new_chat_main.py`（见旧文档/索引；以实际文件为准）
- 用户输入进入台词表：`MessageGenerator.process_message_stream()`（`game_status.add_line(user_line)`）
- 台词 → 记忆：`GameStatus.add_line()` → `GameRoleManager.sync_memories()` → `MemoryBuilder.build()`
- 存档同步：`SaveManager.sync_lines()`
- 剧本扫描/启动：`ScriptManager._init_all_scripts()` / `start_script()` / `_register_script_roles()`
- 事件调度：`EventsHandler.process_next_event()` / `BaseEvent.execute()`

### 9.2 你最常遇到的问题

- **WS 连不上**：先确认后端端口、再确认 `/ws` 注册、再看浏览器控制台与后端日志。
- **剧本按钮灰**：`GET /api/v1/chat/script/list` 返回空；检查 `data/game_data/scripts/*/story_config.yaml` 是否存在。
- **角色“听不到”台词**：检查当时 `present_roles` 是否包含该角色（决定 `perceived_role_ids`）。
- **存档载入后角色不对**：当前载入逻辑默认用“存档第一句台词的 sender_role_id”当主角（见 `SaveManager.get_chat_main_character_id()` 与 `chat_history.load`）。

---

## 10. 扩展开发指南（新增事件/新增存档状态/接入永久记忆）

### 10.1 新增一个剧情事件（Event）

1. 在 `ling_chat/core/ai_service/script_engine/events/` 新增 `<xxx>_event.py`
2. 编写 `BaseEvent` 子类，实现：
   - `can_handle(event_type: str) -> bool`
   - `execute()`：修改 `game_status`（可选）并通过 `message_broker.publish(client_id, response.model_dump())` 推送 WS 事件
3. 如果事件要产生日志/可回放信息：务必 `game_status.add_line(LineBase(...))`

### 10.2 新增一种“可存档的游戏状态”

目标：让载入存档时能恢复 UI/场景（背景、BGM、特效、在场角色等）。

建议做法（按现有结构“最少侵入”）：

- 写入：
  - 在 `chat_history.create/save` 的 TODO 处，把 `game_status.background/background_music/...` 写入 `Save.status`（JSON）
  - 必要时同步 `present_roles/current_character` 的标识（建议存 role_id 列表与 current_role_id）
- 读取：
  - 在 `chat_history.load` 的 TODO 处，从 `Save.status` 反向恢复到 `AIService.game_status`
  - 恢复后务必 `sync_memories()`（或确保 `line_list` 已触发刷新）

### 10.3 接入永久记忆（MemoryBank → LLM 上下文）

现在已经具备：

- DB 表：`MemoryBank`
- CRUD：`MemoryManager`
- 运行时结构预留：`GameRole.memory_bank`

但“插入到 LLM context”的位置需要你实现。按 issue 的推荐与当前代码结构，最自然的接入点是二选一：

- **方案 A（推荐）**：在 `GameRoleManager.sync_memories()` 构建 `role.memory` 前，把 `role.memory_bank`（从 DB 读入）融合进 `MemoryBuilder` 的输出（例如作为 system 前置、或作为 user 的背景块）。
- **方案 B**：在 `MessageGenerator.process_message_stream()` 获取 `current_context` 后，把 `memory_bank` 转成若干条 `system/user` 插入到 `current_context` 的合适位置。

无论哪种方案，都建议：

- **明确格式**：`memory_bank` 的 JSON schema 要固定（便于长期迁移与提示词稳定）
- **明确更新策略**：例如“台词超过 N 条→总结前 M 条→写回 MemoryBank.info”
- **避免重复**：不要把同一段长期记忆同时写进台词表与 memory_bank，防止上下文膨胀

---

## 附：高频文件清单

- 后端入口：`main.py`、`ling_chat/main.py`
- AI 核心：`ling_chat/core/ai_service/core.py`
- 台词/记忆：`ling_chat/core/ai_service/game_system/game_status.py`、`role_manager.py`、`memory_builder.py`
- DB：`ling_chat/game_database/database.py`、`models.py`
- 存档：`ling_chat/game_database/managers/save_manager.py`、`ling_chat/api/chat_history.py`
- 永久记忆：`ling_chat/game_database/managers/memory_manager.py`
- 剧本：`ling_chat/core/ai_service/script_engine/script_manager.py`、`chapter.py`、`events_handler.py`、`events/*`

