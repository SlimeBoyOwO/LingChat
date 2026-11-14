//! 聊天工具的用户配置（与权限矩阵分离），持久化在 `data/tool_settings.toml`。
//!
//! 权限矩阵（`tool_permissions.toml`）决定"哪些工具允许下发给模型"，
//! 这里的配置决定"工具自身如何工作"（API Key、代理等）。
//! `SharedToolSettings` 在 AppState 与工具实例间共享，保存后立即生效。

use std::collections::{HashMap, HashSet};
use std::fs;
use std::path::Path;
use std::sync::{Arc, RwLock};

use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};

use super::permissions::{DEFAULT_ROLE_GROUP, ToolPermissionConfig};

pub const SETTINGS_FILE_NAME: &str = "tool_settings.toml";
pub const DEFAULT_TOOL_CALL_ROUND_LIMIT: u32 = 8;
pub const MIN_TOOL_CALL_ROUND_LIMIT: u32 = 1;
pub const MAX_TOOL_CALL_ROUND_LIMIT: u32 = 64;
/// Tool RAG 激活缓存的默认容量（只算被检索激活的普通工具，
/// tool_search 与 web_search 是常驻工具，不占这个额度）。
pub const DEFAULT_TOOL_CACHE_CAPACITY: usize = 5;
pub const MIN_TOOL_CACHE_CAPACITY: usize = 1;
pub const MAX_TOOL_CACHE_CAPACITY: usize = 20;
pub const DEFAULT_MEDIA_MAX_FILE_MB: u32 = 100;
pub const MAX_MEDIA_MAX_FILE_MB: u32 = 100;
pub const DEFAULT_MEDIA_IMAGE_MAX_EDGE: u32 = 2000;
pub const DEFAULT_MEDIA_JPEG_QUALITY: u8 = 85;
pub const DEFAULT_MEDIA_OUTPUT_TOKENS: u32 = 1024;

/// 云端向量化单次调用的默认超时。
pub const DEFAULT_EMBEDDING_TIMEOUT_SECS: u64 = 30;
pub const MIN_EMBEDDING_TIMEOUT_SECS: u64 = 5;
pub const MAX_EMBEDDING_TIMEOUT_SECS: u64 = 300;

/// 文本嵌入（向量化）后端配置。
///
/// 独立于「大模型管理」里的 provider：常见组合是对话用 A 家、向量化用 B 家，
/// 复用同一份配置会让这种组合无解，代价只是用户多填一次密钥。
#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(default)]
pub struct EmbeddingSettings {
    /// 本地 ONNX ↔ 云端的总开关，即设置页上的「使用云端向量化模型」。
    pub cloud_enabled: bool,
    /// 协议：`openai`（OpenAI 兼容，含第三方 base_url）/ `gemini`。
    pub provider: String,
    /// 兼容端点；留空走官方地址。
    pub base_url: String,
    pub api_key: String,
    /// 模型名，如 `text-embedding-3-small`、`bge-m3`。
    pub model: String,
    /// 单次调用超时（秒）。
    pub timeout_secs: u64,
}

impl Default for EmbeddingSettings {
    fn default() -> Self {
        Self {
            cloud_enabled: false,
            provider: "openai".to_string(),
            base_url: String::new(),
            api_key: String::new(),
            model: String::new(),
            timeout_secs: DEFAULT_EMBEDDING_TIMEOUT_SECS,
        }
    }
}

impl EmbeddingSettings {
    pub fn normalize(&mut self) {
        self.provider = self.provider.trim().to_lowercase();
        if self.provider != "gemini" {
            self.provider = "openai".to_string();
        }
        self.base_url = self.base_url.trim().to_string();
        self.api_key = self.api_key.trim().to_string();
        self.model = self.model.trim().to_string();
        self.timeout_secs = self
            .timeout_secs
            .clamp(MIN_EMBEDDING_TIMEOUT_SECS, MAX_EMBEDDING_TIMEOUT_SECS);
    }

    /// 是否达到可用状态。
    ///
    /// 自定义 base_url 时允许密钥为空（本地自托管端点），官方端点必须带密钥。
    pub fn is_ready(&self) -> bool {
        self.cloud_enabled
            && !self.model.is_empty()
            && (!self.base_url.is_empty() || !self.api_key.is_empty())
    }

