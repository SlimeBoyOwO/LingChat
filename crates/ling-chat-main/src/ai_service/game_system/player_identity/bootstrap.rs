//! 开局引导期：从身份卡推导本局的「我」，并组装注入块。

use std::collections::HashMap;
use std::path::Path;

use super::{
    AliasEntry, IdentityStore, PlayerIdentity, RelationEndpoint, build_player_block,
    effective_user_alias, resolve_relation, role_relations, runtime_relations,
};
use crate::ai_service::game_system::game_status::GameStatus;
use crate::ai_service::types::CharacterSettings;

/// 引导期解析出的玩家身份。
pub struct PlayerBootstrap {
    identity: PlayerIdentity,
    effective_prompt: String,
    /// 该角色对「我」的有效称呼，同时是提示词里的人名
    pub alias: String,
    /// 压到该角色 SYSTEM 人设末尾的注入块
    pub player_block: String,
}

impl PlayerBootstrap {
    /// 把身份写回本局状态。玩家名记身份名，称呼差异只体现在各角色的注入块里。
    pub fn apply_to(self, gs: &mut GameStatus) {
        gs.player.user_name = self.identity.display_name().to_string();
        gs.player.user_subtitle = self.identity.subtitle.clone();
        gs.player.user_prompt = self.effective_prompt;
        gs.player.identity_id = if self.identity.id.trim().is_empty() {
            None
        } else {
            Some(self.identity.id.clone())
        };
        gs.player.relations = self.identity.relations.clone();
    }
}

/// 开局时确定「我」是谁，并对老角色卡做兜底。
///
/// 与 `persona::player_block_for` 的分工：本函数负责在身份尚未落盘时把它定下来，
/// 后者负责身份已定之后重建某个角色的人设行。
pub fn resolve_player_bootstrap(
    data_dir: &Path,
    gs: &GameStatus,
    settings: &CharacterSettings,
) -> PlayerBootstrap {
    let store = IdentityStore::new(data_dir);
    let identity = resolve_identity_for_character(&store, data_dir, settings);
    if !identity.id.trim().is_empty() {
        if let Err(e) = store.set_current_id(&identity.id) {
            tracing::warn!("记录当前身份失败: {e}");
        }
    }

    // 老卡兜底：身份卡 prompt 为空时，用角色卡 system_prompt 里抽出的「我的设定」段落补上。
    let (_, extracted_setting) = crate::utils::prompt::extract_player_setting(
        settings.system_prompt.as_deref().unwrap_or_default(),
    );
    let effective_prompt = if identity.prompt.trim().is_empty() {
        extracted_setting.unwrap_or_default()
    } else {
        identity.prompt.clone()
    };

    let speaker = RelationEndpoint::Ai(settings.character_folder.clone());
    let target = RelationEndpoint::Me(identity.id.clone());
    let speaker_relations =
        runtime_relations::load_effective(data_dir, &settings.character_folder, gs);
    let relation = resolve_relation(
        &speaker,
        &target,
        &speaker_relations,
        &runtime_relations::identity_relations_effective(data_dir, gs, &identity.id),
        Some(effective_prompt.as_str()),
    );

    let identity_id = if identity.id.trim().is_empty() {
        None
    } else {
        Some(identity.id.as_str())
    };
    let alias = effective_user_alias(settings, identity_id, &identity.name);

    // 开场只有本角色一行称呼；其他角色入场时由 persona 重建把他们的称呼补进来
    let roster = vec![AliasEntry {
        name: settings.ai_name.clone(),
        alias: alias.clone(),
        is_self: true,
    }];

    // 「我」对该角色的私人认知，取自身份卡里的 ai:<角色文件夹> 条目
    let player_view = identity
        .relations
        .get(&speaker.key())
        .map(|text| text.trim().to_string())
        .filter(|text| !text.is_empty());

    let player_block = build_player_block(
        identity.display_name(),
        &alias,
        &effective_prompt,
        relation.as_ref(),
        player_view.as_deref(),
        &roster,
    );

    PlayerBootstrap {
        identity,
        effective_prompt,
        alias,
        player_block,
    }
}

/// 老角色卡里的玩家名；空值与反序列化默认值不算。
fn legacy_player_name(settings: &CharacterSettings) -> Option<&str> {
    let name = settings.user_name.trim();
    if name.is_empty() || name == "user_name未设定" {
        None
    } else {
        Some(name)
    }
}

/// 老卡 system_prompt 里「以下是我的设定」段落的正文，迁移成身份卡提示词。
fn legacy_player_prompt(settings: &CharacterSettings) -> String {
    let (_, segment) = crate::utils::prompt::extract_player_setting(
        settings.system_prompt.as_deref().unwrap_or_default(),
    );
    let Some(segment) = segment else {
        return String::new();
    };
    let mut lines = segment.lines();
    let first = lines.next().unwrap_or_default();
    let body = if first.trim_start().starts_with("以下是我的设定") {
        lines.collect::<Vec<_>>().join("\n")
    } else {
        segment.clone()
    };
    body.trim().to_string()
}

