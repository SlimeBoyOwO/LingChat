//! 换身份的闸门与「关系就绪门」判定。

use std::collections::HashMap;

use super::runtime_relations::{load_effective, player_relations_effective};
use crate::ai_service::game_system::game_status::GameStatus;

/// 换身份的统一闸门：剧本进行中拒绝（原地切换与开新局共用）。
pub fn ensure_identity_switchable(gs: &GameStatus) -> Result<(), String> {
    if gs.script_status.is_some() {
        return Err("剧本进行中无法更换身份，请先结束剧情。".to_string());
    }
    Ok(())
}

pub fn relation_side_status_from_maps(
    role_side: &HashMap<i32, String>,
    identity_side: &HashMap<i32, String>,
    role_id: i32,
    player_role_id: Option<i32>,
) -> (bool, bool) {
    let identity_ok = identity_side
        .get(&role_id)
        .map(|text| !text.trim().is_empty())
        .unwrap_or(false);
    // 角色侧必须针对「当前玩家角色」写一条；其他身份的关系不能算数。
    let role_ok = match player_role_id {
        Some(player_role_id) => role_side
            .get(&player_role_id)
            .map(|text| !text.trim().is_empty())
            .unwrap_or(false),
        None => !role_side.is_empty(),
    };
    (role_ok, identity_ok)
}

#[derive(Clone, Debug, serde::Serialize)]
pub struct RoleRelationStatus {
    pub role_id: i32,
    pub role_side: bool,
    pub identity_side: bool,
}

/// 逐个判定在场角色；玩家角色没有关系行时按空处理。
pub fn relation_ready_all(
    gs: &GameStatus,
    player_role_id: Option<i32>,
    role_ids: &[i32],
) -> Vec<RoleRelationStatus> {
    let identity_side = player_role_id
        .map(|id| player_relations_effective(gs, id))
        .unwrap_or_default();
    let mut out: Vec<RoleRelationStatus> = Vec::new();
    for rid in role_ids {
        let role_side = load_effective(*rid, gs);
        let (role_ok, identity_ok) =
            relation_side_status_from_maps(&role_side, &identity_side, *rid, player_role_id);
        out.push(RoleRelationStatus {
            role_id: *rid,
            role_side: role_ok,
            identity_side: identity_ok,
        });
    }
    out
}