    /// 决定向量空间与建库维度的那部分配置。
    ///
    /// 只有它变化才需要重建索引 / 后台预热；单纯改超时不应消耗额度。
    pub fn vector_identity(&self) -> (bool, String, String, String) {
        (
            self.cloud_enabled,
            self.provider.clone(),
            self.base_url.clone(),
            self.model.clone(),
        )
    }

    /// 选路与熔断的判定依据。比 [`Self::vector_identity`] 多含密钥：
    /// 只改了密钥也要重建客户端并解除熔断，但不需要重新向量化。
    pub fn routing_identity(&self) -> (bool, String, String, String, String) {
        (
            self.cloud_enabled,
            self.provider.clone(),
            self.base_url.clone(),
            self.model.clone(),
            self.api_key.clone(),
        )
    }
}

/// 工具分组 → 组内工具注册名。
/// 设置页按组开关，权限同步时组内工具一起放开/收回。
/// web_search 不在此列：它有独立的 enabled + 配置就绪判断。
pub const TOOL_GROUPS: &[(&str, &[&str])] = &[
    (
        "schedule",
        &[
            "schedule_get_all",
            "schedule_add_todo",
            "schedule_update_todo",
            "schedule_delete_todo",
        ],
    ),
    (
        "memory",
        &[
            "memory_get_current",
            "memory_get_notes",
            "memory_add_note",
            "memory_update_note",
            "memory_delete_note",
        ],
    ),
    (
        "character",
        &[
            "character_list",
            "character_switch",
            "character_get_clothes",
            "character_set_clothes",
        ],
    ),
    (
        "scene",
        &["scene_list", "scene_switch", "set_background_effect"],
    ),
    (
        "status",
        &["status_get_current", "status_get_scene", "query_weather"],
    ),
    ("clock", &["get_current_time"]),
    ("skills", &["list_skills", "read_skill"]),
    ("media", &["ReadMediaFile"]),
    (
        "file_ops",
        &[
            "list_files",
            "read_file",
            "write_file",
            "delete_file",
            "edit_file",
            "search_files",
            "grep_files",
            "glob",
            "grep",
        ],
    ),
    ("command", &["execute_command"]),
];

/// 插件工具白名单在 `enabled_tools` 中的保留键。
pub const PLUGIN_TOOL_GROUP: &str = "plugin";

/// 网页搜索工具配置。
#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(default)]
pub struct WebSearchSettings {
    /// 总开关：关闭时工具不下发给模型，执行也会被拒绝。
    pub enabled: bool,
    /// 搜索服务提供商：
    /// "kimi"（Kimi Code 同款 /v1/search；API Key 为空时可复用当前官方 Kimi Code 对话凭据）
    /// "bocha"（BoCha 博查 https://api.bochaai.com/v1/web-search）
    /// "deepseek"（DeepSeek 网页搜索 https://api.deepseek.com/v1/tools/search）
    /// "tavily"（Tavily https://api.tavily.com/search，body 为 query）
    /// "codex"（ChatGPT Codex alpha/search，复用 Codex OAuth，无需 API Key）
    /// "keenable"（Keenable 联网搜索；API Key 为空时走免费公共端点）
    /// "custom"（用户配置的 Kimi /search 兼容端点）
    pub provider: String,
    /// API Key（Bearer 认证）。
    pub api_key: String,
    /// 搜索端点（bocha/deepseek/kimi 固定走官方端点，不读此字段；仅 custom 模式需要）。
    pub base_url: String,
    /// 是否通过本地 HTTP 代理（如 v2rayN）访问搜索端点。
    pub proxy_enabled: bool,
    /// 代理地址，v2rayN（sing-box）默认本地端口 10808。
    pub proxy_addr: String,
    /// 返回给模型的最大结果条数。
    pub max_results: usize,
    /// 为 true 时喂给模型的搜索结果不含网址/来源名，并指示模型
    /// 把信息自然融入回答，避免在对话中念出搜索结果列表。
    pub hide_search_results: bool,
}

