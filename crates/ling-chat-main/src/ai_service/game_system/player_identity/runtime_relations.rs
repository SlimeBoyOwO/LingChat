//! 关系的运行时层：本局游戏里的关系变化，存在存档的 global_variables 里。

use std::collections::HashMap;
use std::path::Path;

use super::store::IdentityStore;
use super::{RelationEndpoint, role_relations};
use crate::ai_service::game_system::game_status::GameStatus;

pub const VAR_KEY: &str = "relations";

/// 取某个说话者的运行时关系层（目标 key -> 文本）。没有或结构损坏返回 None。
pub fn layer_for(gs: &GameStatus, speaker: &RelationEndpoint) -> Option<HashMap<String, String>> {
    let outer = gs.get_variable(VAR_KEY)?.as_object()?;
    let inner = outer.get(&speaker.key())?.as_object()?;
    Some(
        inner
            .iter()
            .filter_map(|(k, v)| {
                let text = v.as_str()?;
                let text = text.trim();
                (!text.is_empty() && !k.trim().is_empty()).then(|| (k.clone(), text.to_string()))
            })
            .collect(),
    )
}

pub fn merge_runtime(
    base: HashMap<String, String>,
    gs: &GameStatus,
    speaker: &RelationEndpoint,
) -> HashMap<String, String> {
    let mut merged = base;
    if let Some(runtime) = layer_for(gs, speaker) {
        merged.extend(runtime);
    }
    merged
}

pub fn load_effective(
    data_dir: &Path,
    role_folder: &str,
    gs: &GameStatus,
) -> HashMap<String, String> {
    let speaker = RelationEndpoint::Ai(role_folder.to_string());
    let mut base = role_relations::load(data_dir, role_folder);
    // 角色对玩家身份的关系以数据库为真源，会话开始时已镜像到 role_manager。
    if let Some(identity_relations) = gs.role_manager.identity_relations_for(role_folder) {
        base.extend(identity_relations.clone());
    }
    merge_runtime(base, gs, &speaker)
}

pub fn identity_relations_effective(
    data_dir: &Path,
    gs: &GameStatus,
    identity_id: &str,
) -> HashMap<String, String> {
    let base = IdentityStore::new(data_dir)
        .find_by_id(identity_id)
        .map(|identity| identity.relations)
        .unwrap_or_else(|| gs.player.relations.clone());
    let speaker = RelationEndpoint::Me(identity_id.to_string());
    merge_runtime(base, gs, &speaker)
}
