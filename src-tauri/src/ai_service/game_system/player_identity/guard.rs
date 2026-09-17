//! 「剧情进行中不能更换身份」这条规则的**唯一实现点**。
//!
//! 现在只用于限制身份切换；以后要放开（世界内切换身份、多个可操作角色同时在场），
//! 只需要改这个文件，不必去各处找散落的判断。

use crate::ai_service::game_system::game_status::GameStatus;

/// 检查当前是否允许更换「我」的身份。
///
/// 当前规则：**本局一旦开始（剧本运行中，或已绑定存档）就拒绝更换**。
/// 对应「身份在开局前确定，本局中途不能换」：一个存档在任一时刻只对应一个身份，
/// 这样 AI 的记忆不会出现「前半段用 A 的身份写、后半段突然变成 B」的错位。
/// 想换身份 → 先删掉本局绑定的存档（删档会一并解除绑定，见 `api::save::delete_save`）。
///
/// 注意 `active_save_id` 也会被**自动存档**置位（见 `auto_save::perform_save`），
/// 所以开了自动存档的用户在首次真实对话后同样会被锁定——这是刻意的：
/// 自动存档建出来的也是一局真实进行中的游戏。
pub fn ensure_identity_mutable(gs: &GameStatus) -> Result<(), String> {
    if gs.script_status.is_some() {
        return Err("剧本进行中无法更换身份，请先结束剧情。".to_string());
    }
    if gs.active_save_id.is_some() {
        return Err(
            "本局已绑定存档（含自动存档），身份已锁定；如需更换身份，请先删除该存档。".to_string(),
        );
    }
    Ok(())
}
