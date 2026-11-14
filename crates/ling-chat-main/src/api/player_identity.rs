use std::collections::HashMap;

use sea_orm::{ActiveModelTrait, Set};
use tauri::{AppHandle, Emitter, Manager};
use tauri_plugin_store::StoreExt;

use crate::AppState;
use crate::ai_service::game_system::player_identity::bootstrap::{
    apply_player_role, load_player_relations,
};
use crate::ai_service::game_system::player_identity::{
    IdentityStore, PlayerIdentity, PlayerIdentityInput, PlayerIdentitySummary, RoleRelationStatus,
    ensure_identity_switchable, relation_ready_all, resolve_relation, runtime_relations,
};
use crate::ai_service::game_system::status_ops::PromptLine;
use crate::ai_service::types::LineBase;
use crate::api::game::{WebInitData, build_web_init_data};
use crate::config::AppConfig;
use crate::db::entities::role::{self, RoleType};
use crate::db::managers::role_repo::{RoleRelation, RoleRepo};
use crate::db::managers::save_repo::SaveRepo;
use crate::utils::prompt::{PromptOptions, PromptRole};

fn store() -> IdentityStore {
    IdentityStore::new(&crate::api::data_dir())
}

/// 系统旁白台词（不归属任何玩家角色）。
fn narrator_line(text: String) -> LineBase {
    PromptLine::new(PromptRole::Narrator, text)
        .display_name("系统")
        .into_base()
}

async fn player_role_and_card(
    state: &AppState,
    role_id: i32,
) -> Result<(role::Model, PlayerIdentity), String> {
    let role = RoleRepo::get_role_by_id(&state.db, role_id)
        .await
        .map_err(|e| format!("查询角色失败: {e}"))?
        .filter(|role| role.role_type == RoleType::Player)
        .ok_or_else(|| "玩家角色不存在".to_string())?;
    let identity_id = role
        .identity_id
        .clone()
        .ok_or_else(|| "玩家角色缺少身份卡 id".to_string())?;
    let card = store()
        .find_by_id(&identity_id)
        .ok_or_else(|| "身份卡文件不存在".to_string())?;
    Ok((role, card))
}

fn input_from_card(
    card: &PlayerIdentity,
    role_id: i32,
    relations: HashMap<i32, String>,
) -> PlayerIdentityInput {
    PlayerIdentityInput {
        role_id: Some(role_id),
        name: card.display_name().to_string(),
        subtitle: card.subtitle.clone(),
        prompt: card.prompt.clone(),
        avatar: card.avatar.clone(),
        relations,
    }
}

#[tauri::command]
pub async fn list_player_identities(app: AppHandle) -> Result<Vec<PlayerIdentitySummary>, String> {
    let state = app.state::<AppState>();
    let data_dir = crate::api::data_dir();
    let _ = crate::db::role_sync::sync_player_roles(&state.db, &data_dir).await;

    let store = store();
    let primary = store.primary_identity();
    let current_id = store
        .current_id()
        .or_else(|| primary.as_ref().map(|identity| identity.id.clone()));

    let roles = RoleRepo::get_player_roles(&state.db)
        .await
        .map_err(|e| format!("读取玩家角色失败: {e}"))?;
    let mut items = Vec::new();
    for role in roles {
        let Some(identity_id) = role.identity_id.as_deref() else {
            continue;
        };
        let Some(card) = store.find_by_id(identity_id) else {
            continue;
        };
        let relation_count = RoleRepo::load_relations(&state.db, role.id)
            .await
            .map(|rows| {
                rows.values()
                    .filter(|value| !value.text.trim().is_empty())
                    .count()
            })
            .unwrap_or(0);
        let mut summary =
            PlayerIdentitySummary::from_identity(&card, role.id, current_id.as_deref());
        summary.relation_count = relation_count;
        items.push(summary);
    }
    items.sort_by_key(|item| (!item.is_primary, item.name.clone()));
    Ok(items)
}

