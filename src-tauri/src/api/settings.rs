//! Settings / LLM provider management Tauri commands.
//!
//! 这些命令原本位于 `config/mod.rs`，重构后移至 `api/` 层，
//! 遵循项目其他 API 模块的约定（command 在 api/，业务逻辑在 config/）。

use std::collections::BTreeMap;
use std::sync::Arc;

use serde_json::Value as JsonValue;
use tauri::AppHandle;
use tauri_plugin_dialog::DialogExt;

use crate::ai_service::god_agent::config::resolve_god_agent_provider;
use crate::ai_service::llm::provider_config::{
    build_llm_client_from_provider, load_providers, load_role_assignment, resolve_chat_provider,
    resolve_translate_provider, save_providers, save_role_assignment, LlmProviderConfig,
    LlmProvidersResponse,
};
use crate::ai_service::llm::LlmModelInfo;
use crate::config::app_config::{MAX_LLM_TIMEOUT_SECS, MIN_LLM_TIMEOUT_SECS};
use crate::config::{self, keys, ConfigSetting, ConfigTree};
use crate::AppState;

// ========== Settings CRUD ==========

#[tauri::command]
pub fn get_settings_tree(app: AppHandle) -> ConfigTree {
    config::build_config_tree(&app)
}

#[tauri::command]
pub fn save_settings(app: AppHandle, values: BTreeMap<String, String>) -> Result<String, String> {
    if let Some(value) = values.get(keys::LLM_TIMEOUT_SECS) {
        let timeout_secs = value
            .parse::<u64>()
            .map_err(|_| "LLM 请求空闲超时必须是整数".to_string())?;
        if !(MIN_LLM_TIMEOUT_SECS..=MAX_LLM_TIMEOUT_SECS).contains(&timeout_secs) {
            return Err(format!(
                "LLM 请求空闲超时必须在 {MIN_LLM_TIMEOUT_SECS}–{MAX_LLM_TIMEOUT_SECS} 秒之间"
            ));
        }
    }

    let store = config::settings_store(&app).map_err(|e| e.to_string())?;

    for (key, value) in &values {
        let json_value = if value == "true" {
            JsonValue::Bool(true)
        } else if value == "false" {
            JsonValue::Bool(false)
        } else if let Ok(n) = value.parse::<i64>() {
            JsonValue::Number(n.into())
        } else if let Ok(n) = value.parse::<f64>() {
            if let Some(f) = serde_json::Number::from_f64(n) {
                JsonValue::Number(f)
            } else {
                JsonValue::String(value.clone())
            }
        } else {
            JsonValue::String(value.clone())
        };
        store.set(key.clone(), json_value);
    }

    store.save().map_err(|e| e.to_string())?;

    Ok("配置已成功保存并已生效！".to_string())
}

#[tauri::command]
pub fn get_setting_by_key(app: AppHandle, key: String) -> Result<ConfigSetting, String> {
    let tree = config::build_config_tree(&app);
    for category in tree.values() {
        for sub in category.subcategories.values() {
            for setting in &sub.settings {
                if setting.key == key {
                    return Ok(setting.clone());
                }
            }
        }
    }
    Err(format!("Key '{}' not found", key))
}

#[tauri::command]
pub fn select_file(app: AppHandle) -> Result<Option<String>, String> {
    let file = app.dialog().file().blocking_pick_file();
    Ok(file.map(|f| f.to_string()))
}

// ========== LLM Multi-Provider Management ==========

#[tauri::command]
pub fn list_llm_providers(app: AppHandle) -> LlmProvidersResponse {
    let providers = load_providers(&app);
    let assignment = load_role_assignment(&app);
    LlmProvidersResponse {
        providers,
        chat_provider_id: assignment.chat_provider_id,
        translate_provider_id: assignment.translate_provider_id,
        god_agent_provider_id: assignment.god_agent_provider_id,
        vision_provider_id: assignment.vision_provider_id,
    }
}

#[tauri::command]
pub fn save_llm_provider(app: AppHandle, provider: LlmProviderConfig) -> Result<(), String> {
    let mut providers = load_providers(&app);

    let id = if provider.id.is_empty() {
        uuid::Uuid::new_v4().to_string()
    } else {
        provider.id.clone()
    };

    let mut updated = provider;
    updated.id = id.clone();

    if let Some(pos) = providers.iter().position(|p| p.id == id) {
        providers[pos] = updated;
    } else {
        providers.push(updated);
    }

    save_providers(&app, &providers).map_err(|e| e.to_string())?;
    Ok(())
}

