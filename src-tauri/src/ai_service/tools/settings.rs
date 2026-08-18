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
    /// 为 true 时使用「模型 API 内置联网」：复用聊天模型的 API（Moonshot/Kimi），
    /// 由服务端执行 $web_search，无需单独的搜索 API Key；
    /// 为 false 时使用独立搜索端点 + api_key。
    pub use_builtin: bool,
    /// 独立端点模式的搜索服务提供商：
    /// "kimi"（Kimi Code 同款 /v1/search，body 为 text_query）
    /// "bocha"（BoCha 博查 https://api.bochaai.com/v1/web-search）
    /// 仅在 use_builtin = false 时生效。
    pub provider: String,
    /// API Key（Bearer 认证，仅 use_builtin = false 时需要）。
    pub api_key: String,
    /// 搜索端点（仅 use_builtin = false 时使用）。
    pub base_url: String,
    /// 是否通过本地 HTTP 代理（如 v2rayN）访问搜索端点。
    pub proxy_enabled: bool,
    /// 代理地址，v2rayN（sing-box）默认本地端口 10808。
    pub proxy_addr: String,
    /// 返回给模型的最大结果条数（仅独立端点模式）。
    pub max_results: usize,
    /// 为 true 时喂给模型的搜索结果不含网址/来源名，并指示模型
    /// 把信息自然融入回答，避免在对话中念出搜索结果列表。
    pub hide_search_results: bool,
}

impl Default for WebSearchSettings {
    fn default() -> Self {
        Self {
            enabled: false,
            use_builtin: true,
            provider: "kimi".to_string(),
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
        self.enabled && (self.use_builtin || !self.api_key.trim().is_empty())
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
#[derive(Clone, Debug, Default, Deserialize, Serialize)]
#[serde(default)]
pub struct ToolSettings {
    pub web_search: WebSearchSettings,
    /// 分组开关：组名（见 `TOOL_GROUPS`）→ 是否启用，缺省关闭。
    pub groups: std::collections::HashMap<String, bool>,
    /// 文件修改与命令执行使用的统一审批模式。
    pub access_mode: ToolAccessMode,
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

impl ToolSettings {
    pub fn allows_any_path(&self) -> bool {
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

    /// 把用户配置同步到权限矩阵的 default 角色组。
    pub fn sync_to_permissions(&self, permissions: &mut ToolPermissionConfig) {
        permissions.set_tool_allowed_for_default_group("web_search", self.web_search.is_ready());
        for (group, tools) in TOOL_GROUPS {
            let enabled = self.groups.get(*group).copied().unwrap_or(false);
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
            return Ok(settings);
        }
        let settings = Self::default();
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

#[cfg(test)]
mod tests {
    use super::*;

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

    #[test]
    fn auto_approve_keeps_destructive_actions_guarded() {
        let settings = ToolSettings {
            access_mode: ToolAccessMode::AutoApprove,
            ..ToolSettings::default()
        };
        assert!(!settings.requires_file_change_approval());
        assert!(!settings.requires_command_approval(false));
        assert!(settings.requires_command_approval(true));
        assert!(settings.requires_file_delete_approval());
        assert!(!settings.allows_any_path());
    }
}
