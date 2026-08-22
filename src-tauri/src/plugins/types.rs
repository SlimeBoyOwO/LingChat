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

/// 网络白名单声明：插件脚本可访问的 URL（审核比对与运行时强制共用）。
///
/// 对应 manifest `[[network]]`。运行时 `http_get/http_post` 仅放行
/// 与某条声明匹配的请求：精确 host（可带端口）+ 可选路径前缀 + https 限制。
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct NetworkDecl {
    /// 精确域名（不含 scheme / path / query），可带端口，如 `api.tavily.com`。
    pub host: String,
    /// 可选：仅放行这些路径前缀（须以 `/` 开头，不含 query/fragment）。
    #[serde(default)]
    pub paths: Vec<String>,
    /// 仅允许 https；默认 true。
    #[serde(default = "default_https_only")]
    pub https_only: bool,
}

fn default_https_only() -> bool {
    true
}

/// call_tool 写工具声明：插件脚本可调用且不在读工具集内的工具名。
///
/// 对应 manifest `[[permissions.tools]]`。读工具（时钟/状态/只读查询）免声明。
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct PermissionToolDecl {
    pub name: String,
}

/// `[[permissions]]` 段（当前只有 tools 子段）。
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct PermissionsSection {
    #[serde(default)]
    pub tools: Vec<PermissionToolDecl>,
}

/// 大文件声明（>5MB 资源，不进 git，独立下载通道）。
///
/// 对应 manifest `[[assets]]`。安装时客户端不随 zip 附带这些文件，
/// 需按 `url` 单独下载并校验 sha256 后放入 payload 对应位置。
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct AssetDecl {
    pub name: String,
    /// 下载地址（市场仓库 Releases，审核时已验证）。
    pub url: String,
    /// 完整 sha256（64 位 hex），安装时校验。
    pub sha256: String,
    /// 字节数。
    pub size: u64,
}

/// `[content]` 段：市场内容元信息（分类/标签），展示用，不影响运行。
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct ContentMeta {
    #[serde(default)]
    pub category: Option<String>,
    #[serde(default)]
    pub tags: Vec<String>,
}

/// 插件 manifest（manifest.toml）。
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PluginManifest {
    pub id: String,
    pub name: String,
    pub description: String,
    pub version: String,
    /// 包类型：`plugin` / `character` / `script` / `voice`（默认 `plugin`，
    /// 兼容旧的手动安装插件）。TOML 字段名是 `type`。
    #[serde(rename = "type", default = "default_package_type")]
    pub package_type: String,
    #[serde(default)]
    pub author: Option<String>,
    #[serde(default)]
    pub config: Vec<ConfigFieldDecl>,
    #[serde(default)]
    pub env: Vec<EnvDecl>,
    #[serde(default)]
    pub tools: Vec<ToolSpec>,
    /// 网络白名单（`[[network]]`），空 = 禁止任何外部请求。
    #[serde(default)]
    pub network: Vec<NetworkDecl>,
    /// call_tool 声明（`[[permissions.tools]]`）。
    #[serde(default)]
    pub permissions: PermissionsSection,
    /// 大文件声明（`[[assets]]`）。
    #[serde(default)]
    pub assets: Vec<AssetDecl>,
    /// 市场内容元信息（`[content]`）。
    #[serde(default)]
    pub content: Option<ContentMeta>,
}

fn default_package_type() -> String {
    "plugin".to_string()
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
    /// 网络白名单（供设置页展示权限）。
    #[serde(default)]
    pub network: Vec<NetworkDecl>,
    /// call_tool 声明工具名。
    #[serde(default)]
    pub declared_tools: Vec<String>,
    /// 大文件声明。
    #[serde(default)]
    pub assets: Vec<AssetDecl>,
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
            network: record.manifest.network.clone(),
            declared_tools: record
                .manifest
                .permissions
                .tools
                .iter()
                .map(|t| t.name.clone())
                .collect(),
            assets: record.manifest.assets.clone(),
            error: record.error.clone(),
        }
    }
}