#[tauri::command]
pub fn delete_llm_provider(app: AppHandle, id: String) -> Result<(), String> {
    let mut providers = load_providers(&app);
    providers.retain(|p| p.id != id);
    save_providers(&app, &providers).map_err(|e| e.to_string())?;

    let mut assignment = load_role_assignment(&app);
    let mut changed = false;
    if assignment.chat_provider_id.as_deref() == Some(&id) {
        assignment.chat_provider_id = None;
        changed = true;
    }
    if assignment.translate_provider_id.as_deref() == Some(&id) {
        assignment.translate_provider_id = None;
        changed = true;
    }
    if assignment.god_agent_provider_id.as_deref() == Some(&id) {
        assignment.god_agent_provider_id = None;
        changed = true;
    }
    if assignment.vision_provider_id.as_deref() == Some(&id) {
        assignment.vision_provider_id = None;
        changed = true;
    }
    if changed {
        save_role_assignment(&app, &assignment).map_err(|e| e.to_string())?;
    }

    Ok(())
}

#[tauri::command]
pub fn set_llm_role(
    app: AppHandle,
    role: String,
    provider_id: Option<String>,
) -> Result<(), String> {
    if let Some(ref pid) = provider_id {
        let providers = load_providers(&app);
        if !providers.iter().any(|p| p.id == *pid) {
            return Err(format!("Provider '{pid}' not found"));
        }
    }

    let mut assignment = load_role_assignment(&app);
    match role.as_str() {
        "chat" => assignment.chat_provider_id = provider_id,
        "translate" => assignment.translate_provider_id = provider_id,
        "god_agent" => assignment.god_agent_provider_id = provider_id,
        "vision" => assignment.vision_provider_id = provider_id,
        other => return Err(format!("Invalid role: {other}")),
    }
    save_role_assignment(&app, &assignment).map_err(|e| e.to_string())?;
    Ok(())
}

/// 热切换 LLM：无需重启即可生效的模型/提供商切换。
/// 重建聊天主 LLM、翻译 LLM、上帝 Agent LLM 三个槽位。
#[tauri::command]
pub async fn switch_llm(app: AppHandle, state: tauri::State<'_, AppState>) -> Result<(), String> {
    // 1. 重建聊天主 LLM 槽位
    let new_chat = resolve_chat_provider(&app)
        .and_then(|p| build_llm_client_from_provider(&app, &p))
        .map(Arc::new);

    {
        let mut guard = state.chat.llm.write().await;
        *guard = new_chat;
        tracing::info!("[switch_llm] 聊天 LLM 槽位已热切换");
    }

    // 2. 重建翻译 LLM 槽位
    let new_translate = resolve_translate_provider(&app)
        .and_then(|p| build_llm_client_from_provider(&app, &p))
        .map(Arc::new);

    {
        let slot = state.chat.translator.slot();
        let mut guard = slot.write().await;
        *guard = new_translate;
        tracing::info!("[switch_llm] 翻译 LLM 槽位已热切换");
    }

    // 3. 重建上帝 Agent LLM 槽位
    let new_god = resolve_god_agent_provider(&app).map(Arc::new);

    if let Some(ref god) = state.god_agent {
        let mut guard = god.llm.write().await;
        *guard = new_god;
        tracing::info!("[switch_llm] 上帝Agent LLM 槽位已热切换");
    }

    Ok(())
}

#[tauri::command]
pub async fn test_llm_provider(
    app: AppHandle,
    provider: LlmProviderConfig,
    message: String,
) -> Result<String, String> {
    let Some(client) = build_llm_client_from_provider(&app, &provider) else {
        return Err("无法创建 LLM 客户端：请检查 API Key 和模型名称".to_string());
    };

    let messages = vec![
        crate::ai_service::types::LlmMessage::system(
            "你是一个有帮助的AI助手。请简洁地回答用户的问题。",
        ),
        crate::ai_service::types::LlmMessage::user(&message),
    ];

    client
        .complete(&messages)
        .await
        .map_err(|e| format!("测试请求失败: {e}"))
}

#[tauri::command]
pub async fn list_llm_models(
    app: AppHandle,
    provider: LlmProviderConfig,
) -> Result<Vec<LlmModelInfo>, String> {
    let Some(client) = build_llm_client_from_provider(&app, &provider) else {
        return Err("无法创建 LLM 客户端，请检查 API Key 和模型名称".to_string());
    };

    client
        .list_models()
        .await
        .map_err(|error| error.to_string())
}
