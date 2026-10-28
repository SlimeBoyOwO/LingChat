use std::sync::OnceLock;

use regex::Regex;
use tauri::{AppHandle, Emitter, Manager};

use crate::AppState;
use crate::ai_service::game_system::game_status::GameStatus;
use crate::ai_service::game_system::player_identity::relation_ready_all;
use crate::ai_service::game_system::status_ops::PromptLine;
use crate::ai_service::message_system::events;
use crate::ai_service::message_system::generator::{
    GeneratorDeps, GeneratorSource, MessageGenerator,
};
use crate::ai_service::message_system::processor::EmotionSegment;
use crate::ai_service::screen_analyzer::{ImageDelivery, ImageIntent, deliver_image};
use crate::ai_service::tts::local::LocalTtsState;
use crate::api::game::{GameLineInit, compute_user_message_seqs};
use crate::config::AppConfig;
use crate::db::entities::line::LineAttribute;
use crate::db::managers::save_repo::SaveRepo;
use crate::utils::prompt::{PromptRole, REPLY_LENGTH_VAR_KEY, ReplyLengthMode};

/// 发送玩家消息，进入正常对话生成流程。
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

    // 关系未就绪（角色侧或身份侧的关系栏为空）时拒绝玩家发言，前端据错误码弹引导
    if !relation_ready_now(&app).await {
        return Err("RELATION_NOT_READY".to_string());
    }

    dispatch_chat_turn(app, Some(text), screenshot_base64, None).await
}

/// 当前阵容所有在场角色与当前身份的关系是否已就绪；尚未进入对话时不拦。
async fn relation_ready_now(app: &AppHandle) -> bool {
    let state = app.state::<AppState>();
    let svc = state.ai_service.lock().await;
    let gs = svc.game_status.lock().await;
    let role_ids = gs.onstage_role_ids.clone();
    if role_ids.is_empty() {
        return true;
    }
    let identity_id = gs.player.identity_id.clone().unwrap_or_default();
    let statuses = relation_ready_all(&gs, &crate::api::data_dir(), &identity_id, &role_ids);
    statuses.iter().all(|s| s.role_side && s.identity_side)
}

#[tauri::command]
pub async fn send_system_message(
    app: AppHandle,
    text: String,
    screenshot_base64: Option<String>,
) -> Result<(), String> {
    let text = text.trim().to_string();
    if text.is_empty() {
        return Err("消息内容不能为空".to_string());
    }

    dispatch_chat_turn(app, None, screenshot_base64, Some(text)).await
}

fn reply_length_adjustable(gs: &GameStatus) -> bool {
    gs.script_status.is_none() && gs.current_role_id.is_some() && !gs.line_list.is_empty()
}

#[tauri::command]
pub async fn get_reply_length_mode(app: AppHandle) -> Result<Option<String>, String> {
    let state = app.state::<AppState>();
    // 试玩会话整个 GameStatus 都会在结束时还原，此时读取的档位没有意义
    if state.pending_preview_restore.lock().await.is_some() {
        return Ok(None);
    }
    let svc = state.ai_service.lock().await;
    let gs = svc.game_status.lock().await;
    if !reply_length_adjustable(&gs) {
        return Ok(None);
    }
    Ok(Some(gs.reply_length_mode().as_str().to_string()))
}

