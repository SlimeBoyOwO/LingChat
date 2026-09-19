//! 玩家身份（`role_type=User` 的统一实体）命令层。
//!
//! 统一实体后玩家身份与 AI 角色同为 `role` 行，这里只暴露"玩家身份"这一侧：
//! 列表/新建/改名改人设/删除，以及运行时附身切换。附身本身是会话态，
//! 具体逻辑委托给 `possession` 模块，命令层只负责参数校验、缓存刷新与事件广播。

use serde::Serialize;
use tauri::{AppHandle, Emitter, Manager};

use crate::AppState;
use crate::ai_service::game_system::possession;
use crate::ai_service::game_system::role_manager::user_identity_settings;
use crate::ai_service::types::{PLAYER_ROLE_ID, RoleProfile};
use crate::config::AppConfig;
use crate::db::managers::role_repo::RoleRepo;
use crate::utils::prompt::PromptOptions;

/// 一条玩家身份（含人设）。
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "snake_case")]
pub struct IdentityInfo {
    pub role_id: i32,
    pub name: String,
    pub profile: RoleProfile,
}

/// 当前被附身实体的简要信息。
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "snake_case")]
pub struct PossessedInfo {
    pub role_id: i32,
    pub name: String,
    pub subtitle: String,
}

/// 提示词选项：与正式游玩保持同一来源，保证重建 SYSTEM 行不改变人设格式。
pub(crate) fn prompt_options(app: &AppHandle) -> PromptOptions {
    let config = AppConfig::load(app).unwrap_or_default();
    PromptOptions {
        output_sec_lang: config.llm_output_sec_lang,
        no_emotion_limit: config.no_emotion_limit_prompt,
    }
}

/// 读取当前被附身实体 id（不做任何加锁以外的副作用）。
async fn current_possessed(app: &AppHandle) -> i32 {
    let state = app.state::<AppState>();
    let service = state.ai_service.lock().await;
    let gs = service.game_status.lock().await;
    gs.possessed_role_id
}

/// 广播当前扮演者变化。负载复用 `PossessedInfo`，与 `get_possessed_entity` 保持同构。
pub(crate) fn emit_possessed(app: &AppHandle, info: &PossessedInfo) {
    if let Err(e) = app.emit("identity:possessed", info) {
        tracing::warn!("emit identity:possessed 失败: {e}");
    }
}

/// 广播当前说话角色切换。负载字段与 `message_system` 的 `character:switch` 逐字段对齐，
/// 前端两处消费者按同一形状解析。
fn emit_character_switch(app: &AppHandle, role_id: i32, name: &str) {
    let payload = serde_json::json!({
        "type": "character_switch",
        "roleId": role_id,
        "characterName": name,
    });
    if let Err(e) = app.emit("character:switch", &payload) {
        tracing::warn!("emit character:switch 失败: {e}");
    }
}

#[tauri::command]
pub async fn list_identities(app: AppHandle) -> Result<Vec<IdentityInfo>, String> {
    let state = app.state::<AppState>();
    let identities = RoleRepo::list_player_identities(&state.db)
        .await
        .map_err(|e| format!("获取玩家身份列表失败: {}", e))?;
    Ok(identities
        .into_iter()
        .map(|(role, profile)| IdentityInfo {
            role_id: role.id,
            name: role.name,
            profile,
        })
        .collect())
}

#[tauri::command]
pub async fn create_identity(
    app: AppHandle,
    name: String,
    profile: RoleProfile,
) -> Result<i32, String> {
    let name = name.trim();
    if name.is_empty() {
        return Err("身份名称不能为空".to_string());
    }
    let state = app.state::<AppState>();
    let id = RoleRepo::create_player_identity(&state.db, name, &profile)
        .await
        .map_err(|e| format!("创建玩家身份失败: {}", e))?;

    // 新身份加入玩家身份集合，刷新缓存让 God Agent 判据即时排除它
    {
        let service = state.ai_service.lock().await;
        let mut gs = service.game_status.lock().await;
        if let Err(e) = gs.refresh_human_role_ids(&state.db).await {
            tracing::warn!("创建身份后刷新玩家身份缓存失败: {e}");
        }
    }
    if let Err(e) = app.emit("role:list-updated", ()) {
        tracing::warn!("emit role:list-updated 失败: {e}");
    }
    Ok(id)
}

