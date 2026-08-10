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
}

/// 插件运行期状态（含持久化开关与配置）。
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct PluginState {
    pub enabled: bool,
    #[serde(default)]
    pub config: HashMap<String, Value>,
}

impl PluginState {
    pub fn new() -> Self {
        Self {
            enabled: false,
            config: HashMap::new(),
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
    pub error: Option<String>,
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
            tools: record.manifest.tools.iter().map(|t| t.name.clone()).collect(),
            error: record.error.clone(),
        }
    }
}
