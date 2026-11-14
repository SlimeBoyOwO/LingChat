//! 关系的运行时层：本局游戏里的关系变化，存在存档的 global_variables 里。

use std::collections::HashMap;

use super::relation_key;
use crate::ai_service::game_system::game_status::GameStatus;

pub const VAR_KEY: &str = "relations";

/// 取某个说话者的运行时关系层（目标角色 id → 文本）。没有或结构损坏返回 None。
pub fn layer_for(gs: &GameStatus, speaker_role_id: i32) -> Option<HashMap<i32, String>> {
    let outer = gs.get_variable(VAR_KEY)?.as_object()?;
    let inner = outer.get(&relation_key(speaker_role_id))?.as_object()?;
    Some(
        inner
            .iter()
            .filter_map(|(k, v)| {
                let target_role_id = k.strip_prefix("role:")?.parse::<i32>().ok()?;
                let text = v.as_str()?.trim();
                (!text.is_empty()).then(|| (target_role_id, text.to_string()))
            })
            .collect(),
    )
}

/// 把旧存档里的身份键迁移成角色 id：
/// - `affection.{ai}.{身份id}` → `affection.{ai}.{玩家角色id}`
/// - `relations` 的说话者/目标键 `me:{身份id}` / `ai:{文件夹}` → `role:{角色id}`
///
/// 映射不到的目标保持原样（新代码不会读取），返回值表示是否有改动。
pub fn migrate_legacy_vars(
    vars: &mut HashMap<String, serde_json::Value>,
    identity_to_role: &HashMap<String, i32>,
    folder_to_role: &HashMap<String, i32>,
) -> bool {
    let mut changed = false;

    let old_keys: Vec<String> = vars
        .keys()
        .filter(|key| key.starts_with(crate::ai_service::affection::VAR_KEY_PREFIX))
        .cloned()
        .collect();
    for key in old_keys {
        let Some(rest) = key.strip_prefix(crate::ai_service::affection::VAR_KEY_PREFIX) else {
            continue;
        };
        let Some((ai_role_id, suffix)) = rest.split_once('.') else {
            continue;
        };
        if suffix.parse::<i32>().is_ok() {
            continue;
        }
        let Some(player_role_id) = identity_to_role.get(suffix.trim()) else {
            continue;
        };
        let new_key = format!(
            "{}{}.{}",
            crate::ai_service::affection::VAR_KEY_PREFIX,
            ai_role_id,
            player_role_id
        );
        if vars.contains_key(&new_key) {
            continue;
        }
        if let Some(value) = vars.remove(&key) {
            vars.insert(new_key, value);
            changed = true;
        }
    }

    let Some(layer) = vars
        .get_mut(VAR_KEY)
        .and_then(|value| value.as_object_mut())
    else {
        return changed;
    };
    let old_outer: Vec<(String, serde_json::Value)> = layer
        .iter()
        .map(|(key, value)| (key.clone(), value.clone()))
        .collect();
    let mut new_outer = serde_json::Map::new();
    for (speaker_key, speaker_layer) in old_outer {
        let speaker =
            map_legacy_key(&speaker_key, identity_to_role, folder_to_role).unwrap_or(speaker_key);
        let Some(inner) = speaker_layer.as_object() else {
            new_outer.insert(speaker, speaker_layer);
            continue;
        };
        let mut new_inner = serde_json::Map::new();
        for (target_key, value) in inner {
            let target = map_legacy_key(target_key, identity_to_role, folder_to_role)
                .unwrap_or_else(|| target_key.clone());
            new_inner.insert(target, value.clone());
        }
        new_outer.insert(speaker, serde_json::Value::Object(new_inner));
    }
    if *layer != new_outer {
        *layer = new_outer;
        changed = true;
    }
    changed
}

fn map_legacy_key(
    key: &str,
    identity_to_role: &HashMap<String, i32>,
    folder_to_role: &HashMap<String, i32>,
) -> Option<String> {
    if key.starts_with("role:") {
        return Some(key.to_string());
    }
    if let Some(identity_id) = key.strip_prefix("me:") {
        return identity_to_role
            .get(identity_id.trim())
            .map(|role_id| relation_key(*role_id));
    }
    if let Some(folder) = key.strip_prefix("ai:") {
        return folder_to_role
            .get(folder.trim())
            .map(|role_id| relation_key(*role_id));
    }
    None
}

pub fn merge_runtime(
    base: HashMap<i32, String>,
    gs: &GameStatus,
    speaker_role_id: i32,
) -> HashMap<i32, String> {
    let mut merged = base;
    if let Some(runtime) = layer_for(gs, speaker_role_id) {
        merged.extend(runtime);
    }
    merged
}

/// AI 角色的有效关系：会话镜像（DB） + 运行时层。
pub fn load_effective(role_id: i32, gs: &GameStatus) -> HashMap<i32, String> {
    let base = gs
        .role_manager
        .relations_for(role_id)
        .cloned()
        .unwrap_or_default();
    merge_runtime(base, gs, role_id)
}

/// 玩家角色的有效关系：当前身份的关系缓存 + 运行时层。
pub fn player_relations_effective(gs: &GameStatus, player_role_id: i32) -> HashMap<i32, String> {
    merge_runtime(gs.player.relations.clone(), gs, player_role_id)
}
