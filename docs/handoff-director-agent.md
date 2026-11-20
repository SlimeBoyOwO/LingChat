# 导演 Agent 实现交接清单（供代码审核）

日期：2026-11-20 · 分支 feature/director-agent-tools · HEAD 9838b42
基线：2e5a80a（origin/dev）· 提交数 15 · 改动 20 文件 +3249/-107

---

## 一、任务来源与目标

Issue #948（自由剧本模式）Roadmap 的第 2、3 步：

- 步骤 2：把本项目所有剧本事件注册为工具函数，供导演 Agent 调用
- 步骤 3：提供导演模式开关，每 X 轮对话由导演生成一段约 10 个事件的小剧本

步骤 1（导演侧栏 MVP）在本次之前已由他人完成（提交 42883fc），本次未改其主体。

---

## 二、核心决策（审核重点，详见 docs/decisions-director-agent.md）

| 编号 | 决策 | 理由 |
|---|---|---|
| D1 | 导演工具仅在自由对话开放；固定剧本运行期间禁用 | 剧本运行时插入事件会与事件下标、剧本变量、存档恢复点竞争 |
| D2 | 工具调用采用入队语义，不等前端播完 | 播放队列本就在前端 src/core/events/event-queue.ts，后端 emit 即入队 |
| D3 | 首期只注册 8 类确定性事件；dialogue / set_variable 降级为拒绝占位 | 两者硬依赖 ScriptStatus，自由对话下为 None，硬造容器会污染存档 |
| D4 | 周期小剧本必须先预览确认才播放 | 模型可能产出非法参数，10 事件直插没有反悔机会 |

审核时请重点确认这四条是否被正确落实。

---

## 三、架构设计

### 后端模块（crates/ling-chat-main/src/ai_service/god_agent/director/）

| 文件 | 行数 | 职责 |
|---|---|---|
| mod.rs | +6 | 模块声明与视图类型重导出 |
| core.rs | +491 | 会话状态、多轮工具循环 run_chat、上下文装配、草案触发 |
| tools.rs | +621 | 导演专用 ToolRegistry、2 个只读工具、循环规划器、共 12 个工具、10 个单测 |
| event_tools.rs | +384 | 8 类事件工具（定义从 schema.rs 生成）+ 2 个拒绝占位 + 参数校验 |
| draft.rs | +471 | 周期草案状态机、JSON 解析校验、6 个单测 |

### 关键设计点

1. 工具注册表完全独立：不接入 GeneratorSource / 权限矩阵，不进工具权限页
2. 事件定义单一真相源：工具 JSON Schema 从 script_engine/schema.rs 的 build_schema() 转换，导演侧不另抄字段表
3. 执行路径复用：S2 工具与 S3 草案确认共用 execute_director_event()，不复制粘贴
4. D1 双层闸门：api/director.rs 的 director_tools_enabled() 判定（不下发定义）+ core.rs 执行层拒绝路径（防模型照历史硬调）
5. 循环上限：MAX_TOOL_ROUNDS=8、MAX_TOOL_CALLS_PER_ROUND=6，超限走不带工具的收尾生成

### 关键参数

```
DEFAULT_INTERVAL_ROUNDS   = 5    // 默认每 5 轮触发
MAX_DRAFT_EVENTS         = 12   // 单份草案最多 12 事件
DRAFT_MAX_ATTEMPTS       = 2    // 生成失败重试次数
MAX_TOOL_ROUNDS          = 8    // 工具循环轮数上限
MAX_TOOL_CALLS_PER_ROUND = 6    // 单轮工具调用上限
```
---

## 四、Tauri 命令（11 个，注册于 src-tauri/src/app/commands.rs）

```
保留（MVP 已有）：
  director_get_state / director_start_chat / director_stop_chat
  director_clear / director_get_context / director_get_tool_history

本次新增：
  director_get_mode()      -> { enabled, interval, lastGeneratedSeq, pendingDraft: Draft|null, status }
  director_set_mode(enabled, intervalRounds?) -> 同上
  director_get_draft()     -> Draft | null
  director_confirm_draft() -> { ok, applied, error }
  director_discard_draft() -> Draft | null
```

Draft 结构：{ status, events: [{event, params}], error, createdSeq }
status 取值：idle | generating | pending | applying | failed

---

## 五、前端改动

| 文件 | 行数 | 内容 |
|---|---|---|
| stores/modules/director.ts | +219 | activeTools 状态机、directorMode/draft 运行态、5 个新 action，全部进 persist exclude |
| components/director/DirectorDraftTab.vue | +288 | 新建：模式开关、轮数输入、草案事件列表、确认/丢弃、轮询 |
| components/director/DirectorToolSummary.vue | +143 | 新建：聊天流内工具调用紧凑摘要（details 折叠） |
| components/director/DirectorToolHistoryTab.vue | +157 | 加「本轮调用」实时区，保留历史区 |
| components/director/DirectorChatTab.vue | +18 | 挂摘要组件、滚动跟随感知工具状态 |
| components/director/DirectorSidebar.vue | +7 | 第 4 个 Tab「小剧本」（Clapperboard 图标） |
| api/services/director.ts | +49 | 4 个 DTO + 5 个 invoke 封装 |
| locales/{zh-CN,en,ja,zh-HK}/game.ts | +284 | tabs.draft + draft.* 键，四语一致 |

