//! Skill Agent 的 Tauri 命令层（`editor_agent_*`）。
//!
//! 薄封装：核心逻辑在 `ai_service::skill_agent`。事件走 `Channel<SkillAgentEvent>`
//! （作用域隔离），对话按「会话」隔离并持久化到 DB。

use std::sync::Arc;
use std::sync::atomic::Ordering;

use serde::{Deserialize, Serialize};
use tauri::{AppHandle, State};
use tauri_plugin_store::StoreExt;

use crate::ai_service::skill_agent::config::{resolve_skill_agent_provider, SkillAgentConfig};
use crate::ai_service::skill_agent::core::{run_chat, SkillAgentRunContext};
use crate::ai_service::skill_agent::events::SkillAgentEvent;
use crate::ai_service::skill_agent::{db, skills};
use crate::ai_service::types::LlmMessage;
use crate::config::keys;
use crate::db::entities::skill_agent_conversation;
use crate::AppState;

/// 单次注入 LLM 的历史消息窗口上限（超出后裁剪旧消息）。
/// 与 run_chat 内部的消息裁剪上限一致，避免首轮请求就超上下文。
const MAX_HISTORY: usize = 60;

// ==================== DTO ====================

/// Agent 设置（前端可读写；沙箱目录为空表示默认 `data/`）。
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AgentSettings {
    pub provider_id: Option<String>,
    pub sandbox_dir: Option<String>,
    pub auto_approve_commands: bool,
    pub allow_any_path: bool,
    pub max_tool_rounds: usize,
    pub system_prompt: Option<String>,
}

/// 技能内容（设置面板预览 SKILL.md 用）。
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SkillContent {
    pub name: String,
    pub base_directory: String,
    pub content: String,
}

/// 会话信息。
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ConversationInfo {
    pub id: i32,
    pub title: Option<String>,
    pub script_key: Option<String>,
    pub created_at: String,
    pub updated_at: String,
}

/// 持久化消息（OpenAI 格式）。
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PersistedMessage {
    pub id: i32,
    pub role: String,
    pub content: Option<String>,
    pub tool_calls: Option<serde_json::Value>,
    pub tool_call_id: Option<String>,
    pub created_at: String,
}

/// 设置面板展示的默认目录。
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AgentDefaultDirs {
    pub data_dir: String,
    pub skills_dir: String,
    pub sandbox_dir: String,
}

fn conv_to_info(c: &skill_agent_conversation::Model) -> ConversationInfo {
    ConversationInfo {
        id: c.id,
        title: c.title.clone(),
        script_key: c.script_key.clone(),
        created_at: c.created_at.to_string(),
        updated_at: c.updated_at.to_string(),
    }
}

/// 保留最近 `max` 条历史；裁剪边界不落在 `role:"tool"` 上，
/// 并把其所属 assistant→tool 整组包含进来，避免 DeepSeek 400。
fn trim_history(history: &mut Vec<LlmMessage>, max: usize) {
    if history.len() <= max {
        return;
    }
    let mut start = history.len() - max;
    while start > 0 && history[start].role == "tool" {
        start -= 1;
    }
    history.drain(..start);
}

// ==================== 设置 ====================

#[tauri::command]
pub async fn editor_agent_get_settings(app: AppHandle) -> AgentSettings {
    let config = SkillAgentConfig::load(&app);
    AgentSettings {
        provider_id: config.provider_id,
        sandbox_dir: config
            .sandbox_dir
            .map(|p| p.to_string_lossy().to_string()),
        auto_approve_commands: config.auto_approve_commands,
        allow_any_path: config.allow_any_path,
        max_tool_rounds: config.max_tool_rounds,
        system_prompt: config.system_prompt,
    }
}

#[tauri::command]
pub async fn editor_agent_save_settings(
    app: AppHandle,
    settings: AgentSettings,
) -> Result<(), String> {
    let store = app
        .store(crate::config::STORE_FILE)
        .map_err(|e| format!("无法打开设置存储: {}", e))?;
    let set_str = |key: &str, v: &Option<String>| {
        store.set(
            key.to_string(),
            v.clone()
                .map_or(serde_json::Value::Null, serde_json::Value::String),
        );
    };
    set_str(keys::AGENT_PROVIDER_ID, &settings.provider_id);
    set_str(keys::AGENT_SANDBOX_DIR, &settings.sandbox_dir);
    store.set(
        keys::AGENT_AUTO_APPROVE_COMMANDS.to_string(),
        serde_json::json!(settings.auto_approve_commands),
    );
    store.set(
        keys::AGENT_ALLOW_ANY_PATH.to_string(),
        serde_json::json!(settings.allow_any_path),
    );
    store.set(
        keys::AGENT_MAX_TOOL_ROUNDS.to_string(),
        serde_json::json!(settings.max_tool_rounds),
    );
    set_str(keys::AGENT_SYSTEM_PROMPT, &settings.system_prompt);
    store
        .save()
        .map_err(|e| format!("保存设置失败: {}", e))?;
    Ok(())
}

#[tauri::command]
pub async fn editor_agent_get_default_dirs(app: AppHandle) -> AgentDefaultDirs {
    let config = SkillAgentConfig::load(&app);
    AgentDefaultDirs {
        data_dir: crate::api::data_dir().to_string_lossy().to_string(),
        skills_dir: config.resolve_skills_dir().to_string_lossy().to_string(),
        sandbox_dir: config.resolve_sandbox_dir().to_string_lossy().to_string(),
    }
}

// ==================== 技能 ====================

