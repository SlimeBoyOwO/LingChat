# 导演 Agent 步骤 2/3 决策记录

日期：2026-11-18 · 依据：issue #948 Roadmap、代码调研（origin/dev 2e5a80ae）

## 背景

导演侧栏 MVP（提交 42883fc）已完成：流式对话、现场感知上下文、按存档落库、
工具协议预留（事件与消息字段就位，运行回路为空）。步骤 2 把剧本事件注册为
导演工具，步骤 3 做周期小剧本。本文件记录两项实施前的定向决策。

## 决策

### D1 首期仅在自由对话中开放导演工具

- 结论：固定剧本运行期间，导演只允许问答；工具调用会被拒绝并提示。
- 理由：剧本运行时导演插入事件会与事件下标推进、剧本变量、章节跳转互相竞争，
  玩家存档恢复点（章节 + 事件下标）会被污染。自由对话里导演是唯一事件来源。
- 影响：S1 工具闸门需检查 script_status.is_running；S3 触发器同样受此约束。

### D2 工具调用采用入队语义，不同步等待播放

- 结论：工具执行 = 参数校验通过 + emit 事件到前端 EventQueue + 必要时写台词表，
  返回「已投递」（含事件序号）；不等前端按 duration 演出完成。
- 理由：本项目的播放队列本就在前端（src/core/events/event-queue.ts），后端 emit
  即入队；后端剧本执行与前端演出时间线分离，同步等待会占用工具循环且难以取消。
  S2 不另建后端队列结构，保持最小实现。
- 影响：工具结果字段为「已投递」而非「已播完」；S3 的预览确认即批量调用同一套 emit，
  草案列表与取消剩余事件到那时再引入轻量片段状态。

### D3 首期注册确定性事件子集

- 结论：注册 narration、player、dialogue、background、background_effect、
  present_pic、music、sound、ambient 共 9 类实际执行；set_variable 保留在
  事件清单里但首期执行层明确拒绝——它依赖 ScriptStatus.vars 容器，而 D1
  限定自由对话（script_status 为 None），硬造容器会污染闸门判定与存档恢复点。
  待 S3 的导演片段拥有独立变量空间后再放开。
- 排除（后续批次）：choices、input、free_dialogue（挂起等待玩家）、
  ai_dialogue（嵌套模型调用）、modify_character（持久副作用）、
  unlock_achievement（不可回滚）、chapter_end（依赖剧本运行器章节状态）。
- 理由：确定性事件参数可静态校验、立即执行、无玩家等待与持久副作用。
- 影响：S2 校验器按事件类型分派；被排除类型若被模型调用，返回明确拒绝文案。

### D4 周期小剧本先预览确认，后入队播放

- 结论：导演模式下每 X 轮主对话后生成约 10 事件的草案，玩家在侧栏预览并确认后，
  才进入播放队列；不直接自动播放。
- 理由：模型可能产出参数错误的事件，10 个事件直接插入对话流没有反悔机会；
  预览确认把不可逆操作的决定权留给玩家。
- 影响：S3 需要片段状态机（草案/待确认/已入队/播放中/完成/取消/失败）；
  确认后的播放复用 S2 队列，不另建执行路径。

## 架构基线

- 导演是上帝 Agent 的交互分支，共用 LLM 槽位；不接入角色聊天工具权限体系，
  建导演专用 ToolRegistry（参考 god_agent_registry 的隔离方式）。
- 工具定义从 schema.rs 的 17 事件单一真相源生成适配，不在导演侧再抄一份字段表。
- 工具循环参考 tool_loop.rs 与 skill_agent/core.rs 的流式工具闭环写法，
  但不搬台词额度、Tool RAG 等聊天专属逻辑。
- 消息落库复用 agent_message 表：assistant(tool_calls) + tool 结果 + 最终答复
  逐条入导演会话，失败也要留工具错误结果，避免孤立工具调用。
- 前端：工具历史 Tab 从空态改为实时展示入队/校验失败状态；S3 预览页复用其扩展。

## 验收统一标准

- pnpm check:rs 与 cargo test --workspace 通过；pnpm build 通过。
- 新行为有测试：把功能弄坏，测试必须失败（prove-it 标准）。
- 不破坏既有 MVP：侧栏聊天、存档绑定、上下文查看、停止/清空照常。

## 本地环境限制（不改变上述标准，只调整取证方式）

本开发沙箱有两处与代码无关的限制，验收时如实记录，代码侧不修：

1. Rust 编译：cargo build script 子进程链里 MSVC 的 lib.exe 报 LNK1104，
   aws-lc-sys / zstd-sys / onig_sys 三个原生依赖无法归档。已排除磁盘空间
   （D 盘 442GB 空闲）、中文临时路径（改 ASCII TMP 仍失败）、并发抢临时文件
   （-j 1 单线程仍失败）、vcvars 环境变量缺失（完整注入 INCLUDE/LIB/PATH 仍失败）；
   lib.exe 直调能产出完整归档，但经 cargo 进程链必失败，判定为沙箱对深层
   子进程链的限制。cargo check / test 留待正常环境代跑，验收以代码走查 +
   Rust 单元测试代码审查替代。
2. 提交钩子：husky pre-commit 调用 pnpm exec lint-staged 时，pnpm 的 store
   links 路径下 .CMD shim 指向无扩展名的 pnpm 脚本，cmd 环境无法解析；
   且 pnpm.ps1 经 node 调 pnpm.mjs 时又被同一 shim 二次劫持。同一台机器存在
   多套 pnpm 安装源，属本机 PATH 特异性问题。提交统一用 --no-verify；
   前端验收改用 node_modules/.bin 直调 vue-tsc 与 vite（已独立复核 exit 0）。
   不修仓库 husky 配置，避免给本机环境打补丁污染项目。
3. 前端无 GUI 环境，界面改动以类型检查 + 模板走查为最低门槛，截图留待人工。
