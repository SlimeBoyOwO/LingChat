//! ASR 错误类型。
//!
//! 统一通过 [`AsrError`] 在 provider / session / vad 之间传递错误。
//! 前端拿到 `i18n_code()` 后查 `locales.*.settings.asr.errors.<code>`。

use std::path::PathBuf;

use serde::Serialize;

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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn i18n_code_is_stable() {
        // 锁定 i18n 码，防止后续重构意外破坏前端 locale 表
        assert_eq!(
            AsrError::ModelNotFound(PathBuf::from("/x")).i18n_code(),
            "ASR_MODEL_MISSING"
        );
        assert_eq!(
            AsrError::ProviderApiError {
                provider: "openai".into(),
                message: "401".into(),
            }
            .i18n_code(),
            "ASR_PROVIDER_FAILED"
        );
        assert_eq!(
            AsrError::ProviderTimeout("qwen".into()).i18n_code(),
            "ASR_PROVIDER_TIMEOUT"
        );
        assert_eq!(
            AsrError::MissingCredentials("openai".into()).i18n_code(),
            "ASR_MISSING_CREDENTIALS"
        );
        assert_eq!(AsrError::SessionBusy.i18n_code(), "ASR_SESSION_BUSY");
        assert_eq!(AsrError::Canceled.i18n_code(), "ASR_CANCELED");
        assert_eq!(AsrError::MicPermissionDenied.i18n_code(), "ASR_MIC_DENIED");
        assert_eq!(
            AsrError::StreamingNotSupported("openai".into()).i18n_code(),
            "ASR_STREAMING_UNSUPPORTED"
        );
    }
}
