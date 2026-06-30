//! 上帝 Agent 配置。对标 Translator 的独立 LLM 配置模式。

use tauri::AppHandle;
use tauri_plugin_store::StoreExt;

use crate::ai_service::llm::provider_config::{
    build_llm_client_from_provider, load_providers, load_role_assignment,
};
use crate::ai_service::llm::LlmClient;
use crate::config::{self, keys};

// ============================================================
// GodAgentConfig
// ============================================================

/// 上帝 Agent 运行参数。
#[derive(Debug, Clone)]
pub struct GodAgentConfig {
    /// LLM provider ID；None 表示使用聊天主 LLM。
    pub provider_id: Option<String>,
    /// 连续 NPC 发言轮数上限（超过后强制返回玩家）。
    pub max_consecutive_npc: usize,
    /// 决策时参考的最近台词行数。
    pub recent_window: usize,
}

impl Default for GodAgentConfig {
    fn default() -> Self {
        Self {
            provider_id: None,
            max_consecutive_npc: 3,
            recent_window: 20,
        }
    }
}

impl GodAgentConfig {
    /// 从 Tauri store 加载配置。
    pub fn load(app: &AppHandle) -> Self {
        let Ok(store) = app.store(config::STORE_FILE) else {
            return Self::default();
        };

        let provider_id = store
            .get(keys::LLM_GOD_AGENT_PROVIDER_ID)
            .and_then(|v| v.as_str().map(|s| s.to_string()));

        let max_consecutive_npc = store
            .get(keys::GOD_AGENT_MAX_CONSECUTIVE_NPC)
            .and_then(|v| v.as_str().and_then(|s| s.parse::<usize>().ok()))
            .unwrap_or(3)
            .max(1);

        let recent_window = store
            .get(keys::GOD_AGENT_RECENT_WINDOW)
            .and_then(|v| v.as_str().and_then(|s| s.parse::<usize>().ok()))
            .unwrap_or(20)
            .max(5);

        Self {
            provider_id,
            max_consecutive_npc,
            recent_window,
        }
    }
}

// ============================================================
// LLM 构建
// ============================================================

/// 解析上帝 Agent 使用的 LLM provider，fallback 到聊天主 LLM。
pub fn resolve_god_agent_provider(app: &AppHandle) -> Option<LlmClient> {
    let config = GodAgentConfig::load(app);
    let assignment = load_role_assignment(app);

    // 1. 显式指定的 God Agent provider
    if let Some(ref id) = config.provider_id {
        let providers = load_providers(app);
        if let Some(p) = providers.iter().find(|p| &p.id == id && p.is_usable()) {
            tracing::info!("上帝Agent 使用专用 LLM: {} ({})", p.label, p.id);
            return build_llm_client_from_provider(p);
        }
    }

    // 2. LLM 角色分配中的 god_agent_provider_id
    if let Some(ref id) = assignment.god_agent_provider_id {
        let providers = load_providers(app);
        if let Some(p) = providers.iter().find(|p| &p.id == id && p.is_usable()) {
            tracing::info!("上帝Agent 使用角色分配的 LLM: {} ({})", p.label, p.id);
            return build_llm_client_from_provider(p);
        }
    }

    // 3. Fallback：聊天主 LLM
    let providers = load_providers(app);
    if let Some(ref id) = assignment.chat_provider_id {
        if let Some(p) = providers.iter().find(|p| &p.id == id && p.is_usable()) {
            tracing::info!("上帝Agent fallback 到聊天 LLM: {} ({})", p.label, p.id);
            return build_llm_client_from_provider(p);
        }
    }

    // 4. 任何可用的 provider
    if let Some(p) = providers.iter().find(|p| p.is_usable()) {
        tracing::info!("上帝Agent 使用第一个可用 LLM: {} ({})", p.label, p.id);
        return build_llm_client_from_provider(p);
    }

    tracing::warn!("上帝Agent 未找到可用 LLM，多人对话功能将禁用");
    None
}