#[tauri::command]
pub async fn update_identity(
    app: AppHandle,
    role_id: i32,
    name: String,
    profile: RoleProfile,
) -> Result<(), String> {
    let name = name.trim();
    if name.is_empty() {
        return Err("身份名称不能为空".to_string());
    }
    let state = app.state::<AppState>();
    RoleRepo::update_player_identity(&state.db, role_id, name, &profile)
        .await
        .map_err(|e| format!("更新玩家身份失败: {}", e))?;

    let options = prompt_options(&app);
    let was_possessed = current_possessed(&app).await == role_id;
    {
        let service = state.ai_service.lock().await;
        let mut gs = service.game_status.lock().await;

        // User 实体没有 settings.yml，用 role + profile 合成 settings 热更新内存副本，
        // 使 display_name 等展示字段与改名结果一致（人设正文由重建从盘上重新取）。
        if let Some(role) = RoleRepo::get_role_by_id(&state.db, role_id)
            .await
            .map_err(|e| format!("查询身份失败: {}", e))?
        {
            let synthesized = user_identity_settings(&role, &profile);
            gs.role_manager
                .update_role_persona_settings(role_id, &synthesized);
        }

        // 改名/改人设会改变玩家缓存与嵌在 AI 人设里的名字：前者只有被附身身份需要
        // 立刻校正，后者与是否被附身无关，故 SYSTEM 行一律重建。
        if was_possessed {
            gs.refresh_possessed_cache(&state.db)
                .await
                .map_err(|e| format!("刷新玩家缓存失败: {}", e))?;
        }
        gs.rebuild_system_prompts(&state.db, options)
            .await
            .map_err(|e| format!("重建角色人设失败: {}", e))?;
    }

    if let Err(e) = app.emit("role:list-updated", ()) {
        tracing::warn!("emit role:list-updated 失败: {e}");
    }
    Ok(())
}

#[tauri::command]
pub async fn delete_identity(app: AppHandle, role_id: i32) -> Result<bool, String> {
    let state = app.state::<AppState>();
    let was_possessed = current_possessed(&app).await == role_id;
    let deleted = RoleRepo::delete_player_identity(&state.db, role_id)
        .await
        .map_err(|e| format!("删除玩家身份失败: {}", e))?;

    if deleted {
        let options = prompt_options(&app);
        let (fell_back, fallback_info) = {
            let service = state.ai_service.lock().await;
            let mut gs = service.game_status.lock().await;

            // 身份集合已变化，先校正缓存供 God Agent 判据使用
            gs.refresh_human_role_ids(&state.db)
                .await
                .map_err(|e| format!("刷新玩家身份缓存失败: {}", e))?;

            // 被附身实体被删后 possessed 会指向不存在的行，必须回落到默认身份，
            // 否则后续发言归属与缓存刷新都会落空。
            if was_possessed {
                gs.possessed_role_id = PLAYER_ROLE_ID;
                gs.refresh_possessed_cache(&state.db)
                    .await
                    .map_err(|e| format!("刷新玩家缓存失败: {}", e))?;
            }

            gs.rebuild_system_prompts(&state.db, options)
                .await
                .map_err(|e| format!("重建角色人设失败: {}", e))?;

            (
                was_possessed,
                PossessedInfo {
                    role_id: gs.possessed_role_id,
                    name: gs.player.user_name.clone(),
                    subtitle: gs.player.user_subtitle.clone(),
                },
            )
        };
        // 附身回退到默认身份时额外广播一次，前端切换器据此同步
        if fell_back {
            emit_possessed(&app, &fallback_info);
        }
    }

    if let Err(e) = app.emit("role:list-updated", ()) {
        tracing::warn!("emit role:list-updated 失败: {e}");
    }
    Ok(deleted)
}

#[tauri::command]
pub async fn possess_entity(app: AppHandle, role_id: i32) -> Result<String, String> {
    let state = app.state::<AppState>();
    let options = prompt_options(&app);
    let (info, handoff) = {
        let service = state.ai_service.lock().await;
        let mut gs = service.game_status.lock().await;
        let outcome = possession::possess_entity(&mut gs, &state.db, role_id, options)
            .await
            .map_err(|e| format!("附身失败: {}", e))?;
        // 移交话筒后前端需要同步当前对话对象，在锁内取好名字再于锁外广播
        let handoff = match outcome.handoff_role_id {
            Some(target_id) => {
                let name = gs
                    .get_role(&state.db, target_id)
                    .await
                    .map_err(|e| format!("读取移交角色失败: {}", e))?
                    .display_name
                    .clone()
                    .unwrap_or_default();
                Some((target_id, name))
            }
            None => None,
        };
        (
            PossessedInfo {
                role_id: gs.possessed_role_id,
                name: outcome.display_name.clone(),
                subtitle: gs.player.user_subtitle.clone(),
            },
            handoff,
        )
    };

    emit_possessed(&app, &info);
    // 附身当前对话对象会触发话筒移交，补发与 God Agent 同形的切换事件
    if let Some((target_id, target_name)) = handoff {
        emit_character_switch(&app, target_id, &target_name);
    }
    Ok(info.name)
}

#[tauri::command]
pub async fn get_possessed_entity(app: AppHandle) -> Result<PossessedInfo, String> {
    let state = app.state::<AppState>();
    let service = state.ai_service.lock().await;
    let gs = service.game_status.lock().await;
    Ok(PossessedInfo {
        role_id: gs.possessed_role_id,
        name: gs.player.user_name.clone(),
        subtitle: gs.player.user_subtitle.clone(),
    })
}
