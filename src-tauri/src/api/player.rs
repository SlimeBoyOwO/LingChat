use tauri::{AppHandle, Emitter, Manager};

use crate::AppState;
use crate::ai_service::game_system::player_profile_sync::{
    rebuild_system_lines, sync_active_persona_to_runtime,
};
use crate::db::managers::player_profile_repo::{PlayerProfileData, PlayerProfileRepo};
use crate::init::static_copy::get_data_dir;
use crate::utils::prompt::{sys_prompt_builder_by_settings, PromptOptions};

/// 展开完整错误链用于提示/日志。
///
/// anyhow 的 Display 只显示最外层 context（如「创建人设卡失败」），底层 DbErr
/// 不会透出，现场排查时看不到真实原因（如 UNIQUE/NOT NULL/SQLITE_BUSY）。
/// 这里把整条链用 `: ` 拼接，让用户看到的 toast 与日志都能定位根因。
fn err_detail(e: &anyhow::Error) -> String {
    e.chain()
        .map(|cause| cause.to_string())
        .collect::<Vec<_>>()
        .join(": ")
}

/// 是否包含不允许的控制字符：仅放行 \n 与 \t，其余 C0 控制字符拒绝。
fn contains_forbidden_control(value: &str) -> bool {
    value
        .chars()
        .any(|c| (c as u32) < 0x20 && c != '\n' && c != '\t')
}

/// 校验玩家昵称：trim 后非空、长度 1~32、无禁控字符，返回归一化昵称。
fn validate_user_name(raw: String) -> Result<String, String> {
    let user_name = raw.trim().to_string();
    if contains_forbidden_control(&user_name) {
        return Err("玩家昵称不能包含控制字符".to_string());
    }
    let name_chars = user_name.chars().count();
    if name_chars == 0 || name_chars > 32 {
        return Err("玩家昵称不能为空，且长度需为 1~32 个字符".to_string());
    }
    Ok(user_name)
}

/// 校验可选文本字段：trim 首尾后返回归一化值；检查控制字符与字符数上限。
fn validate_optional_text(
    label: &str,
    value: Option<String>,
    max_chars: usize,
) -> Result<Option<String>, String> {
    let Some(value) = value else {
        return Ok(None);
    };
    let trimmed = value.trim().to_string();
    if contains_forbidden_control(&trimmed) {
        return Err(format!("{}不能包含除换行和制表符以外的控制字符", label));
    }
    if trimmed.chars().count() > max_chars {
        return Err(format!("{}长度不能超过 {} 个字符", label, max_chars));
    }
    Ok(Some(trimmed))
}

/// 读取全局玩家档案（当前激活人设，纯 DB 存储）。
#[tauri::command]
pub async fn get_player_profile(app: AppHandle) -> Result<serde_json::Value, String> {
    let state = app.state::<AppState>();
    let profile = PlayerProfileRepo::get_profile(&state.db)
        .await
        .map_err(|e| format!("读取玩家档案失败: {}", err_detail(&e)))?;
    Ok(profile_to_json(&profile))
}

/// 读取指定人设卡（供前端编辑非激活人设卡时展示内容）。
#[tauri::command]
pub async fn get_player_persona(
    app: AppHandle,
    card_id: String,
) -> Result<serde_json::Value, String> {
    let state = app.state::<AppState>();
    let card_id = card_id.trim().to_string();
    let persona = PlayerProfileRepo::get_persona(&state.db, &card_id)
        .await
        .map_err(|e| format!("读取人设卡失败: {}", err_detail(&e)))?;
    match persona {
        Some(profile) => Ok(profile_to_json(&profile)),
        None => Err(format!("人设卡不存在: {card_id}")),
    }
}

/// 把 `PlayerProfileData` 序列化成前端 `PlayerProfile` 结构（空字段给空串）。
fn profile_to_json(profile: &PlayerProfileData) -> serde_json::Value {
    serde_json::json!({
        "user_name": profile.user_name,
        "user_subtitle": profile.user_subtitle.clone().unwrap_or_default(),
        "user_prompt": profile.user_prompt.clone().unwrap_or_default(),
        "info": profile.info.clone().unwrap_or_default(),
        "system_prompt_example": profile.system_prompt_example.clone().unwrap_or_default(),
    })
}

/// 归一化 + 校验一组玩家档案字段（昵称单独走 [`validate_user_name`]）。
fn validate_profile_fields(
    user_name: String,
    user_subtitle: Option<String>,
    user_prompt: Option<String>,
    info: Option<String>,
    system_prompt_example: Option<String>,
) -> Result<PlayerProfileData, String> {
    let user_name = validate_user_name(user_name)?;
    let user_subtitle = validate_optional_text("玩家副标题", user_subtitle, 64)?;
    let user_prompt = validate_optional_text("玩家设定", user_prompt, 4000)?;
    let info = validate_optional_text("玩家简介", info, 4000)?;
    let system_prompt_example = validate_optional_text("玩家说话风格示例", system_prompt_example, 4000)?;
    Ok(PlayerProfileData {
        user_name,
        user_subtitle,
        user_prompt,
        info,
        system_prompt_example,
        ..Default::default()
    })
}