#[tauri::command]
pub async fn get_player_identity(
    app: AppHandle,
    role_id: i32,
) -> Result<Option<PlayerIdentityInput>, String> {
    let state = app.state::<AppState>();
    let Ok((role, card)) = player_role_and_card(&state, role_id).await else {
        return Ok(None);
    };
    let relations = load_player_relations(&state.db, role.id).await;
    Ok(Some(input_from_card(&card, role.id, relations)))
}

#[tauri::command]
pub async fn get_current_player_identity(app: AppHandle) -> Result<PlayerIdentityInput, String> {
    let state = app.state::<AppState>();
    let role_id = {
        let service = state.ai_service.lock().await;
        let gs = service.game_status.lock().await;
        gs.player.player_role_id
    };
    if let Some(role_id) = role_id {
        if let Some(input) = get_player_identity(app.clone(), role_id).await? {
            return Ok(input);
        }
    }
    if let Some(primary) = store().primary_identity() {
        if let Ok(Some(role)) = RoleRepo::get_role_by_identity_id(&state.db, &primary.id).await {
            let relations = load_player_relations(&state.db, role.id).await;
            return Ok(input_from_card(&primary, role.id, relations));
        }
        return Ok(PlayerIdentityInput {
            role_id: None,
            name: primary.display_name().to_string(),
            subtitle: primary.subtitle,
            prompt: primary.prompt,
            avatar: primary.avatar,
            relations: HashMap::new(),
        });
    }
    Ok(PlayerIdentityInput {
        name: "玩家A（待填写）".to_string(),
        subtitle: "玩家A（待填写）".to_string(),
        ..Default::default()
    })
}

#[tauri::command]
pub async fn save_player_identity(
    app: AppHandle,
    identity: PlayerIdentityInput,
) -> Result<PlayerIdentityInput, String> {
    let state = app.state::<AppState>();
    let data_dir = crate::api::data_dir();
    let store = store();

    let existing = match identity.role_id {
        Some(role_id) => Some(player_role_and_card(&state, role_id).await?),
        None => None,
    };
    let mut card = match existing.as_ref() {
        Some((_, card)) => card.clone(),
        None => PlayerIdentity::default(),
    };
    card.name = identity.name.clone();
    card.subtitle = identity.subtitle.clone();
    card.prompt = identity.prompt.clone();
    card.avatar = identity.avatar.clone();
    let saved = store
        .save(&card)
        .map_err(|e| format!("保存身份失败: {e}"))?;

    // 文件保存后把玩家角色行对齐（新卡会在这里拿到 role id）。
    crate::db::role_sync::sync_player_roles(&state.db, &data_dir)
        .await
        .map_err(|e| format!("同步玩家角色失败: {e}"))?;
    let role = RoleRepo::get_role_by_identity_id(&state.db, &saved.id)
        .await
        .map_err(|e| format!("查询玩家角色失败: {e}"))?
        .ok_or_else(|| "玩家角色未同步".to_string())?;
    let role_id = role.id;

    let relations: HashMap<i32, RoleRelation> = identity
        .relations
        .iter()
        .filter(|(target, text)| **target != role_id && !text.trim().is_empty())
        .map(|(target, text)| {
            (
                *target,
                RoleRelation {
                    text: text.trim().to_string(),
                    initial_affection: None,
                },
            )
        })
        .collect();
    RoleRepo::replace_relations(&state.db, role_id, &relations)
        .await
        .map_err(|e| format!("保存身份关系失败: {e}"))?;

    let mut active: role::ActiveModel = role.into();
    active.identity_relations_seeded = Set(1);
    active
        .update(&state.db)
        .await
        .map_err(|e| format!("更新玩家角色失败: {e}"))?;

    let service = state.ai_service.lock().await;
    let mut gs = service.game_status.lock().await;
    if gs.player.player_role_id == Some(role_id) {
        let relations = load_player_relations(&state.db, role_id).await;
        let changed = gs.player.user_name != saved.display_name()
            || gs.player.user_subtitle != saved.subtitle
            || gs.player.user_prompt != saved.prompt
            || gs.player.relations != relations;
        if changed {
            // 清空 prompt 时保留旧值，避免本局人设突然丢失。
            let fallback_prompt = gs.player.user_prompt.clone();
            apply_player_role(&mut gs, &state.db, &data_dir, role_id, fallback_prompt)
                .await
                .map_err(|e| format!("更新玩家状态失败: {e}"))?;
            let app_config = AppConfig::load(&app).unwrap_or_default();
            crate::ai_service::game_system::player_identity::persona::rebuild_and_refresh(
                &mut gs,
                &state.db,
                PromptOptions::from_config(&app_config),
            )
            .await
            .map_err(|e| format!("重建人设行失败: {e}"))?;
            gs.persist_lines_if_bound(&state.db).await;
        }
    }
    // 新建身份后玩家角色集合会变，刷新一下，避免被当作普通 AI 角色。
    if let Ok(ids) = RoleRepo::get_player_role_ids(&state.db).await {
        gs.role_manager.set_player_role_ids(ids);
    }
    drop(gs);
    drop(service);

    let relations = load_player_relations(&state.db, role_id).await;
    Ok(input_from_card(&saved, role_id, relations))
}

