//! Multi-provider LLM configuration management.
//!
//! Replaces the old flat `llm.provider` / `llm.model` / ... keys with a list of named
//! provider configs, each assignable as the "chat" or "translate" model.

use anyhow::Context;
use serde::{Deserialize, Serialize};
use serde_json::Value as JsonValue;
use tauri::{AppHandle, Wry};
use tauri_plugin_store::{Store, StoreExt};

use super::codex::model_config::load_client_versions;
use super::factory::create_codex_client;
use super::{LlmClient, LlmConfig, create_llm_client};
use crate::config::app_config::AppConfig;
use crate::config::{self, keys};

mod migration;
pub use migration::{migrate_if_needed, migrate_legacy_vision_keys};

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
    /// 推理深度（如 "low" / "high" / "max"），由支持 reasoning 的模型使用（如 Kimi Code K3 系列）。
    #[serde(default)]
    pub reasoning_effort: Option<String>,
    /// Codex Fast Mode（1.5× 速度，额度消耗更快）；仅 provider = codex 时有意义。
    #[serde(default)]
    pub fast_mode: bool,
    /// 是否支持原生多模态识图（支持把图片直接发给该模型而不用旁白转述）。
    /// 需要该 provider 的协议能承载图片内容（OpenAI 兼容 image_url / Gemini 多模态）。
    #[serde(default)]
    pub support_vision: bool,
    /// 原生识图时是否压缩图片（默认 false：原图直发）。
    /// 开启后可进一步用 `vision_max_edge`/`vision_jpeg_quality` 控制压缩强度，
    /// 以节省多模态输入的 token 与上下文缓存占用。
    #[serde(default)]
    pub vision_compress: bool,
    /// 压缩时图片最大边长（像素），超宽图等比缩放。默认 2048。
    #[serde(default = "default_vision_max_edge")]
    pub vision_max_edge: u32,
    /// 压缩时 JPEG 编码质量（0-100）。默认 85。
    #[serde(default = "default_vision_jpeg_quality")]
    pub vision_jpeg_quality: u8,
}

/// 压缩设置默认值（防旧配置反序列化时字段缺失）。
fn default_vision_max_edge() -> u32 {
    2048
}
fn default_vision_jpeg_quality() -> u8 {
    85
}

impl LlmProviderConfig {
    /// 判断该 provider 是否可用于发起 LLM 请求。
    ///
    /// 允许 api_key 为空（本地模型 / 自托管 OpenAI 兼容服务无需密钥），
    /// 只要求 model 非空。
    pub fn is_usable(&self) -> bool {
        !self.model.is_empty()
    }

    /// 判断对话协议能否直接承载图片，包括 Codex 的 Responses 输入。
    pub fn is_native_multimodal_capable(&self) -> bool {
        matches!(
            self.provider.to_lowercase().as_str(),
            "openai" | "deepseek" | "lmstudio" | "gemini" | "codex"
        )
    }

    /// 解析原生识图的图片压缩参数。
    ///
    /// 关闭压缩时返回「原图直发」配置；开启时返回用户指定的边长上限与 JPEG 质量
    /// （并做边界钳制，防止手滑填出非法值）。
    pub fn native_image_compress(&self) -> crate::ai_service::screen_analyzer::NativeImageCompress {
        let max_edge = self.vision_max_edge.clamp(64, 8192);
        // 质量只保留 1-100 的合法区间；0 视为未设置，用默认 85
        let quality = if (1..=100).contains(&self.vision_jpeg_quality) {
            self.vision_jpeg_quality
        } else {
            85
        };
        crate::ai_service::screen_analyzer::NativeImageCompress {
            enabled: self.vision_compress,
            max_edge,
            jpeg_quality: quality,
        }
    }

