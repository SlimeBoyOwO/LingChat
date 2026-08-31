# LingChat 剧本事件大全（源码级参考）

本文件为 LingChat 脚本引擎全部 17 种事件类型的权威参考，字段与默认值均取自 Rust 源码：
`src-tauri/src/ai_service/game_system/script_engine/events/events/*.rs`

---

## 一、对话事件

### 1. narration — 旁白

```yaml
- type: narration
  text: |
    旁白内容，可多行。
  displayName: 旁白 # 可选，显示名，默认 "旁白"
```

| 字段          | 必填 | 类型   | 默认值 | 说明                         |
| ------------- | ---- | ------ | ------ | ---------------------------- |
| `text`        | ✅   | string | —      | 旁白文本，单行或多行（`\|`） |
| `displayName` | 否   | string | `旁白` | 显示名称                     |

### 2. player — 玩家固定台词

```yaml
- type: player
  text: 哎呀，好不容易有零花钱啦！
  displayName: 我 # 可选，默认玩家名
```

| 字段          | 必填 | 类型   | 默认值 | 说明             |
| ------------- | ---- | ------ | ------ | ---------------- |
| `text`        | ✅   | string | —      | 玩家台词，可多行 |
| `displayName` | 否   | string | 玩家名 | 显示名称         |

### 3. dialogue — AI 角色固定台词

```yaml
- type: dialogue
  character: MAIN # 角色 id，MAIN 表示主对话角色
  text: |
    【生气】台词内容。
  displayName: 钦灵 # 可选，显示名
  displaySubtitle: 副标题 # 可选
  emotion: 生气 # 可选，表情
```

| 字段              | 必填 | 类型   | 默认值 | 说明                         |
| ----------------- | ---- | ------ | ------ | ---------------------------- |
| `character`       | ✅   | string | —      | 角色 id；`MAIN` = 主对话角色 |
| `text`            | ✅   | string | —      | 角色台词，可多行             |
| `displayName`     | 否   | string | —      | 显示名（覆盖默认）           |
| `displaySubtitle` | 否   | string | —      | 副标题                       |
| `emotion`         | 否   | string | —      | 情绪/表情                    |

---

## 二、环境变化事件

### 4. background — 设定背景

```yaml
- type: background
  imagePath: '便利店.png' # Assets/Backgrounds 下的文件名
  transition: 0.5 # 可选，转场时间，默认 1.0
```

| 字段         | 必填 | 类型   | 默认值 | 说明                                   |
| ------------ | ---- | ------ | ------ | -------------------------------------- |
| `imagePath`  | ✅   | string | —      | 背景图片名（放 `Assets/Backgrounds/`） |
| `transition` | 否   | float  | `1.0`  | 转场时间（秒）                         |

> ⚠️ 源码中 `background` **没有 `duration` 字段**，与旧文档不同。

### 5. music — 背景音乐

```yaml
- type: music
  musicPath: 'bgm.mp3' # Assets/Musics 下的文件名
  duration: 0 # 可选
```

| 字段        | 必填 | 类型   | 默认值 | 说明                                                          |
| ----------- | ---- | ------ | ------ | ------------------------------------------------------------- |
| `musicPath` | ✅   | string | —      | 音乐文件名（放 `Assets/Musics/`）；传 `none` 或空串可停止音乐 |

### 6. sound — 音效

```yaml
- type: sound
  soundPath: 'smash1.wav' # Assets/Sounds 下的文件名
  duration: 0
```

| 字段        | 必填 | 类型   | 默认值 | 说明                                                      |
| ----------- | ---- | ------ | ------ | --------------------------------------------------------- |
| `soundPath` | ✅   | string | —      | 音效文件名（放 `Assets/Sounds/`）；传 `none` 或空串可停止 |

### 7. ambient — 环境音

```yaml
- type: ambient
  ambientPath: '蝉鸣.mp3' # Assets/Ambients 下的文件名
  volume: 100 # 可选，音量 0-100，默认 100
  loop: true # 可选，循环，默认 true
  stop: false # 可选，是否停止，默认 false
  fade: true # 可选，淡入淡出，默认 true
```