#[tauri::command]
pub async fn delete_player_identity(app: AppHandle, role_id: i32) -> Result<(), String> {
    let state = app.state::<AppState>();
    let (role, card) = player_role_and_card(&state, role_id).await?;

    let used = SaveRepo::find_saves_using_player_role(&state.db, role.id)
        .await
        .map_err(|e| format!("查询存档引用失败: {e}"))?
        .len();
    if used > 0 {
        return Err(format!("有 {used} 个存档正在使用这个身份，无法删除。"));
    }

    let store = store();
    let removed = store
        .delete(&card.id)
        .map_err(|e| format!("删除身份失败: {e}"))?;
    if !removed {
        return Err("身份不存在".to_string());
    }
    RoleRepo::delete_player_role(&state.db, role.id)
        .await
        .map_err(|e| format!("删除玩家角色失败: {e}"))?;

    // 当前身份被删时切回主身份（若还有），并重建本局人设。
    let fallback = store.primary_identity();
    let service = state.ai_service.lock().await;
    let mut gs = service.game_status.lock().await;
    if let Ok(ids) = RoleRepo::get_player_role_ids(&state.db).await {
        gs.role_manager.set_player_role_ids(ids);
    }
    if gs.player.player_role_id == Some(role.id) {
        let fallback_role = match fallback {
            Some(primary) => RoleRepo::get_role_by_identity_id(&state.db, &primary.id)
                .await
                .ok()
                .flatten(),
            None => None,
        };
        match fallback_role {
            Some(primary_role) => {
                apply_player_role(
                    &mut gs,
                    &state.db,
                    &crate::api::data_dir(),
                    primary_role.id,
                    String::new(),
                )
                .await
                .map_err(|e| format!("切换回主身份失败: {e}"))?;
                let app_config = AppConfig::load(&app).unwrap_or_default();
                crate::ai_service::game_system::player_identity::persona::rebuild_and_refresh(
                    &mut gs,
                    &state.db,
                    PromptOptions::from_config(&app_config),
                )
                .await
                .map_err(|e| format!("重建人设行失败: {e}"))?;
                gs.persist_lines_if_bound(&state.db).await;
            },
            None => {
                gs.player.player_role_id = None;
                gs.player.relations.clear();
            },
        }
    }
    Ok(())
}

