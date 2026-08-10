# 实现方式（architecture）

> 总览数据流见 [diagrams/architecture.html](diagrams/architecture.html)，事件 Schema UML 见 [diagrams/event-schema.html](diagrams/event-schema.html)。

## 1. 分层结构

PR #540 在 `src-tauri/src/api/script_editor/` 下新增了完整的后端模块，并以前端 `src/components/script-editor/` 与 `src/stores/modules/script-editor/` 承载编辑器 UI。整体分为五层：

```
前端组件层  ScriptEditor.vue + 7 个 script-editor/* 子组件
   │ 读写
前端状态层  Pinia setup store（state / getters / actions）
   │ invoke
前端 API 层  api/services/script-editor.ts（纯封装 + 类型）
   ═══════════ Tauri IPC（invoke / event）═══════════
后端命令层  commands.rs（editor_* 前缀，全部读写唯一入口）
   │ 委托
后端子模块  schema.rs · paths.rs · io.rs · validate.rs
   │
磁盘       data/game_data/scripts/<剧本包>/
```

**分层职责：**

| 模块 | 职责 |
|---|---|
| `paths` | 剧本 key ⇄ 磁盘路径、三种布局枚举、路径穿越防护、名称合法性 |
| `io` | YAML ⇄ JSON、原子写、`.bak` 备份、章节文档归一 |
| `schema` | 16 种事件及其全部字段的**单一真相源**，导出给前端驱动表单 |
| `validate` | 校验器：把引擎里的静默失败变成作者能看见的诊断 |
| `commands` | Tauri 命令层 |

## 2. 设计约束（三条核心）

```text
① 前端只见 JSON。
   YAML 语义只存在于 Rust 一侧，不会出现两套解析行为分歧。
   同时也绕开了 fs 插件 scope 覆盖不到剧本目录的问题。

② 所有写入都是原子的，覆盖前留 .bak。
   原型编辑器是 open(f, "w") 直接截断再写，中途崩溃会把章节清零；
   现在改成「同目录临时文件 → fsync → rename」，崩溃最多留一个 .tmp。

③ 任何来自前端的路径都必须过 paths 的校验，命令层不自己拼路径。
```

其中第 ② 条是 `io.rs::atomic_write` 的实现：临时文件必须与目标**同目录**，否则 `rename` 可能跨设备退化成非原子复制；`backup_if_exists` 只保留最近一份 `.bak`（真正的历史由前端撤销栈负责，`.bak` 只防「写坏了而且已经关掉编辑器」）。

## 3. 三种磁盘布局（paths.rs）

引擎接受三种剧本包布局，编辑器用「剧本 key」指代一个剧本包 —— 即相对 `scripts/` 的路径，统一用 `/` 作分隔符：

| 布局 | 路径 | 段数 | 用途 |
|---|---|---|---|
| `character` | `scripts/character/<角色>/<剧本>/` | 3 | 羁绊冒险（两层） |
| `standalone` | `scripts/standalone/<剧本>/` | 2 | 独立剧本 |
| `flat` | `scripts/<剧本>/` | 1 | 兼容布局 |

示例 key：`character/诺一钦灵/想出去玩啦`、`standalone/我的剧本`。

`enumerate_script_keys` 只把含 `story_config.yaml` 的目录算作剧本包（与 `ScriptManager` 判定一致），并跳过点号开头的目录（`.tmp` 临时文件、旧回收目录残留）。

### 路径安全

`paths::split_key` 是**唯一的入口校验点**：拒绝 `..`、绝对路径、空段、Windows 盘符（`:`）、以及任何平台分隔符（`\`）。`resolve_script_dir` 在 `canonicalize` 之后再校验前缀，防御符号链接逃逸。`resolve_new_script_dir`（新建用）刻意**不建目录** —— 「解析路径」不该有副作用；且早期用 `canonicalize` 校验前缀，在打包版首次创建（`scripts/` 还不存在）时会报 os error 3，已改为词法 `starts_with` 校验。

名称校验分两套：

- `sanitize_folder_name`（目录 / 章节名）：拒绝 `/ \ : * ? " < > |`、控制字符、首尾点号/空格、Windows 保留名（CON/NUL/COM1…，对带扩展名同样生效）、以及 `character`/`standalone` 保留名；
- `sanitize_file_name`（素材文件名）：**按字符数**而非字节数计长（64 字节换成中文只有 21 个字，正常背景图文件名都过不了），上限 120 字符，不需要排除保留目录名。

## 4. Schema —— 单一真相源（schema.rs）

同一份事件 schema 此前散落三处：Rust 的 16 个 handler、前端 `types/script.ts` 的运行时 payload 类型、原型编辑器的 `constants/events.ts`。三者互不同步，直接导致原型产出的 `set_variable` / `chapter_end` 跑不通。

现在 `schema.rs::build_schema()` 是唯一真相源，由 `editor_get_schema` 导出，前端只负责渲染。字段结构：

```
ScriptSchema
├── events: Vec<EventSpec>           16 种事件（5 大类别）
│     EventSpec { type_key, label, category, color, fields }
├── common_fields                    所有事件共有：condition / duration
├── story_config_fields              story_config.yaml 的字段
├── action_types                     choices/set_variable 的 action（add_line / set_var）
├── unlock_condition_types           羁绊冒险解锁条件
├── placeholder_fields               %player% 会被替换的字段名
└── condition_syntax                 条件语法说明（直接展示给作者）
```

**词表的归属（不是所有取值都由 Rust 拥有）：**

