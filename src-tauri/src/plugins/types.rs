//! 插件系统的数据结构定义。

use std::collections::HashMap;

use serde::{Deserialize, Serialize};
use serde_json::Value;

/// 配置字段的类型（前端据此渲染表单控件）。
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ConfigKind {
    /// 普通文本输入
    String,
    /// 密码输入（不回显明文）
    Secret,
    /// 数字输入
    Number,
    /// 开关
    Boolean,
}

impl Default for ConfigKind {
    fn default() -> Self {
        Self::String
    }
}

/// 插件级配置字段声明（前端设置页据此生成表单）。
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct ConfigFieldDecl {
    pub key: String,
    pub label: String,
    #[serde(default)]
    pub kind: ConfigKind,
    #[serde(default)]
    pub required: bool,
    #[serde(default)]
    pub default: Option<Value>,
}

/// 环境变量白名单声明。
///
/// 宿主仅把此处声明的环境变量注入 `ctx.env(name)`，插件读不到其他环境变量。
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct EnvDecl {
    pub key: String,
    pub label: String,
}

/// 单个工具的声明。
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct ToolSpec {
    /// 工具名（注册到 ToolRegistry，需全局唯一，建议带插件 id 前缀）。
    pub name: String,
    pub description: String,
    /// 提供给 LLM 的 JSON Schema（内嵌 JSON 字符串，解析时转 Value）。
    pub parameters: String,
    /// 处理该工具的 Python 脚本（相对插件目录）。
    pub script: String,
    /// 单次执行超时（毫秒），默认 30s。
    #[serde(default = "default_timeout_ms")]
    pub timeout_ms: u64,
}

fn default_timeout_ms() -> u64 {
    30_000
}

/// 信号订阅的匹配值：单值等值，或集合（命中任一）。
///
/// 注意 `Many` 必须排在 `One` 前面：`One` 装着 `serde_json::Value`，能吞下任何
/// 值，反过来的话数组会被当成「等于这个数组」而不是「命中其中之一」。
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(untagged)]
pub enum MatchValue {
    /// 命中其中任一即可。
    Many(Vec<Value>),
    /// 等值命中。
    One(Value),
}

impl MatchValue {
    /// `actual` 为 `None`（payload 里没这个字段）时一律不命中。
    pub(crate) fn hits(&self, actual: Option<&Value>) -> bool {
        match (self, actual) {
            (_, None) => false,
            (Self::Many(expected), Some(actual)) => expected.iter().any(|e| e == actual),
            (Self::One(expected), Some(actual)) => expected == actual,
        }
    }
}

/// 插件对宿主信号的订阅声明。
///
/// 宿主收到信号时按 `matches` 在宿主侧筛选（key 对应 payload 顶层字段），
/// 命中才调用 `script` 里的 `handler(ctx)`，避免为不关心的插件新建解释器。
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct SubscribeDecl {
    /// 宿主注册的信号名（如 `scene:switch`）。
    pub signal: String,
    /// 处理该信号的脚本（相对插件目录的单个文件名）。
    pub script: String,
    /// 脚本内的处理函数名，签名为 `handler(ctx)`。
    pub handler: String,
    /// 单次执行超时（毫秒），默认 30s。
    #[serde(default = "default_timeout_ms")]
    pub timeout_ms: u64,
    /// 匹配条件：payload 顶层字段名 → 期望值。空表示不筛选，一律派发。
    #[serde(default, rename = "match")]
    pub matches: HashMap<String, MatchValue>,
}

/// 插件在程序启动（或启用）时执行的入口声明。
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct StartupDecl {
    /// 启动脚本（相对插件目录的单个文件名）。
    pub script: String,
    /// 脚本内的入口函数名，签名为 `handler(ctx)`。
    pub handler: String,
    /// 单次尝试的超时（毫秒），默认 30s。
    #[serde(default = "default_timeout_ms")]
    pub timeout_ms: u64,
    /// 重试次数：**首次失败后再试 N 次**，共最多 N+1 次执行。上限 5，默认 0。
    #[serde(default)]
    pub retries: u64,
    /// 两次尝试之间的等待（毫秒），默认 5s，上限 60s。
    #[serde(default = "default_retry_interval_ms")]
    pub retry_interval_ms: u64,
    /// 全部尝试都失败时是否禁用插件（默认 true）。
    /// 置 false 表示「尽力而为」的初始化，失败只记日志。
    #[serde(default = "default_true")]
    pub required: bool,
}

fn default_retry_interval_ms() -> u64 {
    5_000
}

fn default_true() -> bool {
    true
}

/// 插件可携带的资源类型（与 game_data 下同名子目录一一对应）。
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ResourceKind {
    Characters,
    Scripts,
    Musics,
    Backgrounds,
    Ambients,
}

