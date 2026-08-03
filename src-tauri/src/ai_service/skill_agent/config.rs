//! Skill Agent 配置与 LLM provider 解析。

use std::path::PathBuf;

use tauri::AppHandle;
use tauri_plugin_store::StoreExt;

use crate::ai_service::llm::provider_config::{
    build_llm_client_from_provider, load_providers, load_role_assignment,
};
use crate::ai_service::llm::LlmClient;
use crate::api::{data_dir, game_data_dir};
use crate::config::{self, keys};

/// Skill Agent 运行参数。
#[derive(Debug, Clone)]
pub struct SkillAgentConfig {
    /// LLM provider ID；None 表示跟随聊天主 LLM。
    pub provider_id: Option<String>,
    /// 文件沙箱根目录；None 表示默认 `data/`。
    pub sandbox_dir: Option<PathBuf>,
    /// 命令是否自动审批（无需用户确认）。
    pub auto_approve_commands: bool,
    /// 是否允许文件工具访问沙箱之外的任意路径。
    pub allow_any_path: bool,
    /// 单次对话的工具调用轮数上限。
    pub max_tool_rounds: usize,
    /// 自定义系统提示；None 使用内置默认提示（技能列表与剧本上下文始终追加）。
    pub system_prompt: Option<String>,
}

impl Default for SkillAgentConfig {
    fn default() -> Self {
        Self {
            provider_id: None,
            sandbox_dir: None,
            auto_approve_commands: false,
            allow_any_path: false,
            max_tool_rounds: 20,
            system_prompt: None,
        }
    }
}

impl SkillAgentConfig {
    /// 从 settings.json store 加载配置。
    pub fn load(app: &AppHandle) -> Self {
        let Some(store) = app.store(config::STORE_FILE).ok() else {
            return Self::default();
        };
        let str_opt = |key: &str| {
            store
                .get(key)
                .and_then(|v| v.as_str().map(|s| s.to_string()))
                .filter(|s| !s.trim().is_empty())
        };
        Self {
            provider_id: str_opt(keys::AGENT_PROVIDER_ID),
            sandbox_dir: str_opt(keys::AGENT_SANDBOX_DIR).map(PathBuf::from),
            auto_approve_commands: store
                .get(keys::AGENT_AUTO_APPROVE_COMMANDS)
                .and_then(|v| v.as_bool())
                .unwrap_or(false),
            allow_any_path: store
                .get(keys::AGENT_ALLOW_ANY_PATH)
                .and_then(|v| v.as_bool())
                .unwrap_or(false),
            max_tool_rounds: store
                .get(keys::AGENT_MAX_TOOL_ROUNDS)
                .and_then(|v| v.as_u64().map(|n| n as usize))
                .unwrap_or(20)
                .max(1),
            system_prompt: str_opt(keys::AGENT_SYSTEM_PROMPT),
        }
    }

    /// 解析后的沙箱根目录（默认 `data/`）。
    pub fn resolve_sandbox_dir(&self) -> PathBuf {
        self.sandbox_dir
            .clone()
            .unwrap_or_else(data_dir)
    }

    /// 技能库目录（固定为 `data/game_data/skills`）。
    pub fn resolve_skills_dir(&self) -> PathBuf {
        game_data_dir().join("skills")
    }
}

/// 解析 Skill Agent 使用的 LLM provider，fallback 到聊天主 LLM（镜像 God Agent）。
pub fn resolve_skill_agent_provider(app: &AppHandle) -> Option<LlmClient> {
    let config = SkillAgentConfig::load(app);
    let assignment = load_role_assignment(app);

    // 1. 显式指定的 agent provider
    if let Some(ref id) = config.provider_id {
        let providers = load_providers(app);
        if let Some(p) = providers.iter().find(|p| &p.id == id && p.is_usable()) {
            tracing::info!("Skill Agent 使用专用 LLM: {} ({})", p.label, p.id);
            return build_llm_client_from_provider(app, p);
        }
    }

    // 2. Fallback：聊天主 LLM
    if let Some(ref id) = assignment.chat_provider_id {
        let providers = load_providers(app);
        if let Some(p) = providers.iter().find(|p| &p.id == id && p.is_usable()) {
            tracing::info!("Skill Agent fallback 到聊天 LLM: {} ({})", p.label, p.id);
            return build_llm_client_from_provider(app, p);
        }
    }

    // 3. 任何可用的 provider
    let providers = load_providers(app);
    if let Some(p) = providers.iter().find(|p| p.is_usable()) {
        tracing::info!("Skill Agent 使用第一个可用 LLM: {} ({})", p.label, p.id);
        return build_llm_client_from_provider(app, p);
    }

    tracing::warn!("Skill Agent 未找到可用 LLM");
    None
}
