//! Skill Agent 配置与 LLM provider 解析。

use std::path::PathBuf;

use tauri::AppHandle;
use tauri_plugin_store::StoreExt;

use crate::ai_service::llm::LlmClient;
use crate::ai_service::llm::provider_config::{
    LlmProviderConfig, build_llm_client_from_provider, load_providers, load_role_assignment,
};
use crate::api::{data_dir, game_data_dir};
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
        self.sandbox_dir.clone().unwrap_or_else(data_dir)
    }

    /// 技能库目录（固定为 `data/game_data/skills`）。
    pub fn resolve_skills_dir(&self) -> PathBuf {
        game_data_dir().join("skills")
    }
}

/// 解析 Skill Agent 使用的 LLM provider，fallback 到聊天主 LLM（镜像 God Agent）。
///
/// `stage_thinking` 为阶段级规定，优先于设置项。
pub fn resolve_skill_agent_provider(
    app: &AppHandle,
    stage_thinking: Option<bool>,
) -> Option<LlmClient> {
    let config = SkillAgentConfig::load(app);
    let assignment = load_role_assignment(app);

    // 构建客户端时套用思考模式覆盖：克隆 provider 配置、改 enable_thinking，
    // 只影响本次 agent 的 client，不触碰 llm.providers 存储（主对话设置不受影响）。
    let thinking = stage_thinking.or(config.enable_thinking);
    let build_client = |p: &LlmProviderConfig| {
        let mut cfg = p.clone();
        if let Some(v) = thinking {
            cfg.enable_thinking = v;
        }
        build_llm_client_from_provider(app, &cfg)
    };

    // 1. 显式指定的 agent provider
    if let Some(ref id) = config.provider_id {
        let providers = load_providers(app);
        if let Some(p) = providers.iter().find(|p| &p.id == id && p.is_usable()) {
            tracing::info!("Skill Agent 使用专用 LLM: {} ({})", p.label, p.id);
            return build_client(p);
        }
    }

    // 2. Fallback：聊天主 LLM
    if let Some(ref id) = assignment.chat_provider_id {
        let providers = load_providers(app);
        if let Some(p) = providers.iter().find(|p| &p.id == id && p.is_usable()) {
            tracing::info!("Skill Agent fallback 到聊天 LLM: {} ({})", p.label, p.id);
            return build_client(p);
        }
    }

    // 3. 任何可用的 provider
    let providers = load_providers(app);
    if let Some(p) = providers.iter().find(|p| p.is_usable()) {
        tracing::info!("Skill Agent 使用第一个可用 LLM: {} ({})", p.label, p.id);
        return build_client(p);
    }

    tracing::warn!("Skill Agent 未找到可用 LLM");
    None
}

/// 会话开始时按 `provider/model` 缓存的窗口值。它只在开新会话时解析一次，
/// 缓存是为了换 provider 后不重复问、同一 provider 下不重复查。
fn window_cache() -> &'static std::sync::Mutex<std::collections::HashMap<String, usize>> {
    static CACHE: std::sync::OnceLock<std::sync::Mutex<std::collections::HashMap<String, usize>>> =
        std::sync::OnceLock::new();
    CACHE.get_or_init(|| std::sync::Mutex::new(std::collections::HashMap::new()))
}

/// 探窗口的超时。**这条必须有**：有的 provider 的 `list_models` 会真的发 HTTP 请求
/// （kimi_code 就是），端点不通时会把用户**第一条消息**卡到 LLM 客户端的整体超时。
/// 探不到就用默认值 —— 绝不为了一个预算数字拖住对话。
const WINDOW_PROBE_TIMEOUT: std::time::Duration = std::time::Duration::from_secs(5);

/// 这一轮算上下文预算用的**模型窗口**（token）。
///
/// 优先读 provider 自报的 `context_length`（`/models`）；读不到就用
/// [`stage::DEFAULT_CONTEXT_WINDOW`]（DeepSeek 的 1M）。
///
/// 为什么兜底取"最大值"而不是保守的小值：本项目的默认 provider 就是 DeepSeek，
/// 保守值会让正常会话过早丢记忆；真超限时 `core::run_item_tools` 会主动报错兜住。
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
        // 拉不到就当没报：默认 provider 的 `list_models` 本来就返回空表
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
        .unwrap_or(stage::DEFAULT_CONTEXT_WINDOW);
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