    pub fn to_llm_config(&self, timeout_secs: u64) -> LlmConfig {
        LlmConfig {
            provider: self.provider.clone(),
            model: self.model.clone(),
            api_key: self.api_key.clone(),
            base_url: self.base_url.clone(),
            timeout_secs,
            temperature: self.temperature,
            top_p: self.top_p,
            enable_thinking: self.enable_thinking,
            reasoning_effort: self.reasoning_effort.clone(),
            fast_mode: self.fast_mode,
            support_vision: self.support_vision,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct LlmRoleAssignment {
    #[serde(default)]
    pub chat_provider_id: Option<String>,
    #[serde(default)]
    pub translate_provider_id: Option<String>,
    #[serde(default)]
    pub god_agent_provider_id: Option<String>,
    #[serde(default)]
    pub vision_provider_id: Option<String>,
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

pub fn save_providers(store: &Store<Wry>, providers: &[LlmProviderConfig]) -> anyhow::Result<()> {
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
        god_agent_provider_id: get_string_opt(&store, keys::LLM_GOD_AGENT_PROVIDER_ID),
        vision_provider_id: get_string_opt(&store, keys::LLM_VISION_PROVIDER_ID),
    }
}

pub fn save_role_assignment(app: &AppHandle, assignment: &LlmRoleAssignment) -> anyhow::Result<()> {
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
    store.set(
        keys::LLM_GOD_AGENT_PROVIDER_ID.to_string(),
        json_string_opt(assignment.god_agent_provider_id.as_deref()),
    );
    store.set(
        keys::LLM_VISION_PROVIDER_ID.to_string(),
        json_string_opt(assignment.vision_provider_id.as_deref()),
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
    if let Some(ref id) = assignment.translate_provider_id
        && let Some(p) = providers.iter().find(|p| p.id == *id)
        && p.is_usable()
    {
        tracing::info!("Using explicit translate provider: {} ({})", p.label, p.id);
        return Some(p.clone());
    }

    // 2. Fallback to chat provider
    if let Some(ref id) = assignment.chat_provider_id
        && let Some(p) = providers.iter().find(|p| p.id == *id)
        && p.is_usable()
    {
        tracing::info!(
            "Translate provider not set, falling back to chat provider: {} ({})",
            p.label,
            p.id
        );
        return Some(p.clone());
    }

    tracing::warn!("No usable LLM provider for translation");
    None
}

/// 解析视觉分析（截图理解）使用的 provider。
/// 优先使用「视觉模型」角色显式指定的 provider，缺省时跟随对话模型。
pub fn resolve_vision_provider(app: &AppHandle) -> Option<LlmProviderConfig> {
    let assignment = load_role_assignment(app);
    let providers = load_providers(app);

    // 1. Explicit vision provider
    if let Some(ref id) = assignment.vision_provider_id
        && let Some(p) = providers.iter().find(|p| p.id == *id)
        && p.is_usable()
    {
        tracing::info!("Using explicit vision provider: {} ({})", p.label, p.id);
        return Some(p.clone());
    }

    // 2. Fallback to chat provider
    if let Some(ref id) = assignment.chat_provider_id
        && let Some(p) = providers.iter().find(|p| p.id == *id)
        && p.is_usable()
    {
        tracing::info!(
            "Vision provider not set, falling back to chat provider: {} ({})",
            p.label,
            p.id
        );
        return Some(p.clone());
    }

    tracing::warn!("No usable LLM provider for vision analysis");
    None
}

/// 本轮识图是否走「原生多模态」：把图片直接附在对话请求里，跳过旁白转述。
pub fn native_vision_enabled(app: &AppHandle) -> bool {
    let Some(chat) = resolve_chat_provider(app) else {
        return false;
    };
    if !chat.is_native_multimodal_capable() {
        return false;
    }
    chat.support_vision || resolve_vision_provider(app).is_some_and(|v| v.id == chat.id)
}

pub fn build_llm_client_from_provider(
    app: &AppHandle,
    cfg: &LlmProviderConfig,
) -> Option<LlmClient> {
    build_llm_client_with_catalog_version(app, cfg, None)
}

pub(crate) fn build_llm_client_with_catalog_version(
    app: &AppHandle,
    cfg: &LlmProviderConfig,
    client_version: Option<String>,
) -> Option<LlmClient> {
    if !cfg.is_usable() {
        tracing::warn!("Skipping unusable LLM provider: {} ({})", cfg.label, cfg.id);
        return None;
    }
    let timeout_secs = AppConfig::load(app).unwrap_or_default().llm_timeout_secs;
    let llm_config = cfg.to_llm_config(timeout_secs);
    let client = if cfg.provider.eq_ignore_ascii_case("codex") {
        let client_version = client_version.or_else(|| {
            let store = config::settings_store(app).ok()?;
            load_client_versions(&store).remove(&cfg.id)
        });
        create_codex_client(llm_config, client_version)
    } else {
        create_llm_client(llm_config)
    };
    match client {
        Ok(client) => Some(client),
        Err(e) => {
            tracing::error!("Failed to create LLM client for {}: {e}", cfg.label);
            None
        },
    }
}

// ============================================================
// Private helpers
// ============================================================

fn get_string_opt(store: &tauri_plugin_store::Store<tauri::Wry>, key: &str) -> Option<String> {
    store
        .get(key)
        .and_then(|v| v.as_str().map(|s| s.to_string()))
}

fn json_string_opt(s: Option<&str>) -> JsonValue {
    match s {
        Some(v) => JsonValue::String(v.to_string()),
        None => JsonValue::Null,
    }
}