impl Default for WebSearchSettings {
    fn default() -> Self {
        Self {
            enabled: false,
            provider: "keenable".to_string(),
            api_key: String::new(),
            base_url: "https://api.kimi.com/coding/v1/search".to_string(),
            proxy_enabled: false,
            proxy_addr: "http://127.0.0.1:10808".to_string(),
            max_results: 8,
            hide_search_results: false,
        }
    }
}

impl WebSearchSettings {
    /// 配置是否达到可下发给模型的就绪状态。
    pub fn is_ready(&self) -> bool {
        self.enabled
            && (self.provider.eq_ignore_ascii_case("codex")
                || self.provider.eq_ignore_ascii_case("kimi")
                || self.provider.eq_ignore_ascii_case("keenable")
                || !self.api_key.trim().is_empty())
    }
}

/// 图片/视频识别工具配置。识别请求复用“大模型管理”中指定的视觉模型。
#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(default)]
pub struct MediaFileSettings {
    pub image_enabled: bool,
    pub video_enabled: bool,
    pub max_file_mb: u32,
    pub image_max_edge: u32,
    pub jpeg_quality: u8,
    pub max_output_tokens: u32,
    pub default_prompt: String,
}

impl Default for MediaFileSettings {
    fn default() -> Self {
        Self {
            image_enabled: true,
            video_enabled: true,
            max_file_mb: DEFAULT_MEDIA_MAX_FILE_MB,
            image_max_edge: DEFAULT_MEDIA_IMAGE_MAX_EDGE,
            jpeg_quality: DEFAULT_MEDIA_JPEG_QUALITY,
            max_output_tokens: DEFAULT_MEDIA_OUTPUT_TOKENS,
            default_prompt: "请详细识别并描述这个媒体文件的内容；如果其中包含文字、界面、人物、物体、动作或时间顺序，请准确说明。".to_string(),
        }
    }
}

impl MediaFileSettings {
    pub fn normalize(&mut self) {
        self.max_file_mb = self.max_file_mb.clamp(1, MAX_MEDIA_MAX_FILE_MB);
        self.image_max_edge = self.image_max_edge.clamp(512, 4096);
        self.jpeg_quality = self.jpeg_quality.clamp(50, 95);
        self.max_output_tokens = self.max_output_tokens.clamp(128, 4096);
        if self.default_prompt.trim().is_empty() {
            self.default_prompt = Self::default().default_prompt;
        }
    }
}

/// 主聊天每轮发言前的工具推荐方式。
#[derive(Clone, Copy, Debug, Default, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum ToolRecommendMode {
    /// 不推荐，模型自己决定要不要检索或调用工具。
    Off,
    /// 上帝 Agent 看一遍最近的对话再挑（每轮多一次小模型调用）。
    Agent,
    /// 拿玩家这句话静默检索一次工具，过阈值的才推荐（不额外调用模型）。
    #[default]
    Rag,
}

/// 主聊天工具的统一审批策略。只读工具始终可以直接运行。
#[derive(Clone, Copy, Debug, Default, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum ToolAccessMode {
    /// 写入、编辑、删除文件和执行命令前逐次询问。
    #[default]
    Manual,
    /// 自动批准普通修改和命令；删除文件或删除命令仍需确认。
    AutoApprove,
    /// 不再询问，并允许文件工具访问沙箱外路径。
    FullAccess,
}

