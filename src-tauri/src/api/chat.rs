use std::sync::OnceLock;

use regex::Regex;
use tauri::{AppHandle, Emitter, Manager};

use crate::ai_service::message_system::events;
use crate::ai_service::message_system::generator::{
    GeneratorDeps, GeneratorSource, MessageGenerator,
};
use crate::ai_service::message_system::processor::EmotionSegment;
use crate::ai_service::tts::local::LocalTtsState;
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

/// 判断该台词行是否属于「可补生成语音」的 AI 台词：
/// 剥掉 `{...}` 动作段后仍有正文。与前端 `convertInitLines` 判空规则对齐，
/// 避免纯动作行（如整行都是 `{...}`）前后端计数不一致。
fn has_tts_countable_content(content: &str) -> bool {
    static RE: OnceLock<Regex> = OnceLock::new();
    let re = RE.get_or_init(|| Regex::new(r"\{[\s\S]*?\}").expect("invalid regex"));
    !re.replace_all(content, "").trim().is_empty()
}

/// 为历史中某条 AI 台词补生成语音（「生成语音」按钮后端）。
///
/// `line_seq` 为 0-based 的「AI 台词」全局序号：按 `line_list` 中
/// attribute == Assistant 且有正文、且关联角色（sender_role_id 非空）的行计数。
/// 前端历史展示与后端计数规则一致（同样跳过空内容与无角色行），因此任意来源的
/// AI 台词都能定位——自由对话轮次、AI 开场白、主动对话、剧本台词均适用。
///
/// 语音生成复用该台词角色已构建的 `VoiceMaker`（未加载时从 DB 惰性注册，
/// 保证重启后剧本 NPC 等角色也能正确对应），产物写入 `<data_dir>/voice/`
/// 后回填 `audio_file`，存在活跃存档时同步到 DB，重启后仍可播放。
#[tauri::command]
pub async fn generate_line_voice(app: AppHandle, line_seq: u32) -> Result<String, String> {
    let state = app.state::<AppState>();
    let db = state.db.clone();

    // 串行化：等待正在进行的消息生成完成，避免与生成流程争抢 TTS
    let gen_lock = state.generation_lock.clone();
    let _lock = gen_lock.lock().await;

    // ===== 预热（先于生成，保证配好 TTS 后第一次点击就能成功） =====
    // 1. 本地 TTS 引擎：未就绪且 DeBERTa 已安装时初始化（秒级 ONNX 加载，
    //    此时不持 gs 锁，避免堵住对话）
    {
        let local_state = app.state::<LocalTtsState>();
        if !local_state.engine.is_ready().await && local_state.paths.asset_present("deberta") {
            if let Err(e) = local_state.engine.init(&local_state.paths).await {
                tracing::error!("生成语音前初始化本地 TTS 引擎失败: {e}");
            }
        }
    }

    // 2. 已加载角色按 DB 最新 TTS 配置重建 VoiceMaker：恢复被后台探测禁用的
    //    provider、补齐「角色先于 TTS 配置注册」产生的 None，本次生成即用新配置
    {
        let svc = state.ai_service.lock().await;
        let mut gs = svc.game_status.lock().await;
        gs.role_manager.rebuild_voice_makers_from_db(&db).await;
    }

    // 3. 预热完成后再广播 TTS 状态变化事件：前端（TTS 设置页等）刷新到的是
    //    真实就绪状态（静默刷新，无 toast），而非刷新前的旧状态
    let _ = app.emit("tts://status-changed", ());

    let file_name = {
        let svc = state.ai_service.lock().await;
        let mut gs = svc.game_status.lock().await;

        // 1. 定位目标台词：数「assistant 且有正文且关联角色」的行，取第 line_seq 条。
        //    跳过 sender_role_id 为 None 的行（工具调用回填的 assistant 前缀行，
        //    实时对话时前端不可见，重载后可见——若计入序号会造成前后端计数漂移）。
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
        let idx = target_idx.ok_or_else(|| {
            format!("未找到序号为 {} 的 AI 台词（共 {} 条）", line_seq, count)
        })?;

        // 2. 先克隆台词数据（之后要对 gs 做可变借用）
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
        let japanese_text = gs.line_list[idx].base.tts_content.clone().unwrap_or_default();
        let predicted = gs.line_list[idx]
            .base
            .predicted_emotion
            .clone()
            .unwrap_or_default();

        // 3. 取角色 VoiceMaker：优先已加载角色；未加载（如重启后剧本 NPC）
        //    从 DB 惰性注册再取，保证语音始终对应台词自己的角色
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
        let voice_maker = voice_maker.ok_or_else(|| {
            format!("角色 {} 未配置 TTS，请在角色设置中启用语音", role_id)
        })?;

        // 4. 构造单个情绪片段并生成语音。
        //    选文逻辑与实时生成一致（voice_maker.rs 的 segment_text_for_lang：
        //    语音语言为 ja 且存在日语译文时朗读译文）。
        //    存储的台词已剥离【情绪】标签（生成时提取过），无需二次解析。
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
        voice_maker.generate_voice_files(std::slice::from_mut(&mut seg)).await;

        // 5. 校验产物并回填 audio_file（voice_maker 生成失败时只打日志不留文件）
        let path = std::path::PathBuf::from(&seg.voice_file);
        if seg.voice_file.is_empty() || !path.exists() {
            return Err(
                "语音生成失败：TTS 未启用或生成出错，请检查语音设置后再试".to_string(),
            );
        }
        let file_name = path
            .file_name()
            .map(|n| n.to_string_lossy().to_string())
            .ok_or_else(|| "语音文件路径异常".to_string())?;

        gs.line_list[idx].base.audio_file = Some(file_name.clone());

        // 6. 存在活跃存档时同步到 DB，保证重启后仍可播放
        if let Some(save_id) = gs.active_save_id {
            SaveRepo::sync_lines(&db, save_id, &gs.line_list)
                .await
                .map_err(|e| format!("同步存档失败: {}", e))?;
        }

        file_name
    }; // 释放锁

    tracing::info!("补生成语音完成: line_seq={}, file={}", line_seq, file_name);

    Ok(file_name)
}

