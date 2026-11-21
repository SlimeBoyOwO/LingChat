//! 旧版对话、翻译和视觉配置的迁移。

use serde_json::Value as JsonValue;
use tauri::AppHandle;
use tauri_plugin_store::StoreExt;

use super::{
    LlmProviderConfig, get_string_opt, json_string_opt, load_providers, load_role_assignment,
    save_providers, save_role_assignment,
};
use crate::ai_service::llm::codex::model_config::migrate_preview_client_versions;
use crate::config::{self, keys};

pub fn migrate_if_needed(app: &AppHandle) {
    let Ok(store) = app.store(config::STORE_FILE) else {
        return;
    };

    // Already migrated
    if store.has(keys::LLM_PROVIDERS) {
        migrate_preview_client_versions(&store);
        return;
    }

    // No old config to migrate
    let Some(old_provider) = get_string_opt(&store, keys::LLM_PROVIDER) else {
        return;
    };

    tracing::info!("Migrating flat LLM config keys to provider list...");

    let old_model = get_string_opt(&store, keys::LLM_MODEL).unwrap_or_default();
    let old_api_key = get_string_opt(&store, keys::LLM_API_KEY).unwrap_or_default();
    let old_base_url = get_string_opt(&store, keys::LLM_BASE_URL).unwrap_or_default();
    let old_temperature = get_f64_opt(&store, keys::LLM_TEMPERATURE);
    let old_top_p = get_f64_opt(&store, keys::LLM_TOP_P);
    let old_thinking = get_bool_opt(&store, keys::LLM_ENABLE_THINKING, false);

    let mut providers: Vec<LlmProviderConfig> = Vec::new();
    let mut chat_id: Option<String> = None;

    // Migrate main LLM
    // 仅要求 model 非空；api_key 允许为空（本地/自托管模型无需密钥）
    if !old_model.is_empty() {
        let id = uuid::Uuid::new_v4().to_string();
        let label = old_model.clone();
        providers.push(LlmProviderConfig {
            id: id.clone(),
            label,
            provider: old_provider,
            model: old_model,
            api_key: old_api_key,
            base_url: old_base_url,
            temperature: old_temperature,
            top_p: old_top_p,
            enable_thinking: old_thinking,
            reasoning_effort: None,
            fast_mode: false,
            support_vision: false,
            vision_compress: false,
            vision_max_edge: 2048,
            vision_jpeg_quality: 85,
        });
        chat_id = Some(id);
    }

    // Migrate translate LLM
    let trans_provider = get_string_opt(&store, keys::TRANSLATE_PROVIDER).unwrap_or_default();
    let trans_model = get_string_opt(&store, keys::TRANSLATE_MODEL).unwrap_or_default();
    let trans_api_key = get_string_opt(&store, keys::TRANSLATE_API_KEY).unwrap_or_default();
    let trans_base_url = get_string_opt(&store, keys::TRANSLATE_BASE_URL).unwrap_or_default();

    let mut translate_id: Option<String> = None;
    // 仅要求 model 非空；api_key 允许为空（本地/自托管模型无需密钥）
    if !trans_model.is_empty() {
        // Check if translate config is different from chat
        let is_different = trans_provider
            != get_string_opt(&store, keys::LLM_PROVIDER).unwrap_or_default()
            || trans_model != get_string_opt(&store, keys::LLM_MODEL).unwrap_or_default()
            || trans_api_key != get_string_opt(&store, keys::LLM_API_KEY).unwrap_or_default()
            || trans_base_url != get_string_opt(&store, keys::LLM_BASE_URL).unwrap_or_default();

        if is_different {
            let id = uuid::Uuid::new_v4().to_string();
            let label = format!("{} (翻译)", trans_model);
            providers.push(LlmProviderConfig {
                id: id.clone(),
                label,
                provider: trans_provider,
                model: trans_model,
                api_key: trans_api_key,
                base_url: trans_base_url,
                temperature: None,
                top_p: None,
                enable_thinking: false,
                reasoning_effort: None,
                fast_mode: false,
                support_vision: false,
                vision_compress: false,
                vision_max_edge: 2048,
                vision_jpeg_quality: 85,
            });
            translate_id = Some(id);
        }
        // If same as chat, don't add a duplicate — fallback will handle it
    }

    // Save
    let arr: Vec<JsonValue> = providers
        .iter()
        .map(|p| serde_json::to_value(p).unwrap_or(JsonValue::Null))
        .collect();
    store.set(keys::LLM_PROVIDERS.to_string(), JsonValue::Array(arr));
    store.set(
        keys::LLM_CHAT_PROVIDER_ID.to_string(),
        json_string_opt(chat_id.as_deref()),
    );
    store.set(
        keys::LLM_TRANSLATE_PROVIDER_ID.to_string(),
        json_string_opt(translate_id.as_deref()),
    );

    match store.save() {
        Err(e) => {
            tracing::error!("Migration save failed: {e}");
        },
        _ => {
            tracing::info!("Migration complete: {} provider(s)", providers.len());
        },
    }
}