/// 工具配置根。
#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(default)]
pub struct ToolSettings {
    pub web_search: WebSearchSettings,
    pub media_file: MediaFileSettings,
    /// 笔记语义检索与 Tool RAG 的向量化后端。
    pub embedding: EmbeddingSettings,
    /// 分组开关：组名（见 `TOOL_GROUPS`）→ 是否启用，缺省关闭。
    pub groups: HashMap<String, bool>,
    /// 工具级白名单：组名 → 组内已勾选的工具名。
    /// 缺失某个组时沿用 `groups` 的整组语义，保证旧配置升级后工具不会被全部关掉。
    pub enabled_tools: HashMap<String, Vec<String>>,
    /// 文件修改与命令执行使用的统一审批模式。
    pub access_mode: ToolAccessMode,
    /// 主聊天单次回复可连续执行工具的最大轮数；每轮可能包含多个工具调用。
    pub max_tool_rounds: u32,
    /// 工具按需检索（Tool RAG）：只下发常驻工具 + 检索激活的工具，
    /// 而不是把全部已授权工具定义随每次请求下发。关闭即回到全量下发。
    pub tool_rag: bool,
    /// 每轮用户发言前怎么推荐工具：不推荐 / 上帝 Agent 挑 / 静默检索。
    /// 两种推荐都只走临时记忆通道，不写台词表也不进记忆。
    pub tool_recommend_mode: ToolRecommendMode,
    /// Tool RAG 同时记住多少个检索激活的工具。常驻的 tool_search 与 web_search
    /// 不占这个额度；缓存满了按最久未激活淘汰。
    pub tool_cache_capacity: usize,
    /// 以下四项仅用于读取旧版配置。保存后会迁移成 `access_mode`。
    #[serde(default, skip_serializing)]
    pub command_auto_approve: bool,
    #[serde(default, skip_serializing)]
    pub command_delete_auto_approve: bool,
    #[serde(default, skip_serializing)]
    pub file_delete_auto_approve: bool,
    #[serde(default, skip_serializing)]
    pub file_ops_allow_any_path: bool,
    /// 未保存过新模式的旧配置继续保持原审批行为，避免升级时静默扩大权限。
    #[serde(skip)]
    legacy_approval_behavior: bool,
}

impl Default for ToolSettings {
    fn default() -> Self {
        Self {
            web_search: WebSearchSettings::default(),
            media_file: MediaFileSettings::default(),
            embedding: EmbeddingSettings::default(),
            groups: HashMap::new(),
            enabled_tools: HashMap::new(),
            access_mode: ToolAccessMode::default(),
            max_tool_rounds: DEFAULT_TOOL_CALL_ROUND_LIMIT,
            tool_rag: true,
            tool_recommend_mode: ToolRecommendMode::default(),
            tool_cache_capacity: DEFAULT_TOOL_CACHE_CAPACITY,
            command_auto_approve: false,
            command_delete_auto_approve: false,
            file_delete_auto_approve: false,
            file_ops_allow_any_path: false,
            legacy_approval_behavior: false,
        }
    }
}

impl ToolSettings {
    pub fn normalize(&mut self) {
        self.media_file.normalize();
        self.embedding.normalize();
        self.max_tool_rounds = self
            .max_tool_rounds
            .clamp(MIN_TOOL_CALL_ROUND_LIMIT, MAX_TOOL_CALL_ROUND_LIMIT);
        self.tool_cache_capacity = self
            .tool_cache_capacity
            .clamp(MIN_TOOL_CACHE_CAPACITY, MAX_TOOL_CACHE_CAPACITY);
        self.normalize_enabled_tools();
    }

    /// 白名单去重并丢弃不属于该组的工具名；有白名单的组同步回 `groups`，
    /// 避免「组开着但工具全不勾」这种两份数据互相矛盾的状态。
    fn normalize_enabled_tools(&mut self) {
        let raw = std::mem::take(&mut self.enabled_tools);
        let mut cleaned = HashMap::new();
        for (group, tools) in TOOL_GROUPS {
            let Some(list) = raw.get(*group) else {
                continue;
            };
            let filtered = dedup_tools(list, Some(tools));
            self.groups
                .insert((*group).to_string(), !filtered.is_empty());
            cleaned.insert((*group).to_string(), filtered);
        }
        if let Some(list) = raw.get(PLUGIN_TOOL_GROUP) {
            cleaned.insert(PLUGIN_TOOL_GROUP.to_string(), dedup_tools(list, None));
        }
        self.enabled_tools = cleaned;
    }

    /// 为返回给前端的配置补齐白名单：缺失的组按整组开关展开，
    /// 只影响内存视图，不落盘，旧配置行为不变。
    pub fn fill_enabled_tools_from_groups(&mut self) {
        for (group, tools) in TOOL_GROUPS {
            if self.enabled_tools.contains_key(*group) {
                continue;
            }
            let on = self.groups.get(*group).copied().unwrap_or(false);
            let list = if on {
                tools.iter().map(|tool| (*tool).to_string()).collect()
            } else {
                Vec::new()
            };
            self.enabled_tools.insert((*group).to_string(), list);
        }
    }