#[tauri::command]
pub async fn set_current_player_identity(
    app: AppHandle,
    role_id: i32,
    exit_desc: Option<String>,
    enter_desc: Option<String>,
) -> Result<WebInitData, String> {
    let state = app.state::<AppState>();
    let (role, card) = player_role_and_card(&state, role_id).await?;
    let new_role_id = role.id;
    let new_name = card.display_name().to_string();

    let clean_desc = |s: Option<String>| -> Option<String> {
        s.map(|t| t.trim().chars().take(200).collect::<String>())
            .filter(|t| !t.is_empty())
    };
    let exit_desc = clean_desc(exit_desc);
    let enter_desc = clean_desc(enter_desc);

    let data_dir = crate::api::data_dir();
    let service = state.ai_service.lock().await;
    {
        let mut gs = service.game_status.lock().await;
        ensure_identity_switchable(&gs)?;

        let old_name = gs.player.user_name.clone();
        let old_prompt = gs.player.user_prompt.clone();
        let old_player_role_id = gs.player.player_role_id;

        if old_player_role_id != Some(new_role_id) {
            let onstage_ids = gs.onstage_role_ids.clone();
            for rid in &onstage_ids {
                gs.seed_player_affection(&state.db, *rid, Some(new_role_id))
                    .await
                    .map_err(|e| format!("播种好感度失败: {e}"))?;
            }

            let fallback_prompt = {
                let mut extracted = String::new();
                if let Some(main_id) = gs.main_role_id {
                    if let Some(role) = gs.role_manager.get_loaded(main_id) {
                        let (_, body) = crate::utils::prompt::extract_player_setting(
                            role.settings.system_prompt.as_deref().unwrap_or_default(),
                        );
                        extracted = body.unwrap_or_default();
                    }
                }
                extracted
            };
            apply_player_role(&mut gs, &state.db, &data_dir, new_role_id, fallback_prompt)
                .await
                .map_err(|e| format!("应用玩家身份失败: {e}"))?;
            gs.overlay_affections_for_player(Some(new_role_id));

            let app_config = AppConfig::load(&app).unwrap_or_default();
            crate::ai_service::game_system::player_identity::persona::rebuild_and_refresh(
                &mut gs,
                &state.db,
                PromptOptions::from_config(&app_config),
            )
            .await
            .map_err(|e| format!("换身份后重建人设行失败: {e}"))?;

            let has_old = !old_name.trim().is_empty();
            let has_new = !new_name.trim().is_empty();

            if has_old {
                if let Some(desc) = exit_desc.as_deref() {
                    gs.add_line(&state.db, narrator_line(format!("{old_name}{desc}")))
                        .await
                        .map_err(|e| format!("添加退场描述旁白失败: {e}"))?;
                }
            }
            if has_old && has_new {
                gs.add_line(&state.db, narrator_line(format!("{}离开了对话", old_name)))
                    .await
                    .map_err(|e| format!("添加换身份旁白失败: {e}"))?;
            }

            if has_old {
                let mut notes: Vec<(i32, String)> = Vec::new();
                for rid in onstage_ids.iter().copied() {
                    let Some(loaded) = gs.role_manager.get_loaded(rid) else {
                        continue;
                    };
                    let role_name = loaded
                        .display_name
                        .clone()
                        .unwrap_or_else(|| loaded.settings.ai_name.clone());
                    let alias =
                        crate::ai_service::game_system::player_identity::effective_user_alias(
                            &loaded.settings,
                            &old_name,
                        );
                    let speaker_relations = runtime_relations::load_effective(rid, &gs);
                    let relation = old_player_role_id.and_then(|old_id| {
                        resolve_relation(rid, old_id, &speaker_relations, &HashMap::new(), None)
                    });
                    let affection_tier = crate::ai_service::affection::resolve_var(
                        &gs.global_variables,
                        rid,
                        old_player_role_id,
                    )
                    .and_then(crate::ai_service::affection::state_from_value)
                    .map(|state| crate::ai_service::affection::tier_label(state.vector.average()))
                    .or_else(|| {
                        let override_value = old_player_role_id.and_then(|old_id| {
                            gs.role_manager.relation_affection_of_role(rid, old_id)
                        });
                        crate::ai_service::affection::effective_initial_affection(
                            &loaded.settings,
                            override_value,
                        )
                        .map(|v| crate::ai_service::affection::tier_label(v.clamp(-100, 100)))
                    });
                    let mut note =
                        format!("离场注记（{role_name} 视角）：刚才离开的「{old_name}」");
                    let prompt = old_prompt.trim();
                    if !prompt.is_empty() {
                        note.push_str(&format!("，基础设定是：{prompt}"));
                    }
                    note.push('。');
                    note.push_str(&format!("{role_name} 称呼 ta 为「{alias}」"));
                    if let Some(r) = relation {
                        note.push_str(&format!("，{role_name} 对 ta 的认知（关系）是：{}", r.text));
                    }
                    if let Some(tier) = affection_tier {
                        note.push_str(&format!("，好感度：{tier}"));
                    }
                    note.push_str("。这些认知不会因 ta 离开对话而改变。");
                    notes.push((rid, note));
                }
                for (rid, note) in notes {
                    gs.add_line_perceived(&state.db, narrator_line(note), vec![rid])
                        .await
                        .map_err(|e| format!("添加离场注记失败: {e}"))?;
                }
            }

            if has_old && has_new {
                gs.add_line(&state.db, narrator_line(format!("{new_name}加入了对话")))
                    .await
                    .map_err(|e| format!("添加换身份旁白失败: {e}"))?;
            }
            if has_new {
                if let Some(desc) = enter_desc.as_deref() {
                    gs.add_line(&state.db, narrator_line(format!("{new_name}{desc}")))
                        .await
                        .map_err(|e| format!("添加出场描述旁白失败: {e}"))?;
                }
            }
            gs.persist_lines_if_bound(&state.db).await;
        }
    }

    store()
        .set_current_id(&card.id)
        .map_err(|e| format!("记录当前身份失败: {e}"))?;

    let active_save_id = {
        let gs = service.game_status.lock().await;
        gs.active_save_id
    };
    if let Some(save_id) = active_save_id {
        SaveRepo::update_save_player_role(&state.db, save_id, Some(new_role_id))
            .await
            .map_err(|e| format!("同步存档玩家角色失败: {e}"))?;
    }

    let result = build_web_init_data(&service, &app).await;
    drop(service);
    result
}

