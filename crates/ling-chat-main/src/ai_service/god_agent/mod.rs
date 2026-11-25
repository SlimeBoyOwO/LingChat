//! 上帝 Agent（God Agent）：平台层，持有模型槽位、最高权限工具、世界上下文，
//! 以及挂在它下面的各分支 Agent 的生命周期。
//!
//! 分支有两种运行形态：
//!
//! - 无头决策分支（[`capabilities`]）：输入一次任务、输出一次结果，不持有会话。
//!   - [`capabilities::speaker`]：决定下一个发言者
//!   - [`capabilities::affection`]：定期评估对话对在场角色好感度的影响
//! - 交互会话分支（[`director`]，将来的剧本 Agent）：持有会话、落库、流式推事件。
//! - 后台任务分支（[`task`]）：插件投递的一次性导演任务，提示词与提交工具由插件给。
//!
//! 共用部分：[`core`] 是基础设施（LLM 槽位、配置、工具注册表、激活判定），
//! 一次决策被抽象为「任务」——system_prompt + 工具集 + 数据载荷，由
//! `ai_service::tools::agent` 无头运行。能力的副作用（切发言者、改好感度）
//! 则是 `ai_service::tools::builtin::god_agent` 里的真实工具。
//!
//! 新增 Agent 先判断运行形态，再放进对应目录；共用基础设施都从中枢取，不各建一套。

pub mod capabilities;
pub mod config;
pub mod core;
pub mod director;
pub mod task;

pub use core::GodAgentCore;
