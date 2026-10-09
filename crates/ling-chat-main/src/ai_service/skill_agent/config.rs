//! Skill Agent 配置与 LLM provider 解析。

use std::path::PathBuf;

use tauri::AppHandle;
use tauri_plugin_store::StoreExt;

use crate::ai_service::llm::LlmClient;
use crate::ai_service::llm::provider_config::{
    LlmProviderConfig, build_llm_client_from_provider, load_providers, load_role_assignment,
};
use crate::config::{self, keys};

use super::stage;

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
    /// 单次对话的工具调用轮数上限；-1 表示无上限。
    pub max_tool_rounds: i32,
    /// 自定义系统提示；None 使用内置默认提示（技能列表与剧本上下文始终追加）。
    pub system_prompt: Option<String>,
    /// 思考模式覆盖；None 表示跟随 provider 默认（独立于主对话 LLM 设置）。
    pub enable_thinking: Option<bool>,
}

impl Default for SkillAgentConfig {
    fn default() -> Self {
        Self {
            provider_id: None,
            sandbox_dir: None,
            auto_approve_commands: false,
            allow_any_path: false,
            max_tool_rounds: -1,
            system_prompt: None,
            enable_thinking: None,
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
        let mut config = Self {
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
                .and_then(|v| v.as_i64().map(|n| n as i32))
                .unwrap_or(-1),
            system_prompt: str_opt(keys::AGENT_SYSTEM_PROMPT),
            enable_thinking: store
                .get(keys::AGENT_ENABLE_THINKING)
                .and_then(|v| v.as_bool()),
        };
        if cfg!(mobile) {
            config.auto_approve_commands = false;
            config.allow_any_path = false;
        }
        config
    }

    /// 解析后的沙箱根目录（默认 `data/`）。
    pub fn resolve_sandbox_dir(&self) -> PathBuf {
        self.sandbox_dir
            .clone()
            .unwrap_or_else(|| crate::data_dir::get_data_dir().clone())
    }

    /// 技能库目录（固定为 `data/game_data/skills`）。
    pub fn resolve_skills_dir(&self) -> PathBuf {
        crate::api::game_data_dir().join("skills")
    }
}

/// 解析 Skill Agent 使用的 LLM provider，fallback 到聊天主 LLM；`stage_thinking` 优先于设置项。
pub fn resolve_skill_agent_provider(
    app: &AppHandle,
    stage_thinking: Option<bool>,
) -> Option<LlmClient> {
    let config = SkillAgentConfig::load(app);
    let assignment = load_role_assignment(app);

    let thinking = stage_thinking.or(config.enable_thinking);
    let build_client = |p: &LlmProviderConfig| {
        let mut cfg = p.clone();
        if let Some(v) = thinking {
            cfg.enable_thinking = v;
        }
        build_llm_client_from_provider(app, &cfg)
    };

    if let Some(ref id) = config.provider_id {
        let providers = load_providers(app);
        if let Some(p) = providers.iter().find(|p| &p.id == id && p.is_usable()) {
            tracing::info!("Skill Agent 使用专用 LLM: {} ({})", p.label, p.id);
            return build_client(p);
        }
    }

    if let Some(ref id) = assignment.chat_provider_id {
        let providers = load_providers(app);
        if let Some(p) = providers.iter().find(|p| &p.id == id && p.is_usable()) {
            tracing::info!("Skill Agent fallback 到聊天 LLM: {} ({})", p.label, p.id);
            return build_client(p);
        }
    }

    let providers = load_providers(app);
    if let Some(p) = providers.iter().find(|p| p.is_usable()) {
        tracing::info!("Skill Agent 使用第一个可用 LLM: {} ({})", p.label, p.id);
        return build_client(p);
    }

    tracing::warn!("Skill Agent 未找到可用 LLM");
    None
}

/// 会话开始时按 `provider/model` 缓存的窗口值，只在开新会话时解析一次，免得换 provider 后重复探测。
fn window_cache() -> &'static std::sync::Mutex<std::collections::HashMap<String, usize>> {
    static CACHE: std::sync::OnceLock<std::sync::Mutex<std::collections::HashMap<String, usize>>> =
        std::sync::OnceLock::new();
    CACHE.get_or_init(|| std::sync::Mutex::new(std::collections::HashMap::new()))
}

/// 探窗口的超时：有的 provider（如 kimi_code）的 `list_models` 会真的发 HTTP 请求，
const WINDOW_PROBE_TIMEOUT: std::time::Duration = std::time::Duration::from_secs(5);

/// 这一轮算上下文预算用的模型窗口（token）：优先读 provider 自报的 `context_length`（`/models`），读不到用 [`stage::budget::DEFAULT_CONTEXT_WINDOW`]。
pub async fn resolve_context_window(llm: &LlmClient) -> usize {
    let cfg = llm.config();
    let key = format!("{}/{}", cfg.provider, cfg.model);
    if let Ok(cache) = window_cache().lock() {
        if let Some(found) = cache.get(&key) {
            return *found;
        }
    }

    let reported = match tokio::time::timeout(WINDOW_PROBE_TIMEOUT, llm.list_models()).await {
        Ok(Ok(models)) => models
            .iter()
            .find(|m| m.id == cfg.model)
            .and_then(|m| m.context_length)
            .map(|v| v as usize),
        Ok(Err(e)) => {
            tracing::debug!("[skill_agent] 读取模型窗口失败，用默认值: {e}");
            None
        },
        Err(_) => {
            tracing::debug!("[skill_agent] 读取模型窗口超时，用默认值");
            None
        },
    };

    let window = reported
        .filter(|v| *v > 0)
        .unwrap_or(stage::budget::DEFAULT_CONTEXT_WINDOW);
    tracing::info!(
        "[skill_agent] 上下文窗口 {} token（{}）",
        window,
        if reported.is_some() {
            "provider 自报"
        } else {
            "默认值"
        }
    );
    if let Ok(mut cache) = window_cache().lock() {
        cache.insert(key, window);
    }
    window
}
