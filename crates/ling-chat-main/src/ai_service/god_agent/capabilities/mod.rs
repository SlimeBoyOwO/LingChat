//! 上帝 Agent 的能力集合。
//!
//! 每个能力一个文件，文件内自带「状态快照 → 请求构建 → 结果解读 → 入口函数」。
//! 后续要扩展就往这里加文件（新的能力、或某个能力的额外 prompt / 视图变体），
//! 不用去动 [`super::core`] 的基础设施。
//!
//! 能力的**执行**（切发言者、改好感度）不在这里，而是
//! `ai_service::tools::god_agent` 里的真实工具。

pub mod affection;
pub mod speaker;
