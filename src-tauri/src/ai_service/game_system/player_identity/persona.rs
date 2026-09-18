//! 人设行的**按阵容重建**：把「我是谁」与「你眼里的其他角色」按该角色自己的视角
//! 烘进它的 SYSTEM 人设行。
//!
//! 为什么需要单独一个文件：人设行是**一次性**拼进 `line_list` 的，而多 AI 场景的
//! 阵容会变（角色入场 / 切换说话者 / 出场）。阵容一变，各角色人设行里
//! 「你眼里的其他角色」这段就过期了 —— 例如 A 的人设行里没有「我眼里的 B」，
//! 于是**B 认为 A 是狗**这种设定永远不会被 A 感知到（这正是修这个文件的起因）。
//!
//! 这里提供唯一的实现点：`rebuild_persona_line`（单个角色）与
//! `rebuild_onstage_personas`（所有在场角色）。以后要改"人设行怎么拼"，只改这里。

use std::path::Path;

use anyhow::{Context, Result};
use sea_orm::DatabaseConnection;

use crate::ai_service::game_system::game_status::GameStatus;
use crate::ai_service::game_system::player_identity::{
    RelationEndpoint, ScenePeer, build_peers_block, build_player_block, resolve_relation,
    role_relations,
};
use crate::ai_service::types::{LineAttributeExt, LineBase};
use crate::db::entities::line::LineAttribute;
use crate::utils::prompt::{PromptOptions, sys_prompt_builder_by_settings_with_player};

/// 收集说话者**以外**的在场角色，供「你眼里的其他角色」块使用。
///
/// 只收已加载到 `role_manager` 的角色；每个角色各自读自己的 `relations.yml`，
/// 这样"对方视角"的回退也能用上。
pub fn collect_peers(gs: &GameStatus, data_dir: &Path, self_folder: &str) -> Vec<ScenePeer> {
    gs.onstage_role_ids
        .iter()
        .filter_map(|rid| gs.role_manager.get_loaded(*rid))
        .filter(|role| role.settings.character_folder != self_folder)
        .map(|role| ScenePeer {
            folder: role.settings.character_folder.clone(),
            name: role
                .display_name
                .clone()
                .unwrap_or_else(|| role.settings.ai_name.clone()),
            relations: role_relations::load(data_dir, &role.settings.character_folder),
        })
        .collect()
}

/// 按当前阵容重建某个角色的 SYSTEM 人设行（**原位替换**第一条；没有才追加）。
///
/// - 「我」的部分与开局完全同源：说话者视角 = (该角色, 当前身份)；
/// - 「其他角色」的部分见 [`build_peers_block`]（只注入显式关系，不泄漏人设）；
/// - 原位替换而不是新增，是为了不改动台词序列（存档/回溯都按顺序来）。
pub async fn rebuild_persona_line(
    gs: &mut GameStatus,
    db: &DatabaseConnection,
    data_dir: &Path,
    role_id: i32,
    prompt_options: PromptOptions,
) -> Result<()> {
    gs.get_role(db, role_id)
        .await
        .with_context(|| format!("重建人设前加载角色 {role_id} 失败"))?;

    let (folder, name, settings) = {
        let role = gs
            .role_manager
            .get_loaded(role_id)
            .ok_or_else(|| anyhow::anyhow!("角色 {role_id} 加载后不可用"))?;
        (
            role.settings.character_folder.clone(),
            role.display_name
                .clone()
                .unwrap_or_else(|| role.settings.ai_name.clone()),
            role.settings.clone(),
        )
    };

    let speaker = RelationEndpoint::Ai(folder.clone());
    let speaker_relations = role_relations::load(data_dir, &folder);

    // 「我」：玩家对象在开局/读档时已按身份卡填好，这里直接用（与 add_role_to_scene 同源）
    let target = RelationEndpoint::Me(gs.player.identity_id.clone().unwrap_or_default());
    let relation = resolve_relation(
        &speaker,
        &target,
        &speaker_relations,
        &gs.player.relations,
        Some(gs.player.user_prompt.as_str()),
    );
    let player_block = build_player_block(
        &gs.player.user_name,
        &gs.player.user_subtitle,
        &gs.player.user_prompt,
        relation.as_ref(),
    );

    // 「其他角色」：只注入显式写过关系的对象
    let peers = collect_peers(gs, data_dir, &folder);
    let peers_block = build_peers_block(&speaker, &speaker_relations, &peers);

    // 两段都交给 prompt 构建器，它会（在有身份块时）一起压到 system 末尾
    let injected = format!("{player_block}{peers_block}");
    let system_prompt = sys_prompt_builder_by_settings_with_player(
        &settings,
        &gs.player.user_name,
        &injected,
        prompt_options,
    );

    let existing = gs.line_list.iter().position(|line| {
        matches!(line.attribute(), LineAttribute::System)
            && line.base.sender_role_id == Some(role_id)
    });

    match existing {
        Some(index) => {
            gs.line_list[index].base.content = system_prompt;
            gs.line_list[index].base.display_name = Some(name);
            tracing::info!(
                "角色 {role_id} 的人设行已按当前阵容重建（在场其他角色 {} 个）",
                peers.len()
            );
        },
        None => {
            gs.add_line(
                db,
                LineBase {
                    content: system_prompt,
                    attribute: LineAttributeExt(LineAttribute::System),
                    sender_role_id: Some(role_id),
                    display_name: Some(name),
                    ..Default::default()
                },
            )
            .await?;
        },
    }

    Ok(())
}

/// 重建**所有在场角色**的人设行。阵容变化（角色入场 / 切换 / 出场）后调用。
///
/// 调用方负责在这之后刷新记忆（`refresh_memories`），否则角色上下文里还是旧人设。
pub async fn rebuild_onstage_personas(
    gs: &mut GameStatus,
    db: &DatabaseConnection,
    data_dir: &Path,
    prompt_options: PromptOptions,
) -> Result<()> {
    let role_ids: Vec<i32> = gs.onstage_role_ids.clone();
    for role_id in role_ids {
        rebuild_persona_line(gs, db, data_dir, role_id, prompt_options).await?;
    }
    Ok(())
}
