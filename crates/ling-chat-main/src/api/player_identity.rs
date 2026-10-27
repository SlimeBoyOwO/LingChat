use std::collections::HashMap;

use tauri::{AppHandle, Manager};
use tauri_plugin_store::StoreExt;

use crate::AppState;
use crate::ai_service::game_system::player_identity::{
    IdentityStore, PlayerIdentity, PlayerIdentitySummary, ensure_identity_switchable,
    role_relations, runtime_relations,
};
use crate::ai_service::game_system::status_ops::PromptLine;
use crate::ai_service::types::LineBase;
use crate::api::game::{WebInitData, build_web_init_data};
use crate::config::AppConfig;
use crate::db::managers::role_repo::RoleRepo;
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
    let current_id = store.current_id();
    let mut items: Vec<PlayerIdentitySummary> = store
        .list()
        .iter()
        .map(|identity| PlayerIdentitySummary::from_identity(identity, current_id.as_deref()))
        .collect();

    if items.is_empty() {
        if let Ok(current) = current_identity(&app).await {
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
pub async fn get_current_player_identity(app: AppHandle) -> Result<PlayerIdentity, String> {
    current_identity(&app).await
}

#[tauri::command]
pub async fn save_player_identity(
    app: AppHandle,
    identity: PlayerIdentity,
) -> Result<PlayerIdentity, String> {
    let saved = store()
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

    let removed = store()
        .delete(&id)
        .map_err(|e| format!("删除身份失败: {e}"))?;

    if !removed {
        return Err("身份不存在".to_string());
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
) -> Result<WebInitData, String> {
    let state = app.state::<AppState>();

    let identity = store()
        .find_by_id(&id)
        .ok_or_else(|| "身份不存在".to_string())?;

    let service = state.ai_service.lock().await;
    {
        let mut gs = service.game_status.lock().await;
        ensure_identity_switchable(&gs)?;

        let old_name = gs.player.user_name.clone();
        // 同身份短路：跳过切换（不重建、不加旁白），直接走绑定与界面刷新。
        if gs.player.identity_id.as_deref() != Some(identity.id.as_str()) {
            gs.player.user_name = identity.display_name().to_string();
            gs.player.user_subtitle = identity.subtitle.clone();
            gs.player.user_prompt = identity.prompt.clone();
            gs.player.identity_id = Some(identity.id.clone());
            gs.player.relations = identity.relations.clone();

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

            // 换身份旁白：与角色上下场同款（display_name=系统），双方名字都非空时才加。
            let new_name = identity.display_name().to_string();
            let has_old = !old_name.trim().is_empty();
            let has_new = !new_name.trim().is_empty();

            if has_old && has_new {
                gs.add_line(&state.db, narrator_line(format!("{}离开了对话", old_name)))
                    .await
                    .map_err(|e| format!("添加换身份旁白失败: {e}"))?;
                gs.add_line(&state.db, narrator_line(format!("{}加入了对话", new_name)))
                    .await
                    .map_err(|e| format!("添加换身份旁白失败: {e}"))?;
            }

            gs.persist_lines_if_bound(&state.db).await;
        }
    }

    // 记录全局当前身份 + 同步到当前存档（读档才能恢复到同一个身份）
    store()
        .set_current_id(&identity.id)
        .map_err(|e| format!("记录当前身份失败: {e}"))?;

    let active_save_id = service.game_status.lock().await.active_save_id;
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
    let result = build_web_init_data(&service, &app).await;
    drop(service);
    result
}

#[tauri::command]
pub async fn get_role_relations(
    app: AppHandle,
    role_id: i32,
) -> Result<HashMap<String, String>, String> {
    let state = app.state::<AppState>();
    let folder = role_folder(&state, role_id).await?;
    Ok(role_relations::load(&crate::api::data_dir(), &folder))
}

#[tauri::command]
pub async fn save_role_relations(
    app: AppHandle,
    role_id: i32,
    relations: HashMap<String, String>,
) -> Result<(), String> {
    let state = app.state::<AppState>();
    let folder = role_folder(&state, role_id).await?;
    let data_dir = crate::api::data_dir();

    let service = state.ai_service.lock().await;
    let mut gs = service.game_status.lock().await;

    let before = runtime_relations::load_effective(&data_dir, &folder, &gs);
    if role_relations::sanitize(&relations) == before {
        return Ok(());
    }

    role_relations::save(&data_dir, &folder, &relations)
        .map_err(|e| format!("保存关系失败: {e}"))?;

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

async fn current_identity(app: &AppHandle) -> Result<PlayerIdentity, String> {
    let state = app.state::<AppState>();
    let store = store();

    if let Some(existing) = store.current() {
        return Ok(existing);
    }

    let (name, subtitle) = {
        let service = state.ai_service.lock().await;
        let gs = service.game_status.lock().await;
        let settings = gs
            .main_role_id
            .and_then(|rid| gs.role_manager.get_loaded(rid))
            .map(|role| role.settings.clone());
        match settings {
            Some(s) => (
                s.user_name.clone(),
                s.user_subtitle.clone().unwrap_or_default(),
            ),
            None => ("用户".to_string(), String::new()),
        }
    };

    Ok(PlayerIdentity {
        id: String::new(),
        name,
        subtitle,
        prompt: String::new(),
        avatar: None,
        relations: HashMap::new(),
    })
}

async fn role_folder(state: &AppState, role_id: i32) -> Result<String, String> {
    RoleRepo::get_role_by_id(&state.db, role_id)
        .await
        .map_err(|e| format!("查询角色失败: {e}"))?
        .and_then(|role| role.resource_folder)
        .ok_or_else(|| format!("角色 {role_id} 没有资源目录"))
}
