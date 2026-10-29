//! 上帝 Agent 的能力集合。
//!
//! 每个能力一个文件，文件内自带「状态快照 → 请求构建 → 结果解读 → 入口函数」。
//! 后续要扩展就往这里加文件（新的能力、或某个能力的额外 prompt / 视图变体），

pub mod affection;
pub mod speaker;
pub mod tool_hint;