    /// 判断工具是否属于内置聊天工具（含常驻的 web_search / tool_search），
    /// 用于把插件工具从内置清单里区分出来。
    pub fn is_builtin_chat_tool(name: &str) -> bool {
        matches!(name, "web_search" | "tool_search")
            || TOOL_GROUPS.iter().any(|(_, tools)| tools.contains(&name))
    }

    /// 插件工具是否被勾选。缺省（没保存过插件白名单）视为全部启用，
    /// 升级后插件行为保持不变。
    pub fn plugin_tool_enabled(&self, tool: &str) -> bool {
        self.enabled_tools
            .get(PLUGIN_TOOL_GROUP)
            .is_none_or(|list| list.iter().any(|name| name == tool))
    }

    /// 已归一化的激活缓存容量。
    pub fn cache_capacity(&self) -> usize {
        self.tool_cache_capacity
            .clamp(MIN_TOOL_CACHE_CAPACITY, MAX_TOOL_CACHE_CAPACITY)
    }

    pub fn tool_round_limit(&self) -> usize {
        self.max_tool_rounds
            .clamp(MIN_TOOL_CALL_ROUND_LIMIT, MAX_TOOL_CALL_ROUND_LIMIT) as usize
    }

    pub fn allows_any_path(&self) -> bool {
        if cfg!(any(target_os = "android", target_os = "ios")) {
            return false;
        }
        if self.legacy_approval_behavior {
            self.file_ops_allow_any_path
        } else {
            self.access_mode == ToolAccessMode::FullAccess
        }
    }

    pub fn requires_file_change_approval(&self) -> bool {
        if self.legacy_approval_behavior {
            return false;
        }
        self.access_mode == ToolAccessMode::Manual
    }

    pub fn requires_file_delete_approval(&self) -> bool {
        if self.legacy_approval_behavior {
            return !self.file_delete_auto_approve;
        }
        self.access_mode != ToolAccessMode::FullAccess
    }

    pub fn requires_command_approval(&self, may_delete_files: bool) -> bool {
        if self.legacy_approval_behavior {
            return if may_delete_files {
                !self.command_delete_auto_approve
            } else {
                !self.command_auto_approve
            };
        }
        match self.access_mode {
            ToolAccessMode::Manual => true,
            ToolAccessMode::AutoApprove => may_delete_files,
            ToolAccessMode::FullAccess => false,
        }
    }

    /// 移动端没有可供应用稳定调用的桌面 shell，且 Android/iOS 的分区存储
    /// 不允许把“任意路径”理解为桌面文件系统访问。加载和保存时都收紧这些选项，
    /// 避免旧配置继续把不可执行的工具下发给模型。
    pub fn apply_platform_constraints(&mut self) {
        if cfg!(any(target_os = "android", target_os = "ios")) {
            self.groups.insert("command".to_string(), false);
            self.command_auto_approve = false;
            self.command_delete_auto_approve = false;
            self.file_ops_allow_any_path = false;
        }
    }

    pub fn group_supported_on_current_platform(group: &str) -> bool {
        !(cfg!(any(target_os = "android", target_os = "ios")) && group == "command")
    }

    /// 把用户配置同步到权限矩阵的 default 角色组。
    /// 实际可用工具 = 组开关打开 ∧ 该工具在白名单中；没有白名单的旧配置沿用整组语义。
    pub fn sync_to_permissions(&self, permissions: &mut ToolPermissionConfig) {
        permissions.set_tool_allowed_for_default_group("web_search", self.web_search.is_ready());
        for (group, tools) in TOOL_GROUPS {
            let group_on = Self::group_supported_on_current_platform(group)
                && self.groups.get(*group).copied().unwrap_or(false);
            let selected = self.enabled_tools.get(*group);
            for tool in *tools {
                let tool_on = selected.is_none_or(|list| list.iter().any(|name| name == tool));
                permissions.set_tool_allowed_for_default_group(tool, group_on && tool_on);
            }
        }
        Self::finalize_default_group(permissions);
    }

