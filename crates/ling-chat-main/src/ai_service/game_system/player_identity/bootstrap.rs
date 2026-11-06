//! 开局引导期：从主身份卡推导本局的「我」，并组装注入块。

use std::collections::HashMap;
use std::path::Path;

use anyhow::Result;
use sea_orm::DatabaseConnection;

use super::{
    AliasEntry, IdentityStore, PlayerIdentity, RelationEndpoint, build_player_block,
    effective_user_alias, resolve_relation, runtime_relations,
};
use crate::ai_service::game_system::game_status::GameStatus;
use crate::ai_service::types::CharacterSettings;
use crate::db::managers::role_repo::RoleRepo;

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

/// 开局时确定「我」是谁：只解析数据目录的主身份，不再按角色建身份。
pub fn resolve_player_bootstrap(
    data_dir: &Path,
    gs: &GameStatus,
    settings: &CharacterSettings,
) -> PlayerBootstrap {
    let store = IdentityStore::new(data_dir);
    let identity = resolve_primary_identity(&store);
    if !identity.id.trim().is_empty() {
        if let Err(e) = store.set_current_id(&identity.id) {
            tracing::warn!("记录当前身份失败: {e}");
        }
    }

    let speaker = RelationEndpoint::Ai(settings.character_folder.clone());
    let target = RelationEndpoint::Me(identity.id.clone());

    // 迁移已把「我的设定」聚合进身份卡 prompt；只有 prompt 为空（老数据未迁移）时才回退
    // 到角色卡抽取文本，避免与身份侧关系重复注入。
    let effective_prompt = if identity.prompt.trim().is_empty() {
        let (_, extracted_setting) = crate::utils::prompt::extract_player_setting(
            settings.system_prompt.as_deref().unwrap_or_default(),
        );
        extracted_setting.unwrap_or_default()
    } else {
        identity.prompt.clone()
    };
    let speaker_relations =
        runtime_relations::load_effective(data_dir, &settings.character_folder, gs);
    let relation = resolve_relation(
        &speaker,
        &target,
        &speaker_relations,
        &runtime_relations::identity_relations_effective(data_dir, gs, &identity.id),
        Some(effective_prompt.as_str()),
    );

    let alias = effective_user_alias(settings, &identity.name);

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

/// 当前身份优先；没有当前身份时用主身份兜底；都没有则给内存里的占位身份。
fn resolve_primary_identity(store: &IdentityStore) -> PlayerIdentity {
    store
        .current()
        .or_else(|| store.primary_identity())
        .unwrap_or_else(|| PlayerIdentity {
            name: "玩家A（待填写）".to_string(),
            subtitle: "玩家A（待填写）".to_string(),
            is_primary: true,
            relations: HashMap::new(),
            ..Default::default()
        })
}

/// 老卡 system_prompt 里「以下是我的设定」段落的正文。
pub(crate) fn legacy_player_prompt(settings: &CharacterSettings) -> String {
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

/// 角色卡绑定了专属身份且该身份仍存在时，把它设为当前身份并返回其 id。
/// 只写 `_current.json`，不生成旁白、不写数据库、不改本局状态。
pub async fn apply_bound_identity_for_role(
    db: &DatabaseConnection,
    data_dir: &Path,
    role_id: i32,
) -> Result<Option<String>> {
    let Some(settings) = RoleRepo::get_role_settings_by_id(db, data_dir, role_id).await? else {
        return Ok(None);
    };
    let folder = settings.character_folder.trim();
    if folder.is_empty() {
        return Ok(None);
    }
    let store = IdentityStore::new(data_dir);
    let Some(card) = store
        .list()
        .into_iter()
        .find(|card| card.source_folders.iter().any(|bound| bound == folder))
    else {
        return Ok(None);
    };
    store.set_current_id(&card.id)?;
    Ok(Some(card.id))
}

#[cfg(test)]
mod tests {
    use super::*;

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
        let settings = legacy_settings();
        assert_eq!(legacy_player_prompt(&settings), "我是莱姆。");
    }
}
