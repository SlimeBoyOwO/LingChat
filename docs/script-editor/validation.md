# 剧本自动错误检测机制（validation）

> 校验流程图见 [diagrams/validation-flow.html](diagrams/validation-flow.html)。

## 1. 目标与总原则

**目标：把引擎里所有「静默失败」变成作者能看见的一条诊断。**

判定逻辑**尽量复用引擎自己的函数**，避免校验器和运行时各说一套：

- `resolve_script_media` 查素材是否存在；
- `parse_variable_action` 解析变量表达式；
- `KNOWN_EFFECTS` 判特效是否合法。

诊断分三级：

| 级别 | 含义 | 例子 |
|---|---|---|
| `error` | 一定会出问题（跑不通、跳不过去、素材缺失） | 必填字段缺失、悬空跳转、素材找不到 |
| `warn` | 很可能不是作者的意图 | 写了不生效的字段、孤儿章节、循环 |
| `info` | 提示性的 | 遗留字段、可疑但合法的写法 |

**保存时不拦，只在「试玩 / 导出」时拦 error。**

## 2. 入口与返回

```rust
pub fn validate(
    data_dir: &Path,
    script_dir: &Path,
    script_key: &str,
    other_script_names: &HashMap<String, String>,  // script_name → 剧本 key，用于查重
) -> ValidationReport
```

`editor_validate_script` 会先扫盘收集**其他剧本的 script_name** 用于查重（引擎用 script_name 作索引，重名会让其中一个剧本在列表里完全消失）。

`ValidationReport` 结构：

```
ValidationReport
├── diagnostics: Vec<Diagnostic>    // 已排序：error → warn → info，同级按章节 + 事件序
│     Diagnostic { severity, code, message, chapter?, event_index?, field? }
├── error_count / warn_count / info_count
├── variables: Vec<String>          // 收集到的全部变量名（供编辑器做变量面板）
└── edges: Vec<ChapterEdge>         // 章节跳转边（供前端画真连线 + 判断能否拖拽重排）
```

## 3. 校验流水线

```
① story_config
   ├─ 读失败                 → config.unreadable · error（提前 return）
   ├─ script_name 空         → config.no_script_name · warn（列表显示目录名）
   ├─ script_name 重复       → config.duplicate_name · error
   ├─ intro_chapter 缺失     → config.intro_missing · error（默认回落 "main"）
   └─ Chapters 下没有 .yaml  → chapters.empty · error（引擎只认 .yaml，不认 .yml）

② 剧本内 NPC
   ├─ 缺 script_role_key     → character.no_role_key · warn（引擎不加载它）
   └─ 空人设（system_prompt 为空）→ character.no_persona · info
      （把后台日志「role_id=N 没有找到 SYSTEM 属性的台词，可能人设丢失」提前成可见提示）

③ 逐章节：resolve → read → parse → ChapterDoc
   ├─ 章节无事件             → chapter.no_events · warn（运行时会立刻结束整个剧本）

④ 逐事件（见 §4）

⑤ 章节图 check_graph（见 §5）

⑥ 变量分析（见 §6）

⑦ finish：排序、计数、返回
```

## 4. 事件级检查

对每个事件先做**结构 / 通用检查**，再做**逐类型细查**。

### 结构 / 通用

| 诊断码 | 级别 | 触发 |
|---|---|---|
| `event.not_a_map` | error | 事件不是键值映射 |
| `event.missing_type` | error | 缺 `type` 字段 |
| `event.unknown_type` | error | 未知事件类型，运行到这里整个剧本中断 |
| `field.required_missing` | error | 缺必填字段 |
| `field.unknown` | warn | 未知字段（很可能是拼错），会被静默忽略 |
| `field.inert` | info | 遗留字段（`duration` 等 `enabled == false` 的通用字段），引擎从不读取 |
| `condition.unsupported_operator` | error | 用了 `&& || >= <= > < ! ( )`（长运算符优先匹配） |
| `condition.no_variable` / `condition.bad_variable` | error | 条件左侧没有变量名 / 变量名含空格 |
| `condition.placeholder_not_replaced` | warn | condition 里的 `%player%` 不会被替换 |

