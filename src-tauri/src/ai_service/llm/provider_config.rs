//! Multi-provider LLM configuration management.
//!
//! Replaces the old flat `llm.provider` / `llm.model` / ... keys with a list of named
//! provider configs, each assignable as the "chat" or "translate" model.

use anyhow::Context;
use serde::{Deserialize, Serialize};
use serde_json::Value as JsonValue;
use tauri::AppHandle;
use tauri_plugin_store::StoreExt;

use crate::ai_service::llm::{create_llm_client, LlmClient, LlmConfig};
use crate::config::{self, keys};

// ============================================================
// Data types
// ============================================================

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LlmProviderConfig {
    pub id: String,
    pub label: String,
    pub provider: String,
    pub model: String,
    #[serde(default)]
    pub api_key: String,
    #[serde(default)]
    pub base_url: String,
    #[serde(default)]
    pub temperature: Option<f64>,
    #[serde(default)]
    pub top_p: Option<f64>,
    #[serde(default)]
    pub enable_thinking: bool,
}

impl LlmProviderConfig {
    pub fn is_usable(&self) -> bool {
        !self.api_key.is_empty() && !self.model.is_empty()
    }

    pub fn to_llm_config(&self) -> LlmConfig {
        LlmConfig {
            provider: self.provider.clone(),
            model: self.model.clone(),
            api_key: self.api_key.clone(),
            base_url: self.base_url.clone(),
            timeout_secs: 120,
            temperature: self.temperature,
            top_p: self.top_p,
            enable_thinking: self.enable_thinking,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct LlmRoleAssignment {
    #[serde(default)]
    pub chat_provider_id: Option<String>,
    #[serde(default)]
    pub translate_provider_id: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LlmProvidersResponse {
    pub providers: Vec<LlmProviderConfig>,
    pub chat_provider_id: Option<String>,
    pub translate_provider_id: Option<String>,
}

// ============================================================
// Load / Save helpers
// ============================================================

pub fn load_providers(app: &AppHandle) -> Vec<LlmProviderConfig> {
    let Some(store) = app.store(config::STORE_FILE).ok() else {
        return Vec::new();
    };
    match store.get(keys::LLM_PROVIDERS) {
        Some(JsonValue::Array(arr)) => arr
            .iter()
            .filter_map(|v| serde_json::from_value(v.clone()).ok())
            .collect(),
        _ => Vec::new(),
    }
}

pub fn save_providers(app: &AppHandle, providers: &[LlmProviderConfig]) -> anyhow::Result<()> {
    let store = app
        .store(config::STORE_FILE)
        .context("Failed to open settings store")?;
    let arr: Vec<JsonValue> = providers
        .iter()
        .map(|p| serde_json::to_value(p).unwrap_or(JsonValue::Null))
        .collect();
    store.set(keys::LLM_PROVIDERS.to_string(), JsonValue::Array(arr));
    store.save().context("Failed to save settings store")?;
    Ok(())
}

pub fn load_role_assignment(app: &AppHandle) -> LlmRoleAssignment {
    let Some(store) = app.store(config::STORE_FILE).ok() else {
        return LlmRoleAssignment::default();
    };
    LlmRoleAssignment {
        chat_provider_id: get_string_opt(&store, keys::LLM_CHAT_PROVIDER_ID),
        translate_provider_id: get_string_opt(&store, keys::LLM_TRANSLATE_PROVIDER_ID),
    }
}

pub fn save_role_assignment(
    app: &AppHandle,
    assignment: &LlmRoleAssignment,
) -> anyhow::Result<()> {
    let store = app
        .store(config::STORE_FILE)
        .context("Failed to open settings store")?;
    store.set(
        keys::LLM_CHAT_PROVIDER_ID.to_string(),
        json_string_opt(assignment.chat_provider_id.as_deref()),
    );
    store.set(
        keys::LLM_TRANSLATE_PROVIDER_ID.to_string(),
        json_string_opt(assignment.translate_provider_id.as_deref()),
    );
    store.save().context("Failed to save settings store")?;
    Ok(())
}

// ============================================================
// Resolution
// ============================================================

pub fn resolve_chat_provider(app: &AppHandle) -> Option<LlmProviderConfig> {
    let assignment = load_role_assignment(app);
    let providers = load_providers(app);
    assignment
        .chat_provider_id
        .and_then(|id| providers.into_iter().find(|p| p.id == id && p.is_usable()))
}

pub fn resolve_translate_provider(app: &AppHandle) -> Option<LlmProviderConfig> {
    let assignment = load_role_assignment(app);
    let providers = load_providers(app);

    // 1. Explicit translate provider
    if let Some(ref id) = assignment.translate_provider_id {
        if let Some(p) = providers.iter().find(|p| p.id == *id) {
            if p.is_usable() {
                tracing::info!(
                    "Using explicit translate provider: {} ({})",
                    p.label,
                    p.id
                );
                return Some(p.clone());
            }
        }
    }

    // 2. Fallback to chat provider
    if let Some(ref id) = assignment.chat_provider_id {
        if let Some(p) = providers.iter().find(|p| p.id == *id) {
            if p.is_usable() {
                tracing::info!(
                    "Translate provider not set, falling back to chat provider: {} ({})",
                    p.label,
                    p.id
                );
                return Some(p.clone());
            }
        }
    }

    tracing::warn!("No usable LLM provider for translation");
    None
}

pub fn build_llm_client_from_provider(cfg: &LlmProviderConfig) -> Option<LlmClient> {
    if !cfg.is_usable() {
        tracing::warn!(
            "Skipping unusable LLM provider: {} ({})",
            cfg.label,
            cfg.id
        );
        return None;
    }
    match create_llm_client(cfg.to_llm_config()) {
        Ok(client) => Some(client),
        Err(e) => {
            tracing::error!("Failed to create LLM client for {}: {e}", cfg.label);
            None
        }
    }
}

// ============================================================
// Migration
// ============================================================

pub fn migrate_if_needed(app: &AppHandle) {
    let Ok(store) = app.store(config::STORE_FILE) else {
        return;
    };

    // Already migrated
    if store.has(keys::LLM_PROVIDERS) {
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
    if !old_api_key.is_empty() && !old_model.is_empty() {
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
        });
        chat_id = Some(id);
    }

    // Migrate translate LLM
    let trans_provider = get_string_opt(&store, keys::TRANSLATE_PROVIDER).unwrap_or_default();
    let trans_model = get_string_opt(&store, keys::TRANSLATE_MODEL).unwrap_or_default();
    let trans_api_key = get_string_opt(&store, keys::TRANSLATE_API_KEY).unwrap_or_default();
    let trans_base_url = get_string_opt(&store, keys::TRANSLATE_BASE_URL).unwrap_or_default();

    let mut translate_id: Option<String> = None;
    if !trans_api_key.is_empty() && !trans_model.is_empty() {
        // Check if translate config is different from chat
        let is_different = trans_provider != get_string_opt(&store, keys::LLM_PROVIDER).unwrap_or_default()
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

    if let Err(e) = store.save() {
        tracing::error!("Migration save failed: {e}");
    } else {
        tracing::info!("Migration complete: {} provider(s)", providers.len());
    }
}

// ============================================================
// Private helpers
// ============================================================

fn get_string_opt(store: &tauri_plugin_store::Store<tauri::Wry>, key: &str) -> Option<String> {
    store.get(key).and_then(|v| v.as_str().map(|s| s.to_string()))
}

fn get_f64_opt(store: &tauri_plugin_store::Store<tauri::Wry>, key: &str) -> Option<f64> {
    store.get(key).and_then(|v| v.as_f64())
}

fn get_bool_opt(
    store: &tauri_plugin_store::Store<tauri::Wry>,
    key: &str,
    default: bool,
) -> bool {
    store
        .get(key)
        .and_then(|v| v.as_bool())
        .unwrap_or(default)
}

fn json_string_opt(s: Option<&str>) -> JsonValue {
    match s {
        Some(v) => JsonValue::String(v.to_string()),
        None => JsonValue::Null,
    }
}
