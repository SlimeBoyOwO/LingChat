# 剧本编辑器逻辑（editor）

> 编辑流程数据流图见 [diagrams/editor-flow.html](diagrams/editor-flow.html)，事件 Schema UML 见 [diagrams/event-schema.html](diagrams/event-schema.html)。

## 1. 界面结构（ScriptEditor.vue）

`/script-editor` 是一个独立路由（懒加载，避免进主 chunk）。未打开剧本时是剧本列表 + 「新建剧本」；打开后是五个页签：

| 页签 | 内容 |
|---|---|
| 章节流程 flow | 章节流程图（ChapterFlow）；双击进章节编辑 |
| 剧本设置 config | story_config.yaml 表单 + 羁绊冒险（绑定角色 / 排序） |
| 角色 characters | 剧本内角色列表，新建 / 删除 / 从全局角色库导入 |
| 素材 assets | 五类素材（背景 / 插图 / 音乐 / 音效 / 环境音），剧本内 + 全局两栏，缩略图 / 原生音频播放器 |
| 校验 validate | 诊断列表，按章节聚合，点击跳转 |

章节编辑页是**左右两栏**：左侧事件时间线（ChapterTimeline），右侧事件属性（EventPropertyPanel）。顶部有撤销 / 恢复 / 快捷键按钮，右上角「从本章试玩 / 从开场试玩」。

## 2. Schema 驱动的编辑

**表单不是手写的** —— 全部由 Rust 导出的 schema 驱动：

- `getters.eventSpecs`：`type_key → EventSpec` 映射；
- **插入事件面板**按 `schema.events[].category` 分组，16 种事件全列出；
- `blankEvent(typeKey)` 按字段生成骨架：
  - `choice_options` → `[{ text: '', actions: [] }]`
  - `var_options` → `[{ actions: [{ type: 'set_var', content: '' }] }]`
  - `character` → `'MAIN'`，`select` → 第一个候选，`bool` → `false`
  - `chapter_end` → 自动补 `next_chapter: 'end'`（否则一插入就报「linear 但没写下一章」）；
- **换事件类型**（`replaceEvent`）不是简单改 `type`：按「字段名相同 **且** 控件类型相同」保留旧值。只比字段名会把 `choices.options`（`[{text, actions}]`）原样搬进 `set_variable.options`（`[{condition, actions}]`），语义完全不同 —— 复合类型之间一律不继承；
- `FieldRow` 按 `FieldKind` 渲染控件；`select` 的候选项归属各异（固定表 / 情绪表 / 章节列表 / 角色列表），`asset` 渲染「下拉 + 剧本内导入 + 全局导入」，并**合并**剧本内与全局素材（引擎查找顺序是先剧本内再全局，两处都能被找到）；
- **变量 / 条件结构化表单**（`FieldKind::Condition` + 复合编辑器内的 `ConditionEditor` / `VariableEditor`）：作者不用手写 `route == shop`、`flag = warm` 这类表达式，而是「变量名 + 关系（为真/等于/不等于）+ 值」或「变量名 + 运算（设/加/减）+ 值类型（文本/数字/布尔/随机数）」。底层仍序列化为引擎认的字符串（解析镜像 `evaluate_condition` / `parse_variable_action`，见 `src/utils/scriptVar.ts`），校验器 / 引擎零改动；`>`、`&&` 等不成立的写法在表单里根本输不出来。无法解析的旧写法（如旧版 `name/value/op` 形状、`hp > 5`）只读展示，交给校验器诊断。`choices` 的选项级 `condition` 也在这轮补上了可视化编辑。

可选 `bool` 字段用**三态下拉**（不设置 / 开启 / 关闭）而不是两态开关：引擎对这类字段的默认值往往不是 `false`（比如环境音的 `loop` / `fade` 默认 `true`），两态开关会让「没写过这个字段」和「显式写了 false」长得一模一样。

## 3. 编辑操作与撤销栈

所有修改事件的操作（插入 / 删除 / 复制 / 移动 / 改字段 / 换类型 / 改章节名）都先调 `pushHistory()`：

```
改动前深拷贝整章 events + 选中下标
  → push 到 undoStack（上限 UNDO_LIMIT = 100 帧）
  → 新改动清空 redoStack
```

快捷键：`Ctrl/⌘+Z` 撤销、`Ctrl/⌘+Shift+Z`（或 `Ctrl+Y`）恢复、`Ctrl/⌘+D` 复制事件、`Alt+↑/↓` 移动事件、`Delete` 删除、`Ctrl/⌘+Enter` 试玩、`?` 快捷键表。

几个操作语义值得注意：

- **`insertEvent` 默认插到「最后一条 chapter_end 之前」**而不是数组末尾 —— 新章节自带一条 `chapter_end`，插到它后面每次都会立刻报「章节结束之后还有事件，永远不会执行」；
- **`moveEventRange`（整段移动）**：时间轴把「转场」「AI 互动轮次」折叠成一行，拖那一行时移动的是整块而不是一条，拆开搬会把这几条事件打散；
- **`setEventField` 空值删键**：`value === '' / null / undefined` 时删除键，避免往 YAML 里写一堆空字符串。

## 4. 防抖自动保存

