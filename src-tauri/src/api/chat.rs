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

    let deps = GeneratorDeps {
        app: app.clone(),
        db: state.db.clone(),
        ai_service: state.ai_service.clone(),
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
                log::error!("发送成就事件失败: {}", e);
            }
        }
    });

    let generator = MessageGenerator::new(deps);
    let gen_lock = state.generation_lock.clone();

    tokio::spawn(async move {
        let _lock = gen_lock.lock().await;
        match generator.process_message(Some(text)).await {
            Ok(acc) => log::info!("消息生成完成，长度: {}", acc.len()),
            Err(e) => log::error!("消息生成失败: {:#}", e),
        }
    });

    Ok(())
}
