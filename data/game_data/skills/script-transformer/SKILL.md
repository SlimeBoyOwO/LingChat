---
name: script-transformer
description: LingChat 剧本的落盘技能。把已确认的设计稿逐章转换为可运行的章节 YAML —— 负责目录结构、事件选型与字段、YAML 书写规范、章节首尾注释与状态注释、创建流程的技术步骤。当需要实际写出或修改 story_config.yaml、Chapters/*.yaml、剧本内角色 settings.yml 时使用。字段与事件语法以 LingChat 脚本引擎 Rust 源码为权威依据。
---

# 逐章转换（Transformer）

本角色把设计稿变成磁盘上可运行的章节。**不负责决定剧情走向**（那是设计阶段的事），只负责把已定的内容准确落盘。

## 前置条件

- 设计稿已存在且已列出章节列表（每章含 `id:` / `梗概:` / `登场:` / `素材:` / `结束方式:`）。
- 若你尚未加载 `lingchat-script-editor` 主技能，请先加载它 —— 阶段推进与用户确认门由主技能掌辖。

## 产物

- 剧本包骨架：`story_config.yaml` + `Chapters/`（如需 NPC 再建 `characters/`）
- 章节文件：`Chapters/<id>.yaml`，每章含 `name` + `events`，以 `chapter_end` 收尾，**首尾均带注释**（见「剧本状态注释原则」）
- 每章末尾的状态注释同时是**下一章的输入**：系统会把它取出来交给下一章的转换，所以它必须准确反映本章结束时的真实状态。

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

## 核心剧本设计原则

#### 6.2 写剧情 `yaml` 原则

- **只用引擎已注册的 17 种事件类型**（`game_data/skills/lingchat-script-editor/references/event-reference.md` 有完整清单）。未知 `type` 会在运行时报"未注册的事件类型"。
- 每章必须以 `chapter_end` 结束；`linear` 型必须给 `next_chapter`（或 `next`），结束用 `"end"`。
- `choices` 选项的 `actions` 支持 `add_line`（把玩家选的话加入聊天）与 `set_var`（修改变量）。
- 变量赋值语法：`flag = true`、`count += 1`、`hp -= 5`、`random(1,10)`；条件表达式：`var`（truthy）、`var == value`、`var != value`。
- `ai_dialogue` 的 `prompt` 是剧情提示（注入为 Plot 系统消息），告诉模型"此刻应发生什么/角色处于什么状态"，**不是**角色台词本身。AI 台词的具体写法、固定台词（`dialogue`）的取舍，见 `script-writer` 的「6.3 剧本提示词原则」。
- 角色情绪名（`modify_character.emotion`）用四字之内的任意短句即可，深度学习模型会自动理解并隐射到人物情绪。
- YAML 缩进必须正确：`events` 下每个事件以 `- type:` 开头，事件属性与其 `type` 同级对齐。

#### 6.5 剧本状态注释原则

- 每段剧本的开始，都应当有注释来描述这段剧本的大概内容。
- 每段剧本的末尾，**必须**要包含注释来记录这段剧本所导致的游戏状态，状态记录包括：
  - 当前游戏背景是哪个，游戏背景特效是哪个，游戏背景音乐是哪个
  - 当前环境音音效有哪些（只要出现过 `ambient` 事件，都要记录）
  - 当前台上的角色有哪些（只要出现过`show_character`、`hide_character`，就表示角色上台 / 下台），以及它们的服装。
  - 当前是否有在展示的图片 `present_pic` 事件（如果有，则记录图片名，原则上来讲每章末尾必须没有正在展示的图片，用``空字符串避免正在有展示的图片），

> 以上原则的完整示范（错误示范 / 正确示范 / 固定台词示范）见 `game_data/skills/lingchat-script-editor/references/design-principles.md`。

## 创建流程

> 创作环节（入口判断、类型选择、大纲与工程创建、内容设计、逐章撰写、完成交付）见主技能 `lingchat-script-editor`，本流程为落到文件的技术步骤。

1. **判断剧本类型**：
   - 角色卡羁绊冒险 → 目录 `character/<角色>/<剧本名>/`，`story_config.yaml` 需写 `adventure` 块（见配置参考）。
   - 独立剧本 → 目录 `standalone/<剧本名>/`，不写 `adventure` 块。
2. **与用户确认生成位置**：询问用户把剧本放到哪个具体目录（本技能不预设路径）。
3. **建目录**：创建 `story_config.yaml` 与 `Chapters/`；如需 NPC 再建 `characters/`。
4. **写配置**：按 `game_data/skills/lingchat-script-editor/references/story-config-reference.md` 写 `story_config.yaml`。`script_name` 必须与剧本文件夹名一致。
5. **写章节**：起始章节文件名必须与 `intro_chapter` 一致；每章由 `name` + `events` 列表组成，以 `chapter_end` 收尾。
6. **选事件**：按 `game_data/skills/lingchat-script-editor/references/event-reference.md` 的 17 种事件表选型填字段，**只用引擎注册的类型**。
7. **角色**：剧本 NPC 复制角色卡字段并加 `script_role_key`（唯一 id），见 `game_data/skills/lingchat-script-editor/references/character-reference.md`。
8. **资源**：媒体文件放入对应 `Assets/` 子目录，事件里只写文件名；引擎按资源类型自动在子目录中查找。
9. **占位符**：文本中可用 `%player%`（玩家名）、`%main%`（主角色名），运行时自动替换。