#[tauri::command]
pub async fn set_reply_length_mode(app: AppHandle, mode: String) -> Result<String, String> {
    let new_mode = match mode.trim().to_ascii_lowercase().as_str() {
        "short" => ReplyLengthMode::Short,
        "normal" => ReplyLengthMode::Normal,
        "free" => ReplyLengthMode::Free,
        other => return Err(format!("未知的回复台词数量档位：{other}")),
    };

    let state = app.state::<AppState>();
    // 试玩会话会整体快照并还原 GameStatus，此时写入注定被回滚，直接拒绝
    if state.pending_preview_restore.lock().await.is_some() {
        return Err("剧本编辑器试玩中无法调整回复台词数量".to_string());
    }

    let svc = state.ai_service.lock().await;
    let mut gs = svc.game_status.lock().await;

    if gs.script_status.is_some() {
        return Err("剧本模式下无法调整回复台词数量".to_string());
    }
    if !reply_length_adjustable(&gs) {
        return Err("尚未进入对话，无法调整回复台词数量".to_string());
    }
    // 幂等：档位未变化（含变量缺失 + 选「正常」）就不插提示
    if gs.reply_length_mode() == new_mode {
        return Ok(new_mode.as_str().to_string());
    }

    gs.add_system_hint_line(&state.db, new_mode.announce_text())
        .await
        .map_err(|e| format!("写入系统提示台词失败: {e}"))?;
    gs.set_variable(REPLY_LENGTH_VAR_KEY, serde_json::json!(new_mode.as_str()));
    let mode = new_mode.as_str().to_string();
    // 插件也会调这个命令（连上 MC 自动切短档），设置页可能已经挂在 KeepAlive 里，
    // 不广播的话它会一直显示旧档位。先放锁再广播，别让前端处理事件时撞上同一把锁。
    drop(gs);
    drop(svc);
    let _ = app.emit("reply-length:changed", serde_json::json!({ "mode": mode }));
    Ok(mode)
}

