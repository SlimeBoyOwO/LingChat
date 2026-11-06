use std::collections::HashMap;

use tauri::{AppHandle, Emitter, Manager};
use tauri_plugin_store::StoreExt;

use crate::AppState;
use crate::ai_service::game_system::player_identity::{
    IdentityStore, PlayerIdentity, PlayerIdentitySummary, RelationEndpoint, RoleRelationStatus,
    effective_user_alias, ensure_identity_switchable, relation_ready_all, resolve_relation,
    role_relations, runtime_relations,
};
use crate::ai_service::game_system::status_ops::PromptLine;
use crate::ai_service::types::LineBase;
use crate::api::game::{WebInitData, build_web_init_data};
use crate::config::AppConfig;
use crate::db::managers::role_repo::{IdentityRelation, RoleRepo};
use crate::db::managers::save_repo::SaveRepo;
use crate::utils::prompt::{PromptOptions, PromptRole};

fn store() -> IdentityStore {
    IdentityStore::new(&crate::api::data_dir())
}

/// 系统旁白台词（不归属任何身份）。
fn narrator_line(text: String) -> LineBase {
    PromptLine::new(PromptRole::Narrator, text)
        .display_name("系统")
        .into_base()
}

#[tauri::command]
pub async fn list_player_identities(app: AppHandle) -> Result<Vec<PlayerIdentitySummary>, String> {
    let store = store();
    let primary = store.primary_identity();
    let current_id = store
        .current_id()
        .or_else(|| primary.as_ref().map(|identity| identity.id.clone()));
    let mut items: Vec<PlayerIdentitySummary> = store
        .list()
        .iter()
        .map(|identity| PlayerIdentitySummary::from_identity(identity, current_id.as_deref()))
        .collect();

    // 主身份排在最前；其余是旧角色卡迁移时按名字去重生成的专属身份。
    if let Some(primary) = primary.as_ref() {
        items.sort_by_key(|item| item.id != primary.id);
    }

    // 还没有任何身份卡时（迁移未跑到），按产品默认文案给一张内存占位，不落盘。
    if items.is_empty() {
        let has_main_role = {
            let state = app.state::<AppState>();
            let service = state.ai_service.lock().await;
            let gs = service.game_status.lock().await;
            gs.main_role_id.is_some()
        };
        if has_main_role {
            let current = current_identity().await?;
            items.push(PlayerIdentitySummary::from_identity(
                &current,
                Some(&current.id),
            ));
        }
    }

    Ok(items)
}

#[tauri::command]
pub async fn get_player_identity(
    _app: AppHandle,
    id: String,
) -> Result<Option<PlayerIdentity>, String> {
    Ok(store().find_by_id(&id))
}

#[tauri::command]
pub async fn get_current_player_identity(_app: AppHandle) -> Result<PlayerIdentity, String> {
    current_identity().await
}

#[tauri::command]
pub async fn save_player_identity(
    app: AppHandle,
    identity: PlayerIdentity,
) -> Result<PlayerIdentity, String> {
    let store = store();
    let mut identity = identity;
    // 前端可能不认识 is_primary 字段，保存主身份时强制保留，避免被回写成 false。
    if store
        .primary_identity()
        .map(|primary| primary.id == identity.id)
        .unwrap_or(false)
    {
        identity.is_primary = true;
    }
    // 角色绑定由迁移维护，前端不认识该字段，保存时不能把它清空。
    if identity.source_folders.is_empty() {
        if let Some(existing) = store.find_by_id(&identity.id) {
            identity.source_folders = existing.source_folders;
        }
    }
    let saved = store
        .save(&identity)
        .map_err(|e| format!("保存身份失败: {e}"))?;

    let state = app.state::<AppState>();
    // 索引表跟随文件更新（存档列表等查询用）。
    if let Err(e) =
        crate::db::role_sync::sync_player_identities(&state.db, &crate::api::data_dir()).await
    {
        tracing::warn!("身份索引同步失败: {e}");
    }
    let service = state.ai_service.lock().await;
    let mut gs = service.game_status.lock().await;
    if gs.player.identity_id.as_deref() == Some(saved.id.as_str()) {
        let changed = gs.player.user_name != saved.display_name()
            || gs.player.user_subtitle != saved.subtitle
            || gs.player.user_prompt != saved.prompt
            || gs.player.relations != saved.relations;
        if changed {
            gs.player.user_name = saved.display_name().to_string();
            gs.player.user_subtitle = saved.subtitle.clone();
            gs.player.user_prompt = saved.prompt.clone();
            gs.player.relations = saved.relations.clone();

            let app_config = AppConfig::load(&app).unwrap_or_default();
            crate::ai_service::game_system::player_identity::persona::rebuild_and_refresh(
                &mut gs,
                &state.db,
                &crate::api::data_dir(),
                PromptOptions::from_config(&app_config),
            )
            .await
            .map_err(|e| format!("重建人设行失败: {e}"))?;
            gs.persist_lines_if_bound(&state.db).await;
        }
    }
    Ok(saved)
}