| 字段          | 必填 | 类型   | 默认值  | 说明                                  |
| ------------- | ---- | ------ | ------- | ------------------------------------- |
| `ambientPath` | ✅   | string | —       | 环境音文件名（放 `Assets/Ambients/`） |
| `volume`      | 否   | int    | `100`   | 音量                                  |
| `loop`        | 否   | bool   | `true`  | 是否循环                              |
| `stop`        | 否   | bool   | `false` | 为 true 时停止环境音                  |
| `fade`        | 否   | bool   | `true`  | 是否淡入淡出                          |

### 8. background_effect — 背景特效

```yaml
- type: background_effect
  effect: 'Sakura' # 特效类型，默认 "none"
  duration: 3
```

| 字段       | 必填 | 类型   | 默认值 | 说明                     |
| ---------- | ---- | ------ | ------ | ------------------------ |
| `effect`   | 否   | string | `none` | 特效类型（如 Sakura 等） |
| `duration` | 否   | number | —      | 持续时间                 |

### 9. present_pic — 展示图片（Galgame 小窗口立绘）

```yaml
- type: present_pic
  imagePath: 'Q版动画.png' # Assets/Pics 下的文件名
  scale: 1 # 可选，缩放，默认 1.0
  duration: 1
```

| 字段        | 必填 | 类型   | 默认值 | 说明                                                        |
| ----------- | ---- | ------ | ------ | ----------------------------------------------------------- |
| `imagePath` | ✅   | string | —      | 图片文件名（放 `Assets/Pics/`）；传 `none` 或空串可关闭展示 |
| `scale`     | 否   | float  | `1.0`  | 缩放比例                                                    |

---

## 三、人物相关事件

### 10. modify_character — 修改角色

```yaml
- type: modify_character
  action: show_character # show_character / hide_character
  character: MAIN # 角色 id，默认 MAIN
  emotion: 生气 # 可选，情绪（自动匹配立绘）
  clothes: 便服 # 可选，服装
  perceive: true # 可选，角色是否感知上下文
  duration: 1.5
```

| 字段        | 必填 | 类型        | 默认值 | 说明                                               |
| ----------- | ---- | ----------- | ------ | -------------------------------------------------- |
| `action`    | 否   | string      | —      | `show_character`（显示）/ `hide_character`（隐藏） |
| `character` | ✅   | string      | `MAIN` | 角色 id（`MAIN` 或剧本角色 `script_role_key`）     |
| `emotion`   | 否   | string      | —      | 情绪，由情感识别系统自动归类匹配立绘               |
| `clothes`   | 否   | string      | —      | 服装                                               |
| `perceive`  | 否   | bool/string | —      | 为 true 时该期间台词进入 AI 上下文                 |
| `duration`  | 否   | number      | —      | 持续时间                                           |

> 必填仅 `character`，其余按需填写。

---

## 四、AI 对话控制事件

### 11. input — 玩家输入

```yaml
- type: input
  hint: '尝试对着风雪打个招呼吧！' # 输入提示
```

| 字段   | 必填 | 类型   | 默认值      | 说明           |
| ------ | ---- | ------ | ----------- | -------------- |
| `hint` | 否   | string | `请输入...` | 输入框提示文本 |

### 12. ai_dialogue — AI 对话

```yaml
- type: ai_dialogue
  character: MAIN # 角色 id，默认 MAIN
  prompt: 风雪打算开心点回答 # AI 回复前的剧情提示
```

| 字段        | 必填 | 类型   | 默认值 | 说明                                    |
| ----------- | ---- | ------ | ------ | --------------------------------------- |
| `character` | 否   | string | `MAIN` | 角色 id                                 |
| `prompt`    | 否   | string | —      | 剧情提示（Plot 系统消息），不是角色台词 |

### 13. choices — 玩家选项