#[tauri::command]
pub async fn editor_agent_list_skills(app: AppHandle) -> Vec<skills::SkillInfo> {
    let config = SkillAgentConfig::load(&app);
    skills::find_all_skills(&config.resolve_skills_dir())
}

#[tauri::command]
pub async fn editor_agent_read_skill(app: AppHandle, name: String) -> Result<SkillContent, String> {
    let config = SkillAgentConfig::load(&app);
    let res = skills::find_skill(&config.resolve_skills_dir(), &name)
        .ok_or_else(|| format!("未找到技能: {}", name))?;
    Ok(SkillContent {
        name: res.name,
        base_directory: res.base_directory.to_string_lossy().to_string(),
        content: res.content,
    })
}

// ==================== 会话 ====================

/// 新建会话。只记录创建时的剧本 key（不存剧本内容快照）。
#[tauri::command]
pub async fn editor_agent_create_conversation(
    state: State<'_, AppState>,
    script_key: Option<String>,
) -> Result<ConversationInfo, String> {
    let id = db::create_conversation(&state.db, Some("新对话".to_string()), script_key).await?;
    let conv = db::get_conversation(&state.db, id)
        .await?
        .ok_or_else(|| "创建会话失败".to_string())?;
    Ok(conv_to_info(&conv))
}

#[tauri::command]
pub async fn editor_agent_list_conversations(
    state: State<'_, AppState>,
) -> Result<Vec<ConversationInfo>, String> {
    let convs = db::list_conversations(&state.db).await?;
    Ok(convs.iter().map(conv_to_info).collect())
}

#[tauri::command]
pub async fn editor_agent_delete_conversation(
    state: State<'_, AppState>,
    conversation_id: i32,
) -> Result<(), String> {
    db::delete_conversation(&state.db, conversation_id).await
}

#[tauri::command]
pub async fn editor_agent_get_messages(
    state: State<'_, AppState>,
    conversation_id: i32,
) -> Result<Vec<PersistedMessage>, String> {
    let msgs = db::list_messages(&state.db, conversation_id).await?;
    Ok(msgs
        .iter()
        .map(|m| PersistedMessage {
            id: m.id,
            role: m.role.clone(),
            content: m.content.clone(),
            tool_calls: m
                .tool_calls
                .as_ref()
                .and_then(|s| serde_json::from_str(s).ok()),
            tool_call_id: m.tool_call_id.clone(),
            created_at: m.created_at.to_string(),
        })
        .collect())
}

#[tauri::command]
pub async fn editor_agent_clear_conversation(
    state: State<'_, AppState>,
    conversation_id: i32,
) -> Result<(), String> {
    db::clear_messages(&state.db, conversation_id).await
}

// ==================== 对话 ====================

#[tauri::command]
pub async fn editor_agent_start_chat(
    app: AppHandle,
    state: State<'_, AppState>,
    conversation_id: i32,
    message: String,
    channel: tauri::ipc::Channel<SkillAgentEvent>,
) -> Result<(), String> {
    if message.trim().is_empty() {
        return Err("消息不能为空".to_string());
    }
    let llm = resolve_skill_agent_provider(&app)
        .ok_or_else(|| "未配置可用的 LLM provider，请在「LLM 设置」中配置模型后再试".to_string())?;
    let config = SkillAgentConfig::load(&app);
    let sandbox_dir = config.resolve_sandbox_dir();
    let skills_dir = config.resolve_skills_dir();

    let conv = db::get_conversation(&state.db, conversation_id)
        .await?
        .ok_or_else(|| "会话不存在".to_string())?;

    let mut history = db::list_messages(&state.db, conversation_id)
        .await?
        .iter()
        .map(db::message_to_llm)
        .collect::<Vec<_>>();

    // 追加用户消息并持久化
    let user_msg = LlmMessage::user(message.trim());
    db::insert_message(&state.db, conversation_id, &user_msg).await?;

    // 历史过长时裁剪（保留尾部窗口，不切 tool 组）
    trim_history(&mut history, MAX_HISTORY);

    let ctx = SkillAgentRunContext {
        conversation_id,
        channel: channel.clone(),
        approvals: state.skill_agent.approvals.clone(),
        db: state.db.clone(),
        llm: Arc::new(llm),
        config,
        sandbox_dir,
        skills_dir,
        script_key: conv.script_key.clone(),
    };

    let cancelled = state.skill_agent.cancelled.clone();
    cancelled.store(false, Ordering::SeqCst);

    let handle = tauri::async_runtime::spawn(async move {
        let _ = run_chat(ctx, history, cancelled).await;
    });
    let mut task_guard = state.skill_agent.task.lock().await;
    *task_guard = Some(handle);

    let _ = db::touch_conversation(&state.db, conversation_id).await;
    Ok(())
}

#[tauri::command]
pub async fn editor_agent_stop_chat(state: State<'_, AppState>) -> Result<(), String> {
    state.skill_agent.cancelled.store(true, Ordering::SeqCst);
    let mut guard = state.skill_agent.task.lock().await;
    if let Some(handle) = guard.take() {
        handle.abort();
    }
    Ok(())
}

#[tauri::command]
pub async fn editor_agent_resolve_approval(
    state: State<'_, AppState>,
    request_id: String,
    allowed: bool,
) -> Result<(), String> {
    let mut approvals = state.skill_agent.approvals.lock().await;
    if let Some(req) = approvals.remove(&request_id) {
        let _ = req.tx.send(allowed);
    }
    Ok(())
}