//拉起AI回复（无用户输入，直接触发对话）
pub async fn trigger_ai_response(app: AppHandle) -> Result<(), String> {
    let state = app.state::<AppState>();
    let llm = crate::ai_service::llm::slot_snapshot(&state.chat.llm)
        .await
        .ok_or_else(|| "LLM 未配置".to_string())?;
    let concurrency = AppConfig::load(&app).map(|c| c.consumers as usize).unwrap_or(1).max(1);
    let gs = { let svc = state.ai_service.lock().await; svc.game_status.clone() };
    // 捕获当前试玩代号（自由对话恒等，行为不变）
    let preview_generation = gs.lock().await.preview_generation;
    let deps = GeneratorDeps {
        source: GeneratorSource::Proactive,
        app: app.clone(), db: state.db.clone(), game_status: gs,
        processor: state.chat.processor.clone(), translator: state.chat.translator.clone(),
        llm, tool_registry: state.tool_registry.clone(), concurrency, god_agent: state.god_agent.clone(), suppress_thinking: false,
        generation: preview_generation,
        is_preview: false,
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
pub async fn feed_image(
    app: AppHandle,
    path: String,
) -> Result<(), String> {
    let state = app.state::<AppState>();
    let (user_name, game_status) = {
        let svc = state.ai_service.lock().await;
        let gs = svc.game_status.lock().await;
        (gs.player.user_name.clone(), svc.game_status.clone(),)
    };

    tracing::info!("[FileFeed] 收到图片投喂");
    let prompt = format!("用户（名字是\"{}\"）给你看了一张图片，请你用第三人称叙述把你看到的画面描述给其他AI让他理解用户的图片内容",user_name);

    events::emit_thinking(&app, true);
    let analysis = {
        let mut sa = state.screen_analyzer.lock().await;
        sa.analyze_image_file(&path, &prompt).await
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
        tracing::info!("[FileFeed] 图片分析已注入上下文: {}", path);
    }

    events::emit_thinking(&app, false);
    let _ = trigger_ai_response(app).await;

    Ok(())
}

#[tauri::command]
pub async fn feed_text(
    app: AppHandle,
    text: String,
) -> Result<(), String> {
    let state = app.state::<AppState>();
    let (user_name, game_status, ai_name) = {
        let svc = state.ai_service.lock().await;
        let gs = svc.game_status.lock().await;
        let ai_name = gs
            .current_role_id.and_then(|id| gs.role_manager.get_loaded(id)).and_then(|r| r.display_name.clone()).unwrap_or_else(|| "AI".to_string());
        (gs.player.user_name.clone(), svc.game_status.clone(), ai_name)
    };

    tracing::info!("[FileFeed] 收到文本投喂");
    // 截断过长文本，避免 token 爆炸
    let truncated: String = if text.chars().count() > 2000 {
        text.chars().take(2000).chain("...(内容已截断)".chars()).collect()
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
        LineBase {
            content: PromptRole::Narrator.build_prompt(&prompt),
            attribute: LineAttributeExt(LineAttribute::User),
            display_name: Some("旁白".to_string()),
            ..Default::default()
        },
    )
    .await
    .map_err(|e| format!("添加投喂文本台词失败: {}", e))?;
    tracing::info!("[FileFeed] 文本投喂已注入上下文, 长度: {}", truncated.len());

    let _ = trigger_ai_response(app).await;

    Ok(())
}