条件检查有个细节：**只扫运算符左侧**。右值是任意字符串，`bg == city/night` 里的 `/` 是合法内容 —— 早先在整串上找 `/ * ( )` 会把它误判成「用了不支持的运算符」并跳过变量收集。只支持 `var == 值` / `var != 值` / 裸变量真值三种写法；比较是**字符串比较**，未定义变量 `==` 恒假、`!=` 恒真。

### 逐类型细查（`match type`）

**素材类** `background / present_pic / music / sound / ambient`：

| 诊断码 | 级别 | 触发 |
|---|---|---|
| `asset.missing` | error | `resolve_script_media` 找不到素材（运行时只静默清空画面/声音） |
| `music.bad_speed` | warn | 播放速度 ≤0 或 >4（>2 通常失真） |

`ambient` 有个刻意例外：`stop: true` 或路径为空时跳过路径解析（留空表示「停掉全部轨道」，标成必填会让这种正常用法被误判）。

**`background_effect`：**

| 诊断码 | 级别 | 触发 |
|---|---|---|
| `effect.case` | info | 大小写不对（前端打开章节时自动纠正，故只给 Info） |
| `effect.unknown` | warn | 不是内置特效（前端无法纠正），引擎会清空当前特效 |

**`choices`：**

| 诊断码 | 级别 | 触发 |
|---|---|---|
| `choices.empty` | error | 选项列表为空 |
| `choices.option_not_a_map` | error | 选项不是键值映射 |
| `choices.catch_all_not_last` | warn | 无文案的兜底项没放最后（会吞掉后面的选项） |
| `choices.duplicate_text` | warn | 文案重复，后一个永远选不到 |
| `choices.placeholder_in_text` | warn | 文案里有 `%player%`（引擎只替换顶层字段） |
| `choices.option_next_ignored` | error | 选项写了 `next`（choices 不支持选项级跳转，该字段被完全忽略） |

`set_variable` / `chapter_end` 的事件，其 `options[].actions` 还会过 `check_actions`：

| 诊断码 | 级别 | 触发 |
|---|---|---|
| `action.not_a_map` | error | action 不是键值映射 |
| `action.bad_expression` | error | `set_var` 表达式无法解析（只支持 `= += -=` 三个运算符） |
| `action.not_supported_here` | warn | 在 `set_variable` 里写了 `add_line`（该事件只处理 set_var） |
| `action.empty_content` | warn | `add_line` 内容为空 |
| `action.unknown_type` | warn | 未知动作类型 |

**`set_variable`：**

| 诊断码 | 级别 | 触发 |
|---|---|---|
| `set_variable.no_options` | error | 缺 options（**检测原型形状**：原型写的是 `{name, value}`，引擎只读 `options[]`） |
| `set_variable.empty` | warn | 赋值组为空，事件什么都不做 |

**`free_dialogue`：**

| 诊断码 | 级别 | 触发 |
|---|---|---|
| `free_dialogue.no_exit` | error | `max_rounds ≤ 0` 且 `end_line` 为空 → 永远无法结束 |

**`chapter_end`：** 见下。整章还要求**必须有一条 `chapter_end`**：没有 → `chapter.no_end · error`（引擎会把这当成整个剧本结束，而不是接着下一章）。

## 5. 章节图分析（check_graph）

用收集到的 `ChapterEdge` 建图，做三类分析：

| 诊断码 | 级别 | 说明 |
|---|---|---|
| `graph.unreachable` | warn | 孤儿章节：从开场章节 DFS 走不到（分「有入边但从开场不可达」与「无任何入边」两种文案） |
| `graph.cycle` | warn | 章节之间存在循环（DFS 三色法找环，输出环路径）。**引擎没有循环检测**，玩家可能被困在里面出不来 |
| `chapter_end.dangling` | error | 跳转目标章节不存在（在 `push_target` 里报，同时产出边） |

`chapter_end` 的专项检查较细：

