//! 换身份的两条规则（原地切换 / 开新局切换），唯一实现点。

use crate::ai_service::game_system::game_status::GameStatus;

/// 是否允许原地更换身份。本局已开始（剧本进行中，或已绑定存档）则拒绝：
/// 一个存档只对应一个身份，中途换会让 AI 记忆前后错位。
/// 注意自动存档也会置位 `active_save_id`，所以开了自动存档同样会被锁定。
pub fn ensure_identity_mutable(gs: &GameStatus) -> Result<(), String> {
    if gs.script_status.is_some() {
        return Err("剧本进行中无法更换身份，请先结束剧情。".to_string());
    }
    if gs.active_save_id.is_some() {
        return Err(
            "本局已绑定存档（含自动存档），身份已锁定；如需更换身份，请用「用它开新对话」。"
                .to_string(),
        );
    }
    Ok(())
}

/// 是否允许用另一个身份开新对话。只有剧本进行中会被拒：已绑定存档不是障碍，
/// 换身份正是靠开新局实现的（新局不绑存档，旧存档连身份原样保留）。
pub fn ensure_identity_switchable(gs: &GameStatus) -> Result<(), String> {
    if gs.script_status.is_some() {
        return Err("剧本进行中无法更换身份，请先结束剧情。".to_string());
    }
    Ok(())
}
