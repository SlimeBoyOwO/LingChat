# LingChat 剧本设计原则（示范与示例）

本文件收录「创作剧本原则 · 剧本设计原则」的完整示范。字段与事件语法以
`references/event-reference.md` 为权威依据，写剧本时对照使用。

---

## 1. AI 台词提示原则（prompt 只做提示，不写完整台词）

对于所有带有 `prompt` 的对话，只允许轻微地给 AI 提示，**不能直接提示出完整的台词**。
如果需要提示出完整的台词，建议用旁白。

### 错误示范（在 `prompt` 里直接写出了完整台词）

```yaml
name: 错误示范
events:
  - type: narration
    text: |
      深夜，钦灵的房间里只剩下屏幕还亮着。
      她偷偷摸摸地打开了《植物大战僵尸》，嘴上还念念有词……

  - type: modify_character
    action: show_character
    character: MAIN
    emotion: 正常
    duration: 1.5

  - type: ai_dialogue
    character: MAIN
    prompt: 钦灵一边小声嘟囔着"我才不是想玩游戏，是为了研究游戏机制才打开的"，一边心虚地悄悄点下了"开始游戏"，尾巴却因为兴奋已经轻轻摇了起来。
```

### 正确示范（只提示状态/意图，让 AI 自己创作台词）

```yaml
name: 正确示范
events:
  - type: narration
    text: |
      深夜，钦灵的房间里只剩下屏幕还亮着。
      她偷偷摸摸地打开了《植物大战僵尸》，嘴上还念念有词……

  - type: modify_character
    action: show_character
    character: MAIN
    emotion: 正常
    duration: 1.5

  - type: ai_dialogue
    character: MAIN
    prompt: 钦灵打算用"学习游戏开发"安慰自己大半夜偷偷打游戏，兴奋的不行。
```

> **核心原则**：不要直接提示出完整的台词，而是引导 AI 自己创作出符合剧情的台词。

---

## 2. 固定 AI 台词（dialogue 事件）

假如**必须**要固定 AI 的台词（某些特殊情况下，一般不建议这么做），请使用 `dialogue`
事件，并使用 `text` 字段：

```yaml
name: 正确示范 2（固定 AI 台词，但多数场景不推荐）
events:
  - type: narration
    text: |
      深夜，钦灵的房间里只剩下屏幕还亮着。
      她偷偷摸摸地打开了《植物大战僵尸》，嘴上还念念有词……

  - type: modify_character
    action: show_character
    character: MAIN
    emotion: 正常
    duration: 1.5

  - type: dialogue
    character: MAIN
    text: |
      【嘟哝】嘛，我这样的好学生怎么会大半夜偷偷打游戏呢？
      【调皮】当然是为了研究游戏机制才打开的！一定是这样！
      【兴奋】嘿嘿，小玩一会没什么问题的啦！（摇了摇尾巴）
```

> 对于 AI 台词：必须使用【情绪】台词（可选的动作）这样的格式。为了防止生成错误和剧本过于固定，
> 建议本用法只给戏份不多的 NPC 或者剧情中需要固定台词的对话使用。

---

## 3. MAIN 角色的对话要求

剧情中，对于 `MAIN` 人物，尽可能**不使用** `dialogue` 事件，所有对话必须通过
`ai_dialogue` 事件实现，用 `prompt` 提示。