/// 一轮对话的公共流程：截图处理 → 组装生成依赖 → 入队生成。
async fn dispatch_chat_turn(
    app: AppHandle,
    text: Option<String>,
    screenshot_base64: Option<String>,
    preamble: Option<String>,
) -> Result<(), String> {
    let state = app.state::<AppState>();

    let llm = crate::ai_service::llm::slot_snapshot(&state.chat.llm)
        .await
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
    // 捕获当前试玩代号（自由对话恒等，行为不变）
    let preview_generation = game_status.lock().await.preview_generation;

    // 当轮附带的多模态图片 data URL（原生识图路径专用，不写入记忆）
    let mut transient_image: Option<String> = None;

    // 发送思考事件
    events::emit_thinking(&app, true);

    // 截图交给识图投递决定：原生生图当轮直发（不写记忆），否则转述成旁白台词写进 line_list。
    // 必须在创建 GeneratorDeps 之前完成，旁白要排在本轮上下文里。
    if let Some(ref b64) = screenshot_base64 {
        if let Ok(image_bytes) = base64::Engine::decode(&base64::prelude::BASE64_STANDARD, b64) {
            let delivery = {
                let mut sa = state.screen_analyzer.lock().await;
                deliver_image(
                    &app,
                    &mut sa,
                    &image_bytes,
                    ImageIntent::ChatScreenshot {
                        user_name: &user_name,
                        user_text: text.as_deref().unwrap_or_default(),
                    },
                )
                .await
            };

            transient_image = delivery.attach;
            if let Some(narration) = delivery.text {
                let mut gs = game_status.lock().await;
                gs.add_line(
                    &state.db,
                    PromptLine::new(PromptRole::Narrator, narration.as_str()).into_base(),
                )
                .await
                .map_err(|e| format!("添加旁白台词失败: {}", e))?;
                tracing::info!("[Chat] Screenshot analysis narration added to game_status.");
            }
        }
    }

    // game_status / db 随后会被移入 deps，系统台词落库要用，先留一份句柄
    let preamble_gs = game_status.clone();
    let preamble_db = state.db.clone();

    let deps = GeneratorDeps {
        source: GeneratorSource::UserChat,
        app: app.clone(),
        db: state.db.clone(),
        game_status,
        processor: state.chat.processor.clone(),
        translator: state.chat.translator.clone(),
        llm,
        tool_registry: state.tool_registry.clone(),
        concurrency,
        god_agent: state.god_agent.clone(),
        suppress_thinking: false,
        generation: preview_generation,
        is_preview: false,
        transient_image,
        round_anchor_hint: None,
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
    let trigger_text = text.clone().unwrap_or_default();
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

        // 上一轮生成期间插件攒下的旁白先补进来，按发生顺序排在本轮输入之前。
        {
            let mut gs = preamble_gs.lock().await;
            gs.flush_pending_prompt_lines(&preamble_db).await;
            // 上一轮评估排队的近期记忆，赶在玩家消息落库之前落位。
            gs.flush_recent_memories(&preamble_db).await;
        }

        // 系统台词先于本轮玩家消息落库；sender_role_id 保持 None，不计入玩家消息序号。
        if let Some(content) = preamble {
            let mut gs = preamble_gs.lock().await;
            let line = PromptLine::new(PromptRole::System, content).into_base();
            if let Err(e) = gs.add_line(&preamble_db, line).await {
                tracing::error!("写入系统台词失败: {:#}", e);
            }
        }

        match generator.process_message(text).await {
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
                },
            };

            let role = match gs.role_manager.get_loaded(current_id) {
                Some(r) => r,
                None => {
                    tracing::warn!("角色 ID {} 未加载。", current_id);
                    return Ok(());
                },
            };

            tracing::info!(
                "=== 角色记忆 [{}] (role_id={}) ===",
                role.display_name.as_deref().unwrap_or("未知"),
                current_id
            );

            for msg in &role.memory {
                tracing::info!("[{}] {}", msg.role, msg.content);
                // 工具调用与调用 ID 是消息的另一半：只打正文的话，历史里会出现
                // 「有工具结果却没有谁发起的调用」，排查工具链路时完全对不上。
                if let Some(calls) = &msg.tool_calls {
                    let calls = calls
                        .iter()
                        .map(|call| format!("{}({})", call.function.name, call.function.arguments))
                        .collect::<Vec<_>>()
                        .join("; ");
                    tracing::info!("  工具调用: {calls}");
                }
                if let Some(id) = &msg.tool_call_id {
                    tracing::info!("  对应调用: {id}");
                }
            }
        },
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
        },
        // 与 /查看记忆 的区别：那边看的是记忆构建器的产物（多条 system 各自独立），
        // 这边看的是 provider 真正交给适配器的请求——所有 system 已被合并成一条，
        cmd if cmd.starts_with("/查看原始记忆") => {
            let limit = cmd
                .trim_start_matches("/查看原始记忆")
                .trim()
                .parse::<usize>()
                .ok()
                .filter(|n| *n > 0);
            let recent = crate::utils::llm_request_logger::recent_request_summaries();
            if recent.is_empty() {
                tracing::warn!("还没有发出过 LLM 请求（或当前 provider 未走 genai 通道）。");
                return Ok(());
            }

            let total = recent.len();
            let shown = limit.unwrap_or(total).min(total);
            tracing::info!("=== 原始请求历史：共 {total} 条（最新在前），本次输出 {shown} 条 ===");
            for (i, summary) in recent.iter().take(shown).enumerate() {
                tracing::info!("--- [{}/{}] ---\n{}", i + 1, total, summary);
            }
        },
        "/查看注册工具" => {
            let state = app.state::<AppState>();
            let definitions = state.tool_registry.definitions();
            tracing::info!("=== 已注册工具（共 {} 个）===", definitions.len());
            for def in &definitions {
                let purpose = def.function.description.lines().next().unwrap_or("").trim();
                tracing::info!("- {}：{}", def.function.name, purpose);
            }
        },
        "/查看所有工具" => {
            let Some(role_name) = debug_role_name(app).await else {
                tracing::warn!("没有当前绑定的角色。");
                return Ok(());
            };
            let state = app.state::<AppState>();
            let allowed = state
                .tool_registry
                .allowed_tools(GeneratorSource::UserChat, role_name.as_deref());
            let mut names: Vec<&String> = allowed.iter().collect();
            names.sort();
            tracing::info!(
                "=== 当前角色可用的全部工具（自由聊天场景，共 {} 个）===",
                names.len()
            );
            for name in names {
                tracing::info!("- {name}");
            }
        },
        "/查看推荐工具" => {
            let Some(role_name) = debug_role_name(app).await else {
                tracing::warn!("没有当前绑定的角色。");
                return Ok(());
            };
            let state = app.state::<AppState>();
            let activated = state.tool_rag.activated_tools(role_name.as_deref());
            let pending = {
                let svc = state.ai_service.lock().await;
                let gs = svc.game_status.lock().await;
                gs.pending_tool_hint().map(|names| names.to_vec())
            };
            tracing::info!(
                "=== 工具推荐 [{}]（缓存容量 {} 个，不含常驻工具）===",
                role_name.as_deref().unwrap_or("未知"),
                state.tool_rag.capacity()
            );
            tracing::info!(
                "Tool RAG 激活缓存（共 {} 个，最近激活在前）: {}",
                activated.len(),
                if activated.is_empty() {
                    "无".to_string()
                } else {
                    activated.join(", ")
                }
            );
            tracing::info!(
                "下一轮暂存的回复后推荐: {}",
                match pending {
                    Some(names) => names.join(", "),
                    None => "无".to_string(),
                }
            );
            match state.tool_rag.last_probe() {
                Some((query, scores)) => {
                    tracing::info!("最近一次静默检索: {query}");
                    for (name, score) in scores {
                        tracing::info!("  - {name}: {score:.3}");
                    }
                },
                None => tracing::info!("还没有跑过静默检索（或嵌入引擎未启用）"),
            }
        },
        other if other.starts_with('/') => {
            tracing::warn!(
                "未知调试指令: {}。可用指令: /查看记忆, /查看原始记忆, /查看台词, /查看注册工具, /查看所有工具, /查看推荐工具",
                other
            );
        },
        _ => unreachable!(),
    }
    Ok(())
}