| 取值 | 归属 | 说明 |
|---|---|---|
| 情绪 emotion | 前端 `src/controllers/emotion/config.ts` | 决定情绪 → 立绘文件名的映射 |
| 章节名 | 前端从已加载的章节列表填 | 每个剧本自己的 |
| 素材文件名 | 前端从素材索引填 | 剧本内 + 全局 |
| 角色 | `MAIN` + 剧本 `characters/` 下的目录名 | 由后端枚举 |
| 背景特效 | Rust（`KNOWN_EFFECTS` 常量） | 对应前端组件是否存在 |

**字段控件 `FieldKind`：** `text / textarea / number / bool / select / character / emotion / chapter / asset / choice_options / branch_options / var_options / deprecated`。其中 `deprecated` 只展示不可编辑（如遗留字段 `duration` —— 引擎从不读取，但保存时原样保留，不丢数据）。

**强制同步的测试：** `schema_covers_every_registered_event_type` 硬编码引擎注册的 16 种事件，任何一侧增删事件都会让测试失败；另有「字段键唯一」「素材字段必带 asset_kind」「特效下拉来自 KNOWN_EFFECTS」「duration 必须以不可编辑形态出现」「set_variable 只允许 set_var action」等测试。

## 5. 前端后端的分工

- **前端驱动表单**：`getters.eventSpecs` 把事件类型映射到定义；`blankEvent(typeKey)` 按 schema 生成骨架（`chapter_end` 自动补 `next_chapter: end`）；`FieldRow` / `CompositeField` 按 `FieldKind` 渲染 13 种控件（含素材下拉 + 导入、复合列表编辑器、condition 语法帮助）。
- **后端驱动校验**：`validate.rs` 用同一份 schema 做必填 / 未知字段检查（见 [validation.md](validation.md)）。

## 6. 命令清单（commands.rs）

命令统一 `editor_` 前缀，避免与既有 `list_scripts` 混淆。全部写入（章节 / 配置 / 剧本 / 角色 / 素材）都只能走这里 —— 这是 PR 之前完全不存在的入口。

**读：**

| 命令 | 返回 |
|---|---|
| `editor_get_schema` | `ScriptSchema` |
| `editor_list_scripts` | `Vec<ScriptPackage>`（含 `loaded_by_engine` 标记） |
| `editor_read_script` | `ScriptDetail`（package + story_config + chapters + assets + characters） |
| `editor_read_chapter` | `ChapterContent`（events + extra） |
| `editor_validate_script` | `ValidationReport` |
| `editor_list_global_assets` | `AssetIndex`（sound 返回空 —— 全局没有音效目录） |
| `editor_list_asset_files` | `AssetFileIndex`（带绝对路径与体积，供预览） |
| `editor_list_global_characters` | `Vec<GlobalCharacter>`（标出已导入） |
| `editor_preview_readiness` | `PreviewReadiness`（试玩可行性） |

**写：**

| 命令 | 说明 |
|---|---|
| `editor_write_chapter` | 整章 `{name, events, extra}` 落盘 |
| `editor_write_story_config` | 改写 config（警告会丢注释） |
| `editor_create_chapter` | 逐段过 `sanitize_folder_name`；新章节自带一条 `chapter_end`，否则一保存就报「缺少章节结束」 |
| `editor_delete_chapter` | 删除章节文件 |
| `editor_create_script` | 建目录骨架 + config + 开场章节；`characters` 小写（原型建的大写 `Characters`，Linux/Android 上断裂） |
| `editor_delete_script` / `editor_delete_character` | 删除整包 / 角色目录 |
| `editor_upload_asset` | **只收源文件路径**，Rust 自己 `fs::copy`；不用 `plugin-fs` 读字节（不在 scope 内会被拒，64MB 图转数组 IPC 会 OOM） |
| `editor_delete_asset` | 删除素材文件 |
| `editor_create_character` | 显式写 `script_role_key`（缺了它引擎每次启动都新建重复角色） |
| `editor_import_global_character` | **复制 settings.yml 而不是直接引用**：引擎解析 `character:` 只在剧本自己的 `characters/` 里找，全局角色库不在那条路径上；立绘默认不复制（引擎查找顺序本来就先命中全局同名目录） |
| `editor_rescan_scripts` | 增量 merge，不整体替换（见下） |
| `editor_start_preview` / `editor_stop_preview` | 试玩（见 [preview.md](preview.md)） |
| `editor_open_script_folder` | 在系统文件管理器打开剧本目录 |

### 为什么 `rescan` 是增量 merge 而不是整体替换

引擎只在启动时扫一次目录。`editor_rescan_scripts` 刻意做**增量 merge** 而不是整体替换 `script_manager`：

- `ScriptStatus` 里的 `current_chapter_key` / `current_event_process` / `vars` / `running_client_id` 是运行进度，整体替换会把**所有**剧本的进度清零；
- `is_running` 是 `Arc<AtomicBool>`，调用方会先 clone 出来、放掉锁之后才 `store(true)`；整体替换会换掉这个 Arc，让运行中的任务把状态写到一个已被孤立的对象上，之后 `is_running` 永远是 false。

## 7. 一个值得注意的设计取舍

`commands.rs` 里**删掉了一个 `editor_rename_chapter`（改章节文件名）命令**，注释说明了理由：

1. 章节 id 会被别的章节的 `chapter_end.next_chapter` / `next` 以及 `story_config.yaml` 的 `intro_chapter` 引用，只改文件名不重写引用等于悄悄断链；
2. 作者真正想改的是**显示名**（章节 YAML 里的 `name:`），那已经在章节编辑页顶部直接可改。

同理，`editor_reorder_chapters`（拖动章节换序）也删了：章节先后是 `chapter_end` 串出来的，只有纯线性的一段才谈得上顺序；真正天天要调的是**章节内部的事件顺序**（已有拖拽），章节之间的接线应该在「章节结束」事件里显式指定 —— 那里看得见、可校验、可撤销。
