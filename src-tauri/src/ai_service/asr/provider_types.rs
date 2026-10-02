//! ASR provider 的对外类型：识别结果、调用级参数、凭证、模型元数据。
//!
//! 只有类型定义与构造函数，不含 provider 实现或网络逻辑。经
//! `crate::ai_service::asr::provider` 重导出，历史路径 `provider::X` 保持有效。

use serde::Serialize;

/// provider 识别返回结果。
#[derive(Debug, Clone, Serialize)]
pub struct AsrResult {
    /// 识别出的文本。
    pub text: String,
    /// provider **检测到**的语言；拿不到时为 `None`（如 qwen 同步端不返回）。
    /// 不要把入参的 `language_hint` 回填进来。
    pub language: Option<String>,
    /// provider 报告的置信度 0~1（可选）。
    pub confidence: Option<f32>,
    /// provider id（与 `list_provider_info` 一致）。
    pub provider_id: String,
}

/// 一条热词。
///
/// `weight` 只对 DashScope 的**即时热词**（`parameters.vocabulary`）有意义；
/// 预编译热词（`vocabulary_id`）与本地 llama-asr 的 `prompt` 偏置都忽略它。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Hotword {
    pub text: String,
    pub weight: u8,
}

impl Hotword {
    /// 默认权重。DashScope 文档推荐从 4 起测。
    pub const DEFAULT_WEIGHT: u8 = 4;
    /// 权重上界：50 是文档里的「超级热词」特殊值（召回率大幅提升，最多 50 个）。
    pub const MAX_WEIGHT: u8 = 50;

    /// 用默认权重构造。
    pub fn new(text: impl Into<String>) -> Self {
        Self {
            text: text.into(),
            weight: Self::DEFAULT_WEIGHT,
        }
    }

    /// 指定权重构造（clamp 到 `1..=MAX_WEIGHT`）。
    pub fn with_weight(text: impl Into<String>, weight: u8) -> Self {
        Self {
            text: text.into(),
            weight: weight.clamp(1, Self::MAX_WEIGHT),
        }
    }

    /// 纯词表 → 热词列表（统一用默认权重的降级入口）。
    pub fn from_text_list<I, S>(texts: I) -> Vec<Self>
    where
        I: IntoIterator<Item = S>,
        S: Into<String>,
    {
        texts
            .into_iter()
            .map(|t| Self::new(t))
            .filter(|h| !h.text.trim().is_empty())
            .collect()
    }
}

/// 一次识别调用的可选参数。
///
/// 用结构体而非继续加函数参数：热词之后还会有采样率、角色 id（日志归因）等，
/// 参数列表会持续膨胀，而调用点数量有限，一次改到位成本更低。
#[derive(Debug, Clone, Default)]
pub struct AsrOptions {
    /// 可选 BCP-47 语言码，如 `"zh"` / `"en"` / `"ja"`。
    pub language_hint: Option<String>,
    /// **按调用传入**的热词（未来 = 当前角色的热词，随角色切换而变）。
    /// 空 = 无热词；没有配置级回退（`4fed1124` 已移除）。
    pub hotwords: Vec<Hotword>,
}

/// provider 运行时凭证：api_key + 端点 + model + 地域。
///
/// **不含热词**：热词是逐角色的，按调用经 [`AsrOptions::hotwords`] 传入，
/// 不经过配置存储（见 `settings::ProviderConfig`）。
#[derive(Debug, Clone, Default)]
pub struct ProviderCredentials {
    pub api_key: String,
    /// 同步（非实时）识别端点；空/非 http(s) = 按 [`Self::region_enum`] 派生。
    pub endpoint: String,
    /// 实时识别 WebSocket 端点；空/非 ws(s) = 按 [`Self::region_enum`] 派生。
    ///
    /// 与 [`Self::endpoint`] **分开**存储：两者协议不同，且用户可能只覆盖其一
    /// （业务空间专属域名 / 自建代理的 HTTP 与 WS 未必同源）。历史上共用一个
    /// 字段，选中流式模型时会被 `ModelInfo` 预设改写成 `wss://`，导致同步路径
    /// 拿到一个 WebSocket 地址——拆开是对这个既有问题的根治。
    pub ws_endpoint: String,
    /// 识别的模型名；空串 = provider 默认模型。
    pub model: String,
    /// 地域 id（见 [`super::region::DashScopeRegion`]）；空/未知 = 默认地域。
    pub region: String,
}

impl ProviderCredentials {
    /// 从 endpoint 字符串中剔除末尾 `/`，便于直接拼 `/audio/transcriptions`。
    pub fn normalized_endpoint(&self) -> String {
        self.endpoint.trim_end_matches('/').to_string()
    }

    /// api_key 是否非空（剪掉首尾空白后判断）。
    pub fn has_api_key(&self) -> bool {
        !self.api_key.trim().is_empty()
    }

    /// 解析后的地域（空/未知 → 默认地域）。
    pub fn region_enum(&self) -> super::region::DashScopeRegion {
        super::region::DashScopeRegion::parse(&self.region)
    }

    /// 实际使用的同步端点：配置为空或非 http(s) 时按地域派生默认。
    ///
    /// 「非 http(s) 一律回退」同时是历史数据的防御：老配置里 `endpoint` 可能被
    /// 模型预设写成 `wss://...`（见 [`Self::ws_endpoint`] 的说明），直接拿去发
    /// HTTP 请求会让 reqwest 报 builder error。
    pub fn effective_http_endpoint(&self) -> String {
        let e = self.normalized_endpoint();
        if e.starts_with("http://") || e.starts_with("https://") {
            e
        } else {
            self.region_enum().http_endpoint()
        }
    }

    /// 实际使用的实时端点：配置为空或非 ws(s) 时按地域派生默认。
    ///
    /// 注意裁剪的是 [`Self::ws_endpoint`] 而不是 [`Self::endpoint`] —— 两者是
    /// 独立的配置项，取错字段会让用户手填的 WS 地址被静默忽略。
    pub fn effective_ws_endpoint(&self) -> String {
        let e = self.ws_endpoint.trim_end_matches('/');
        if e.starts_with("wss://") || e.starts_with("ws://") {
            e.to_string()
        } else {
            self.region_enum().ws_endpoint()
        }
    }
}

/// 模型对应的端点类型（选中该模型时应把哪个端点字段切到该协议）。
#[derive(Debug, Clone, Copy, Serialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum EndpointKind {
    /// 同步（非实时）端点 → `provider_configs[id].endpoint`。
    Http,
    /// 实时 WebSocket 端点 → `provider_configs[id].ws_endpoint`。
    Ws,
}

/// 模型元数据（`asr_list_models` 返回给前端渲染下拉）。
#[derive(Debug, Clone, Serialize)]
pub struct ModelInfo {
    /// 模型 id（写入 `provider_configs[id].model`）。qwen 是协议名；
    /// llama-asr 是 `/v1/models` 返回的动态全名（非静态，用 String）。
    pub id: String,
    /// UI 显示名。
    pub display_name: String,
    /// 是否支持流式协议（前端流式开关可用性的权威判定）。
    pub supports_streaming: bool,
    /// 是否默认模型（`provider_configs[id].model` 为空时生效）。
    pub is_default: bool,
    /// 端点预设：选中该模型时把对应端点字段填成**当前地域**的默认值。
    /// `None` = 不干预端点（llama-asr 的端点与模型无关）。
    ///
    /// 只给类型不给完整 URL：端点是地域相关的，由前端从
    /// [`ProviderInfo::regions`] 取当前地域的默认值，避免两处真相。
    pub endpoint_kind: Option<EndpointKind>,
}
