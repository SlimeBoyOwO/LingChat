use tauri::{AppHandle, Emitter, Manager};

use crate::ai_service::message_system::generator::{GeneratorDeps, MessageGenerator};
use crate::config::AppConfig;
use crate::AppState;

#[tauri::command]
pub async fn send_chat_message(app: AppHandle, text: String) -> Result<(), String> {
    let text = text.trim().to_string();
    if text.is_empty() {
        return Err("消息内容不能为空".to_string());
    }

    // --- 调试指令处理 ---
    if text.starts_with('/') {
        return handle_debug_command(&app, &text).await;
    }

    let state = app.state::<AppState>();

    let llm = state
        .chat
        .llm
        .clone()
        .ok_or_else(|| "LLM 未配置，请在设置中配置 API Key 和模型".to_string())?;

    let concurrency = AppConfig::load(&app)
        .map(|c| c.consumers as usize)
        .unwrap_or(1)
        .max(1);

    let game_status = {
        let svc = state.ai_service.lock().await;
        svc.game_status.clone()
    };
    let deps = GeneratorDeps {
        app: app.clone(),
        db: state.db.clone(),
        game_status,
        processor: state.chat.processor.clone(),
        translator: state.chat.translator.clone(),
        llm,
        concurrency,
    };

    // Notify proactive system of user input
    if let Some(proactive) = &state.proactive_system {
        let proactive_clone = proactive.clone();
        tokio::spawn(async move {
            let mut sys = proactive_clone.lock().await;
            sys.on_user_message_received().await;
        });
    }

    // 成就触发检查
    let achievement_manager = state.achievement_manager.clone();
    let app_handle = app.clone();
    let trigger_text = text.clone();
    tokio::spawn(async move {
        let mut mgr = achievement_manager.lock().await;
        let unlocks =
            crate::achievements::triggers::AchievementTriggerHandler::handle_user_message(
                &trigger_text,
                &mut mgr,
            );
        for achievement in unlocks {
            if let Err(e) = app_handle.emit("achievement:unlocked", &achievement) {
                tracing::error!("发送成就事件失败: {}", e);
            }
        }
    });

    // 冒险解锁检查
    let adventure_db = state.db.clone();
    let adventure_ai_service = state.ai_service.clone();
    let adventure_ach_mgr = state.achievement_manager.clone();
    let adventure_app = app.clone();
    tokio::spawn(async move {
        let newly_unlocked = {
            let service = adventure_ai_service.lock().await;
            let adventures: Vec<&crate::ai_service::types::ScriptStatus> = service
                .script_manager
                .get_all_adventures()
                .into_iter()
                .collect();
            let gs = service.game_status.lock().await;
            let ach_mgr = adventure_ach_mgr.lock().await;
            crate::adventures::trigger::check_all_adventures(
                &adventure_db,
                &ach_mgr,
                &gs,
                &adventures,
            )
            .unwrap_or_default()
        };
        for info in &newly_unlocked {
            let _ = adventure_app.emit("adventure:unlocked", info);
        }
    });

    let generator = MessageGenerator::new(deps);
    let gen_lock = state.generation_lock.clone();

    tokio::spawn(async move {
        let _lock = gen_lock.lock().await;
        match generator.process_message(Some(text)).await {
            Ok(acc) => tracing::info!("消息生成完成，长度: {}", acc.len()),
            Err(e) => tracing::error!("消息生成失败: {:#}", e),
        }
    });

    Ok(())
}

/// 处理以 "/" 开头的调试指令（仅在后端日志输出，不发往前端）。
async fn handle_debug_command(app: &AppHandle, text: &str) -> Result<(), String> {
    match text {
        "/查看记忆" => {
            let state = app.state::<AppState>();
            let svc = state.ai_service.lock().await;
            let gs = svc.game_status.lock().await;

            let current_id = match gs.current_role_id {
                Some(id) => id,
                None => {
                    tracing::warn!("没有当前绑定的角色。");
                    return Ok(());
                }
            };

            let role = match gs.role_manager.get_loaded(current_id) {
                Some(r) => r,
                None => {
                    tracing::warn!("角色 ID {} 未加载。", current_id);
                    return Ok(());
                }
            };

            tracing::info!("=== 角色记忆 [{}] (role_id={}) ===",
                role.display_name.as_deref().unwrap_or("未知"), current_id);

            for msg in &role.memory {
                tracing::info!("[{}] {}", msg.role, msg.content);
            }
        }
        "/查看台词" => {
            let state = app.state::<AppState>();
            let svc = state.ai_service.lock().await;
            let gs = svc.game_status.lock().await;

            tracing::info!("=== 台词列表（共 {} 条）===", gs.line_list.len());

            for (i, line) in gs.line_list.iter().enumerate() {
                let name = line.base.display_name.as_deref().unwrap_or("未知");
                let emotion = line
                    .base
                    .original_emotion
                    .as_deref()
                    .filter(|v| !v.is_empty())
                    .map(|v| format!("【{}】", v))
                    .unwrap_or_default();
                let content = &line.base.content;
                let tts = line
                    .base
                    .tts_content
                    .as_deref()
                    .filter(|v| !v.is_empty())
                    .map(|v| format!("<{}>", v))
                    .unwrap_or_default();
                let action = line
                    .base
                    .action_content
                    .as_deref()
                    .filter(|v| !v.is_empty())
                    .map(|v| format!("（{}）", v))
                    .unwrap_or_default();

                tracing::info!("[{}] {} : {}{}{}{}", i, name, emotion, content, tts, action);
            }
        }
        other if other.starts_with('/') => {
            tracing::warn!("未知调试指令: {}。可用指令: /查看记忆, /查看台词", other);
        }
        _ => unreachable!(),
    }
    Ok(())
}