#[tauri::command]
pub async fn start_new_game_with_identity(
    app: AppHandle,
    role_id: i32,
) -> Result<WebInitData, String> {
    let state = app.state::<AppState>();
    let (_, card) = player_role_and_card(&state, role_id).await?;

    let app_config = AppConfig::load(&app).unwrap_or_default();
    let prompt_options = PromptOptions::from_config(&app_config);

    {
        let service = state.ai_service.lock().await;
        let gs = service.game_status.lock().await;
        ensure_identity_switchable(&gs)?;
    }

    store()
        .set_current_id(&card.id)
        .map_err(|e| format!("记录当前身份失败: {e}"))?;

    let mut service = state.ai_service.lock().await;
    let character_id = match service.init_character_id {
        Some(id) => Some(id),
        None => app
            .store(crate::config::STORE_FILE)
            .ok()
            .and_then(|store| store.get(crate::config::session::LAST_CHARACTER_ID))
            .and_then(|v| v.as_i64())
            .map(|v| v as i32),
    };
    if character_id.is_none() {
        return Err("还没有选择 AI 角色，无法开始新对话。".to_string());
    }

    service
        .init_game_status(character_id, prompt_options)
        .await
        .map_err(|e| format!("开始新对话失败: {e}"))?;
    if let Some(cid) = character_id {
        service
            .seed_initial_affection(cid)
            .await
            .map_err(|e| format!("初始化初始好感度失败: {e}"))?;
    }

    let result = build_web_init_data(&service, &app).await;
    drop(service);
    result
}

/// 关系编辑器的一份数据：关系与初始好感度都按目标角色 id 作为键。
#[derive(serde::Serialize)]
pub struct RoleRelationsPayload {
    pub relations: HashMap<String, String>,
    pub affections: HashMap<String, Option<i32>>,
}

#[tauri::command]
pub async fn get_role_relations(
    app: AppHandle,
    role_id: i32,
) -> Result<RoleRelationsPayload, String> {
    let state = app.state::<AppState>();
    let rows = RoleRepo::load_relations(&state.db, role_id)
        .await
        .map_err(|e| format!("读取角色关系失败: {e}"))?;
    let mut relations = HashMap::new();
    let mut affections = HashMap::new();
    for (target_role_id, value) in rows {
        let key = target_role_id.to_string();
        if !value.text.trim().is_empty() {
            relations.insert(key.clone(), value.text);
        }
        affections.insert(key, value.initial_affection);
    }
    Ok(RoleRelationsPayload {
        relations,
        affections,
    })
}

