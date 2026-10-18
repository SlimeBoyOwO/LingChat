//! 开局引导期：从身份卡推导本局的「我」，并组装注入块。

use std::path::Path;

use super::{
    AliasEntry, IdentityStore, PlayerIdentity, RelationEndpoint, build_player_block,
    effective_user_alias, resolve_relation, runtime_relations,
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
    let identity = IdentityStore::new(data_dir).current_or_synthesize(
        &settings.user_name,
        settings.user_subtitle.as_deref().unwrap_or_default(),
    );

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