`markDirty()` 做三件事：

1. `dirty = true`，`revision++`；
2. 设置**自动保存防抖**（`AUTOSAVE_DELAY = 800ms`）→ 到期 `save()`；
3. **同时**设置**校验防抖**（`VALIDATE_DELAY = 2500ms`）→ 到期 `runValidation()`。

`save()` 的关键是 **revision 比对 + savePending**：

- 保存是异步的，落盘期间用户可能又改了东西 —— 用 `revision` 快照比对：**只有期间没有新改动才算干净**（`dirty = false`），否则保持脏；
- 若已有一次落盘在飞（`saving`），记 `savePending = true`，等它结束后**再写一次**，而不是直接丢掉。

校验比保存重得多（要扫全部剧本查重 + 逐章读盘），所以单独用更长的防抖。

`backToFlow()`（从章节编辑退回流程图）刻意**先强制落盘再重新校验**：流程图读的是 `report.edges`，而校验读的是**磁盘**。改完「下一章」立刻退回来时，两个防抖都还没到点，图上仍是旧连线 —— 作者会以为改动没生效。这里的强制走一遍，代价是退回时多等一下，换来「看到的就是真的」。

## 5. 事件折叠（useEventFolding）

折叠依据是官方六个剧本的实际写法，把高频复现的固定套路折成一行：

| 复合块 | 模式 | 摘要 |
|---|---|---|
| 转场 | `角色退场 → 旁白* → 背景 → 特效? → 角色出场` | 直接显示目标背景名 |
| AI 互动轮次 | `AI对话 → 等待输入 → AI对话` | 显示 prompt |

折叠后 `main4` 从 15 行降到 8 行。折叠用**稳定 key**（`g-transition-<i>`），不用行下标 —— 否则在插入 / 删除事件后展开态会跟错行。`foldEvents` 的 `enabled` 为 false 时原样返回逐条事件（一键退回不折叠）。`firstVisibleIndex` 返回第一个没被折叠进复合块的事件下标，避免「选中项看不见」（官方剧本每章开头都是一个转场块，直接选 0 会出现右侧显示字段、左侧那行是收起的转场）。

`eventSummary` 把每种事件的关键字段浓缩成一句人话（时间线上显示）。

## 6. 章节流程图（ChapterFlow）

**这张图是读出来的，不是排出来的。** 连线来自每章最后那条「章节结束」（由校验器反推出 `ChapterEdge` 列表），章节的先后也由它决定。要改走向，进对应章节改那条事件的「下一章」。

- 分层布局：**从开场章节沿 edges 广度优先**分层，每层一行；分支层上方画分叉提示；孤章节（走不到）单独挂末尾；
- 节点徽标：开场绿标、孤章节黄标（`graph.unreachable` 诊断）、错误 / 提醒计数、分支类型（条件分支 / AI 判定分支）、章节内事件数；
- 这是对早期实现的重写：早先按章节文件名字典序排一列，箭头表达的是「id 的字母顺序」而不是真实跳转 —— 看起来对但完全不对。

## 7. 素材与角色管理

### 素材（assets）

- 落点分 `script`（剧本独有，随剧本分发）与 `global`（所有剧本共享，导出时不带走）；
- 引擎查找顺序是「先本剧本 `Assets/`，再全局 `game_data/`」，所以两处都能被找到，下拉里必须都列出；
- **音效（sound）例外**：全局没有音效目录（`MediaType::fallback_dir()` 会 fallback 到全局音乐目录，但音效本就该是剧本私有素材），所以全局列表直接返回空；
- 导入只传**源文件路径**，Rust 自己 `fs::copy`（理由见 architecture.md §6）；
- 素材页用 `editor_list_asset_files` 拿到绝对路径 + 体积，图片出缩略图（`convertFileSrc` 转 asset URL）、音频给原生播放器（隐藏 Chromium 的「更多选项」溢出菜单，速度由自制 `{rate}× ▾` 按钮提供）。

### 角色（characters）

- 剧本里用 `character: <引用名>` 指代角色，写 `MAIN` 表示当前主角（羁绊剧本里就是绑定的那位）；
- **引擎只在本剧本的 `characters/` 里找人** —— 想用全局角色库已有的人设必须「导入」一份；
- 导入复制的是 settings.yml（**不是直接引用**，引擎解析 `character:` 只走剧本自己的目录），并补写 `script_role_key`、摘掉全局角色特有字段；立绘默认不复制（引擎找立绘本来就先看全局同名目录），可勾选「连立绘一起复制」用于单独分发剧本；
- 角色卡片显示：`ai_name`、`character: roleKey`、表情 / 服装数、立绘缩略图（本地 avatar 优先，回退全局，都没有时提示「立绘不会显示」）、「立绘读自全局」徽标。

## 8. 打开章节时的自动纠错

`openChapter` 在读入章节内容后做一次**背景特效大小写自动纠错**：`starfield → StarField`（用前端粒子注册表 `canonicalEffectKey`）。命中且与原值不同就改回并标脏，让防抖自动保存把规范写法落盘 —— 下次校验 / 运行就不再 warn；未命中的（真未知特效）不强行改写，留给 validate / runtime warn。