/// 保存全局玩家档案。
///
/// 解耦玩家与 AI：纯 DB 写入**当前激活人设卡**，同时**同步运行时**
/// （`GameStatus.player` 与 AI 系统提示词），这样玩家改名/改设定后 LLM 立即感知，
/// 不会继续用旧的默认"玩家"。
#[tauri::command]
pub async fn set_player_profile(
    app: AppHandle,
    user_name: String,
    user_subtitle: Option<String>,
    user_prompt: Option<String>,
    info: Option<String>,
    system_prompt_example: Option<String>,
) -> Result<serde_json::Value, String> {
    let state = app.state::<AppState>();
    let profile = validate_profile_fields(
        user_name,
        user_subtitle,
        user_prompt,
        info,
        system_prompt_example,
    )?;

    // 1. 持久化到 DB（player_profile 表，当前激活人设卡）
    PlayerProfileRepo::save_profile(&state.db, &profile)
        .await
        .map_err(|e| format!("保存玩家档案失败: {}", err_detail(&e)))?;
    let active_card_id = PlayerProfileRepo::active_persona_id(&state.db)
        .await
        .map_err(|e| format!("读取激活人设失败: {}", err_detail(&e)))?;

    // 2. 同步运行时 + 重建系统提示词（让 LLM 立即感知新名字/玩家设定）
    let app_config = crate::config::AppConfig::load(&app).unwrap_or_default();
    let prompt_options = PromptOptions {
        output_sec_lang: app_config.llm_output_sec_lang,
        no_emotion_limit: app_config.no_emotion_limit_prompt,
    };

    let player_prompt = profile.to_prompt_fragment();

    {
        // 只加一次 AI 服务锁，在其内部依次锁 GameStatus 并完成全部状态修改，
        // 避免持有不同层级的锁跨 await 时出现顺序死锁。
        let mut svc = state.ai_service.lock().await;

        {
            let mut gs = svc.game_status.lock().await;
            gs.player.card_id = active_card_id;
            gs.player.user_name = profile.user_name.clone();
            gs.player.user_subtitle = profile.user_subtitle.clone().unwrap_or_default();
            gs.player.user_prompt = player_prompt.clone();

            // 改名/改设定热更新：按新玩家档案整体重建所有 System 人设行，
            // 而不是用裸字符串替换（会误伤短名，如「你」「A」）。
            // 档案已经落库，这里失败只降级告警：命令仍返回成功，重启/重新载入
            // 后会生效；若返回失败，前端会回滚本地表单，反而与 DB 新值不一致。
            if let Err(e) = rebuild_system_lines(&state.db, &svc.data_dir, &mut gs, prompt_options)
                .await
            {
                tracing::error!("重建 System 人设行失败，玩家档案已保存，重启或载入后生效: {e}");
            } else {
                gs.role_manager.invalidate_memory_history();
                if let Err(e) = gs.refresh_memories(&state.db).await {
                    tracing::error!("刷新角色记忆失败，玩家档案已保存，重启或载入后生效: {e}");
                }
            }
        }

        // 用新玩家名 + 玩家设定重建 AI 服务自身的系统提示词快照。
        svc.user_name = profile.user_name.clone();
        svc.user_subtitle = profile.user_subtitle.clone();
        svc.player_prompt = player_prompt.clone();

        if let Some(settings) = svc.settings.clone() {
            svc.ai_prompt = sys_prompt_builder_by_settings(
                &settings,
                Some(&profile.user_name),
                prompt_options,
                &player_prompt,
            );
        }
    }

    // 锁释放后再广播事件，避免事件回调（其他窗口）等待后端锁造成串行等待。
    // 多窗口同步：主窗口与设置窗口都会收到此事件并刷新本地玩家档案展示。
    let _ = app.emit(
        "player-profile-updated",
        serde_json::json!({
            "user_name": profile.user_name,
            "user_subtitle": profile.user_subtitle.unwrap_or_default(),
            "user_prompt": profile.user_prompt.unwrap_or_default(),
            "info": profile.info.unwrap_or_default(),
            "system_prompt_example": profile.system_prompt_example.unwrap_or_default(),
        }),
    );

    Ok(serde_json::json!({"success": true}))
}

/// 保存到**指定**人设卡（非激活卡的内容编辑用；不触碰激活位与运行时）。
#[tauri::command]
pub async fn set_player_persona(
    app: AppHandle,
    card_id: String,
    user_name: String,
    user_subtitle: Option<String>,
    user_prompt: Option<String>,
    info: Option<String>,
    system_prompt_example: Option<String>,
) -> Result<serde_json::Value, String> {
    let state = app.state::<AppState>();
    let card_id = card_id.trim().to_string();
    let profile = validate_profile_fields(
        user_name,
        user_subtitle,
        user_prompt,
        info,
        system_prompt_example,
    )?;
    PlayerProfileRepo::save_persona(&state.db, &card_id, &profile)
        .await
        .map_err(|e| format!("保存人设卡失败: {}", err_detail(&e)))?;

    // 人设列表可能跨窗口展示，轻量事件提示其它窗口刷新。
    let _ = app.emit("player-profile-updated", serde_json::json!({}));
    Ok(serde_json::json!({"success": true}))
}

