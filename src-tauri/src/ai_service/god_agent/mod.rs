//! 上帝 Agent（God Agent）：多人自由对话的 AI 导演。
//!
//! 职责：
//! - 在自由对话模式且在场角色 > 1 时，自动决策下一个发言者
//! - 提供可扩展的 function calling 工具注册
//! - 实现连续 NPC 轮数限制，防止玩家被边缘化

pub mod config;
pub mod core;
pub mod tools;

pub use core::GodAgentCore;

