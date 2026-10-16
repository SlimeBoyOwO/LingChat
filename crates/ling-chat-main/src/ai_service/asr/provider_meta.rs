//! provider 的 UI 配置元数据（设置页据此动态渲染表单）。

use serde::Serialize;

/// provider 配置字段类型，供前端 SettingsAsr.vue 渲染输入框。
#[derive(Debug, Clone, Copy, Serialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum ConfigFieldKind {
    /// 普通文本。
    Text,
    /// 密码框（API key 等敏感字段）。
    Password,
    /// 整数。
    Number,
    /// 布尔开关。
    Boolean,
    /// 下拉单选（配合 [`AsrConfigField::options`]，如地域选择）。
    Select,
    /// 密码框，但**值按地域分开存**（`provider_configs[id].api_keys[地域id]`）。
    ///
    /// UI 只显示当前地域的那一个框（切地域时内容随之切换），存储却是每个
    /// 地域一份——因为各地域的 Key 互相独立、不能混用（见 [`super::region`]）。
    /// 前端需要的当前地域取自 `ProviderInfo.regions` + 配置里的 `region`，
    /// 因此本字段不需要自己的 `options`。
    PasswordMap,
}

/// [`ConfigFieldKind::Select`] 的一个选项。
#[derive(Debug, Clone, Serialize)]
pub struct AsrConfigFieldOption {
    /// 写入配置的值。
    pub value: &'static str,
    /// UI 显示名。
    pub label: &'static str,
}

/// provider 在 UI 上展示需要填写的字段。
///
/// **注意**：字段 key 会被前端写成 `provider_configs[id]` 的**顶层键**
/// （见 SettingsAsr.vue 的 `providerCfgRecord[field.key]`），而 `ProviderConfig`
/// 是没有 `deny_unknown_fields` 的固定 struct —— 新增字段必须同步加进 struct，
/// 否则用户设置后会被 serde 静默丢弃、刷新即失。
#[derive(Debug, Clone, Serialize)]
pub struct AsrConfigField {
    /// 字段 key（写入 `provider_configs[id].<key>`）。
    pub key: &'static str,
    /// 字段显示名（前端可自行 i18n）。
    pub label: &'static str,
    /// 字段类型。
    pub kind: ConfigFieldKind,
    /// 是否必填。
    pub required: bool,
    /// 默认值（字符串形式）。
    pub default_value: Option<&'static str>,
    /// 占位提示文字。
    pub placeholder: Option<&'static str>,
    /// 提示说明（显示在输入框下方）。
    pub hint: Option<&'static str>,
    /// [`ConfigFieldKind::Select`] 的选项列表；其它类型为空。
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub options: Vec<AsrConfigFieldOption>,
}

/// provider 静态元数据（id / 显示名 / 配置字段）。
#[derive(Debug, Clone, Serialize)]
pub struct ProviderInfo {
    /// 唯一 id，写入配置 `active_provider` 用。
    pub id: &'static str,
    /// UI 显示名（如 "OpenAI Whisper"）。
    pub display_name: &'static str,
    /// 简短描述。
    pub description: &'static str,
    /// 是否支持流式协议（前端据此决定流式开关是否可用）。
    pub supports_streaming: bool,
    /// UI 需要展示的配置字段。
    pub config_fields: Vec<AsrConfigField>,
    /// 该 provider 可选的地域列表（含各地域的端点默认值）。
    ///
    /// 空数组 = 该 provider 无地域概念（如本地 llama-asr），前端不渲染地域下拉。
    /// 非空时，前端切地域会按这里的 `http_endpoint` / `ws_endpoint` 自动填端点
    /// —— 端点默认值的单一真相在后端（与 `ModelInfo` 的预设机制一致）。
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub regions: Vec<super::region::AsrRegionInfo>,
}
