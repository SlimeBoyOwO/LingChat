//! 聊天工具的用户配置（与权限矩阵分离），持久化在 `data/tool_settings.toml`。
//!
//! 权限矩阵（`tool_permissions.toml`）决定"哪些工具允许下发给模型"，
//! 这里的配置决定"工具自身如何工作"（API Key、代理等）。
//! `SharedToolSettings` 在 AppState 与工具实例间共享，保存后立即生效。

use std::fs;
use std::path::Path;
use std::sync::{Arc, RwLock};

use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};

use super::permissions::ToolPermissionConfig;

pub const SETTINGS_FILE_NAME: &str = "tool_settings.toml";
pub const DEFAULT_TOOL_CALL_ROUND_LIMIT: u32 = 8;
pub const MIN_TOOL_CALL_ROUND_LIMIT: u32 = 1;
pub const MAX_TOOL_CALL_ROUND_LIMIT: u32 = 64;
pub const DEFAULT_MEDIA_MAX_FILE_MB: u32 = 100;
pub const MAX_MEDIA_MAX_FILE_MB: u32 = 100;
pub const DEFAULT_MEDIA_IMAGE_MAX_EDGE: u32 = 2000;
pub const DEFAULT_MEDIA_JPEG_QUALITY: u8 = 85;
pub const DEFAULT_MEDIA_OUTPUT_TOKENS: u32 = 1024;

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
    ("character", &["character_list", "character_switch"]),
    ("scene", &["scene_list", "scene_switch"]),
    ("status", &["status_get_current", "status_get_scene"]),
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

/// 网页搜索工具配置。
#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(default)]
pub struct WebSearchSettings {
    /// 总开关：关闭时工具不下发给模型，执行也会被拒绝。
    pub enabled: bool,
    /// 搜索服务提供商：
    /// "kimi"（Kimi Code 同款 /v1/search；API Key 为空时可复用当前官方 Kimi Code 对话凭据）
    /// "bocha"（BoCha 博查 https://api.bochaai.com/v1/web-search）
    /// "deepseek"（DeepSeek Responses API，服务端内置 web_search）
    /// "tavily"（Tavily https://api.tavily.com/search，body 为 query）
    /// "codex"（ChatGPT Codex alpha/search，复用 Codex OAuth，无需 API Key）
    /// "custom"（用户配置的 Kimi /search 兼容端点）
    pub provider: String,
    /// DeepSeek Responses API 使用的模型（仅 provider = "deepseek" 时生效）。
    #[serde(default = "default_deepseek_model")]
    pub model: String,
    /// API Key（Bearer 认证）。
    pub api_key: String,
    /// 搜索端点（deepseek 固定走官方端点，不读此字段；仅 custom 模式需要）。
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

/// DeepSeek Responses API 的默认模型（旧配置缺省该字段时使用）。
fn default_deepseek_model() -> String {
    "deepseek-v4-flash".to_string()
}

impl Default for WebSearchSettings {
    fn default() -> Self {
        Self {
            enabled: false,
            provider: "kimi".to_string(),
            model: "deepseek-v4-flash".to_string(),
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
    /// 分组开关：组名（见 `TOOL_GROUPS`）→ 是否启用，缺省关闭。
    pub groups: std::collections::HashMap<String, bool>,
    /// 文件修改与命令执行使用的统一审批模式。
    pub access_mode: ToolAccessMode,
    /// 主聊天单次回复可连续执行工具的最大轮数；每轮可能包含多个工具调用。
    pub max_tool_rounds: u32,
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
            groups: std::collections::HashMap::new(),
            access_mode: ToolAccessMode::default(),
            max_tool_rounds: DEFAULT_TOOL_CALL_ROUND_LIMIT,
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
        self.max_tool_rounds = self
            .max_tool_rounds
            .clamp(MIN_TOOL_CALL_ROUND_LIMIT, MAX_TOOL_CALL_ROUND_LIMIT);
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
    pub fn sync_to_permissions(&self, permissions: &mut ToolPermissionConfig) {
        permissions.set_tool_allowed_for_default_group("web_search", self.web_search.is_ready());
        for (group, tools) in TOOL_GROUPS {
            let enabled = Self::group_supported_on_current_platform(group)
                && self.groups.get(*group).copied().unwrap_or(false);
            for tool in *tools {
                permissions.set_tool_allowed_for_default_group(tool, enabled);
            }
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

    /// 整体替换配置，立即对所有工具生效。
    pub fn update(&self, settings: ToolSettings) {
        *self.0.write().expect("工具配置锁已中毒") = settings;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn web_search_readiness_allows_provider_managed_credentials() {
        let mut web = WebSearchSettings::default();
        assert!(!web.is_ready());

        web.enabled = true;
        web.provider = "codex".into();
        web.api_key.clear();
        assert!(web.is_ready());

        web.provider = "kimi".into();
        assert!(web.is_ready());

        web.provider = "tavily".into();
        assert!(!web.is_ready());
        web.api_key = "configured".into();
        assert!(web.is_ready());

        web.enabled = false;
        assert!(!web.is_ready());
    }

    #[test]
    fn legacy_settings_keep_delete_confirmation_enabled() {
        let legacy = r#"
command_auto_approve = false
file_ops_allow_any_path = false
"#;
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(dir.path().join(SETTINGS_FILE_NAME), legacy).unwrap();
        let settings = ToolSettings::load_or_create(dir.path()).unwrap();
        assert!(settings.requires_file_delete_approval());
        assert!(settings.requires_command_approval(false));
        assert!(!settings.requires_file_change_approval());
    }

    #[test]
    fn save_can_replace_existing_settings_file() {
        let dir = tempfile::tempdir().unwrap();
        let mut settings = ToolSettings::default();
        settings.save(dir.path()).unwrap();
        settings.access_mode = ToolAccessMode::FullAccess;
        settings.save(dir.path()).unwrap();

        let loaded = ToolSettings::load_or_create(dir.path()).unwrap();
        assert_eq!(loaded.access_mode, ToolAccessMode::FullAccess);
        assert!(loaded.allows_any_path());
        assert!(!loaded.requires_file_delete_approval());
    }
}
