---
name: lingchat-script-editor
description: LingChat 剧本编辑器技能。当用户需要为 LingChat 创建新剧本（story_config.yaml、Chapters/*.yaml 章节、剧本内角色 settings.yml、Assets 资源引用）时使用。本技能以 LingChat 脚本引擎 Rust 源码（src-tauri/src/ai_service/game_system/script_engine/）为准提供精确字段与事件语法，并附带可复制的最小可运行模板。内置完整创作工作流（剧本类型确认、大纲设计、逐章撰写）与 AI 台词设计原则。审查/校验功能为预留能力。
---

# LingChat 剧本编辑器

本技能指导如何为 LingChat 创建全新的剧本（羁绊冒险 / 独立剧本）。所有字段、事件类型、默认值均以脚本引擎 Rust 源码为权威依据，下文为源码的精确映射，

## 何时使用

- 用户要求为 LingChat **创建新剧本**、新章节、新事件、新剧本角色（含完整创作工作流：剧本类型确认 → 大纲设计 → 逐章撰写与确认）
- 需要遵循 AI 台词设计原则（`prompt` 只做状态/意图提示、不直接写完整台词；必要时用 `dialogue` 固定台词）
- 需要判断某事件类型支持哪些字段、默认值是什么
- 需要生成剧本目录结构、story_config.yaml、章节 YAML 或角色 settings.yml
- （预留）后续版本将支持剧本审查/校验：目前仅覆盖创建

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

## 创作剧本原则

> 本技能遵循「先沟通、后动笔」的创作工作流。写任何 YAML 文件之前，必须先完成下面的剧本类型确认与大纲设计，并逐章与用户确认。

### 1. 询问剧本类型

- **羁绊剧情（角色卡冒险）**：必须要有 `MAIN` 角色，其他角色（NPC）是可选项，正常情况下没有其他角色。羁绊剧情在 `story_config.yaml` 中需要额外的 `adventure` 配置块（见 `references/story-config-reference.md`）。
- **独立剧本**：可以没有 `MAIN` 角色，也可以有其他角色。

### 2. 确定剧本大纲

1. 假如用户没有说具体的剧情，先指导用户具体阐明剧情内容。
2. 假如是羁绊剧情或者和角色性格强相关的剧情，可以在通用角色文件夹中先读取角色的 `settings.yml`，了解角色的性格、背景、性格、喜好等，根据这些信息设计剧情。
3. 了解剧本大纲后，根据大纲先进行剧本剧情设计，包括剧情分为几章节、剧本的分支有哪些、每个分支的剧情大概是什么样子，并交给用户确认。
4. 和用户充分沟通，确保剧情的设计符合用户需求后，确定最终的剧本大纲，确定需要登场的人物、所需要的 Assets 素材，并交给用户确认，要求用户提供所能提供的素材（包括需要的背景、音乐、音效、环境音、图片等）。
5. 用户会与你沟通所能提供的素材，你需要根据所能提供的素材反复修改剧本大纲，进行增删改查，直到确定一份既满足素材、又满足用户剧情需求的大纲，等待用户最终确认。
6. 完成预备工作后，开始逐章节编写剧本。每一章节写完后要求给用户介绍章节内容，并要求用户确认。
7. 直到剧本编写完成，用户确认后，剧本编写工作结束。否则反复修改剧本内容。

### 3. 核心剧本设计原则

#### 3.1 分章节原则

- 每一个章节的剧情内容最好是关于某个场景的，当剧本出现分支的时候，必须要分章节，不能将多个分支的剧情写在一个章节中。
- 每个章节的内容不宜太短，除非是分支情况，否则每个章节应当有起码 20~100 个事件。
- 当剧本整体内容不大的时候，可以把所有的章节写在 `Chapters` 目录下。但剧本内容较多的时候，可以使用多个文件夹分级处理，例如：`Chapters/Chapter1/01.yaml`、`Chapters/Chapter2/01.yaml` 等。如果想要在 `next_chapter` 指向那个章节，可以使用 `Chapter1/01`、`Chapter2/02` 等。

#### 3.2 写剧情 `yaml` 原则

- **只用引擎已注册的 16 种事件类型**（`references/event-reference.md` 有完整清单）。未知 `type` 会在运行时报"未注册的事件类型"。
- 每章必须以 `chapter_end` 结束；`linear` 型必须给 `next_chapter`（或 `next`），结束用 `"end"`。
- `choices` 选项的 `actions` 支持 `add_line`（把玩家选的话加入聊天）与 `set_var`（修改变量）。
- 变量赋值语法：`flag = true`、`count += 1`、`hp -= 5`、`random(1,10)`；条件表达式：`var`（truthy）、`var == value`、`var != value`。
- `ai_dialogue` 的 `prompt` 是剧情提示（注入为 Plot 系统消息），告诉模型"此刻应发生什么/角色处于什么状态"，**不是**角色台词本身。AI 台词的具体写法、固定台词（`dialogue`）的取舍，见上方「创作剧本原则 · 剧本设计原则」。
- 角色情绪名（`modify_character.emotion` / `dialogue.emotion`）建议与角色卡表情图片名一致（如 `正常`、`高兴`、`害羞`、`生气`、`心动`、`惊讶` 等）。
- YAML 缩进必须正确：`events` 下每个事件以 `- type:` 开头，事件属性与其 `type` 同级对齐。

#### 3.3 剧本提示词原则

- 对于所有带有 `prompt` 的对话，只允许轻微地给 AI 提示，**不能直接提示出完整的台词**。如果需要提示出完整的台词，建议用旁白。
- **核心原则**：不要直接提示出完整的台词，而是引导 AI 自己创作出符合剧情的台词。
- 假如**必须**要固定 AI 的台词（某些特殊情况下，一般不建议这么做），请使用 `dialogue` 事件，并使用 `text` 字段。
- 对于 AI 台词：必须使用【情绪】台词（可选的动作）这样的格式。为防止生成错误和剧本过于固定，建议只给戏份不多的 NPC 或剧情中需要固定台词的对话使用。
- 剧情中，对于 `MAIN` 人物，尽可能**不使用** `dialogue` 事件，所有对话必须通过 `ai_dialogue` 事件实现，用 `prompt` 提示。

#### 3.4 玩家参与原则

- 剧本应当鼓励玩家参与，而不是让玩家只是看戏。你应当在剧本中多次使用玩家输入事件如`input`、`choice`，输入，选择等事件来让玩家有参与感。
- 此外，对于`ai_dialogue`和`free_dialogue`事件，不需要每个都为其编写`prompt`。假如上一个事件包含玩家输入，则下一个事件可以省略`prompt`以让剧本角色能完整的与玩家对话。
- 直到剧本需要推进的时候，再使用`prompt`来引导剧情走向。剧本中应当在一些地方留给玩家与角色互动的机会，让玩家有故事的参与感。

#### 3.5 剧本状态注释原则

- 每段剧本的开始，都应当有注释来描述这段剧本的大概内容。
- 每段剧本的末尾，**必须**要包含注释来记录这段剧本所导致的游戏状态，状态记录包括：
  - 当前游戏背景是哪个，游戏背景特效是哪个，游戏背景音乐是哪个
  - 当前环境音音效有哪些（只要出现过 `ambient` 事件，都要记录）
  - 当前台上的角色有哪些（只要出现过`show_character`、`hide_character`，就表示角色上台 / 下台），以及它们的服装。
  - 当前是否有在展示的图片 `present_pic` 事件（如果有，则记录图片名，原则上来讲每章末尾必须没有正在展示的图片，用``空字符串避免正在有展示的图片），

> 以上原则的完整示范（错误示范 / 正确示范 / 固定台词示范）见 `references/design-principles.md`。

## 创建流程

> 创作环节（剧本类型确认、大纲设计、逐章撰写与确认）见上方「创作剧本原则」，本流程为落到文件的技术步骤。

1. **判断剧本类型**：
   - 角色卡羁绊冒险 → 目录 `character/<角色>/<剧本名>/`，`story_config.yaml` 需写 `adventure` 块（见配置参考）。
   - 独立剧本 → 目录 `standalone/<剧本名>/`，不写 `adventure` 块。
2. **与用户确认生成位置**：询问用户把剧本放到哪个具体目录（本技能不预设路径）。
3. **建目录**：创建 `story_config.yaml` 与 `Chapters/`；如需 NPC 再建 `characters/`。
4. **写配置**：按 `references/story-config-reference.md` 写 `story_config.yaml`。`script_name` 必须与剧本文件夹名一致。
5. **写章节**：起始章节文件名必须与 `intro_chapter` 一致；每章由 `name` + `events` 列表组成，以 `chapter_end` 收尾。
6. **选事件**：按 `references/event-reference.md` 的 16 种事件表选型填字段，**只用引擎注册的类型**。
7. **角色**：剧本 NPC 复制角色卡字段并加 `script_role_key`（唯一 id），见 `references/character-reference.md`。
8. **资源**：媒体文件放入对应 `Assets/` 子目录，事件里只写文件名；引擎按资源类型自动在子目录中查找。
9. **占位符**：文本中可用 `%player%`（玩家名）、`%main%`（主角色名），运行时自动替换。

## 模板与参考文件

- 配置模板：`assets/templates/story_config.yaml`
- 章节模板：`assets/templates/chapter_template.yaml`
- 角色模板：`assets/templates/character_settings.yml`
- 事件大全（16 种，源码级字段与默认值）：`references/event-reference.md`
- 配置字段参考：`references/story-config-reference.md`
- 角色设定参考：`references/character-reference.md`
- 剧本设计原则示范（错误示范 / 正确示范 / 固定台词示范）：`references/design-principles.md`

## 预留能力（后续版本）

- 剧本审查（事件合法性、字段完整性、章节链路检查）
- 剧本校验脚本（YAML 语法 + 事件 schema 校验）