#[tauri::command]
pub async fn delete_player_identity(app: AppHandle, id: String) -> Result<(), String> {
    let state = app.state::<AppState>();

    let used = SaveRepo::find_saves_using_identity(&state.db, &id)
        .await
        .map_err(|e| format!("查询身份引用失败: {e}"))?
        .len();

    if used > 0 {
        return Err(format!("有 {used} 个存档正在使用这个身份，无法删除。"));
    }

    let store = store();
    let is_primary = store
        .primary_identity()
        .map(|primary| primary.id == id)
        .unwrap_or(false);
    let removed = store
        .delete(&id)
        .map_err(|e| format!("删除身份失败: {e}"))?;

    if !removed {
        return Err("身份不存在".to_string());
    }
    if is_primary {
        store.clear_primary_marker();
    }
    // 索引表跟随文件更新（被删的卡从索引里移除）。
    if let Err(e) =
        crate::db::role_sync::sync_player_identities(&state.db, &crate::api::data_dir()).await
    {
        tracing::warn!("身份索引同步失败: {e}");
    }
    Ok(())
}

#[tauri::command]
pub async fn set_current_player_identity(
    app: AppHandle,
    id: String,
    exit_desc: Option<String>,
    enter_desc: Option<String>,
) -> Result<WebInitData, String> {
    let state = app.state::<AppState>();

    let identity = store()
        .find_by_id(&id)
        .ok_or_else(|| "身份不存在".to_string())?;

    // 转场描述（可选）：去空白、限长 200 字符；留空视为不生成对应描述旁白。
    let clean_desc = |s: Option<String>| -> Option<String> {
        s.map(|t| t.trim().chars().take(200).collect::<String>())
            .filter(|t| !t.is_empty())
    };
    let exit_desc = clean_desc(exit_desc);
    let enter_desc = clean_desc(enter_desc);

    let service = state.ai_service.lock().await;
    {
        let mut gs = service.game_status.lock().await;
        ensure_identity_switchable(&gs)?;

        let old_name = gs.player.user_name.clone();
        // 同身份短路：跳过切换（不重建、不加旁白），直接走绑定与界面刷新。
        if gs.player.identity_id.as_deref() != Some(identity.id.as_str()) {
            let old_identity_id = gs.player.identity_id.clone();
            let old_prompt = gs.player.user_prompt.clone();
            gs.player.user_name = identity.display_name().to_string();
            gs.player.user_subtitle = identity.subtitle.clone();
            gs.player.user_prompt = identity.prompt.clone();
            gs.player.identity_id = Some(identity.id.clone());
            gs.player.relations = identity.relations.clone();

            // 循环内要为每个角色改写状态，先取出在场名单，避免与 gs 的可变借用冲突。
            let onstage_ids = gs.onstage_role_ids.clone();
            for rid in onstage_ids {
                gs.seed_identity_affection(&state.db, rid, Some(identity.id.as_str()))
                    .await
                    .map_err(|e| format!("播种好感度失败: {e}"))?;
            }
            gs.overlay_affections_for_identity(Some(identity.id.as_str()));

            let app_config = AppConfig::load(&app).unwrap_or_default();
            let data_dir = crate::api::data_dir();
            crate::ai_service::game_system::player_identity::persona::rebuild_and_refresh(
                &mut gs,
                &state.db,
                &data_dir,
                PromptOptions::from_config(&app_config),
            )
            .await
            .map_err(|e| format!("换身份后重建人设行失败: {e}"))?;

            // 顺序：退场描述（如有）→「A离开了对话」→ 离场注记×N →「B加入了对话」→ 出场描述（如有）；
            // 描述留空则该句完全不生成，标准旁白与注记无论填不填都照常。
            let new_name = identity.display_name().to_string();
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

            // 离场注记：对每个在场角色各投递一条只有该角色看得到的注记，保留旧身份的
            // 设定/称呼/关系认知（感知列表只含该角色自己），防止换身份后各角色视角失忆；
            // 上面/下面两条旁白仍是全员可见的公共提示。
            if has_old {
                let mut notes: Vec<(i32, String)> = Vec::new();
                for rid in gs.onstage_role_ids.iter().copied() {
                    let Some(role) = gs.role_manager.get_loaded(rid) else {
                        continue;
                    };
                    let role_name = role
                        .display_name
                        .clone()
                        .unwrap_or_else(|| role.settings.ai_name.clone());
                    let folder = role.settings.character_folder.clone();
                    let alias = effective_user_alias(&role.settings, &old_name);
                    let speaker_relations =
                        runtime_relations::load_effective(&data_dir, &folder, &gs);
                    let relation = resolve_relation(
                        &RelationEndpoint::Ai(folder),
                        &RelationEndpoint::Me(old_identity_id.clone().unwrap_or_default()),
                        &speaker_relations,
                        &HashMap::new(),
                        None,
                    );
                    let affection_tier = crate::ai_service::affection::resolve_var(
                        &gs.global_variables,
                        rid,
                        old_identity_id.as_deref(),
                    )
                    .and_then(crate::ai_service::affection::state_from_value)
                    .map(|state| crate::ai_service::affection::tier_label(state.vector.average()))
                    .or_else(|| {
                        let override_value = old_identity_id
                            .as_deref()
                            .and_then(|id| gs.role_manager.identity_affection_of_role(rid, id));
                        crate::ai_service::affection::effective_initial_affection(
                            &role.settings,
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
                gs.add_line(&state.db, narrator_line(format!("{}加入了对话", new_name)))
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

    // 记录全局当前身份 + 同步到当前存档（读档才能恢复到同一个身份）
    store()
        .set_current_id(&identity.id)
        .map_err(|e| format!("记录当前身份失败: {e}"))?;

    let active_save_id = {
        let gs = service.game_status.lock().await;
        gs.active_save_id
    };
    if let Some(save_id) = active_save_id {
        if let Err(e) = SaveRepo::upsert_save_identity(&state.db, save_id, &identity.id).await {
            tracing::warn!("同步存档身份失败: {e}");
        }
    }

    let result = build_web_init_data(&service, &app).await;
    drop(service);
    result
}

#[tauri::command]
pub async fn start_new_game_with_identity(
    app: AppHandle,
    id: String,
) -> Result<WebInitData, String> {
    let state = app.state::<AppState>();

    let identity = store()
        .find_by_id(&id)
        .ok_or_else(|| "身份不存在".to_string())?;

    let app_config = AppConfig::load(&app).unwrap_or_default();
    let prompt_options = PromptOptions::from_config(&app_config);

    {
        let service = state.ai_service.lock().await;
        let gs = service.game_status.lock().await;
        ensure_identity_switchable(&gs)?;
    }

    store()
        .set_current_id(&identity.id)
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

/// 角色关系编辑器的一份数据：关系文本按 `ai:` / `me:` 键，初始好感度按 `me:<身份 id>` 键。
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
    let folder = role_folder(&state, role_id).await?;
    let mut relations = role_relations::load(&crate::api::data_dir(), &folder);
    let identity_rows = RoleRepo::load_identity_relations(&state.db, role_id)
        .await
        .map_err(|e| format!("读取身份关系失败: {e}"))?;
    let mut affections = HashMap::new();
    for (identity_id, value) in identity_rows {
        let key = format!("me:{identity_id}");
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
    let folder = role_folder(&state, role_id).await?;
    let data_dir = crate::api::data_dir();

    // 角色间关系写 relations.yml，角色对玩家身份的关系与初始好感度写数据库。
    let mut file_relations: HashMap<String, String> = HashMap::new();
    let mut identity_relations: HashMap<String, IdentityRelation> = HashMap::new();
    for (key, text) in relations {
        let key = key.trim();
        if key.is_empty() {
            continue;
        }
        match key.strip_prefix("me:") {
            Some(identity_id) => {
                identity_relations
                    .entry(identity_id.trim().to_string())
                    .or_default()
                    .text = text;
            },
            None => {
                file_relations.insert(key.to_string(), text);
            },
        }
    }
    for (key, value) in affections {
        let Some(identity_id) = key.trim().strip_prefix("me:") else {
            continue;
        };
        if identity_id.trim().is_empty() {
            continue;
        }
        identity_relations
            .entry(identity_id.trim().to_string())
            .or_default()
            .initial_affection = value.map(|v| v.clamp(-100, 100));
    }
    let file_relations = role_relations::sanitize(&file_relations);

    let service = state.ai_service.lock().await;
    let mut gs = service.game_status.lock().await;

    let before = runtime_relations::load_effective(&data_dir, &folder, &gs);
    let mut next = file_relations.clone();
    next.extend(
        identity_relations
            .iter()
            .map(|(identity_id, value)| {
                (format!("me:{identity_id}"), value.text.trim().to_string())
            })
            .filter(|(_, text)| !text.is_empty()),
    );
    let stored = RoleRepo::load_identity_relations(&state.db, role_id)
        .await
        .map_err(|e| format!("读取身份关系失败: {e}"))?;
    let relations_unchanged = role_relations::sanitize(&next) == before;
    let affections_unchanged = stored.iter().all(|(identity_id, row)| {
        identity_relations
            .get(identity_id)
            .map(|value| value.initial_affection)
            == Some(row.initial_affection)
    }) && identity_relations.iter().all(|(identity_id, value)| {
        stored
            .get(identity_id)
            .and_then(|row| row.initial_affection)
            == value.initial_affection
    });
    if relations_unchanged && affections_unchanged {
        return Ok(());
    }

    role_relations::save(&data_dir, &folder, &file_relations)
        .map_err(|e| format!("保存关系失败: {e}"))?;
    RoleRepo::replace_identity_relations(&state.db, role_id, &identity_relations)
        .await
        .map_err(|e| format!("保存身份关系失败: {e}"))?;
    let cache: HashMap<String, String> = identity_relations
        .iter()
        .map(|(identity_id, value)| (format!("me:{identity_id}"), value.text.trim().to_string()))
        .filter(|(_, text)| !text.is_empty())
        .collect();
    let affection_cache: HashMap<String, i32> = identity_relations
        .iter()
        .filter_map(|(identity_id, value)| {
            value
                .initial_affection
                .map(|affection| (identity_id.clone(), affection))
        })
        .collect();
    gs.role_manager
        .set_identity_relations(&folder, cache, affection_cache);

    // 改的是当前身份的初始好感度：把它同步进当前存档并广播，规则与角色设置保存一致。
    let current_identity = gs.player.identity_id.clone();
    if let Some(identity_id) = current_identity.as_deref() {
        let old_initial = gs.role_manager.get_loaded(role_id).and_then(|role| {
            crate::ai_service::affection::effective_initial_affection(
                &role.settings,
                stored
                    .get(identity_id)
                    .and_then(|row| row.initial_affection),
            )
        });
        let new_initial = gs.role_manager.get_loaded(role_id).and_then(|role| {
            crate::ai_service::affection::effective_initial_affection(
                &role.settings,
                identity_relations
                    .get(identity_id)
                    .and_then(|row| row.initial_affection),
            )
        });
        if let Some(init) = new_initial.filter(|value| Some(*value) != old_initial) {
            let payload = gs
                .sync_identity_affection_initial(&state.db, role_id, Some(identity_id), init)
                .await;
            let _ = app.emit("affection:changed", payload);
        }
    }

    if gs.role_manager.get_loaded(role_id).is_some() {
        let app_config = AppConfig::load(&app).unwrap_or_default();
        crate::ai_service::game_system::player_identity::persona::rebuild_and_refresh(
            &mut gs,
            &state.db,
            &data_dir,
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
    let identity_id = gs.player.identity_id.clone().unwrap_or_default();
    let data_dir = crate::api::data_dir();

    let mut role_ids = gs.onstage_role_ids.clone();
    if role_ids.is_empty() {
        role_ids.push(role_id);
    }
    let roles = relation_ready_all(&gs, &data_dir, &identity_id, &role_ids);
    let ready = roles.iter().all(|s| s.role_side && s.identity_side);
    Ok(RelationReadyStatus { ready, roles })
}

async fn current_identity() -> Result<PlayerIdentity, String> {
    let store = store();
    if let Some(current) = store.current() {
        return Ok(current);
    }
    if let Some(primary) = store.primary_identity() {
        return Ok(primary);
    }

    // 主身份迁移尚未跑过时的只读兜底：返回产品默认文案，不落盘。
    Ok(PlayerIdentity {
        id: String::new(),
        name: "玩家A（待填写）".to_string(),
        subtitle: "玩家A（待填写）".to_string(),
        prompt: String::new(),
        avatar: None,
        relations: HashMap::new(),
        source_folders: Vec::new(),
        is_primary: true,
    })
}

async fn role_folder(state: &AppState, role_id: i32) -> Result<String, String> {
    RoleRepo::get_role_by_id(&state.db, role_id)
        .await
        .map_err(|e| format!("查询角色失败: {e}"))?
        .and_then(|role| role.resource_folder)
        .ok_or_else(|| format!("角色 {role_id} 没有资源目录"))
}
