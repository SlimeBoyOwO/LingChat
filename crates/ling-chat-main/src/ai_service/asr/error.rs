//! ASR 错误类型。
//!
//! 统一通过 [`AsrError`] 在 provider / session / vad 之间传递错误。
//! 前端拿到 `i18n_code()` 后查 `locales.*.settings.asr.errors.<code>`。

use std::path::PathBuf;

use serde::Serialize;
use tracing::warn;

#[derive(Debug, thiserror::Error, Serialize, Clone)]
#[serde(tag = "code", content = "data")]
pub enum AsrError {
    #[error("VAD 模型未找到: {0}")]
    ModelNotFound(PathBuf),

    #[error("VAD 引擎加载失败: {0}")]
    EngineLoadFailed(String),

    #[error("ASR provider 未找到: {0}")]
    ProviderNotFound(String),

    #[error("provider {provider} API 错误: {message}")]
    ProviderApiError { provider: String, message: String },

    #[error("provider {0} 请求超时")]
    ProviderTimeout(String),

    #[error("缺少凭据: {0}")]
    MissingCredentials(String),

    #[error("音频格式无效: {0}")]
    InvalidAudioFormat(String),

    #[error("ASR 会话忙")]
    SessionBusy,

    #[error("ASR 已取消")]
    Canceled,

    #[error("麦克风权限被拒绝")]
    MicPermissionDenied,

    #[error("流式识别不受支持: {0}")]
    StreamingNotSupported(String),
}

impl AsrError {
    /// 国际化错误码，前端通过此字符串在 locale 表中查找用户可读消息。
    pub fn i18n_code(&self) -> &'static str {
        match self {
            Self::ModelNotFound(_) => "ASR_MODEL_MISSING",
            Self::EngineLoadFailed(_) => "ASR_ENGINE_LOAD_FAILED",
            Self::ProviderNotFound(_) => "ASR_PROVIDER_NOT_FOUND",
            Self::ProviderApiError { .. } => "ASR_PROVIDER_FAILED",
            Self::ProviderTimeout(_) => "ASR_PROVIDER_TIMEOUT",
            Self::MissingCredentials(_) => "ASR_MISSING_CREDENTIALS",
            Self::InvalidAudioFormat(_) => "ASR_INVALID_AUDIO",
            Self::SessionBusy => "ASR_SESSION_BUSY",
            Self::Canceled => "ASR_CANCELED",
            Self::MicPermissionDenied => "ASR_MIC_DENIED",
            Self::StreamingNotSupported(_) => "ASR_STREAMING_UNSUPPORTED",
        }
    }
}

// ============================================================================
// reqwest 错误映射
// ============================================================================

///
/// reqwest 的网络/超时/协议错误统一归类为 provider 错误；上层无需关心细节。
pub(crate) fn map_reqwest_error(e: reqwest::Error) -> AsrError {
    if e.is_timeout() {
        AsrError::ProviderTimeout("network".into())
    } else if e.is_connect() || e.is_request() {
        AsrError::ProviderApiError {
            provider: "network".into(),
            message: format!("请求失败: {e}"),
        }
    } else {
        warn!("reqwest 错误: {e}");
        AsrError::ProviderApiError {
            provider: "network".into(),
            message: format!("{e}"),
        }
    }
}