/// 调试指令用：当前角色的显示名。外层 None 表示没有当前角色。
async fn debug_role_name(app: &AppHandle) -> Option<Option<String>> {
    let state = app.state::<AppState>();
    let svc = state.ai_service.lock().await;
    let mut gs = svc.game_status.lock().await;
    let role_id = gs.current_role_id?;
    Some(
        gs.get_role(&state.db, role_id)
            .await
            .ok()
            .and_then(|role| role.display_name.clone()),
    )
}

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
        gs.role_manager.invalidate_memory_history();
        gs.line_list.truncate(idx);
        gs.refresh_memories(&db)
            .await
            .map_err(|e| format!("刷新记忆失败: {}", e))?;

        // 若存在活跃存档，同步截断到 DB
        if let Some(save_id) = gs.active_save_id {
            SaveRepo::sync_lines(&db, save_id, &gs.line_list, false)
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
            thinking: gl.base.thinking.clone(),
            tts_content: gl.base.tts_content.clone(),
        })
        .collect();

    tracing::info!(
        "回溯对话完成: message_seq={}, 剩余台词 {} 条",
        message_seq,
        init_lines.len()
    );

    Ok(init_lines)
}

fn has_tts_countable_content(content: &str) -> bool {
    static RE: OnceLock<Regex> = OnceLock::new();
    let re = RE.get_or_init(|| Regex::new(r"\{[\s\S]*?\}").expect("invalid regex"));
    !re.replace_all(content, "").trim().is_empty()
}

