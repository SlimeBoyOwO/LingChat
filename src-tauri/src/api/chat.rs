use tauri::{AppHandle, Emitter, Manager};

use crate::ai_service::message_system::events;
use crate::ai_service::message_system::generator::{GeneratorDeps, MessageGenerator};
use crate::ai_service::types::{LineAttributeExt, LineBase};
use crate::api::game::{compute_user_message_seqs, GameLineInit};
use crate::config::AppConfig;
use crate::db::entities::line::LineAttribute;
use crate::db::managers::save_repo::SaveRepo;
use crate::utils::prompt::PromptRole;
use crate::AppState;

#[tauri::command]
pub async fn send_chat_message(
    app: AppHandle,
    text: String,
    screenshot_base64: Option<String>,
) -> Result<(), String> {
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

    let user_name = game_status.lock().await.player.user_name.clone();

    // 发送思考事件
    events::emit_thinking(&app, true);

    // 截图分析：在创建 GeneratorDeps 之前，确保旁白台词已写入 line_list
    if let Some(ref b64) = screenshot_base64 {
        if let Ok(image_bytes) = base64::Engine::decode(&base64::prelude::BASE64_STANDARD, b64) {
            let prompt = format!(
                "你是一个图像信息转述者，你将饰演旁白这一角色输出台词，用第三人称叙述把你看到的画面描述给其他AI让他理解用户的图片内容。用户（名字是\"{}\"）的信息是：\"{}\"\n\n以上是用户发的消息，请切合用户实际获取信息的需要，获取画面中的重点内容，用200字描述主体部分即可。如果你看到一个聊天窗口，有角色的立绘和对话框，不要描述这部分，只描述桌面上的其他内容。因为那部分是玩家与AI的聊天窗口。但如果用户信息中明确提到了AI的立绘，背景等（比如用户消息说“看看你的周围，这是哪里呀？”）的时候，你可以描述AI的立绘或背景来告诉主AI的环境感知能力。",
                user_name, text
            );

            let analysis = {
                let mut sa = state.screen_analyzer.lock().await;
                sa.analyze_image(&image_bytes, &prompt).await
            };

            if let Some(narration) = analysis {
                let mut gs = game_status.lock().await;
                gs.add_line(
                    &state.db,
                    LineBase {
                        content: PromptRole::Narrator.build_prompt(&narration),
                        attribute: LineAttributeExt(LineAttribute::User),
                        display_name: Some("旁白".to_string()),
                        ..Default::default()
                    },
                )
                .await
                .map_err(|e| format!("添加旁白台词失败: {}", e))?;
                tracing::info!("[Chat] Screenshot analysis narration added to game_status.");
            }
        }
    }

    let deps = GeneratorDeps {
        app: app.clone(),
        db: state.db.clone(),
        game_status,
        processor: state.chat.processor.clone(),
        translator: state.chat.translator.clone(),
        llm,
        concurrency,
        god_agent: state.god_agent.clone(),
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
        let unlocks = crate::achievements::triggers::AchievementTriggerHandler::handle_user_message(
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

            tracing::info!(
                "=== 角色记忆 [{}] (role_id={}) ===",
                role.display_name.as_deref().unwrap_or("未知"),
                current_id
            );

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

/// 回溯对话：将台词列表截断到指定玩家消息之前（移除该消息及之后所有内容）。
///
/// `message_seq` 为 1-indexed 的玩家消息序号（由 `sender_role_id == Some(0)` 标识）。
#[tauri::command]
pub async fn rollback_conversation(
    app: AppHandle,
    message_seq: u32,
) -> Result<Vec<GameLineInit>, String> {
    let state = app.state::<AppState>();
    let db = state.db.clone();

    // 串行化：等待正在进行的消息生成完成再截断
    let gen_lock = state.generation_lock.clone();
    let _lock = gen_lock.lock().await;

    let remaining = {
        let svc = state.ai_service.lock().await;
        let mut gs = svc.game_status.lock().await;

        // 按序号定位第 N 条玩家消息（1-indexed）
        let mut count = 0u32;
        let idx = gs
            .line_list
            .iter()
            .position(|line| {
                if line.base.sender_role_id == Some(0)
                    && matches!(line.attribute(), LineAttribute::User)
                {
                    count += 1;
                    count == message_seq
                } else {
                    false
                }
            })
            .ok_or_else(|| format!("未找到序号为 {} 的用户消息", message_seq))?;

        // truncate(idx) 移除 idx..len（含目标消息及之后所有内容）
        gs.line_list.truncate(idx);
        gs.refresh_memories(&db)
            .await
            .map_err(|e| format!("刷新记忆失败: {}", e))?;

        // 若存在活跃存档，同步截断到 DB
        if let Some(save_id) = gs.active_save_id {
            SaveRepo::sync_lines(&db, save_id, &gs.line_list)
                .await
                .map_err(|e| format!("同步存档失败: {}", e))?;
        }

        gs.line_list.clone()
    }; // 释放锁

    // 转换为前端格式（带序号）
    let seqs = compute_user_message_seqs(&remaining);
    let init_lines: Vec<GameLineInit> = remaining
        .iter()
        .zip(seqs.iter())
        .map(|(gl, &seq)| GameLineInit {
            content: gl.base.content.clone(),
            attribute: gl.base.attribute.as_str().to_string(),
            sender_role_id: gl.base.sender_role_id,
            display_name: gl.base.display_name.clone(),
            original_emotion: gl.base.original_emotion.clone(),
            predicted_emotion: gl.base.predicted_emotion.clone(),
            action_content: gl.base.action_content.clone(),
            audio_file: gl.base.audio_file.clone(),
            perceived_role_ids: gl.perceived_role_ids.clone(),
            user_message_seq: seq,
        })
        .collect();

    tracing::info!(
        "回溯对话完成: message_seq={}, 剩余台词 {} 条",
        message_seq,
        init_lines.len()
    );

    Ok(init_lines)
}