    /// 工具全部关闭时 `set_tool_allowed_for_default_group(false)` 只会清空列表，
    /// 留下 enabled=true + tools=[]，而场景组 all_tools 会把空列表回退成全量工具。
    /// 因此这里显式关闭 default 组并强制 all_tools=false。
    pub fn finalize_default_group(permissions: &mut ToolPermissionConfig) {
        let Some(group) = permissions.role_groups.get_mut(DEFAULT_ROLE_GROUP) else {
            return;
        };
        group.all_tools = false;
        if group.tools.is_empty() {
            group.enabled = false;
        }
    }
}

impl ToolSettings {
    /// 加载配置；文件不存在时写入一份默认配置。
    pub fn load_or_create(data_dir: &Path) -> Result<Self> {
        let path = data_dir.join(SETTINGS_FILE_NAME);
        if path.exists() {
            let text = fs::read_to_string(&path)
                .with_context(|| format!("读取工具配置失败: {}", path.display()))?;
            let mut settings: Self = toml::from_str(&text)
                .with_context(|| format!("解析工具配置失败: {}", path.display()))?;
            if !text.lines().any(|line| {
                line.trim_start()
                    .strip_prefix("access_mode")
                    .is_some_and(|tail| tail.trim_start().starts_with('='))
            }) {
                settings.legacy_approval_behavior = true;
                settings.access_mode = if settings.command_auto_approve
                    && settings.command_delete_auto_approve
                    && settings.file_delete_auto_approve
                    && settings.file_ops_allow_any_path
                {
                    ToolAccessMode::FullAccess
                } else if settings.command_auto_approve {
                    ToolAccessMode::AutoApprove
                } else {
                    ToolAccessMode::Manual
                };
            }
            settings.normalize();
            settings.apply_platform_constraints();
            return Ok(settings);
        }
        let mut settings = Self::default();
        settings.apply_platform_constraints();
        settings.save(data_dir)?;
        Ok(settings)
    }

    /// 原子写入 `data/tool_settings.toml`。
    pub fn save(&self, data_dir: &Path) -> Result<()> {
        let path = data_dir.join(SETTINGS_FILE_NAME);
        let mut normalized = self.clone();
        normalized.normalize();
        let text = toml::to_string_pretty(&normalized).context("序列化工具配置失败")?;
        super::atomic_replace(&path, text.as_bytes())
            .map_err(anyhow::Error::msg)
            .with_context(|| format!("保存工具配置失败: {}", path.display()))?;
        Ok(())
    }
}

/// 在线程间共享、可热更新的工具配置句柄。
#[derive(Clone)]
pub struct SharedToolSettings(Arc<RwLock<ToolSettings>>);

impl SharedToolSettings {
    pub fn new(settings: ToolSettings) -> Self {
        Self(Arc::new(RwLock::new(settings)))
    }

    /// 读取当前配置快照。
    pub fn get(&self) -> ToolSettings {
        self.0.read().expect("工具配置锁已中毒").clone()
    }

    /// 只取向量化配置。嵌入选路在每次检索时都会读一次，避免为此克隆整份工具配置。
    pub fn embedding(&self) -> EmbeddingSettings {
        self.0.read().expect("工具配置锁已中毒").embedding.clone()
    }

    /// 整体替换配置，立即对所有工具生效。
    pub fn update(&self, settings: ToolSettings) {
        *self.0.write().expect("工具配置锁已中毒") = settings;
    }
}

/// 去重；`allowed` 为 Some 时只保留其中的工具名。
fn dedup_tools(list: &[String], allowed: Option<&[&str]>) -> Vec<String> {
    let mut seen = HashSet::new();
    list.iter()
        .filter(|tool| allowed.is_none_or(|names| names.contains(&tool.as_str())))
        .filter(|tool| seen.insert(tool.as_str()))
        .cloned()
        .collect()
}

#[cfg(test)]
mod tests {
    use std::collections::HashSet;