/// 将旧的主动视觉独立配置（VD_API_KEY / VD_BASE_URL / VD_MODEL）迁移为
/// 大模型管理中的一个 provider，并分配为「视觉模型」角色。
/// 视觉模型配置已统一到大模型管理，旧键迁移后会被清理。
pub fn migrate_legacy_vision_keys(app: &AppHandle) {
    let Ok(store) = app.store(config::STORE_FILE) else {
        return;
    };

    let api_key = get_string_opt(&store, keys::VD_API_KEY).unwrap_or_default();
    let model = get_string_opt(&store, keys::VD_MODEL).unwrap_or_default();
    // 仅要求 model 非空；api_key 允许为空（本地/自托管模型无需密钥）
    if model.trim().is_empty() {
        return;
    }

    // 已分配过视觉角色则不重复迁移
    if get_string_opt(&store, keys::LLM_VISION_PROVIDER_ID).is_some() {
        return;
    }

    let base_url = get_string_opt(&store, keys::VD_BASE_URL).unwrap_or_default();

    tracing::info!("Migrating legacy VD_* vision config into LLM provider list...");

    let id = uuid::Uuid::new_v4().to_string();
    let label = if model.is_empty() {
        "视觉模型".to_string()
    } else {
        format!("{model} (视觉)")
    };
    let provider = LlmProviderConfig {
        id: id.clone(),
        label,
        provider: "openai".to_string(),
        model,
        api_key,
        base_url,
        temperature: None,
        top_p: None,
        enable_thinking: false,
        reasoning_effort: None,
        fast_mode: false,
        support_vision: false,
        vision_compress: false,
        vision_max_edge: 2048,
        vision_jpeg_quality: 85,
    };

    let mut providers = load_providers(app);
    providers.push(provider);
    if let Err(e) = save_providers(&store, &providers) {
        tracing::error!("Legacy vision config migration failed to save providers: {e}");
        return;
    }

    let mut assignment = load_role_assignment(app);
    assignment.vision_provider_id = Some(id);
    if let Err(e) = save_role_assignment(app, &assignment) {
        tracing::error!("Legacy vision config migration failed to assign vision role: {e}");
        return;
    }

    // 清理旧键，避免重复迁移
    store.delete(keys::VD_API_KEY);
    store.delete(keys::VD_BASE_URL);
    store.delete(keys::VD_MODEL);
    if let Err(e) = store.save() {
        tracing::warn!("Legacy vision keys cleanup save failed: {e}");
    }
}

fn get_f64_opt(store: &tauri_plugin_store::Store<tauri::Wry>, key: &str) -> Option<f64> {
    store.get(key).and_then(|v| v.as_f64())
}

fn get_bool_opt(store: &tauri_plugin_store::Store<tauri::Wry>, key: &str, default: bool) -> bool {
    store.get(key).and_then(|v| v.as_bool()).unwrap_or(default)
}