/// 列出所有玩家人设卡（含当前激活人设）。
#[tauri::command]
pub async fn get_player_profiles(app: AppHandle) -> Result<serde_json::Value, String> {
    let state = app.state::<AppState>();
    // 先确保默认人设存在（空表播种），保证 active_profile_id 恒有值。
    PlayerProfileRepo::ensure_profile(&state.db, None)
        .await
        .map_err(|e| format!("初始化玩家档案失败: {}", err_detail(&e)))?;
    let personas = PlayerProfileRepo::list_personas(&state.db)
        .await
        .map_err(|e| format!("列出玩家档案失败: {}", err_detail(&e)))?;
    let active = PlayerProfileRepo::active_persona_id(&state.db)
        .await
        .map_err(|e| format!("读取激活人设失败: {}", err_detail(&e)))?;
    Ok(serde_json::json!({
        "profiles": personas,
        "active_profile_id": active,
    }))
}

/// 人设已激活后同步运行时并广播完整档案（切换与新建共用）。
///
/// 广播完整档案，前端据此更新游戏展示字段（玩家名/副标题）与档案表单。
/// 前端监听器只在事件带 user_name 时才整体覆盖 gameStore/userStore；若这里读不到
/// 新档案（失败），只带 active_profile_id 也能让前端至少更新激活人设 id，而不
/// 会把玩家名/副标题重置成默认「玩家」。
async fn sync_and_broadcast_active(app: &AppHandle, card_id: &str) -> Result<(), String> {
    let state = app.state::<AppState>();

    // 同步运行时（GameStatus.player + AIService），让 LLM 立即感知新身份；
    // 失败只降级告警（人设已切换，重启或载入后生效）。
    if let Err(e) = sync_active_persona_to_runtime(app, &state.db, get_data_dir()).await {
        tracing::error!("激活人设后同步运行时失败（人设已切换，重启或载入后生效）: {e}");
    }

    let profile = match PlayerProfileRepo::get_profile(&state.db).await {
        Ok(p) => p,
        Err(e) => {
            tracing::warn!("激活人设后读取新档案失败，仅广播 active_profile_id: {e}");
            let _ = app.emit("player-profile-updated", serde_json::json!({
                "active_profile_id": card_id,
            }));
            return Ok(());
        }
    };
    let mut payload = profile_to_json(&profile).as_object().cloned().unwrap_or_default();
    payload.insert(
        "active_profile_id".to_string(),
        serde_json::Value::String(card_id.to_string()),
    );
    let _ = app.emit("player-profile-updated", serde_json::Value::Object(payload));
    Ok(())
}

/// 切换当前激活人设。
#[tauri::command]
pub async fn set_active_player_card(
    app: AppHandle,
    card_id: String,
) -> Result<serde_json::Value, String> {
    let state = app.state::<AppState>();
    let card_id = card_id.trim().to_string();
    PlayerProfileRepo::set_active_persona(&state.db, &card_id)
        .await
        .map_err(|e| format!("切换玩家档案失败: {}", err_detail(&e)))?;
    sync_and_broadcast_active(&app, &card_id).await?;
    Ok(serde_json::json!({"success": true}))
}

/// 新建一张玩家人设卡并**直接激活**（单事务内完成，前端无需二次切换）。
#[tauri::command]
pub async fn create_player_card(
    app: AppHandle,
    card_id: String,
    user_name: String,
    user_subtitle: Option<String>,
    user_prompt: Option<String>,
    info: Option<String>,
    system_prompt_example: Option<String>,
) -> Result<serde_json::Value, String> {
    let state = app.state::<AppState>();
    let card_id = card_id.trim().to_string();
    if card_id.is_empty() {
        return Err("人设卡 id 不能为空".to_string());
    }
    let profile = validate_profile_fields(
        user_name,
        user_subtitle,
        user_prompt,
        info,
        system_prompt_example,
    )?;

    PlayerProfileRepo::create_persona_active(&state.db, &card_id, &profile)
        .await
        .map_err(|e| format!("创建玩家档案失败: {}", err_detail(&e)))?;
    sync_and_broadcast_active(&app, &card_id).await?;
    Ok(serde_json::json!({"success": true}))
}

/// 删除一张玩家人设卡。禁止删除当前激活人设。
#[tauri::command]
pub async fn delete_player_card(
    app: AppHandle,
    card_id: String,
) -> Result<serde_json::Value, String> {
    let state = app.state::<AppState>();
    let card_id = card_id.trim().to_string();
    PlayerProfileRepo::delete_persona(&state.db, &card_id)
        .await
        .map_err(|e| format!("删除玩家档案失败: {}", err_detail(&e)))?;

    let _ = app.emit("player-profile-updated", serde_json::json!({}));
    Ok(serde_json::json!({"success": true}))
}
