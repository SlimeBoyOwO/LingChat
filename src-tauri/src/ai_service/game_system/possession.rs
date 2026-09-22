//! 运行时附身：把「玩家」从字面值 0 哨兵升级为统一实体模型下的会话态。
//!
//! 附身只改动 `GameStatus.possessed_role_id` 及由其派生的缓存与提示词，**不写回**
//! `role.role_type`——实体是不是 AI，与"此刻谁在扮演它"是两件互不影响的事。
//! 附身期间玩家以被附身实体的身份发言（玩家台词 `sender_role_id` = 该实体），
//! 该实体的 AI 生成休眠；切走后它恢复 AI 控制，期间台词按 sender 天然留在其记忆中。

use anyhow::{anyhow, Result};
use sea_orm::DatabaseConnection;

use crate::ai_service::game_system::game_status::GameStatus;
use crate::ai_service::types::PLAYER_ROLE_ID;
use crate::db::entities::role::RoleType;
use crate::db::managers::role_repo::RoleRepo;
use crate::utils::prompt::PromptOptions;

/// 附身执行结果：命令层据此决定是否需要补发前端事件。
pub struct PossessOutcome {
    /// 该实体在玩家视角下的显示名（即附身后的玩家名）。
    pub display_name: String,
    /// 若因附身当前对话对象而移交了话筒，这里是被移交到的在场 AI 实体 id。
    pub handoff_role_id: Option<i32>,
}

/// 把玩家附身到指定实体上，返回该实体的显示名与话筒移交结果。
///
/// 附身后：
/// 1. 实体必须"在场"（否则感知不到台词，God Agent 也看不见它）；
/// 2. 若当前对话对象恰好是该实体（自己跟自己说话的死锁），把话筒移交给在场的
///    另一个 AI；没有可移交给的 AI（真·一对一）则拒绝附身；
/// 3. 重建全部 SYSTEM 人设行（玩家名嵌在每条 AI 人设里），并为被附身实体补建人设。
pub async fn possess_entity(
    gs: &mut GameStatus,
    db: &DatabaseConnection,
    role_id: i32,
    prompt_options: PromptOptions,
) -> Result<PossessOutcome> {
    let role = RoleRepo::get_role_by_id(db, role_id)
        .await?
        .ok_or_else(|| anyhow!("实体不存在: role_id={}", role_id))?;

    // ── 校验阶段：所有拒绝都在任何状态修改之前返回 ──
    if gs.script_status.is_some() {
        return Err(anyhow!("剧本/试玩进行中，无法切换扮演"));
    }

    // AI 角色必须有立绘在舞台上：附身一个不在场的角色，玩家会以一个"看不见的人"
    // 的身份说话，该角色也感知不到台词、God Agent 看不见它。默认身份与 User 身份
    // 没有立绘、本就不属舞台，故不受此限。
    if role.role_type != RoleType::User
        && role_id != PLAYER_ROLE_ID
        && !gs.onstage_role_ids.contains(&role_id)
    {
        return Err(anyhow!("该角色不在场，请先让其入场后再扮演"));
    }

    // 玩家身份集合与 God Agent 判据同源（role_repo）。校验阶段只读不入缓存，
    // 保证被拒绝时 GameStatus 完全不被改动；通过后再写回缓存。
    let human_role_ids = RoleRepo::get_user_role_ids(db).await?;

    // 附身当前对话对象意味着原发言者要被玩家接管：必须把话筒移交给另一个在场 AI，
    // 否则就成了"自己跟自己说话"。User 身份实体永不由 AI 生成（冻结待机语义），
    // 故接管者只能是 AI；真·一对一没有可移交对象时直接拒绝附身。
    // 候选排除被附身目标本身与全部 User 身份；正在被玩家附身的 AI 切换后即恢复
    // AI 控制，可以承接话筒。
    let handoff = if gs.current_role_id == Some(role_id) {
        gs.present_role_ids
            .iter()
            .copied()
            .find(|id| *id != role_id && !human_role_ids.contains(id))
    } else {
        None
    };
    if gs.current_role_id == Some(role_id) && handoff.is_none() {
        return Err(anyhow!("不能附身正在对话的角色"));
    }

    // 校验通过：缓存玩家身份集合供后续判据使用，从此处开始改动状态
    gs.human_role_ids = human_role_ids;
    gs.possessed_role_id = role_id;
    gs.refresh_possessed_cache(db).await?;

    // 确保被附身实体已加载到运行时：God Agent 简介、记忆同步都从 role_manager 取数据；
    // 对 User 实体这里会走合成 settings 路径（没有 settings.yml 也能加载）。
    let _ = gs.get_role(db, role_id).await?;

    // User 身份没有立绘资源，只进感知集合不上台；AI 目标已由前置校验保证在舞台上，
    // 这里的 insert 对已存在的 AI 目标幂等无副作用。
    if !gs.present_role_ids.contains(&role_id) {
        gs.present_role_ids.insert(role_id);
    }

    // 死锁回退：把当前对话对象移交给选定的在场 AI
    if handoff.is_some() {
        gs.current_role_id = handoff;
    }

    gs.rebuild_system_prompts(db, prompt_options).await?;

    tracing::info!(
        "附身成功: possessed_role_id={}, current_role_id={:?}, present_role_ids={:?}",
        gs.possessed_role_id,
        gs.current_role_id,
        gs.present_role_ids
    );

    Ok(PossessOutcome {
        display_name: gs.player.user_name.clone(),
        handoff_role_id: handoff,
    })
}