/// 为历史中某条 AI 台词补生成语音；`line_seq` 为 0-based 的「AI 台词」全局序号。
#[tauri::command]
pub async fn generate_line_voice(app: AppHandle, line_seq: u32) -> Result<String, String> {
    let state = app.state::<AppState>();
    let db = state.db.clone();

    // 串行化：等待正在进行的消息生成完成，避免与生成流程争抢 TTS
    let gen_lock = state.generation_lock.clone();
    let _lock = gen_lock.lock().await;

    // ===== 预热（先于生成，保证配好 TTS 后第一次点击就能成功） =====
    {
        let local_state = app.state::<LocalTtsState>();
        if !local_state.engine.is_ready().await && local_state.paths.asset_present("deberta") {
            if let Err(e) = local_state.engine.init(&local_state.paths).await {
                tracing::error!("生成语音前初始化本地 TTS 引擎失败: {e}");
            }
        }
    }

    {
        let svc = state.ai_service.lock().await;
        let mut gs = svc.game_status.lock().await;
        gs.role_manager.rebuild_voice_makers_from_db(&db).await;
    }

    let _ = app.emit("tts://status-changed", ());

    let file_name = {
        let svc = state.ai_service.lock().await;
        let mut gs = svc.game_status.lock().await;

        let mut count = 0u32;
        let mut target_idx: Option<usize> = None;
        for (i, gl) in gs.line_list.iter().enumerate() {
            if matches!(gl.attribute(), LineAttribute::Assistant)
                && gl.base.sender_role_id.is_some()
                && has_tts_countable_content(&gl.base.content)
            {
                if count == line_seq {
                    target_idx = Some(i);
                    break;
                }
                count += 1;
            }
        }
        let idx = target_idx
            .ok_or_else(|| format!("未找到序号为 {} 的 AI 台词（共 {} 条）", line_seq, count))?;

        let role_id = gs.line_list[idx].base.sender_role_id;
        let role_id = role_id.ok_or_else(|| "该台词没有关联角色，无法生成语音".to_string())?;
        let text = gs.line_list[idx].base.content.clone();
        if text.trim().is_empty() {
            return Err("该台词没有可朗读的文本".to_string());
        }
        let original_tag = gs.line_list[idx]
            .base
            .original_emotion
            .clone()
            .unwrap_or_default();
        let motion_text = gs.line_list[idx]
            .base
            .action_content
            .clone()
            .unwrap_or_default();
        let japanese_text = gs.line_list[idx]
            .base
            .tts_content
            .clone()
            .unwrap_or_default();
        let predicted = gs.line_list[idx]
            .base
            .predicted_emotion
            .clone()
            .unwrap_or_default();

        let voice_maker = {
            let loaded = gs
                .role_manager
                .get_loaded(role_id)
                .and_then(|r| r.voice_maker.clone());
            if loaded.is_some() {
                loaded
            } else {
                gs.get_role(&db, role_id)
                    .await
                    .ok()
                    .and_then(|r| r.voice_maker.clone())
            }
        };
        let voice_maker = voice_maker
            .ok_or_else(|| format!("角色 {} 未配置 TTS，请在角色设置中启用语音", role_id))?;

        let mut seg = EmotionSegment {
            index: 0,
            original_tag,
            following_text: text,
            motion_text,
            japanese_text,
            predicted,
            confidence: 1.0,
            voice_file: String::new(),
            character: None,
            role_id: Some(role_id),
        };
        voice_maker
            .generate_voice_files(std::slice::from_mut(&mut seg))
            .await;

        // 5. 校验产物并回填 audio_file（voice_maker 生成失败时只打日志不留文件）
        let path = std::path::PathBuf::from(&seg.voice_file);
        if seg.voice_file.is_empty() || !path.exists() {
            return Err("语音生成失败：TTS 未启用或生成出错，请检查语音设置后再试".to_string());
        }
        let file_name = path
            .file_name()
            .map(|n| n.to_string_lossy().to_string())
            .ok_or_else(|| "语音文件路径异常".to_string())?;

        gs.line_list[idx].base.audio_file = Some(file_name.clone());

        // 6. 存在活跃存档时同步到 DB，保证重启后仍可播放
        if let Some(save_id) = gs.active_save_id {
            SaveRepo::sync_lines(&db, save_id, &gs.line_list, false)
                .await
                .map_err(|e| format!("同步存档失败: {}", e))?;
        }

        file_name
    }; // 释放锁

    tracing::info!("补生成语音完成: line_seq={}, file={}", line_seq, file_name);

    Ok(file_name)
}