    use super::{
        DEFAULT_ROLE_GROUP, EmbeddingSettings, PLUGIN_TOOL_GROUP, TOOL_GROUPS,
        ToolPermissionConfig, ToolSettings,
    };
    use crate::ai_service::message_system::generator::GeneratorSource;
    use crate::ai_service::tools::permissions::{
        GeneratorSourceKey, GroupPermission, ToolPermission,
    };

    /// 构造只含 scene_admin（all_tools=true）的权限矩阵，用来复现提权路径。
    fn permission_config() -> ToolPermissionConfig {
        let mut config = ToolPermissionConfig::default();
        config
            .scene_mapping
            .insert(GeneratorSourceKey::UserChat, "scene_admin".into());
        config.scene_groups.insert(
            "scene_admin".into(),
            ToolPermission {
                enabled: true,
                tools: HashSet::new(),
                all_tools: true,
            },
        );
        config
    }

    fn synced_allowed(settings: &ToolSettings) -> HashSet<String> {
        let mut config = permission_config();
        settings.sync_to_permissions(&mut config);
        config
            .role_groups
            .get_mut(DEFAULT_ROLE_GROUP)
            .expect("default 组应存在")
            .roles
            .insert("alice".into());

        let mut all_names = HashSet::new();
        for (_, tools) in TOOL_GROUPS {
            all_names.extend(tools.iter().map(|tool| (*tool).to_string()));
        }
        all_names.insert("web_search".into());
        config.allowed_tools(GeneratorSource::UserChat, Some("alice"), &all_names)
    }

    #[test]
    fn legacy_config_without_whitelist_follows_group_switch() {
        let mut settings = ToolSettings::default();
        settings.groups.insert("schedule".into(), true);

        let allowed = synced_allowed(&settings);
        assert!(allowed.contains("schedule_get_all"));
        assert!(allowed.contains("schedule_delete_todo"));
        assert!(!allowed.contains("memory_add_note"), "没开的组不该下发");
    }

    #[test]
    fn whitelist_limits_tools_within_enabled_group() {
        let mut settings = ToolSettings::default();
        settings.groups.insert("schedule".into(), true);
        settings.enabled_tools.insert(
            "schedule".into(),
            vec!["schedule_get_all".into(), "schedule_get_all".into()],
        );
        settings.normalize();

        let allowed = synced_allowed(&settings);
        assert_eq!(allowed.len(), 1);
        assert!(allowed.contains("schedule_get_all"));
    }

    #[test]
    fn all_groups_off_disables_default_group_instead_of_falling_back() {
        let settings = ToolSettings::default();
        let mut config = permission_config();
        settings.sync_to_permissions(&mut config);

        let group = &config.role_groups[DEFAULT_ROLE_GROUP];
        assert!(!group.enabled, "空工具集必须显式关闭，不能靠组开着回退全量");
        assert!(group.tools.is_empty());
        assert!(!group.all_tools);

        config
            .role_groups
            .get_mut(DEFAULT_ROLE_GROUP)
            .unwrap()
            .roles
            .insert("alice".into());
        let mut all_names = HashSet::new();
        all_names.insert("schedule_get_all".into());
        let allowed = config.allowed_tools(GeneratorSource::UserChat, Some("alice"), &all_names);
        assert!(allowed.is_empty(), "全部关闭后不能拿到任何工具");
    }

    #[test]
    fn finalize_forces_all_tools_off() {
        let mut config = permission_config();
        config.role_groups.insert(
            DEFAULT_ROLE_GROUP.into(),
            GroupPermission {
                enabled: true,
                tools: HashSet::from(["schedule_get_all".to_string()]),
                all_tools: true,
                roles: HashSet::new(),
            },
        );
        ToolSettings::finalize_default_group(&mut config);
        let group = &config.role_groups[DEFAULT_ROLE_GROUP];
        assert!(group.enabled, "还有工具时不该关组");
        assert!(!group.all_tools);
    }

