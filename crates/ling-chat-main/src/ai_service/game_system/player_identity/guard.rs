//! 换身份的闸门与「关系就绪门」判定。

use std::collections::HashMap;
use std::path::Path;

use super::RelationEndpoint;
use super::runtime_relations::{identity_relations_effective, load_effective};
use crate::ai_service::game_system::game_status::GameStatus;

/// 换身份的统一闸门：剧本进行中拒绝（原地切换与开新局共用）。
pub fn ensure_identity_switchable(gs: &GameStatus) -> Result<(), String> {
    if gs.script_status.is_some() {
        return Err("剧本进行中无法更换身份，请先结束剧情。".to_string());
    }
    Ok(())
}

pub fn relation_side_status_from_maps(
    role_side: &HashMap<String, String>,
    identity_side: &HashMap<String, String>,
    role_folder: &str,
    identity_id: &str,
) -> (bool, bool) {
    let ai_key = RelationEndpoint::Ai(role_folder.to_string()).key();
    let identity_ok = identity_side
        .get(&ai_key)
        .map(|text| !text.trim().is_empty())
        .unwrap_or(false);
    // 角色侧必须针对「当前身份」写一条；other identities 的关系不能算数。
    let role_ok = if identity_id.trim().is_empty() {
        !role_side.is_empty()
    } else {
        let me_key = RelationEndpoint::Me(identity_id.to_string()).key();
        role_side
            .get(&me_key)
            .map(|text| !text.trim().is_empty())
            .unwrap_or(false)
    };
    (role_ok, identity_ok)
}

#[derive(Clone, Debug, serde::Serialize)]
pub struct RoleRelationStatus {
    pub role_id: i32,
    pub role_side: bool,
    pub identity_side: bool,
}

/// 逐个判定在场角色；拿不到 character_folder 的角色跳过。
pub fn relation_ready_all(
    gs: &GameStatus,
    data_dir: &Path,
    identity_id: &str,
    role_ids: &[i32],
) -> Vec<RoleRelationStatus> {
    let identity_side = identity_relations_effective(data_dir, gs, identity_id);
    let mut out: Vec<RoleRelationStatus> = Vec::new();
    for rid in role_ids {
        let Some(role) = gs.role_manager.get_loaded(*rid) else {
            continue;
        };
        let folder = role.settings.character_folder.clone();
        if folder.trim().is_empty() {
            continue;
        }
        let role_side = load_effective(data_dir, &folder, gs);
        let (role_ok, identity_ok) =
            relation_side_status_from_maps(&role_side, &identity_side, &folder, identity_id);
        out.push(RoleRelationStatus {
            role_id: *rid,
            role_side: role_ok,
            identity_side: identity_ok,
        });
    }
    out
}
