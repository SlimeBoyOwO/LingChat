//! 上帝 Agent（God Agent）：多人自由对话的 AI 导演。
//!
//! 每个能力一个文件，都放在 [`capabilities`] 文件夹里，各自自带「状态快照 →
//! 请求构建 → 结果解读」，便于继续长大：
//!
//! - [`capabilities::speaker`]：决定下一个发言者
//! - [`capabilities::affection`]：定期评估对话对在场角色好感度的影响
//!
//! 共用部分：[`core`] 是基础设施（LLM 槽位、配置、工具注册表、激活判定），
//! 一次决策被抽象为「任务」——system_prompt + 工具集 + 数据载荷，由
//! `ai_service::tools::agent` 无头运行。能力的副作用（切发言者、改好感度）
//! 则是 `ai_service::tools::god_agent` 里的真实工具。

pub mod capabilities;
pub mod config;
pub mod core;

pub use core::GodAgentCore;
