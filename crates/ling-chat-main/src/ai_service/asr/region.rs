//! DashScope 地域（站点）抽象。
//!
//! 各地域的**域名、API Key、模型列表三者独立、不能混用**（阿里云「选择地域」文档）。
//! 本模块是端点默认值的**单一真相**：[`super::provider`]（同步 HTTP）与
//! [`super::provider_stream`]（实时 WebSocket）都从这里派生，不在别处硬编码 host。
//!
//! 阿里云已为华北2（北京）、新加坡推出业务空间专属域名，并声明旧 DashScope 域名
//! （`dashscope.aliyuncs.com`）自 2026-09-30 起不再支持新特性。本模块给出的是
//! **开箱可用的默认值**，用户可在设置页手填专属域名覆盖（`{WorkspaceId}.<region>.maas.aliyuncs.com`）。

use serde::Serialize;
use tracing::warn;

/// DashScope 接入地域。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DashScopeRegion {
    /// 华北2（北京）。
    CnBeijing,
    /// 新加坡（国际站）。
    ApSoutheast1,
}

impl DashScopeRegion {
    /// 全部地域。顺序即设置页下拉的展示顺序。
    pub const ALL: &'static [DashScopeRegion] = &[Self::CnBeijing, Self::ApSoutheast1];

    /// 默认地域：空值 / 未知值一律回退到它。
    pub const DEFAULT: DashScopeRegion = Self::CnBeijing;

    /// 持久化 id（写入 `ProviderConfig.region`）。
    pub fn id(self) -> &'static str {
        match self {
            Self::CnBeijing => "cn-beijing",
            Self::ApSoutheast1 => "ap-southeast-1",
        }
    }

    /// 设置页显示名。
    pub fn label(self) -> &'static str {
        match self {
            Self::CnBeijing => "华北2（北京）",
            Self::ApSoutheast1 => "新加坡（国际站）",
        }
    }

    /// DashScope 接入域名（不含协议）。
    pub fn host(self) -> &'static str {
        match self {
            Self::CnBeijing => "dashscope.aliyuncs.com",
            Self::ApSoutheast1 => "dashscope-intl.aliyuncs.com",
        }
    }

    /// 非实时（同步）识别端点：`multimodal-generation`。
    pub fn http_endpoint(self) -> String {
        format!(
            "https://{}/api/v1/services/aigc/multimodal-generation/generation",
            self.host()
        )
    }

    /// 实时识别 WebSocket 端点。
    pub fn ws_endpoint(self) -> String {
        format!("wss://{}/api-ws/v1/inference", self.host())
    }

    /// 宽松解析持久化值：空串 / 未知值一律回退 [`Self::DEFAULT`] 并 warn。
    ///
    /// **刻意不实现 `Deserialize`**：`ProviderConfig` 反序列化失败会让整份 settings
    /// 被 `settings::load()` 丢弃（见那里的 malformed 分支），一个拼错的地域 id
    /// 不该有这种破坏力。
    pub fn parse(s: &str) -> Self {
        let v = s.trim();
        if v.is_empty() {
            return Self::DEFAULT;
        }
        Self::ALL
            .iter()
            .copied()
            .find(|r| r.id() == v)
            .unwrap_or_else(|| {
                warn!("[ASR] 未知地域 '{v}'，回退 {}", Self::DEFAULT.id());
                Self::DEFAULT
            })
    }

    /// 按端点 URL 反推地域（老数据迁移用）：URL 含该地域的 host 或地域 id 即命中。
    ///
    /// 覆盖两类写法：旧 DashScope 域名（`dashscope-intl.aliyuncs.com`）与
    /// 业务空间专属域名（`{WorkspaceId}.ap-southeast-1.maas.aliyuncs.com`）。
    /// 无法识别（自建代理等）返回 `None`，由调用方决定回退策略。
    pub fn from_endpoint_hint(url: &str) -> Option<Self> {
        let u = url.trim().to_ascii_lowercase();
        if u.is_empty() {
            return None;
        }
        Self::ALL
            .iter()
            .copied()
            .find(|r| u.contains(r.host()) || u.contains(r.id()))
    }
}

/// 下发给前端的单个地域信息（`ProviderInfo.regions`）。
///
/// 字段是 `String` 而非 `&'static str`：端点由 [`DashScopeRegion::host`] 拼接而来。
#[derive(Debug, Clone, Serialize)]
pub struct AsrRegionInfo {
    pub id: String,
    pub label: String,
    pub http_endpoint: String,
    pub ws_endpoint: String,
}

impl From<DashScopeRegion> for AsrRegionInfo {
    fn from(r: DashScopeRegion) -> Self {
        Self {
            id: r.id().to_string(),
            label: r.label().to_string(),
            http_endpoint: r.http_endpoint(),
            ws_endpoint: r.ws_endpoint(),
        }
    }
}