impl ResourceKind {
    /// 插件内与游戏目录同名同构的子目录名。
    pub fn subdir(&self) -> &'static str {
        match self {
            Self::Characters => "characters",
            Self::Scripts => "scripts",
            Self::Musics => "musics",
            Self::Backgrounds => "backgrounds",
            Self::Ambients => "ambients",
        }
    }

    pub fn as_str(&self) -> &'static str {
        self.subdir()
    }
}

/// 插件 manifest（manifest.toml）。
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PluginManifest {
    pub id: String,
    pub name: String,
    pub description: String,
    pub version: String,
    #[serde(default)]
    pub author: Option<String>,
    #[serde(default)]
    pub config: Vec<ConfigFieldDecl>,
    #[serde(default)]
    pub env: Vec<EnvDecl>,
    #[serde(default)]
    pub tools: Vec<ToolSpec>,
    /// 插件携带的资源类型声明（空 = 纯工具插件，向后兼容）。
    #[serde(default)]
    pub resources: Vec<ResourceKind>,
    /// 插件订阅的宿主信号（空 = 不订阅任何信号）。
    #[serde(default)]
    pub subscribe: Vec<SubscribeDecl>,
    /// 启动（或启用）时执行的入口。
    #[serde(default)]
    pub startup: Option<StartupDecl>,
    /// 前置插件 id：这些插件必须已安装且已启用，本插件才能启用；
    /// 启动时也会等它们的启动函数执行完再执行自己的。与是否有启动函数无关。
    #[serde(default)]
    pub depends_on: Vec<String>,
}

/// 插件运行期状态（含持久化开关与配置）。
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct PluginState {
    pub enabled: bool,
    #[serde(default)]
    pub config: HashMap<String, Value>,
    /// 软删除标记：`"<kind>/<key>"`（如 `"characters/爱丽丝"`、`"backgrounds/夜晚.webp"`、
    /// `"scripts/神秘の魔法药水"`）。被标记的插件资源对游戏隐藏，但不删除文件。
    #[serde(default)]
    pub hidden_resources: Vec<String>,
}

impl PluginState {
    pub fn new() -> Self {
        Self {
            enabled: false,
            config: HashMap::new(),
            hidden_resources: Vec::new(),
        }
    }
}

/// 插件清单与运行期状态、脚本目录的聚合视图（插件管理器内部持有）。
#[derive(Clone, Debug)]
pub struct PluginRecord {
    pub manifest: PluginManifest,
    pub state: PluginState,
    /// 插件目录绝对路径（data/plugins/<id>/）。
    pub dir: std::path::PathBuf,
    /// 启动/加载时的错误信息（如 manifest 解析失败）。
    pub error: Option<String>,
    /// 启动阶段未能运行的原因（`错误码|补充信息`，见 `PLUGIN_*` 错误码）。
    ///
    /// 与 `error` 分开：`error` 非空会让插件**无法启用**（manifest 坏了），
    /// 而这里只表示「这次没跑起来」，用户修好原因后可以重新启用重试。
    /// 不持久化，重扫即清空。
    pub startup_error: Option<String>,
}

/// 暴露给前端的插件信息。
#[derive(Clone, Debug, Serialize)]
pub struct PluginInfo {
    pub id: String,
    pub name: String,
    pub description: String,
    pub version: String,
    pub author: Option<String>,
    pub enabled: bool,
    pub config_schema: Vec<ConfigFieldDecl>,
    pub env: Vec<EnvDecl>,
    pub tools: Vec<String>,
    /// 该插件声明携带的资源类型（如 `["characters", "musics"]`）。
    pub resources: Vec<String>,
    /// 本插件依赖的前置插件 id（前端据此提示「需要先启用谁」）。
    pub depends_on: Vec<String>,
    pub error: Option<String>,
    /// 启动阶段未能运行的原因（`错误码|补充信息`）。
    pub startup_error: Option<String>,
}

impl From<&PluginRecord> for PluginInfo {
    fn from(record: &PluginRecord) -> Self {
        Self {
            id: record.manifest.id.clone(),
            name: record.manifest.name.clone(),
            description: record.manifest.description.clone(),
            version: record.manifest.version.clone(),
            author: record.manifest.author.clone(),
            enabled: record.state.enabled,
            config_schema: record.manifest.config.clone(),
            env: record.manifest.env.clone(),
            tools: record
                .manifest
                .tools
                .iter()
                .map(|t| t.name.clone())
                .collect(),
            resources: record
                .manifest
                .resources
                .iter()
                .map(|k| k.as_str().to_string())
                .collect(),
            depends_on: record.manifest.depends_on.clone(),
            error: record.error.clone(),
            startup_error: record.startup_error.clone(),
        }
    }
}
