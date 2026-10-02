//! Skill Agent 的 Tauri 命令层（`editor_agent_*`）。

use std::path::Path;
use std::sync::Arc;
use std::sync::atomic::Ordering;

use serde::{Deserialize, Serialize};
use tauri::{AppHandle, State};
use tauri_plugin_store::StoreExt;

use crate::AppState;
use crate::ai_service::skill_agent::config::{
    SkillAgentConfig, resolve_context_window, resolve_skill_agent_provider,
};
use crate::ai_service::skill_agent::core::{SkillAgentRunContext, run_chat};
use crate::ai_service::skill_agent::events::SkillAgentEvent;
use crate::ai_service::skill_agent::{db, skills, stage};
use crate::ai_service::types::LlmMessage;
use crate::config::keys;
use crate::db::entities::skill_agent_conversation;

/// Agent 设置（前端可读写；沙箱目录为空表示默认 `data/`）。
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AgentSettings {
    pub provider_id: Option<String>,
    pub sandbox_dir: Option<String>,
    pub auto_approve_commands: bool,
    pub allow_any_path: bool,
    /// 工具调用轮数上限；-1 表示无上限。
    pub max_tool_rounds: i32,
    pub system_prompt: Option<String>,
    /// 思考模式覆盖；None 表示跟随 provider 默认（独立于主对话 LLM 设置）。
    pub enable_thinking: Option<bool>,
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
    /// assistant 的思考链（仅展示，不参与 LLM 上下文）。
    pub reasoning: Option<String>,
    /// 产生该消息那一轮 LLM 调用的 token 用量（前端据此恢复「总计」；未上报为 NULL）。
    pub prompt_tokens: Option<i64>,
    pub completion_tokens: Option<i64>,
    /// 输入中命中缓存（cache read）的 token 数（缓存命中统计；未上报为 NULL）。
    pub cached_tokens: Option<i64>,
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

#[tauri::command]
pub async fn editor_agent_get_settings(app: AppHandle) -> AgentSettings {
    let config = SkillAgentConfig::load(&app);
    AgentSettings {
        provider_id: config.provider_id,
        sandbox_dir: config.sandbox_dir.map(|p| p.to_string_lossy().to_string()),
        auto_approve_commands: config.auto_approve_commands,
        allow_any_path: config.allow_any_path,
        max_tool_rounds: config.max_tool_rounds,
        system_prompt: config.system_prompt,
        enable_thinking: config.enable_thinking,
    }
}

#[tauri::command]
pub async fn editor_agent_save_settings(
    app: AppHandle,
    mut settings: AgentSettings,
) -> Result<(), String> {
    if cfg!(mobile) {
        settings.auto_approve_commands = false;
        settings.allow_any_path = false;
    }
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
    store.set(
        keys::AGENT_ENABLE_THINKING.to_string(),
        settings
            .enable_thinking
            .map_or(serde_json::Value::Null, |v| serde_json::json!(v)),
    );
    store.save().map_err(|e| format!("保存设置失败: {}", e))?;
    Ok(())
}

#[tauri::command]
pub async fn editor_agent_get_default_dirs(app: AppHandle) -> AgentDefaultDirs {
    let config = SkillAgentConfig::load(&app);
    AgentDefaultDirs {
        data_dir: crate::data_dir::get_data_dir()
            .to_string_lossy()
            .to_string(),
        skills_dir: config.resolve_skills_dir().to_string_lossy().to_string(),
        sandbox_dir: config.resolve_sandbox_dir().to_string_lossy().to_string(),
    }
}

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

/// 新建会话。只记录创建时的剧本 key（不存剧本内容快照）。
#[tauri::command]
pub async fn editor_agent_create_conversation(
    state: State<'_, AppState>,
    script_key: Option<String>,
) -> Result<ConversationInfo, String> {
    let id = db::create_conversation(&state.db, None, script_key).await?;
    let conv = db::get_conversation(&state.db, id)
        .await?
        .ok_or_else(|| "创建会话失败".to_string())?;
    Ok(conv_to_info(&conv))
}

/// 重命名会话（用户自定义标题）。空标题拒绝，避免「清空标题」导致下次首轮
#[tauri::command]
pub async fn editor_agent_rename_conversation(
    state: State<'_, AppState>,
    conversation_id: i32,
    title: String,
) -> Result<(), String> {
    let title = title.trim().to_string();
    if title.is_empty() {
        return Err("会话标题不能为空".to_string());
    }
    db::update_conversation_title(&state.db, conversation_id, title).await
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
            reasoning: m.reasoning.clone(),
            prompt_tokens: m.prompt_tokens,
            completion_tokens: m.completion_tokens,
            cached_tokens: m.cached_tokens,
            tool_calls: m
                .tool_calls
                .as_ref()
                .and_then(|s| serde_json::from_str(s).ok()),
            tool_call_id: m.tool_call_id.clone(),
            created_at: m.created_at.to_string(),
        })
        .collect())
}

/// 会话归属的剧本 key。库里没绑定时从历史写入路径反推（老会话、建包早于绑定逻辑的会话）。
#[tauri::command]
pub async fn editor_agent_resolve_script_key(
    state: State<'_, AppState>,
    conversation_id: i32,
) -> Result<Option<String>, String> {
    let conv = db::get_conversation(&state.db, conversation_id)
        .await?
        .ok_or_else(|| "会话不存在".to_string())?;
    if conv.script_key.is_some() {
        return Ok(conv.script_key);
    }
    Ok(db::derive_script_key(&state.db, conversation_id).await)
}

