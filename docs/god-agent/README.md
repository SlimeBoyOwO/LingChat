# 上帝 Agent（God Agent）

> 面向维护者与贡献者。代码基准：`dev` 分支，`src-tauri/src/ai_service/god_agent/`（4 个文件、约 700 行）。
> 文档语言与代码注释对齐（中文）。行号会漂移，本文以**文件路径 + 符号名**定位。

## 一句话

上帝 Agent 是**多人自由对话的导演**。它自己**不生成任何台词**，只在回合与回合之间做两个决策：

| 决策         | 工具                  | 时机                                     |
| ------------ | --------------------- | ---------------------------------------- |
| 谁接着说     | `select_next_speaker` | 用户发消息时（前置）+ 每轮回复后（后置） |
| 感情变了多少 | `update_affection`    | 每累计若干段真实对话，后台评估一次       |

它管的是"**轮到谁**"和"**关系走到哪了**"，台词与情绪仍由各角色自己的对话管线产出。

## 文档索引

| 文档                                                             | 内容                                                                       |
| ---------------------------------------------------------------- | -------------------------------------------------------------------------- |
| [architecture.md](architecture.md)                               | **实现说明**：模块职责、激活条件、上下文管理、两条流水线、失败模式、扩展点 |
| [diagrams/architecture.html](diagrams/architecture.html)         | 组件结构图（UML 类图）：谁持有谁、依赖方向、与生成管线/前端的边界          |
| [diagrams/speaker-flow.html](diagrams/speaker-flow.html)         | 发言者选择工作流（活动图）：一条用户消息从进管线到交还玩家的完整分支       |
| [diagrams/context-assembly.html](diagrams/context-assembly.html) | 上下文组装数据流：每次决策的 prompt 从哪些字段拼出来                       |
| [diagrams/affection-flow.html](diagrams/affection-flow.html)     | 好感度评估时序图：快照 → 后台 LLM → 回写存档变量 → 旁白台词 → 前端事件     |

四张图都是自包含的 HTML（内联 SVG），直接双击用浏览器打开即可，不依赖任何外部资源。

## 模块结构

```
src-tauri/src/ai_service/god_agent/
├── mod.rs      模块入口，re-export GodAgentCore
├── config.rs   GodAgentConfig（运行参数）+ resolve_god_agent_provider（4 级 LLM 兜底）
├── core.rs     GodAgentCore：should_activate / decide_next_speaker / evaluate_affection
└── tools.rs    工具定义与容错解析：select_next_speaker、update_affection
```

## 运行参数

5 个键，全部存在 tauri-plugin-store（`settings.json`）。其中 3 个 `god_agent.*` 运行参数**没有 UI**，只能在文件里手改：

| 键                                  | 默认   | 下限 | 说明                                                   | 有 UI？                        |
| ----------------------------------- | ------ | ---- | ------------------------------------------------------ | ------------------------------ |
| `llm.god_agent_provider_id`         | 空     | —    | 专用 LLM provider；空则走角色分配 / 聊天主 LLM         | ✅ LLM 设置页「上帝Agent」角色 |
| `god_agent.max_consecutive_npc`     | `3`    | 1    | 连续 NPC 发言轮数上限，超过强制交还玩家                | ❌ 手改 settings.json          |
| `god_agent.recent_window`           | `20`   | 5    | 决策时参考的最近台词行数（滑动窗口大小）               | ❌ 同上                        |
| `god_agent.affection_eval_interval` | `5`    | 1    | 每累计多少段「真实对话」评估一次好感度                 | ❌ 同上                        |
| `affection.enabled`                 | `true` | —    | 好感度系统总开关；关掉则不评估、不写情感旁白、面板隐藏 | ✅ 高级设置 → 好感度           |

> ⚠️ **store 里的值必须写成字符串**。`GodAgentConfig::load` 用 `.as_str().and_then(|s| s.parse())` 读取，写成 JSON 数字会被当作"没配置"而回落到默认值。

## 相关文档

- [../function_call/README.md](../function_call/README.md) —— 通用工具调用架构。上帝 Agent 早于它存在（PR #523 之前是项目里**唯一**的工具调用路径），PR #523 之后两者并存但**互不共用**：上帝 Agent 走非流式的 `LlmClient::complete_with_tools` 单次调用，聊天管线走 `stream_with_tool_loop` 闭环。
- [../memory/runtime.md](../memory/runtime.md) —— 角色侧的长程记忆（MemoryBank）。**上帝 Agent 不读它**，见 architecture.md「上下文管理」一节。
- [../utils/role-archive.md](../utils/role-archive.md) —— 角色文件与设置字段（`ai_subtitle` / `info` 的来源）。
