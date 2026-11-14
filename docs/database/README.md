# 数据库 ER 图

数据库是 `data/game_database.db`（SQLite，sea-orm）。本目录用两种表示法描述同一套结构：

| 文件              | 表示法              | 内容                                                                         |
| ----------------- | ------------------- | ---------------------------------------------------------------------------- |
| `er-crowfoot.svg` | 鸦脚（Crow's Foot） | 物理模型：14 张表、全部列、主外键、唯一/检查约束、索引，以及每张表涉及的迁移 |
| `er-chen.svg`     | Chen                | 概念模型：实体、联系、属性、基数；按功能分区，关联表画成菱形                 |
| `gen-er.mjs`      | —                   | 生成脚本，`node docs/database/gen-er.mjs` 重新出图（无第三方依赖）           |

两张图都按**最终状态**绘制，即把 2024-01-01 建表之后所有迁移的加列、重建、删表都合并进去了；列前的黄点表示该列由后续迁移新增。

## 读图约定

- **实线**：SQLite 里真实存在的 `FOREIGN KEY`。
- **虚线**：应用层维护的引用，SQLite 没有约束（迁移里显式不写 FK，级联清理在代码里做）。
- **PK / FK 实心徽标**：物理主键 / 物理外键；**空心徽标**：应用层引用。
- 类型后的 `?`：该列可空。
- Chen 图中矩形是实体，菱形是联系（菱形内小字标注对应的物理表名），椭圆是属性，下划线是主键，虚线矩形是跨分区引用的实体。

## 表和分组

| 分组       | 表                         | 说明                                                        |
| ---------- | -------------------------- | ----------------------------------------------------------- |
| 核心对话   | `role`                     | 角色主数据，剧本角色按 `(script_key, script_role_key)` 唯一 |
|            | `save`                     | 存档，`status` 是 JSON 字符串                               |
|            | `running_script`           | 剧本运行态（变量、章节、事件序号），一个存档一份            |
|            | `line`                     | 台词/消息，含情绪、TTS、思考链、工具轮标识                  |
|            | `line_perception`          | 台词对哪些角色可见（`line` × `role` 的关联表）              |
|            | `memory_bank`              | 持久记忆条目，按存档 + 角色归属                             |
|            | `adventure_unlock`         | 冒险的解锁/完成状态，用文件夹名做业务键                     |
| 身份与认知 | `role_relation`            | 角色 → 角色的认知文本 + 初始好感覆盖（多对多）              |
| ASR 热词   | `role_hotword`             | 角色级热词，`(role_id, text)` 唯一                          |
|            | `global_hotword`           | 全局热词，`text` 唯一                                       |
|            | `hotword_category`         | 词库分类                                                    |
|            | `library_hotword`          | 词库热词，分类内唯一（未分类时全局唯一）                    |
| AI 助手    | `skill_agent_conversation` | 剧本编辑器的助手会话                                        |
|            | `skill_agent_message`      | 会话消息（OpenAI 格式 + 思考链 + token 用量）               |

玩家身份不是单独的表：身份卡（`game_data/player_identities/<目录>/identity.yml`）仍是真源，启动时投影成 `role` 里 `role_type = 'player'` 的行，`role.identity_id` 就是身份卡的稳定键。`role_type` 另外保留 `user`（`id = 0` 的旧台词哨兵，不代表具体身份）。

热词 `weight` 的口径是 `[1, 5] ∪ {50}`：`1..5` 为普通权重，`50` 为超级热词；DDL 只做 `CHECK 1..50` 兜底，离散取值在实体层归一。

## 关系清单

物理外键：

- `save.main_role_id → role.id`，`line.sender_role_id → role.id`，`line.save_id → save.id`
- `line.parent_line_id → line.id`（自引用，台词树）
- `line_perception.(line_id, role_id) → line.id / role.id`
- `memory_bank.(save_id, role_id) → save.id / role.id`
- `running_script.save_id → save.id`
- `role_relation.(role_id, target_role_id) → role.id`（两侧级联删除）

应用层引用（图中虚线）：

- `save.running_script_id → running_script.id`：与 `running_script.save_id` 互为指针，两者构成 1:1
- `save.last_message_id → line.id`
- `save.player_role_id → role.id`：本存档使用的玩家身份；迁移时回填，删除玩家角色前由代码解绑
- `role_hotword.role_id`、`library_hotword.category_id`
- `skill_agent_message.conversation_id`：只有普通索引，没有外键
- `adventure_unlock.character_folder`：对应角色目录名，不是 `role.id`