```yaml
- type: choices
  options:
    - text: '要不要一起吃红烧牛肉面！'
      actions:
        - type: add_line
          content: 要不要一起吃红烧牛肉面！
      condition: flag == true # 可选，显示条件
    - text: 要不要一起吃香菇肉鸡面！
      actions:
        - type: add_line
          content: 要不要一起吃香菇肉鸡面！
  allow_free: true # 可选，允许玩家自由输入
```

| 字段                  | 必填 | 类型   | 默认值  | 说明                                            |
| --------------------- | ---- | ------ | ------- | ----------------------------------------------- |
| `options`             | ✅   | list   | —       | 选项列表                                        |
| `options[].text`      | ✅   | string | —       | 选项文本                                        |
| `options[].actions`   | ✅   | list   | —       | 选择后执行的动作（`add_line` / `set_var`）      |
| `options[].condition` | 否   | expr   | —       | 选项显示条件（`var` / `var == v` / `var != v`） |
| `allow_free`          | 否   | bool   | `false` | true 时玩家可自由输入台词                       |

`actions` 支持的两种动作：

```yaml
# 把玩家选择的台词加入聊天
- type: add_line
  content: 选择的台词文本

# 修改变量（表达式写法；旧原型 name/value/op 会被校验器标记为 action.legacy_shape）
- type: set_var
  content: 'flag = true' # 表达式：变量 = 值 / 变量 += 值 / 变量 -= 值
```

### 14. free_dialogue — 自由对话（多轮）

```yaml
- type: free_dialogue
  character: MAIN # 角色 id，默认 default
  hint: 试着安慰钦灵吧，输入"结束"结束对话
  max_rounds: 3 # 可选，最大轮数，默认 -1（无限）
  end_line: 结束 # 可选，玩家输入包含此词则结束，默认 "结束"
  prompt: 莱姆似乎尝试安慰你 # 可选，给 AI 的提示（注意是 prompt，不是 dialog_prompt）
  end_prompt: 莱姆安慰好你啦 # 可选，结束时的 AI 提示
```

| 字段         | 必填 | 类型   | 默认值    | 说明                                      |
| ------------ | ---- | ------ | --------- | ----------------------------------------- |
| `character`  | 否   | string | `default` | 参与对话的角色 id                         |
| `hint`       | 否   | string | —         | 玩家提示                                  |
| `max_rounds` | 否   | int    | `-1`      | 最大对话轮数，-1 = 无限                   |
| `end_line`   | 否   | string | `结束`    | 玩家输入包含此文本则结束                  |
| `prompt`     | 否   | string | —         | 给 AI 的对话提示（源码字段名是 `prompt`） |
| `end_prompt` | 否   | string | —         | 结束回合给 AI 的提示                      |

> ⚠️ 旧文档写 `dialog_prompt`，源码实际字段为 **`prompt`**。

---

## 五、Galgame 机制事件

### 15. set_variable — 设置变量

```yaml
- type: set_variable
  options:
    - condition: flag == true # 可选，执行条件
      actions:
        - type: set_var
          content: 'money += 100' # 表达式：变量 = 值 / 变量 += 值 / 变量 -= 值
```

| 字段                  | 必填 | 类型 | 默认值 | 说明                  |
| --------------------- | ---- | ---- | ------ | --------------------- |
| `options`             | ✅   | list | —      | 条件-动作组列表       |
| `options[].condition` | 否   | expr | —      | 满足才执行 actions    |
| `options[].actions`   | ✅   | list | —      | 执行的 `set_var` 动作 |

`set_var` 详情（引擎用**表达式写法**；旧原型 `name/value/op` 会被校验器标记为 `action.legacy_shape`）：

| 字段      | 必填 | 类型   | 说明                                                                                                    |
| --------- | ---- | ------ | ------------------------------------------------------------------------------------------------------- |
| `content` | ✅   | string | 表达式：`变量 = 值` / `变量 += 值` / `变量 -= 值`；值支持 `random(min,max)`、`null`、字符串、数字、布尔 |

