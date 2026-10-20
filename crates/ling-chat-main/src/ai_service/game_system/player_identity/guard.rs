//! 换身份的闸门。

use crate::ai_service::game_system::game_status::GameStatus;

/// 换身份的统一闸门：剧本进行中拒绝（原地切换与开新局共用）。
pub fn ensure_identity_switchable(gs: &GameStatus) -> Result<(), String> {
    if gs.script_status.is_some() {
        return Err("剧本进行中无法更换身份，请先结束剧情。".to_string());
    }
    Ok(())
}