#[tauri::command]
pub async fn save_role_relations(
    app: AppHandle,
    role_id: i32,
    relations: HashMap<String, String>,
    affections: HashMap<String, Option<i32>>,
) -> Result<(), String> {
    let state = app.state::<AppState>();
    let relation_roles = RoleRepo::get_player_roles(&state.db)
        .await
        .map_err(|e| format!("读取玩家角色失败: {e}"))?;
    let player_role_ids: std::collections::HashSet<i32> =
        relation_roles.into_iter().map(|role| role.id).collect();

    let mut next: HashMap<i32, RoleRelation> = HashMap::new();
    for (key, text) in relations {
        let Ok(target_role_id) = key.trim().parse::<i32>() else {
            continue;
        };
        next.entry(target_role_id).or_default().text = text;
    }
    for (key, value) in affections {
        let Ok(target_role_id) = key.trim().parse::<i32>() else {
            continue;
        };
        if player_role_ids.contains(&target_role_id) {
            next.entry(target_role_id).or_default().initial_affection = value;
        }
    }
    let next: HashMap<i32, RoleRelation> = next
        .into_iter()
        .filter(|(_, value)| !value.text.trim().is_empty() || value.initial_affection.is_some())
        .map(|(target, value)| {
            (
                target,
                RoleRelation {
                    text: value.text.trim().to_string(),
                    initial_affection: value.initial_affection,
                },
            )
        })
        .collect();

    let stored = RoleRepo::load_relations(&state.db, role_id)
        .await
        .map_err(|e| format!("读取角色关系失败: {e}"))?;
    RoleRepo::replace_relations(&state.db, role_id, &next)
        .await
        .map_err(|e| format!("保存角色关系失败: {e}"))?;

    let service = state.ai_service.lock().await;
    let mut gs = service.game_status.lock().await;
    let text_cache: HashMap<i32, String> = next
        .iter()
        .filter(|(_, value)| !value.text.trim().is_empty())
        .map(|(target, value)| (*target, value.text.clone()))
        .collect();
    let affection_cache: HashMap<i32, i32> = next
        .iter()
        .filter_map(|(target, value)| value.initial_affection.map(|v| (*target, v)))
        .collect();
    gs.role_manager
        .set_role_relations(role_id, text_cache, affection_cache);

    if let Some(player_role_id) = gs.player.player_role_id {
        let old_initial = stored
            .get(&player_role_id)
            .and_then(|row| row.initial_affection);
        let new_initial = next
            .get(&player_role_id)
            .and_then(|row| row.initial_affection);
        if old_initial != new_initial {
            if let Some(init) = gs.role_manager.get_loaded(role_id).and_then(|role| {
                crate::ai_service::affection::effective_initial_affection(
                    &role.settings,
                    new_initial,
                )
            }) {
                let payload = gs
                    .sync_player_affection_initial(&state.db, role_id, Some(player_role_id), init)
                    .await;
                let _ = app.emit("affection:changed", payload);
            }
        }
    }

    if gs.role_manager.get_loaded(role_id).is_some() {
        let app_config = AppConfig::load(&app).unwrap_or_default();
        crate::ai_service::game_system::player_identity::persona::rebuild_and_refresh(
            &mut gs,
            &state.db,
            PromptOptions::from_config(&app_config),
        )
        .await
        .map_err(|e| format!("重建人设行失败: {e}"))?;
        gs.persist_lines_if_bound(&state.db).await;
    }
    Ok(())
}

#[derive(serde::Serialize)]
pub struct RelationReadyStatus {
    pub ready: bool,
    pub roles: Vec<RoleRelationStatus>,
}

#[tauri::command]
pub async fn check_relation_ready(
    app: AppHandle,
    role_id: i32,
) -> Result<RelationReadyStatus, String> {
    let state = app.state::<AppState>();
    let service = state.ai_service.lock().await;
    let gs = service.game_status.lock().await;

    let mut role_ids = gs.onstage_role_ids.clone();
    if role_ids.is_empty() {
        role_ids.push(role_id);
    }
    let roles = relation_ready_all(&gs, gs.player.player_role_id, &role_ids);
    let ready = roles.iter().all(|s| s.role_side && s.identity_side);
    Ok(RelationReadyStatus { ready, roles })
}