注意：en / ja / zh-HK 原本整个 director 键区都缺失，本次一并补齐（各 62 行）。

---

## 六、验证记录（实际执行过的命令）

### 已通过

```
cargo check --workspace                     -> exit 0（5分33秒，7 个无关 warning）
cargo build --release -p ling_chat          -> exit 0（36分21秒）
node .../vue-tsc.js --noEmit --skipLibCheck -> exit 0
node .../vite.js build                      -> exit 0
cargo test -p ling-chat-main --lib director -> 20 passed / 0 failed
```

### 测试覆盖（20 个）

- tools.rs：注册表按名取工具、未知工具拒绝、事件清单覆盖、8 类工具名与必填字段、未知事件拒绝、必填缺失、缺宿主报错、字段类型与 Select 校验、工具轮数上限、单轮调用上限、模型不调工具收尾
- draft.rs：轮次差值、合法草案解析、白名单与非法参数整份拒绝、状态机防重复、全部成功才推进进度、切档重置令牌失效

### 未验证（受环境限制）

- GUI 端到端（无法在开发沙箱创建 WebView2 窗口）
- 真实 LLM 多轮工具调用
- 事件 emit 到前端队列的实际播放效果
---

## 七、已知问题与待办

### 打包问题（当前阻塞用户使用）

用户报告打包后的 exe「打开后自己关掉」。已确认：

1. 首次打包错了：用 cargo build --release 直接编译，exe 只有 58.4MB，前端未嵌入。正确应走 tauri build
2. 已重打：tauri build --no-bundle 产出 111MB exe（与可运行的 LingChat-Portable 版 110.1MB 相当）
3. 分发目录：D:\personal_vibe\galgame\LingChat-dev-dist\（231.5MB），含 exe + 3 DLL + data/ + webview2-data/
4. 仍未确认能否运行：用户的崩溃原因待查

### 疑似根因线索

- 用户机器 WebView2 版本为 133.0.3065.92（2025/3/1），8 个多月未更新
- 系统 %LOCALAPPDATA%\com.noiq.ling-chat\ 下 EBWebView 本体缺失，只有两个改名备份（_bak_20261015、_old_20261111b），曾用后者恢复
- 应用中 apply_webview2_color_profile() 会写 WEBVIEW2_ADDITIONAL_BROWSER_ARGUMENTS（HDR 关闭时）
- 建议审核方向：确认 WebView2 版本兼容性、webview2-data 是否被正确使用

### 环境限制（非代码缺陷）

- 开发沙箱禁止命名管道，WebView2 无法创建 -> GUI 无法运行
- 沙箱 cargo 编译需把 TMP 指向 workspace 内并注入 MSVC 环境变量（docs/decisions-director-agent.md 有记录）

---

## 八、审核建议顺序

1. 先读 docs/decisions-director-agent.md，确认 D1-D4 与实现一致
2. 审 director/tools.rs：注册表独立性、循环规划器纯函数、测试有效性
3. 审 director/event_tools.rs：schema 转换正确性、参数校验覆盖度、execute_director_event 复用是否干净
4. 审 director/core.rs 的 run_chat：多轮循环终止条件、取消处理、工具消息配对、落库顺序
5. 审 director/draft.rs：状态机竞态（令牌机制）、parse_draft 校验严格性
6. 审 api/director.rs：D1 闸门、返回值契约
7. 审前端：store 状态机、DirectorDraftTab 轮询与清理、四语一致性
8. 跑验证：pnpm check:rs、cargo test --workspace、pnpm build

---

## 九、提交历史

```
9838b42 test：修正导演注册表的两个错误断言
ac230b1 fix：修复导演草案触发时的临时值借用错误
4be5d1a chore：全局pnpm修复后还原通用钩子写法
bb8285a fix：提交钩子绕开本机损坏的pnpm shim
a8d3552 docs：同步决策记录与S1-S3最终实现
06eb174 feat：导演模式开关与周期小剧本预览确认
e05e86b feat：导演Agent注册确定性剧本事件工具
6d38ac4 docs：D3补记dialogue因角色空间缺失首期降级
dc57906 docs：校准D2入队语义为emit到前端事件队列
fb6ff11 docs：修正D3事件范围，set_variable首期因变量容器缺失延后
9aa14fd feat：导演Agent接入多轮工具循环与剧本运行闸门
2a84842 feat：新增导演Agent专用工具注册表与首批只读工具
da1827d docs：补充本地环境限制与验收取证调整
64e4ffc docs：记录导演Agent步骤2/3的架构决策
6521f56 feat：导演侧栏工具调用实时状态展示（本轮调用 + 聊天流摘要）
```

未提交改动：vite.config.ts（host: host || false 改为 true，修复 localhost/IPv6 白屏）

---

## 十、代码风格约定（本项目 CLAUDE.md）

- 中文注释与 commit message，commit 单行标题用全角冒号（feat：xxx），不加 Co-Authored-By
- 最小化改动，MVP 原则，不过度设计
- Rust 格式化字符串优先内联捕获变量
- Tailwind CSS v4，不使用传统 CSS
- src-tauri/src/app/commands.rs 是唯一命令注册表

---

## 十一、文档索引

| 文档 | 内容 |
|---|---|
| docs/decisions-director-agent.md | D1-D4 决策记录、架构基线、环境限制、最终实现状态 |
| docs/handoff-director-agent.md | 本文件，交接清单 |
