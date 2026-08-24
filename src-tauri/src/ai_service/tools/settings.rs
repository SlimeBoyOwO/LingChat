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
    /// "kimi"（Kimi Code 同款 /v1/search，body 为 text_query）
    /// "bocha"（BoCha 博查 https://api.bochaai.com/v1/web-search）
    /// "deepseek"（DeepSeek Responses API，服务端内置 web_search）
    /// "tavily"（Tavily https://api.tavily.com/search，body 为 query）
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
        self.enabled && !self.api_key.trim().is_empty()
    }
}

/// 工具配置根。
#[derive(Clone, Debug, Default, Deserialize, Serialize)]
#[serde(default)]
pub struct ToolSettings {
    pub web_search: WebSearchSettings,
    /// 分组开关：组名（见 `TOOL_GROUPS`）→ 是否启用，缺省关闭。
    pub groups: std::collections::HashMap<String, bool>,
    /// 命令执行：免审批直接运行 shell（危险，仅在信任当前角色/模型时开启）。
    pub command_auto_approve: bool,
    /// 命令执行：识别到删除操作时免审批继续执行（危险；缺省 false）。
    pub command_delete_auto_approve: bool,
    /// 删除文件：免审批直接删除（危险；缺省 false，旧配置升级后仍会弹窗）。
    pub file_delete_auto_approve: bool,
    /// 文件操作：允许访问文件沙箱（默认 data/）之外的任意路径。
    pub file_ops_allow_any_path: bool,
}

impl ToolSettings {
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
        let text = toml::to_string_pretty(self).context("序列化工具配置失败")?;
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