条件表达式（`evaluate_condition`）：

- `var` — 变量为真（truthy）
- `var == value` — 等于
- `var != value` — 不等于

### 16. chapter_end — 章节结束

```yaml
# ① linear：线性结束
- type: chapter_end
  end_type: linear
  next_chapter: 'end' # 或 next: 'end'；'end' 表示剧本结束

# ② branching：按变量分支
- type: chapter_end
  end_type: branching
  options:
    - condition: flag == true
      next: 'good_end'
    - condition: flag == false
      next: 'bad_end'
    - default: true # 可选，兜底分支
      next: 'normal_end'

# ③ ai_judged：AI 判定分支
- type: chapter_end
  end_type: ai_judged
  prompt: 根据当前剧情走向判断玩家会进入哪个结局
  options:
    - name: good # 分支名（AI 从中选择）
      next: 'good_end'
    - name: bad
      next: 'bad_end'
    - default: true # 可选，兜底
      next: 'normal_end'
```

| 字段                    | 必填      | 类型   | 默认值   | 说明                                           |
| ----------------------- | --------- | ------ | -------- | ---------------------------------------------- |
| `end_type`              | 否        | string | `linear` | `linear` / `branching` / `ai_judged`           |
| `next_chapter` / `next` | 条件必填  | string | —        | 下一章节路径（相对 `Chapters/`），`end` = 结束 |
| `options`               | 分支必填  | list   | —        | 分支选项列表                                   |
| `options[].condition`   | 分支      | expr   | —        | branching 分支条件                             |
| `options[].name`        | ai_judged | string | —        | AI 可选的结局名                                |
| `options[].next`        | 分支      | string | —        | 对应下一章节                                   |
| `options[].default`     | 否        | bool   | —        | 兜底分支标记                                   |
| `prompt`                | ai_judged | string | —        | 给 AI 的判定提示                               |

### 17. unlock_achievement — 解锁成就

```yaml
- type: unlock_achievement
  achievement_id: 'summer_star' # 成就键名（唯一，英文标识）
  title: 夏日之星 # 成就标题
  description: 在夏天的星空下许下愿望。 # 成就描述
```

| 字段             | 必填 | 类型   | 默认值 | 说明                                                                         |
| ---------------- | ---- | ------ | ------ | ---------------------------------------------------------------------------- |
| `achievement_id` | ✅   | string | —      | 成就键名（英文标识），**不能与内置成就或本剧本其他成就重名**（校验器会提示） |
| `title`          | ✅   | string | —      | 成就标题，玩家在成就列表里看到的名字                                         |
| `description`    | ✅   | string | —      | 达成条件说明，展示给玩家                                                     |

---

## 事件类型速查表

| type                 | 用途             | 必填字段                                 |
| -------------------- | ---------------- | ---------------------------------------- |
| `narration`          | 旁白             | `text`                                   |
| `player`             | 玩家固定台词     | `text`                                   |
| `dialogue`           | AI 固定台词      | `character`, `text`                      |
| `ai_dialogue`        | AI 生成台词      | 无（`character` 默认 MAIN）              |
| `input`              | 玩家输入         | 无                                       |
| `choices`            | 玩家选项         | `options`                                |
| `free_dialogue`      | 多轮自由对话     | 无                                       |
| `background`         | 背景             | `imagePath`                              |
| `music`              | 背景音乐         | `musicPath`                              |
| `sound`              | 音效             | `soundPath`                              |
| `ambient`            | 环境音           | `ambientPath`                            |
| `background_effect`  | 背景特效         | 无                                       |
| `present_pic`        | 展示图片         | `imagePath`                              |
| `modify_character`   | 显示/隐藏/改角色 | `character`                              |
| `set_variable`       | 设置变量         | `options`                                |
| `chapter_end`        | 章节结束         | `end_type` + (next/options)              |
| `unlock_achievement` | 解锁成就         | `achievement_id`, `title`, `description` |