//拉起AI回复（无用户输入，直接触发对话）
pub async fn trigger_ai_response(
    app: AppHandle,
    transient_image: Option<String>,
) -> Result<(), String> {
    let state = app.state::<AppState>();
    let llm = crate::ai_service::llm::slot_snapshot(&state.chat.llm)
        .await
        .ok_or_else(|| "LLM 未配置".to_string())?;
    let concurrency = AppConfig::load(&app)
        .map(|c| c.consumers as usize)
        .unwrap_or(1)
        .max(1);
    let gs = {
        let svc = state.ai_service.lock().await;
        svc.game_status.clone()
    };
    // 捕获当前试玩代号（自由对话恒等，行为不变）
    let preview_generation = gs.lock().await.preview_generation;
    let deps = GeneratorDeps {
        source: GeneratorSource::Proactive,
        app: app.clone(),
        db: state.db.clone(),
        game_status: gs,
        processor: state.chat.processor.clone(),
        translator: state.chat.translator.clone(),
        llm,
        tool_registry: state.tool_registry.clone(),
        concurrency,
        god_agent: state.god_agent.clone(),
        suppress_thinking: false,
        generation: preview_generation,
        is_preview: false,
        transient_image,
        // 这一轮没有玩家 user 行（纯图片/通知触发），不挂旁白。
        round_anchor_hint: None,
    };
    let gen_lock = state.generation_lock.clone();
    tokio::spawn(async move {
        let _lock = gen_lock.lock().await;
        let _ = MessageGenerator::new(deps).process_message(None).await;
    });
    tracing::info!("[chat] 触发 AI 回复");

    Ok(())
}

//处理图片投喂
#[tauri::command]
pub async fn feed_image(app: AppHandle, path: String) -> Result<(), String> {
    let state = app.state::<AppState>();
    let (user_name, game_status) = {
        let svc = state.ai_service.lock().await;
        let gs = svc.game_status.lock().await;
        (gs.player.user_name.clone(), svc.game_status.clone())
    };

    tracing::info!("[FileFeed] 收到图片投喂");

    events::emit_thinking(&app, true);

    // 图片交给识图投递决定：原生生图当轮直发（不写记忆），否则转述成旁白；读不到文件就照常触发本轮。
    let delivery = match std::fs::read(&path) {
        Ok(image_bytes) => {
            let mut sa = state.screen_analyzer.lock().await;
            deliver_image(
                &app,
                &mut sa,
                &image_bytes,
                ImageIntent::DroppedFile {
                    user_name: &user_name,
                },
            )
            .await
        },
        Err(e) => {
            tracing::warn!("[FileFeed] 读取图片失败: {e}");
            ImageDelivery::default()
        },
    };

    if let Some(text) = delivery.text {
        let mut gs = game_status.lock().await;
        gs.add_line(
            &state.db,
            PromptLine::new(PromptRole::Narrator, text.as_str()).into_base(),
        )
        .await
        .map_err(|e| format!("添加旁白台词失败: {}", e))?;
        tracing::info!("[FileFeed] 图片内容已注入上下文: {}", path);
    }

    events::emit_thinking(&app, false);
    let _ = trigger_ai_response(app, delivery.attach).await;

    Ok(())
}

#[tauri::command]
pub async fn feed_text(app: AppHandle, text: String) -> Result<(), String> {
    let state = app.state::<AppState>();
    let (user_name, game_status, ai_name) = {
        let svc = state.ai_service.lock().await;
        let gs = svc.game_status.lock().await;
        let ai_name = gs
            .current_role_id
            .and_then(|id| gs.role_manager.get_loaded(id))
            .and_then(|r| r.display_name.clone())
            .unwrap_or_else(|| "AI".to_string());
        (
            gs.player.user_name.clone(),
            svc.game_status.clone(),
            ai_name,
        )
    };

    tracing::info!("[FileFeed] 收到文本投喂");
    // 截断过长文本，避免 token 爆炸
    let truncated: String = if text.chars().count() > 2000 {
        text.chars()
            .take(2000)
            .chain("...(内容已截断)".chars())
            .collect()
    } else {
        text
    };

    let prompt = format!(
        "{} 给 {} 看了一段文字：\n\n{}\n\n",
        user_name, ai_name, truncated
    );

    let mut gs = game_status.lock().await;
    gs.add_line(
        &state.db,
        PromptLine::new(PromptRole::Narrator, prompt.as_str()).into_base(),
    )
    .await
    .map_err(|e| format!("添加投喂文本台词失败: {}", e))?;
    tracing::info!("[FileFeed] 文本投喂已注入上下文, 长度: {}", truncated.len());

    let _ = trigger_ai_response(app, None).await;

    Ok(())
}