    #[test]
    fn normalize_whitelist_filters_and_syncs_groups() {
        let mut settings = ToolSettings::default();
        settings.enabled_tools.insert(
            "schedule".into(),
            vec![
                "schedule_get_all".into(),
                "schedule_get_all".into(),
                "bogus".into(),
            ],
        );
        settings.enabled_tools.insert("memory".into(), Vec::new());
        settings
            .enabled_tools
            .insert("unknown".into(), vec!["x".into()]);
        settings
            .enabled_tools
            .insert(PLUGIN_TOOL_GROUP.into(), vec!["p1".into(), "p1".into()]);
        settings.normalize();

        assert_eq!(
            settings.enabled_tools["schedule"],
            vec!["schedule_get_all".to_string()]
        );
        assert!(settings.enabled_tools["memory"].is_empty());
        assert!(!settings.enabled_tools.contains_key("unknown"));
        assert_eq!(
            settings.enabled_tools[PLUGIN_TOOL_GROUP],
            vec!["p1".to_string()]
        );
        assert!(settings.groups["schedule"]);
        assert!(!settings.groups["memory"], "清空白名单等于关组");
    }

    #[test]
    fn plugin_whitelist_defaults_to_enabled() {
        let settings = ToolSettings::default();
        assert!(settings.plugin_tool_enabled("whatever"));

        let mut settings = ToolSettings::default();
        settings
            .enabled_tools
            .insert(PLUGIN_TOOL_GROUP.into(), vec!["p1".into()]);
        assert!(settings.plugin_tool_enabled("p1"));
        assert!(!settings.plugin_tool_enabled("p2"));
    }

    #[test]
    fn fill_whitelist_expands_group_switch() {
        let mut settings = ToolSettings::default();
        settings.groups.insert("clock".into(), true);
        settings.fill_enabled_tools_from_groups();

        assert_eq!(
            settings.enabled_tools["clock"],
            vec!["get_current_time".to_string()]
        );
        assert!(settings.enabled_tools["scene"].is_empty());
    }

    #[test]
    fn builtin_tool_detection() {
        assert!(ToolSettings::is_builtin_chat_tool("execute_command"));
        assert!(ToolSettings::is_builtin_chat_tool("tool_search"));
        assert!(ToolSettings::is_builtin_chat_tool("web_search"));
        assert!(!ToolSettings::is_builtin_chat_tool("plugin_ping"));
    }

    #[test]
    fn embedding_ready_requires_model_and_credential() {
        // 默认即本地态：没开云端、没配模型。
        let default = EmbeddingSettings::default();
        assert!(!default.is_ready());

        let switched = EmbeddingSettings {
            cloud_enabled: true,
            ..Default::default()
        };
        assert!(!switched.is_ready(), "只有开关、没有模型时不可用");

        let named = EmbeddingSettings {
            model: "text-embedding-3-small".into(),
            ..switched.clone()
        };
        assert!(!named.is_ready(), "官方端点必须带密钥");

        let keyed = EmbeddingSettings {
            api_key: "sk-test".into(),
            ..named.clone()
        };
        assert!(keyed.is_ready());

        // 自托管端点允许不带密钥。
        let self_hosted = EmbeddingSettings {
            base_url: "http://127.0.0.1:8080/v1".into(),
            ..named
        };
        assert!(self_hosted.is_ready());
    }

    #[test]
    fn vector_identity_ignores_timeout() {
        let config = EmbeddingSettings {
            cloud_enabled: true,
            model: "bge-m3".into(),
            ..Default::default()
        };
        let before = config.vector_identity();

        let changed_timeout = EmbeddingSettings {
            timeout_secs: 120,
            ..config.clone()
        };
        assert_eq!(
            before,
            changed_timeout.vector_identity(),
            "改超时不该触发重新向量化"
        );

        let changed_model = EmbeddingSettings {
            model: "bge-large".into(),
            ..config.clone()
        };
        assert_ne!(
            before,
            changed_model.vector_identity(),
            "换模型必须重新向量化"
        );
    }

    #[test]
    fn provider_normalizes_to_known_values() {
        let mut gemini = EmbeddingSettings {
            provider: "  Gemini ".into(),
            ..Default::default()
        };
        gemini.normalize();
        assert_eq!(gemini.provider, "gemini");

        let mut unknown = EmbeddingSettings {
            provider: "unknown".into(),
            ..Default::default()
        };
        unknown.normalize();
        assert_eq!(unknown.provider, "openai");
    }
}
