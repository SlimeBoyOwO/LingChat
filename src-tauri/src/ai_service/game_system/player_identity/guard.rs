//! 「剧情进行中不能更换身份」这条规则的**唯一实现点**。
//!
//! 现在只用于限制身份切换；以后要放开（世界内切换身份、多个可操作角色同时在场），
//! 只需要改这个文件，不必去各处找散落的判断。

use crate::ai_service::game_system::game_status::GameStatus;

/// 检查当前是否允许更换「我」的身份。
///
/// 当前规则：**剧本运行中一律拒绝**（对应「剧情开始前确定，中途不能更换」）。
/// 自由对话阶段允许更换；更换时会同步更新当前存档记录的 identity，
/// 因此不会出现「历史用 A 的身份写、之后突然变成 B 的记忆」这种错位
/// ——因为一个存档在任一时刻只对应一个身份，AI 的记忆按存档隔离，天然不会串。
pub fn ensure_identity_mutable(gs: &GameStatus) -> Result<(), String> {
    if gs.script_status.is_some() {
        return Err("剧本进行中无法更换身份，请先结束剧情。".to_string());
    }
    Ok(())
}
