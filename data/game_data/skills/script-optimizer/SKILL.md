---
name: script-optimizer
description: LingChat 剧本的校验与修复技能。运行引擎级校验器、按稳定诊断码修复结构性错误、并把需要新编剧情内容的缺口交回用户。当剧本已写完需要交付前校验、或需要按诊断报告修正剧本时使用。
---

# 校验与修复（Optimizer）

本角色负责让剧本通过引擎级校验，并在**不越界**的前提下修好能修的问题。

## 前置条件

- 计划中的章节都已在磁盘上。
- 若你尚未加载 `lingchat-script-editor` 主技能，请先加载它 —— 阶段推进与用户确认门由主技能掌辖。

## 产物

- 修复后的章节文件 / `story_config.yaml`
- 一份修复说明：每条诊断的 `code`、如何处理、哪些没处理及原因

## 校验（必须）

- **引擎级校验（必须）**：调用 `validate_script` 工具检查整个剧本。会话已绑定剧本时可省略 `script_key` 参数；新建剧本请显式传入剧本 key（如 `standalone/剧本名`、`character/角色/剧本名`）。
- **检查报告**：阅读返回的校验报告（错误 / 警告 / 提示 三级）。以 **`error_count == 0`** 为通过门槛。
- **修复循环**：只要 `error_count > 0`，就用 `read_file` 定位问题文件、按下方「校验诊断 → 修复指南」用 `write_file` 修复，然后**重新运行 `validate_script`**，直到 `error_count == 0`。警告（warn）也应按指南尽量修复；提示（info）可在向用户说明后保留。
- **结束剧本**：在剧本引擎中终结此剧情（即最终章节以 `chapter_end` + `next_chapter: 'end'` 收尾）。

## 修复边界（只能修能安全修的）

结构性错误（拼错字段、悬空章节链接、YAML 语法、重复选项、表达式格式）可直接修复。凡是需要**新编剧情内容**才能补上的必填字段（如某段对白的台词、成就标题），不得擅自编造用户未认可的内容——补上合理占位并明确标注，或先询问用户确认再写入（沿用「内容设计 / 逐章撰写」的确认门流程）。

## 核心剧本设计原则

#### 6.7 校验诊断 → 修复指南

`validate_script` 返回的诊断带稳定代码（`code`）与中文说明（`message`）。遇到诊断时按下表修复。**只能修能安全修的**：结构性错误直接修；需要新编剧情内容的缺口交给用户（见 `lingchat-script-editor` 的「5. 完成与交付」的修复边界）。

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
  - `event.unknown_type` / `missing_type` — 事件类型非法/缺失 → 用事件大全里引擎注册的类型（见 `game_data/skills/lingchat-script-editor/references/event-reference.md`，共 17 种），别拼错。
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
