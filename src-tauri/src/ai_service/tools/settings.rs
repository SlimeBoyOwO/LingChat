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
    ("scene", &["scene_list", "scene_switch", "scene_return"]),
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

/// NovelAI 免费额度上限（Opus 订阅）：面积与步数同时满足才不计费。
/// 与 novelai_SDK 的 `FREE_MAX_PIXELS` / `FREE_MAX_STEPS` 对齐。
pub const NAI_FREE_MAX_PIXELS: u32 = 1_048_576;
pub const NAI_FREE_MAX_STEPS: u32 = 28;

/// 场景背景生成（NovelAI）配置。
///
/// 直连 NovelAI 官方图像端点（`image.novelai.net/ai/generate-image`），
/// 不依赖任何本地 Python 服务。默认参数刻意压在 Opus 免费额度内：
/// 1216×832 = 1,011,712 px ≤ 1,048,576，steps 23 ≤ 28。
#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(default)]
pub struct ImageGenSettings {
    /// 总开关：关闭时工具不下发给模型，执行也会被拒绝。
    pub enabled: bool,
    /// NovelAI 持久 API Token（Bearer 认证）。
    pub api_token: String,
    /// 图像端点基址，私有部署或代理转发时可改。
    pub base_url: String,
    /// 模型 ID。注意 API 实际接受的拼写：`nai-diffusion-4-curated-preview`
    /// 与 `nai-diffusion-furry-3`（另两种旧拼写会被服务端拒绝）。
    pub model: String,
    pub width: u32,
    pub height: u32,
    pub steps: u32,
    pub scale: f32,
    pub sampler: String,
    pub noise_schedule: String,
    /// 负面预设：strong / light / furry_focus / human_focus / none。
    pub uc_preset: String,
    /// 是否追加品质标（`, very aesthetic, masterpiece, no text`）。
    pub quality_toggle: bool,
    /// 风格前缀标签，拼在每次生成的提示词最前面，用于统一画风。
    pub style_prompt: String,
    /// 追加的负面提示词，拼在 UC 预设文本之后。
    /// 默认压掉人物：立绘会叠在背景上，背景本身不能烤进人。
    pub negative_prompt: String,
    /// 模型在对话中触发生成时，是否先弹确认框由用户拍板。
    /// **默认关闭** —— 确认框会把英文提示词摊在对话中间，很出戏。
    /// 关掉它不影响花费安全：那由 `free_tier_only` 负责。
    pub require_confirm: bool,
    /// 仅允许免费额度内的参数：超出面积/步数上限时直接拒绝，不发请求。
    /// 这是「不偷花钱」的最后一道保证，默认开启。
    pub free_tier_only: bool,
    /// 是否通过本地 HTTP 代理访问 NovelAI。
    pub proxy_enabled: bool,
    pub proxy_addr: String,
}

impl Default for ImageGenSettings {
    fn default() -> Self {
        Self {
            enabled: false,
            api_token: String::new(),
            base_url: "https://image.novelai.net".to_string(),
            model: "nai-diffusion-4-5-full".to_string(),
            // 3:2 横幅，1,011,712 px —— 贴着免费额度上限但不越线。
            width: 1216,
            height: 832,
            steps: 23,
            scale: 5.0,
            sampler: "k_euler_ancestral".to_string(),
            noise_schedule: "karras".to_string(),
            uc_preset: "light".to_string(),
            quality_toggle: true,
            style_prompt: "no humans, scenery, detailed background".to_string(),
            negative_prompt: "1girl, 1boy, person, character focus".to_string(),
            // 默认不弹确认框：对话里跳出一个满是英文 tag 的窗口很出戏。
            // 花费防护不靠这个开关，靠下面的 free_tier_only —— 超出免费额度会
            // 在发请求之前就被拒绝，所以「不问」也不会偷花钱。
            require_confirm: false,
            free_tier_only: true,
            proxy_enabled: false,
            proxy_addr: "http://127.0.0.1:10808".to_string(),
        }
    }
}

impl ImageGenSettings {
    /// 配置是否达到可下发给模型的就绪状态。
    pub fn is_ready(&self) -> bool {
        self.enabled && !self.api_token.trim().is_empty()
    }

    /// 校验尺寸/步数是否落在免费额度内。
    /// `free_tier_only` 开启时，越线即拒绝请求（返回人类可读原因）。
    pub fn check_free_tier(&self) -> Result<(), String> {
        if !self.free_tier_only {
            return Ok(());
        }
        let pixels = self.width.saturating_mul(self.height);
        if pixels > NAI_FREE_MAX_PIXELS {
            return Err(format!(
                "尺寸 {}×{} = {} 像素，超出免费额度上限 {} 像素，会开始扣 Anlas。\
                 请调小尺寸，或在设置中关闭「仅使用免费额度」。",
                self.width, self.height, pixels, NAI_FREE_MAX_PIXELS
            ));
        }
        if self.steps > NAI_FREE_MAX_STEPS {
            return Err(format!(
                "步数 {} 超出免费额度上限 {}，会开始扣 Anlas。\
                 请调低步数，或在设置中关闭「仅使用免费额度」。",
                self.steps, NAI_FREE_MAX_STEPS
            ));
        }
        Ok(())
    }
}

/// 工具配置根。
#[derive(Clone, Debug, Default, Deserialize, Serialize)]
#[serde(default)]
pub struct ToolSettings {
    pub web_search: WebSearchSettings,
    /// 场景背景生成（NovelAI 直连）。
    pub image_gen: ImageGenSettings,
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
    /// 把用户配置同步到权限矩阵的 default 角色组。
    pub fn sync_to_permissions(&self, permissions: &mut ToolPermissionConfig) {
        permissions.set_tool_allowed_for_default_group("web_search", self.web_search.is_ready());
        for (group, tools) in TOOL_GROUPS {
            let enabled = self.groups.get(*group).copied().unwrap_or(false);
            for tool in *tools {
                permissions.set_tool_allowed_for_default_group(tool, enabled);
            }
        }

        // 场景背景生成隐含依赖整套场景工具：`scene_generate` 的正确用法是
        // 「先 scene_list 看有没有现成的，没有才生成，剧情回来时 scene_return」。
        // 只放开 scene_generate 的话，模型拿到的说明会指向它没有的工具，
        // 结果是干脆不调用 —— 表现为「开了背景生成却什么都没发生」。
        // 放在分组循环**之后**，否则会被上面那轮按分组开关的写入覆盖掉。
        let image_gen_ready = self.image_gen.is_ready();
        permissions.set_tool_allowed_for_default_group("scene_generate", image_gen_ready);
        if image_gen_ready {
            for tool in ["scene_list", "scene_switch", "scene_return"] {
                permissions.set_tool_allowed_for_default_group(tool, true);
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
            return toml::from_str(&text)
                .with_context(|| format!("解析工具配置失败: {}", path.display()));
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
        let settings: ToolSettings = toml::from_str(legacy).unwrap();
        assert!(!settings.command_delete_auto_approve);
        assert!(!settings.file_delete_auto_approve);
    }

    #[test]
    fn save_can_replace_existing_settings_file() {
        let dir = tempfile::tempdir().unwrap();
        let mut settings = ToolSettings::default();
        settings.save(dir.path()).unwrap();
        settings.command_auto_approve = true;
        settings.save(dir.path()).unwrap();

        let loaded = ToolSettings::load_or_create(dir.path()).unwrap();
        assert!(loaded.command_auto_approve);
    }
}