#[tauri::command]
pub async fn editor_agent_clear_conversation(
    state: State<'_, AppState>,
    conversation_id: i32,
) -> Result<(), String> {
    db::clear_messages(&state.db, conversation_id).await
}

/// 剧本包 `.agent/` 下的一份流程产物（设计稿、任务队列、用户约束……）。
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AgentArtifact {
    pub name: String,
    pub content: String,
}

/// 单份产物的内容上限。超了就只列名字：这些是给人看的文本，不该把浮窗顶爆。
const ARTIFACT_MAX_BYTES: u64 = 64 * 1024;

/// 列出剧本包里的流程产物：直接读目录而不是列固定清单，加新产物不用改这里。
#[tauri::command]
pub fn editor_agent_list_artifacts(script_key: String) -> Result<Vec<AgentArtifact>, String> {
    let dir = crate::utils::script_paths::resolve_script_dir(&script_key)?;
    Ok(read_artifacts(&dir))
}

fn read_artifacts(script_dir: &Path) -> Vec<AgentArtifact> {
    let Ok(entries) = std::fs::read_dir(script_dir.join(stage::AGENT_DIR)) else {
        return Vec::new();
    };

    let mut out: Vec<AgentArtifact> = Vec::new();
    for entry in entries.flatten() {
        let Ok(meta) = entry.metadata() else {
            continue;
        };
        if !meta.is_file() || meta.len() > ARTIFACT_MAX_BYTES {
            continue;
        }
        let name = entry.file_name().to_string_lossy().to_string();
        if name.starts_with('.') || !name.ends_with(".md") {
            continue;
        }
        match std::fs::read_to_string(entry.path()) {
            Ok(content) => out.push(AgentArtifact { name, content }),
            Err(e) => tracing::warn!("[agent] 流程产物读取失败 {}: {}", name, e),
        }
    }

    let design = stage::DESIGN_REL_PATH.rsplit('/').next().unwrap_or("");
    out.sort_by(|a, b| (a.name != design, &a.name).cmp(&(b.name != design, &b.name)));
    out
}

/// 开始一轮对话。返回本次用户消息的 DB id（前端用于「回溯删除」定位删除起点）。
#[tauri::command]
pub async fn editor_agent_start_chat(
    app: AppHandle,
    state: State<'_, AppState>,
    conversation_id: i32,
    message: String,
    channel: tauri::ipc::Channel<SkillAgentEvent>,
) -> Result<i32, String> {
    if message.trim().is_empty() {
        return Err("消息不能为空".to_string());
    }
    let conv = db::get_conversation(&state.db, conversation_id)
        .await?
        .ok_or_else(|| "会话不存在".to_string())?;

    let script_key = match conv.script_key.clone() {
        Some(key) => Some(key),
        None => match db::derive_script_key(&state.db, conversation_id).await {
            Some(key) => {
                if let Err(e) =
                    db::update_conversation_script_key(&state.db, conversation_id, key.clone())
                        .await
                {
                    tracing::warn!("[skill_agent] 补绑剧本 key 失败: {}", e);
                }
                Some(key)
            },
            None => None,
        },
    };

    let stage_snapshot = stage::derive(script_key.as_deref());
    let llm = resolve_skill_agent_provider(&app, Some(true))
        .ok_or_else(|| "未配置可用的 LLM provider，请在「LLM 设置」中配置模型后再试".to_string())?;
    let config = SkillAgentConfig::load(&app);
    let sandbox_dir = config.resolve_sandbox_dir();
    let skills_dir = config.resolve_skills_dir();
    let context_window = resolve_context_window(&llm).await;

    let mut history = db::list_messages(&state.db, conversation_id)
        .await?
        .iter()
        .map(db::message_to_llm)
        .collect::<Vec<_>>();

    let user_msg = LlmMessage::user(message.trim());
    let user_msg_id = db::insert_message(&state.db, conversation_id, &user_msg, None, None).await?;
    history.push(user_msg);

    let ctx = SkillAgentRunContext {
        conversation_id,
        channel: channel.clone(),
        approvals: state.skill_agent.approvals.clone(),
        db: state.db.clone(),
        llm: Arc::new(llm),
        app: app.clone(),
        config,
        sandbox_dir,
        skills_dir,
        data_dir: crate::data_dir::get_data_dir().clone(),
        script_key,
        existing_script_keys: crate::utils::script_paths::enumerate_script_keys(),
        stage_snapshot,
        bound_script_key: std::sync::Mutex::new(None),
        context_window,
    };

    let cancelled = state.skill_agent.cancelled.clone();
    cancelled.store(false, Ordering::SeqCst);

    let handle = tauri::async_runtime::spawn(async move {
        let _ = run_chat(ctx, history, cancelled).await;
    });
    let mut task_guard = state.skill_agent.task.lock().await;
    if let Some(prev) = task_guard.take() {
        prev.abort();
    }
    *task_guard = Some(handle);

    let _ = db::touch_conversation(&state.db, conversation_id).await;
    Ok(user_msg_id)
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

/// 回溯：删除会话中 id >= message_id 的消息，把对话回退到该消息发送前。
#[tauri::command]
pub async fn editor_agent_rewind(
    state: State<'_, AppState>,
    conversation_id: i32,
    message_id: i32,
) -> Result<(), String> {
    db::delete_messages_from(&state.db, conversation_id, message_id).await
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