/// 为当前角色挑身份：角色绑定优先；老卡按 user_name 迁移；都没有才回落到当前/合成身份。
fn resolve_identity_for_character(
    store: &IdentityStore,
    data_dir: &Path,
    settings: &CharacterSettings,
) -> PlayerIdentity {
    let folder = settings.character_folder.trim();
    if let Some(bound) = store.bound_identity(folder) {
        return bound;
    }

    let current = store.current();
    let Some(legacy_name) = legacy_player_name(settings) else {
        let fallback_name = if settings.user_name.trim() == "user_name未设定" {
            ""
        } else {
            settings.user_name.trim()
        };
        return current.unwrap_or_else(|| {
            store.current_or_synthesize(
                fallback_name,
                settings.user_subtitle.as_deref().unwrap_or_default(),
            )
        });
    };

    // 同名身份优先：同一个玩家名在多张老卡之间复用同一张卡。
    if let Some(existing) = store.find_by_name(legacy_name) {
        let _ = store.bind_character(folder, &existing.id);
        return existing;
    }

    // 当前身份与老卡玩家名一致时直接复用；不一致则走迁移。
    // 不拿 user_aliases 当「用户已选择身份」的证据：称呼覆写只说明这个角色怎么叫 ta，
    // 不代表身份卡本身已经迁移过。
    if let Some(current) = current.clone() {
        if current.display_name() == legacy_name {
            if !current.id.trim().is_empty() {
                let _ = store.bind_character(folder, &current.id);
            }
            return current;
        }
    }

    // 迁移：按老卡里的玩家设定合成一张身份卡，并绑定到该角色目录。
    // 关系一并搬：新身份承接当前身份对该角色的认知，角色侧的关系表也复制一份，
    // 避免迁移后立刻被「关系未就绪」拦住。
    let ai_key = RelationEndpoint::Ai(folder.to_string()).key();
    let mut relations = HashMap::new();
    if let Some(current) = current.as_ref() {
        if let Some(text) = current.relations.get(&ai_key) {
            let text = text.trim();
            if !text.is_empty() {
                relations.insert(ai_key.clone(), text.to_string());
            }
        }
    }

    let card = PlayerIdentity {
        id: String::new(),
        name: legacy_name.to_string(),
        subtitle: settings
            .user_subtitle
            .as_deref()
            .unwrap_or_default()
            .trim()
            .to_string(),
        prompt: legacy_player_prompt(settings),
        avatar: None,
        relations,
    };
    match store.save(&card) {
        Ok(saved) => {
            let _ = store.bind_character(folder, &saved.id);
            if let Some(current) = current.as_ref() {
                migrate_role_relations(data_dir, folder, &current.id, &saved.id);
            }
            tracing::info!(
                "已从老角色卡迁移玩家身份: role_folder={folder} identity={}",
                saved.id
            );
            saved
        },
        Err(e) => {
            tracing::warn!("迁移老角色卡玩家身份失败: {e}");
            current.unwrap_or(card)
        },
    }
}

/// 把角色关系表里「旧身份」那条复制到「新身份」名下；只补空位，不覆盖已有值。
fn migrate_role_relations(data_dir: &Path, folder: &str, from_id: &str, to_id: &str) {
    if folder.trim().is_empty() || from_id.trim().is_empty() || to_id.trim().is_empty() {
        return;
    }
    let mut relations = role_relations::load(data_dir, folder);
    let from_key = RelationEndpoint::Me(from_id.to_string()).key();
    let Some(text) = relations.get(&from_key).cloned() else {
        return;
    };
    if text.trim().is_empty() {
        return;
    }
    let to_key = RelationEndpoint::Me(to_id.to_string()).key();
    if relations
        .get(&to_key)
        .is_some_and(|value| !value.trim().is_empty())
    {
        return;
    }
    relations.insert(to_key, text);
    if let Err(e) = role_relations::save(data_dir, folder, &relations) {
        tracing::warn!("迁移角色关系失败: {e}");
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    fn legacy_settings() -> CharacterSettings {
        CharacterSettings {
            character_folder: "诺一钦灵".to_string(),
            user_name: "莱姆".to_string(),
            user_subtitle: Some("Bilibili".to_string()),
            system_prompt: Some("人设\n以下是我的设定：\n我是莱姆。\n".to_string()),
            ..Default::default()
        }
    }

    #[test]
    fn extracts_legacy_player_prompt_without_heading() {
        assert_eq!(legacy_player_prompt(&legacy_settings()), "我是莱姆。");
    }

    #[test]
    fn migrates_legacy_name_and_binds_character() {
        let dir = tempdir().unwrap();
        let store = IdentityStore::new(dir.path());
        let identity = resolve_identity_for_character(&store, dir.path(), &legacy_settings());
        assert_eq!(identity.name, "莱姆");
        assert_eq!(identity.subtitle, "Bilibili");
        assert_eq!(identity.prompt, "我是莱姆。");
        let bound = store.bound_identity("诺一钦灵").expect("绑定应写入");
        assert_eq!(bound.id, identity.id);
    }
}
