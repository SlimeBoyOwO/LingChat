---
name: lingchat-script-editor
description: LingChat 剧本编辑器技能。当用户需要为 LingChat 创建新剧本或修改已有剧本（story_config.yaml、Chapters/*.yaml 章节、剧本内角色 settings.yml、Assets 资源引用）时使用。本技能遵循完整剧本创作工作流：创建/修改入口判断 → 剧本类型选择（羁绊冒险/独立剧本）→ 大纲与工程创建 → 内容设计与用户确认 → 逐章撰写与用户确认 → 完成交付（结束剧本、回归自由对话）。字段与事件语法以 LingChat 脚本引擎 Rust 源码为准，并附带可复制的最小可运行模板。
---

# LingChat 剧本编辑器

本技能指导如何为 LingChat 创建或修改剧本（羁绊冒险 / 独立剧本）。所有字段、事件类型、默认值均以脚本引擎 Rust 源码为权威依据，下文为源码的精确映射，

## 何时使用

- 用户要求为 LingChat **创建新剧本**、新章节、新事件、新剧本角色（含完整创作工作流：入口判断 → 剧本类型 → 大纲与工程创建 → 内容设计确认 → 逐章撰写确认 → 完成交付）
- 用户要求 **修改已有剧本**（读取现有 story_config.yaml 与章节，编辑章节/事件/分支/配置）
- 需要遵循 AI 台词设计原则（`prompt` 只做状态/意图提示、不直接写完整台词；必要时用 `dialogue` 固定台词）
- 需要判断某事件类型支持哪些字段、默认值是什么
- 需要生成剧本目录结构、story_config.yaml、章节 YAML 或角色 settings.yml
- 写完或修改完剧本后，用 validate_script 工具做引擎级校验并修复（错误归零后再交付）

## 剧本存放位置与目录结构

剧本放在 `<数据目录>/game_data/scripts/` 下，引擎自动扫描三种位置：

```
game_data/scripts/
├── character/<角色文件夹>/<剧本名>/    # 角色卡羁绊冒险（两级），需在 story_config 写 adventure 配置
│   ├── story_config.yaml
│   ├── Chapters/                      # 章节目录
│   │   ├── 01.yaml
│   │   └── Intro/intro.yaml           # 支持子目录
│   ├── Assets/                        # 可选：媒体资源
│   │   ├── Backgrounds/*.webp|png
│   │   ├── Musics/*.mp3|ogg
│   │   ├── Sounds/*.mp3|ogg
│   │   ├── Pics/*.png
│   │   └── Ambients/*.mp3|ogg
│   └── Characters/                    # 可选：剧本专属 NPC
│       └── <NPC文件夹>/settings.yml
├── standalone/<剧本名>/               # 独立剧本（一级）
└── <剧本名>/                          # 根级（向后兼容）
```

**章节路径规则**（`script_manager.rs`）：`intro_chapter` 与 `chapter_end.next_chapter` 是相对 `Chapters/` 的路径。`main` → `Chapters/main.yaml`；`Intro/intro` → `Chapters/Intro/intro.yaml`；已含 `.yaml` 后缀则直接拼接。`"end"` 是保留字，表示剧本结束（恢复自由对话）。

**最小实现**：仅需 `story_config.yaml` + `Chapters/` 即可运行，Assets/characters 均为可选。

## 创作工作流

重要规则：

- 严格按下述状态机推进。每个「用户确认」门都必须在获得用户明确认可后才能进入下一阶段；用户表示不满意时，返回对应阶段重新修改，直到用户确认。未获得用户确认前，禁止擅自推进到下一阶段。
- 你应该随时提醒自己当前处于哪个阶段，并根据用户的对话实时调整你的阶段在哪。章节必须逐章编写，禁止一次性写完所有章节，你应该在每段工作阶段随时告知用户也告知自己当前是在什么阶段。严格遵循阶段划分执行。

**流程总览**：

- 创建新剧本 → ① 剧本类型 → ② 大纲与工程创建 → ③ 内容设计（确认·第一层）→ ④ 逐章撰写循环（确认·第二层）→ ⑤ 完成与交付
- 修改已有剧本 → 读取并修改内容 → ⑤ 完成与交付

### 0. 入口判断：创建 vs 修改

- **创建新剧本** → 进入「1. 剧本类型与分支走向」。
- **修改已有剧本** → 先用 `read_file` 读取现有剧本（story_config.yaml + Chapters/），按用户提出的修改需求逐一修改内容（改章节/事件/分支/配置）；用户确认修改完成后，再进入「5. 完成与交付」。跳过类型选择、大纲、内容设计、逐章撰写循环。

### 1. 剧本类型与分支走向

- 询问用户剧本类型：**羁绊冒险** 还是 **独立剧本**？
  - **羁绊剧情（角色卡冒险）**：必须要有 `MAIN` 角色，其他角色（NPC）可选；`story_config.yaml` 需写 `adventure` 配置块（见 `references/story-config-reference.md`）。
  - **独立剧本**：可以没有 `MAIN` 角色，也可以有其他角色。
- 【分支 A：羁绊冒险】→ **必须**先读取人物卡（读取角色的性格、设定等）→ 进入「2. 确定故事大纲」。
- 【分支 B：独立剧本】→ 判断是否需要已有角色：
  - **需要** → 读取人物卡（获取已有角色数据）→ 进入「2. 确定故事大纲」。
  - **不需要** → 直接进入「2. 确定故事大纲」。

### 2. 大纲与工程创建

- **确定故事大纲**：基于角色设定或用户的想法，设计故事大纲。若用户没有具体剧情，先引导用户阐明。
- **创建项目工程**：确定文件夹名称，生成 `story_config.yaml`，创建所有所需文件夹（如 `Chapters/`、`Assets/`、`characters/` 等）。落盘技术细节见下方「创建流程」。

### 3. 内容设计与确认循环（用户确认 · 第一层）

- **内容设计**：设计故事内容、章节划分、分支划分；同时与用户确认登场人物与所需 Assets 素材（背景/音乐/音效/图片等），素材不足时按素材调整设计（反复增删改，直到既满足素材又满足剧情需求）。
- 将设计提交给用户确认。
  - 不满意 → 返回「内容设计」，按用户反馈修改。
  - 满意 → 进入「4. 撰写剧本主循环」。

### 4. 撰写剧本主循环（核心迭代 · 按章节推进）

- 设章节索引 i = 1。
- **开始设计第 i 章**：创作当前章节的具体内容（遵循下方「6. 核心剧本设计原则」，并按「创建流程」落盘）。
- 将刚写好的章节提交用户审查（用户确认 · 第二层）。
  - 不满意 → 返回「开始设计第 i 章」，重新修改当前章节。
  - 满意 → 判断是否为最后一章？
    - 否 → i = i + 1，回到「开始设计第 i 章」。
    - 是 → 跳出循环，进入「5. 完成与交付」。

### 5. 完成与交付

- **剧本编写完成**：确认所有章节已写完，且最终章节的 `chapter_end` 正确结束到 `'end'`（使剧本引擎能终结剧情，恢复自由对话）。
- **引擎级校验（必须）**：调用 `validate_script` 工具检查整个剧本。会话已绑定剧本时可省略 `script_key` 参数；新建剧本请显式传入剧本 key（如 `standalone/剧本名`、`character/角色/剧本名`）。
- **检查报告**：阅读返回的校验报告（错误 / 警告 / 提示 三级）。以 **`error_count == 0`** 为通过门槛。
- **修复循环**：只要 `error_count > 0`，就用 `read_file` 定位问题文件、按下方「6.7 校验诊断 → 修复指南」用 `write_file` 修复，然后**重新运行 `validate_script`**，直到 `error_count == 0`。警告（warn）也应按指南尽量修复；提示（info）可在向用户说明后保留。
- **修复边界（只能修能安全修的）**：结构性错误（拼错字段、悬空章节链接、YAML 语法、重复选项、表达式格式）可直接修复。凡是需要**新编剧情内容**才能补上的必填字段（如某段对白的台词、成就标题），不得擅自编造用户未认可的内容——补上合理占位并明确标注，或先询问用户确认再写入（沿用「内容设计 / 逐章撰写」的确认门流程）。
- **结束剧本**：在剧本引擎中终结此剧情（即最终章节以 `chapter_end` + `next_chapter: 'end'` 收尾）。
- **最终状态**：校验通过后，告知用户——剧本运行到结尾后，将交给 Agent 和玩家自由发挥（回归日常/自由对话模式）。

### 6. 核心剧本设计原则

#### 6.1 分章节原则

- 每一个章节的剧情内容最好是关于某个场景的，当剧本出现分支的时候，必须要分章节，不能将多个分支的剧情写在一个章节中。
- 每个章节的内容不宜太短，除非是分支情况，否则每个章节应当有起码 20~100 个事件。
- 当剧本整体内容不大的时候，可以把所有的章节写在 `Chapters` 目录下。但剧本内容较多的时候，可以使用多个文件夹分级处理，例如：`Chapters/Chapter1/01.yaml`、`Chapters/Chapter2/01.yaml` 等。如果想要在 `next_chapter` 指向那个章节，可以使用 `Chapter1/01`、`Chapter2/02` 等。

#### 6.2 写剧情 `yaml` 原则

- **只用引擎已注册的 17 种事件类型**（`references/event-reference.md` 有完整清单）。未知 `type` 会在运行时报"未注册的事件类型"。
- 每章必须以 `chapter_end` 结束；`linear` 型必须给 `next_chapter`（或 `next`），结束用 `"end"`。
- `choices` 选项的 `actions` 支持 `add_line`（把玩家选的话加入聊天）与 `set_var`（修改变量）。
- 变量赋值语法：`flag = true`、`count += 1`、`hp -= 5`、`random(1,10)`；条件表达式：`var`（truthy）、`var == value`、`var != value`。
- `ai_dialogue` 的 `prompt` 是剧情提示（注入为 Plot 系统消息），告诉模型"此刻应发生什么/角色处于什么状态"，**不是**角色台词本身。AI 台词的具体写法、固定台词（`dialogue`）的取舍，见下方「6.3 剧本提示词原则」。
- 角色情绪名（`modify_character.emotion`）用四字之内的任意短句即可，深度学习模型会自动理解并隐射到人物情绪。
- YAML 缩进必须正确：`events` 下每个事件以 `- type:` 开头，事件属性与其 `type` 同级对齐。

#### 6.3 剧本提示词原则

- 对于所有带有 `prompt` 的对话，只允许轻微地给 AI 提示，**不能直接提示出完整的台词**。
- **核心原则**：不要直接提示出完整的台词，而是引导 AI 自己创作出符合剧情的台词。
- 假如**必须**要固定 AI 的台词（某些特殊情况下，一般不建议这么做），请使用 `dialogue` 事件，并使用 `text` 字段，建议短词、语气词如「啊」、「哦」、「嗯」等可用这类事件固定台词。
- 对于 AI 台词：必须使用【情绪】台词（可选的动作）这样的格式。为防止生成错误和剧本过于固定，建议只给戏份不多的 NPC 或剧情中需要固定台词的对话使用。
- 剧情中，对于 `MAIN` 人物，尽可能**不使用** `dialogue` 事件，所有对话通过 `ai_dialogue` 事件实现，用 `prompt` 提示。

#### 6.4 玩家参与原则与弱提示词引导规则

- 剧本应当鼓励玩家参与，而不是让玩家只是看戏。你应当在剧本中多次使用玩家输入事件如`input`、`choice`，输入，选择等事件来让玩家有参与感。
- 此外，对于`ai_dialogue`和`free_dialogue`事件，不需要每个都为其编写`prompt`。假如上一个事件包含玩家输入，则下一个事件可以省略`prompt`以让剧本角色能完整的与玩家对话。
- 直到剧本需要推进的时候，再使用`prompt`来引导剧情走向。剧本中应当在一些地方留给玩家与角色互动的机会，让玩家有故事的参与感。
- 作为一个`AI-GALGAME`引擎，你作为剧本编写者，主要任务是提供故事背景，引导剧情走向，充分的给角色和玩家自由发挥的空间，仅当必要的时候使用`prompt`来引导剧情走向。可以通过大量的`无prompt`，极少量的`旁白`来实现这样的效果最佳。

#### 6.5 剧本状态注释原则

- 每段剧本的开始，都应当有注释来描述这段剧本的大概内容。
- 每段剧本的末尾，**必须**要包含注释来记录这段剧本所导致的游戏状态，状态记录包括：
  - 当前游戏背景是哪个，游戏背景特效是哪个，游戏背景音乐是哪个
  - 当前环境音音效有哪些（只要出现过 `ambient` 事件，都要记录）
  - 当前台上的角色有哪些（只要出现过`show_character`、`hide_character`，就表示角色上台 / 下台），以及它们的服装。
  - 当前是否有在展示的图片 `present_pic` 事件（如果有，则记录图片名，原则上来讲每章末尾必须没有正在展示的图片，用``空字符串避免正在有展示的图片），

> 以上原则的完整示范（错误示范 / 正确示范 / 固定台词示范）见 `references/design-principles.md`。

#### 6.6 独立创作原则

- 在你打算编写/修改某个剧本的时候，**严格禁止**查看同目录的其他剧本作为参考，以免受到其他剧本的干扰！
- 如果用户明确需求参考其他剧本，本规则不作数。

#### 6.7 校验诊断 → 修复指南

`validate_script` 返回的诊断带稳定代码（`code`）与中文说明（`message`）。遇到诊断时按下表修复。**只能修能安全修的**：结构性错误直接修；需要新编剧情内容的缺口交给用户（见「5. 完成与交付」的修复边界）。

- **配置（story_config.yaml）**
  - `config.no_script_name` — 没填剧本名 → 把 `script_name` 填成剧本文件夹名。
  - `config.duplicate_name` — 与其他剧本重名（引擎按名索引会互相覆盖）→ 换一个全局唯一的 `script_name`。
  - `config.intro_missing` — `intro_chapter` 指向不存在的章节 → 改成 `Chapters/` 下真实存在的章节 id。
- **章节结构**
  - `chapters.empty` — `Chapters/` 下没有 `.yaml` → 创建章节文件（引擎只认 `.yaml`，不认 `.yml`）。
  - `chapter.no_end` — 章节缺「章节结束」事件 → 补 `chapter_end`（linear 型必须给 `next_chapter`/`next`，结尾用 `"end"`）。
  - `chapter.no_events` — 章节没有任何事件 → 补事件。
  - `chapter.unreadable` / `parse_failed` / `bad_shape` — YAML 语法或结构问题 → 修正格式（顶层 `name` + `events` 列表，`- type:` 与属性同级对齐）。
- **事件与字段**
  - `event.unknown_type` / `missing_type` — 事件类型非法/缺失 → 用事件大全里引擎注册的类型（见 `references/event-reference.md`，共 17 种），别拼错。
  - `event.not_a_map` — 事件不是键值映射 → 修 YAML 缩进。
  - `field.required_missing` — 缺必填字段 → 按事件大全补该事件的必填字段；需要创作内容的先与用户确认（见「修复边界」）。
  - `field.unknown` — 写了引擎不认识的字段（多半拼错）→ 删除或改正。
  - `field.inert` — 遗留字段引擎从不读取 → 删除。
- **条件（condition）**
  - `condition.unsupported_operator` / `no_variable` / `bad_variable` — 条件语法错误 → 只支持 `变量 == 值`、`变量 != 值` 或单个变量判真假；变量名不能含空格；不要用 `&&`、`||`、`>`、`<`、`!`、括号、算术。
  - `condition.placeholder_not_replaced` — `%player%` 写在 condition 里不会被替换 → 移走。
- **素材与媒体**
  - `asset.missing` — 素材找不到 → 引用已存在的文件，或把素材放到对应 `Assets/` 子目录（Backgrounds / Musics / Sounds / Pics / Ambients）。
  - `ambient.no_path` — 播放环境音但没给路径 → 填 `ambientPath`；要停掉全部轨道请用「停止该轨」。
  - `music.bad_speed` — 播放速度超范围 → 改到 0–4。
- **特效**
  - `effect.unknown` — 用了非内置特效 → 换内置特效键。
  - `effect.case` — 大小写不对 → 改成规范写法（编辑器会自动纠正）。
- **选项（choices）**
  - `choices.empty` / `option_not_a_map` — 选项列表空 / 选项不是映射 → 补选项、修格式。
  - `choices.duplicate_text` — 选项文案重复 → 改成不同的文案。
  - `choices.option_next_ignored` — 选项写了 `next`（choices 不支持选项级跳转）→ 删掉；要按选择分支请用 `set_var` 记录选择 + 章节结束用 `branching`。
  - `choices.catch_all_not_last` — 空文案选项放中间会吞掉后面的选项 → 移到最后一个或加条件。
  - `choices.placeholder_in_text` / `lock_hint_without_condition` — 选项文案里的 `%player%` 不替换 / 没条件却写了不可选提示 → 移走 / 补条件或删提示。
- **设置变量（set_variable）**
  - `set_variable.no_options` — 缺 `options` 列表 → 改成 `options[].actions[]` 形状，不要直接写 `name/value`。
  - `set_variable.empty` — 赋值组为空 → 补 `actions`。
- **动作（actions）**
  - `action.unknown_type` — 未知动作类型 → 只用 `set_var` / `add_line`。
  - `action.legacy_shape` — 旧式 `name/value/op` → 改成表达式，如 `flag = warm`。
  - `action.empty_expression` / `bad_expression` — 表达式空 / 无法解析 → 填 `变量 = 值`、`变量 += 值`、`变量 -= 值`。
  - `action.not_supported_here` / `empty_content` — 在 set_variable 里写了 add_line / add_line 内容为空 → 删除或补内容。
- **章节结束（chapter_end）**
  - `chapter_end.dangling` — 指向的章节不存在 → 指向已存在的章节 id，或创建该章节。
  - `chapter_end.empty_target` — 没写目标章节 → 补 `next_chapter` / `next`。
  - `chapter_end.no_next` — linear 但没写下一章 → 补下一章（结尾用 `"end"`）。
  - `chapter_end.end_suffix` — 写了 `end.yaml` → 直接写 `end`。
  - `chapter_end.both_next_fields` — 同时写了 `next` 和 `next_chapter` → 只留一个。
  - `chapter_end.not_last` — 章节结束之后还有事件 → 把 `chapter_end` 移到最后一个事件。
  - `chapter_end.unknown_end_type` — 结束方式非法 → 用 `linear` / `branching` / `ai_judged`。
  - `chapter_end.no_options` — branching / ai_judged 缺分支列表 → 补 `options`。
  - `chapter_end.choice_shaped_option` — 分支写成了选项形状（text/actions）→ 改成 `condition/next/default`（ai_judged 用 `name/next/default`）。
  - `chapter_end.no_default_branch` — 没有 default 兜底分支 → 补 default 分支。
  - `chapter_end.branch_no_condition` / `branch_no_next` — 分支缺条件 / 缺 next → 补上。
  - `chapter_end.ai_option_no_name` / `ai_condition_ignored` — ai_judged 分支缺 name / 写了 condition（引擎忽略）→ 用 name 匹配，删掉 condition。
- **章节图**
  - `graph.unreachable` — 某章节从开场走不到 → 让一个已可达章节的 `chapter_end` 指向它（确保每章都从 `intro_chapter` 可达）。
  - `graph.cycle` — 章节之间存在循环 → 打破循环。
- **变量**
  - `variable.never_set` — 条件里用了未赋值的变量 → 用 `set_variable` 赋值，或改掉变量名。
  - `variable.never_read` — 赋值了但从未在条件里用 → 接线或删除（info，可忽略）。
- **角色**
  - `character.unknown` — 引用的角色在 `characters/` 下找不到 → 引用已存在的角色，或写 `MAIN`。
  - `character.no_role_key` — `settings.yml` 缺 `script_role_key` → 补上（剧本 NPC 必须显式声明）。
  - `character.no_persona` — 人设为空 → 按需补 `system_prompt`（info，可忽略）。
  - `character.action_unknown` — `modify_character` 动作非法 → 用 `show_character`（登场）/ `hide_character`（退场）。
- **成就**
  - `achievement.id_conflicts_builtin` / `id_duplicated` — 成就键名与内置成就冲突 / 本剧本内重复 → 换一个唯一键名。
- **自由对话**
  - `free_dialogue.no_exit` — 不限轮数且结束语为空 → 设 `max_rounds > 0` 或给 `end_line`。

> 记住：**只能修能安全修的**。结构性错误直接修；需要新编剧情内容的缺口交给用户确认，不要编造用户未认可的内容。

## 创建流程

> 创作环节（入口判断、类型选择、大纲与工程创建、内容设计、逐章撰写、完成交付）见上方「创作工作流」，本流程为落到文件的技术步骤。

1. **判断剧本类型**：
   - 角色卡羁绊冒险 → 目录 `character/<角色>/<剧本名>/`，`story_config.yaml` 需写 `adventure` 块（见配置参考）。
   - 独立剧本 → 目录 `standalone/<剧本名>/`，不写 `adventure` 块。
2. **与用户确认生成位置**：询问用户把剧本放到哪个具体目录（本技能不预设路径）。
3. **建目录**：创建 `story_config.yaml` 与 `Chapters/`；如需 NPC 再建 `characters/`。
4. **写配置**：按 `references/story-config-reference.md` 写 `story_config.yaml`。`script_name` 必须与剧本文件夹名一致。
5. **写章节**：起始章节文件名必须与 `intro_chapter` 一致；每章由 `name` + `events` 列表组成，以 `chapter_end` 收尾。
6. **选事件**：按 `references/event-reference.md` 的 17 种事件表选型填字段，**只用引擎注册的类型**。
7. **角色**：剧本 NPC 复制角色卡字段并加 `script_role_key`（唯一 id），见 `references/character-reference.md`。
8. **资源**：媒体文件放入对应 `Assets/` 子目录，事件里只写文件名；引擎按资源类型自动在子目录中查找。
9. **占位符**：文本中可用 `%player%`（玩家名）、`%main%`（主角色名），运行时自动替换。

## 模板与参考文件

- 配置模板：`assets/templates/story_config.yaml`
- 章节模板：`assets/templates/chapter_template.yaml`
- 角色模板：`assets/templates/character_settings.yml`
- 事件大全（17 种，源码级字段与默认值）：`references/event-reference.md`
- 配置字段参考：`references/story-config-reference.md`
- 角色设定参考：`references/character-reference.md`
- 剧本设计原则示范（错误示范 / 正确示范 / 固定台词示范）：`references/design-principles.md`