- **linear**：`next` 与 `next_chapter` 并存 → `chapter_end.both_next_fields · warn`（引擎只优先用 `next`）；都没写 → `chapter_end.no_next · warn`（直接结束整个剧本）；`next` 是遗留字段（Deprecated），编辑器只展示不给编辑；
- **branching / ai_judged**：缺 options / 空 options → `chapter_end.no_options · error`；分支写成选项形状（`text/actions`）→ `chapter_end.choice_shaped_option · error`（**检测原型形状**）；分支无条件 → `branch_no_condition · warn`；分支无 `next` → `branch_no_next · error`；无 `default` 兜底 → `no_default_branch · warn`；`ai_judged` 分支缺 `name` → `ai_option_no_name · warn`；未知 `end_type` → `chapter_end.unknown_end_type · error`；
- 位置：`chapter_end` 不在最后 → `chapter_end.not_last · warn`（后面的事件永远不会执行）。

**章节图的边就是前端流程图的连线**：在这之前流程图只是把章节按文件名字典序排了一列，箭头表达的是「章节 id 的字母顺序」而不是真实跳转 —— 看起来对但完全不对。现在 `ChapterEdge { from, to, is_end, label, end_type }` 由校验器从每章最后一条 `chapter_end` 反推，前端据此画真连线，并据此判断能否拖拽重排（分支章节后端拒绝重排）。

## 6. 变量分析

校验过程收集两类变量集合：

- `vars_written` —— 被赋值过（来自 `set_variable` / `choices` 的 `set_var` action，用 `parse_variable_action` 解析出变量名）；
- `vars_read` —— 在条件里出现过（来自 `condition` 字段）。

| 诊断码 | 级别 | 触发 |
|---|---|---|
| `variable.never_set` | warn | 条件里用到但整个剧本都没赋值。没赋值用「等于」比较恒不成立、「不等于」恒成立 |
| `variable.never_read` | info | 被赋值但从未在任何条件里使用 |

变量全集（`vars_written ∪ vars_read`）随报告返回，供编辑器做**变量面板**。

## 7. 诊断码设计

诊断码是**稳定的机器可读字符串**，带类型前缀，前端据此做跳转 / 过滤：

```
config.* / chapter.* / event.* / field.* / asset.* / music.* / effect.*
choices.* / action.* / set_variable.* / free_dialogue.* / chapter_end.*
graph.* / variable.*
```

一个设计细节：`Severity` 原先是 `&'static str`，写错一个 `"warning"` 能编译、能序列化、排序落到兜底分支、前端当成 info —— **全链路静默**。改成 enum 让编译器兜住。

## 8. 触发时机与前后端协作

- **触发**：编辑器打开剧本时跑一次；`markDirty()` 后 2.5s 防抖自动跑；手动「重新校验」；`backToFlow()` 退回流程图时强制跑；试玩启动前强制跑。
- **前端消费**：
  - 校验页：按章节聚合 + 点击跳转到对应事件（`jumpTo` 处理 `openChapter` 失败）；
  - 时间线：事件行打标（`chapterDiagnostics` 按事件下标归组，复合块显示「含错误」）；
  - 流程图：节点错误 / 提醒角标 + 孤章节标识 + 真实连线；
  - 属性面板：逐字段诊断提示（`fieldDiagnostics` 按 field 过滤）；
  - 试玩拦截：`hasBlockingErrors`（`errorCount > 0`）→ 跳校验页，拒绝试玩。
- **大小写纠错**：`background_effect` 的大小写问题在校验里只给 info，前端 `openChapter` 时会用 `canonicalEffectKey` **自动纠正**并标脏落盘（见 editor.md §8）—— 下次校验 / 运行就不再报。

## 9. 校验器的自测

`validate.rs` 内嵌单测覆盖了各种典型误用，包括「原型编辑器犯的错」：

- 条件的不支持运算符 / 空格变量名 / 合法形式与变量收集；
- `choices` 的选项级 `next`、文案重复、兜底没放最后、action 类型写成 `set_variable`（引擎只认 `set_var`）、文案里的 `%player%`；
- `set_variable` 的原型形状（`{name, value}`）；
- `chapter_end` 分支写成选项形状、悬空目标、`end` 是合法终点；
- 图的孤儿与环（`a→b→a` 环 + 两个不可达章节）。