## 迁移时间线

| 迁移               | 变更                                                                                                                                                                                                                                            |
| ------------------ | ----------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| `m20240101_000001` | 建表：`role`、`save`、`running_script`、`line`、`line_perception`、`memory_bank`、`adventure_unlock`                                                                                                                                            |
| `m20260727_000002` | `line` + `tool_call`                                                                                                                                                                                                                            |
| `m20260729_000002` | `line` + `thinking`                                                                                                                                                                                                                             |
| `m20260803_000001` | 新建 `skill_agent_conversation`、`skill_agent_message`                                                                                                                                                                                          |
| `m20260807_000001` | `skill_agent_message` + `reasoning`                                                                                                                                                                                                             |
| `m20260814_000001` | `skill_agent_message` + `prompt_tokens`、`completion_tokens`                                                                                                                                                                                    |
| `m20260815_000001` | `skill_agent_message` + `cached_tokens`                                                                                                                                                                                                         |
| `m20260918_000001` | 新建 `save_identity`                                                                                                                                                                                                                            |
| `m20261004_000001` | 新建 `role_hotword`                                                                                                                                                                                                                             |
| `m20261005_000001` | 新建 `global_hotword`                                                                                                                                                                                                                           |
| `m20261009_000001` | 新建 `hotword_category`、`library_hotword`                                                                                                                                                                                                      |
| `m20261010_000001` | 三张热词表去掉 `lang` 列并重建（同词异语言归并，不可回滚）                                                                                                                                                                                      |
| `m20261011_000001` | 删除废弃的 `role_asr_config`、`global_asr_config`                                                                                                                                                                                               |
| `m20261012_000001` | 热词 `weight` 归一到 `[1, 5] ∪ {50}`                                                                                                                                                                                                            |
| `m20261015_000001` | `line` + `identity_id`                                                                                                                                                                                                                          |
| `m20261016_000001` | 新建 `player_identity`                                                                                                                                                                                                                          |
| `m20261020_000001` | `line` + `round_id`（工具轮分组）                                                                                                                                                                                                               |
| `m20261023_000001` | `library_hotword` 唯一键从全局 `text` 改为分类内唯一（两个部分唯一索引）                                                                                                                                                                        |
| `m20261030_000001` | 新建 `role_identity_relation`                                                                                                                                                                                                                   |
| `m20261030_000002` | `role_identity_relation` + `initial_affection`                                                                                                                                                                                                  |
| `m20261114_000001` | 玩家身份并入 `role`：加 `identity_id`、`identity_relations_seeded`；新建 `role_relation` 承接旧关系；`save` + `player_role_id`；台词改指玩家角色并删 `identity_id`；删 `player_identity`、`save_identity`、`role_identity_relation`（不可回滚） |

另有一层不走迁移的兼容处理 `db/compat.rs`：识别 Python 后端旧库（存在 `user_info`）后，删除 `user_info`、`adventure_progress`，重建去掉 `user_id` 的 `save` 和 `adventure_unlock`，并统一 `attribute` / `role_type` 的大小写。这段是 0.3.0 之前的历史包袱，不参与新库结构。

已删除的历史结构不会出现在图中：`user_info`、`adventure_progress`、`role_asr_config`、`global_asr_config`、`player_identity`、`save_identity`、`role_identity_relation`，以及 `save.user_id`、`adventure_unlock.user_id`、`line.identity_id` 和三张热词表的 `lang` 列。

## 不在迁移体系内的数据库

`game_database.db` 之外还有一个 sqlite-vec 向量库：`vector_entries` 虚表只有 `id TEXT PRIMARY KEY` 和 `embedding FLOAT[n]` 两列，按「嵌入命名空间 / 角色」分文件存放（`crates/ling-chat-notes`）。它没有迁移框架，表结构由 `VectorStore::open` 按当时维度创建，所以没有画进这两张图。

## 外部真源

数据库不是所有数据的唯一来源，读图时注意：

- 角色、剧本、背景、音乐、日程在 `data/game_data/` 目录；`role` 表只是运行期镜像，`role_sync` 从文件夹同步。
- 玩家身份卡以文件为真源（`name/subtitle/prompt/avatar`），启动时投影成 `role` 里 `role_type = 'player'` 的行，`role.identity_id` 是卡 id。认知关系只在 `role_relation`，`characters/<角色>/relations.yml` 已废弃（历史测试数据，不读不写）；身份卡 YAML 里的 `relations` 只在首次导入时读取一次。
- 笔记、成就、插件数据不进这个库，分别落在数据目录的文件里。
