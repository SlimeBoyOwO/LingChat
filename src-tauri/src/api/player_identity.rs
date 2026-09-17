//! 「我的身份」相关命令。
//!
//! 所有命令都是**纯新增**：不改动任何既有命令的签名与语义，
//! 旧版本前端/旧版本程序都感知不到这些命令的存在。

use std::collections::HashMap;

use tauri::{AppHandle, Manager};

use crate::AppState;
use crate::ai_service::game_system::game_status::GameStatus;
use crate::ai_service::game_system::player_identity::{
    IdentityStore, PlayerIdentity, PlayerIdentitySummary, RelationEndpoint, build_player_block,
    ensure_identity_mutable, resolve_relation, role_relations,
};
use crate::ai_service::types::{LineAttributeExt, LineBase};
use crate::api::game::{WebInitData, build_web_init_data};
use crate::db::entities::line::LineAttribute;
use crate::db::managers::role_repo::RoleRepo;
use crate::db::managers::save_repo::SaveRepo;

fn store() -> IdentityStore {
    IdentityStore::new(&crate::api::data_dir())
}

/// 列出全部身份卡（含「当前使用」标记）。
#[tauri::command]
pub async fn list_player_identities(app: AppHandle) -> Result<Vec<PlayerIdentitySummary>, String> {
    let store = store();
    let current_id = store.current_id();
    let mut items: Vec<PlayerIdentitySummary> = store
        .list()
        .iter()
        .map(|identity| PlayerIdentitySummary::from_identity(identity, current_id.as_deref()))
        .collect();

    // 从未选择过身份时，界面上也应该能看到一张卡（否则用户会以为功能没生效）。
    // 这里只做展示，不落盘——落盘发生在载入游戏时（`current_or_synthesize`）。
    if items.is_empty() {
        if let Ok(current) = current_identity(&app).await {
            items.push(PlayerIdentitySummary::from_identity(&current, Some(&current.id)));
        }
    }

    Ok(items)
}

/// 读取单张身份卡（编辑页用）。
#[tauri::command]
pub async fn get_player_identity(
    _app: AppHandle,
    id: String,
) -> Result<Option<PlayerIdentity>, String> {
    Ok(store().find_by_id(&id))
}

/// 读取当前使用的身份（不存在的场合返回合成的默认卡，不落盘）。
#[tauri::command]
pub async fn get_current_player_identity(app: AppHandle) -> Result<PlayerIdentity, String> {
    current_identity(&app).await
}

/// 新建或更新一张身份卡。
///
/// `identity.id` 为空视为新建（后端生成不可变 id）。
#[tauri::command]
pub async fn save_player_identity(
    _app: AppHandle,
    identity: PlayerIdentity,
) -> Result<PlayerIdentity, String> {
    store().save(&identity).map_err(|e| format!("保存身份失败: {e}"))
}

/// 删除身份卡（软删除，移入回收站目录）。
///
/// 被任何存档引用时拒绝删除：否则老存档会解析不到身份、回退成合成默认卡，
/// 用户会以为身份丢了。
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
    Ok(())
}

/// 切换当前使用的身份，并返回刷新后的初始化数据。
///
/// 闸门：剧本运行中拒绝（见 `player_identity::guard`）。
/// 切换是**玩家侧操作**，不产生世界台词；但会补一条系统提示行，
/// 让当前 AI 之后按新身份与关系称呼——否则名字变了而模型仍按旧名字叫。
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
        ensure_identity_mutable(&gs)?;

        gs.player.user_name = identity.display_name().to_string();
        gs.player.user_subtitle = identity.subtitle.clone();
        gs.player.user_prompt = identity.prompt.clone();
        gs.player.identity_id = Some(identity.id.clone());
        gs.player.relations = identity.relations.clone();

        inject_identity_refresh_line(&state.db, &mut gs, &identity).await;
    }

    // 记录全局当前身份 + 同步到当前存档（这样读档能恢复到同一个身份）
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

/// 读取某个 AI 角色的关系表。
#[tauri::command]
pub async fn get_role_relations(
    app: AppHandle,
    role_id: i32,
) -> Result<HashMap<String, String>, String> {
    let state = app.state::<AppState>();
    let folder = role_folder(&state, role_id).await?;
    Ok(role_relations::load(&crate::api::data_dir(), &folder))
}

/// 写入某个 AI 角色的关系表。
#[tauri::command]
pub async fn save_role_relations(
    app: AppHandle,
    role_id: i32,
    relations: HashMap<String, String>,
) -> Result<(), String> {
    let state = app.state::<AppState>();
    let folder = role_folder(&state, role_id).await?;
    role_relations::save(&crate::api::data_dir(), &folder, &relations)
        .map_err(|e| format!("保存关系失败: {e}"))
}

// ============ 内部辅助 ============

async fn current_identity(app: &AppHandle) -> Result<PlayerIdentity, String> {
    let state = app.state::<AppState>();
    let store = store();

    if let Some(existing) = store.current() {
        return Ok(existing);
    }

    // 没有身份卡时：用当前主 AI 角色卡上的「对玩家的称呼」合成一张（不落盘）。
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
        synthetic: true,
    })
}

async fn role_folder(state: &AppState, role_id: i32) -> Result<String, String> {
    RoleRepo::get_role_by_id(&state.db, role_id)
        .await
        .map_err(|e| format!("查询角色失败: {e}"))?
        .and_then(|role| role.resource_folder)
        .ok_or_else(|| format!("角色 {role_id} 没有资源目录"))
}

/// 追加一条系统提示行，让当前 AI 之后的回复按新的身份与关系来称呼。
///
/// 身份切换本身是玩家侧操作（世界里的其他人并没有看见什么），
/// 所以注入的是 `System` 属性行而不是旁白——它表达「你感知到/被告知的信息」，
/// 与场景切换旁白走同一套机制。
///
/// 之所以必须补这一行：人设是以**台词行**的形式留在对话历史里的
/// （见 `service.rs` 初始化时的 System 行）。只改运行时状态而不补行，
/// 模型在后续轮次里仍会按旧名字称呼玩家。
async fn inject_identity_refresh_line(
    db: &sea_orm::DatabaseConnection,
    gs: &mut GameStatus,
    identity: &PlayerIdentity,
) {
    let Some(main_role_id) = gs.main_role_id else {
        return;
    };

    let (folder, ai_name) = {
        let Some(role) = gs.role_manager.get_loaded(main_role_id) else {
            return;
        };
        (
            role.settings.character_folder.clone(),
            role.settings.ai_name.clone(),
        )
    };

    let data_dir = crate::api::data_dir();
    let speaker = RelationEndpoint::Ai(folder.clone());
    let target = RelationEndpoint::Me(identity.id.clone());
    let speaker_relations = role_relations::load(&data_dir, &folder);
    let relation = resolve_relation(
        &speaker,
        &target,
        &speaker_relations,
        &identity.relations,
        Some(identity.prompt.as_str()),
    );
    let block = build_player_block(&identity.name, &identity.subtitle, &identity.prompt, relation.as_ref());
    if block.trim().is_empty() {
        return;
    }

    let line = LineBase {
        content: block.trim().to_string(),
        attribute: LineAttributeExt(LineAttribute::System),
        sender_role_id: Some(main_role_id),
        display_name: Some(ai_name),
        ..Default::default()
    };

    if let Err(e) = gs.add_line(db, line).await {
        tracing::warn!("注入身份提示行失败: {e}");
    }
}
