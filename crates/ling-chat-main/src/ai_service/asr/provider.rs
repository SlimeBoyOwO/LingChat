//! 云 ASR provider 抽象 + qwen-asr 实现（阿里云 DashScope）。
//!
//! 设计目标：v1 只做"调用云 API"的最薄一层；端点检测、会话编排、配置持久化
//! 由同目录其它子模块负责（vad / session / settings，后续 Task）。
//!
//! 复用策略：
//! - HTTP 客户端由调用方传入（`&reqwest::Client`），调用方负责 TLS / 超时（30s）。
//! - 错误统一返回 [`AsrError`]，不外泄 `reqwest::Error` / `serde_json::Error`。
//! - 不引入新依赖（reqwest / serde / serde_json / async-trait / tracing / thiserror
//!   / base64 都已在 Cargo.toml）。
//!
//! ============================================================================
//! 扩展指南（新增 provider / 接入 OpenAI 兼容服务）
//! ============================================================================
//!
//! 新增一个**专用协议** provider（3 步，参照 [`QwenAsrProvider`]）：
//! 1. 实现 [`AsrProvider`] trait（recognize 必选；流式见下）
//! 2. 写 `config_fields()`——设置页据此动态渲染输入框，前端零改动
//! 3. 注册到 `list_provider_info()` 与 `get_provider()`
//!
//! 接入**OpenAI 兼容服务**（whisper.cpp / faster-whisper-server / Groq /
//! 阿里云 DashScope compatible-mode 的 qwen-audio-asr 等）：
//! - 协议是 `POST {endpoint}/v1/audio/transcriptions`（multipart：file/model/
//!   可选 language/prompt）+ `GET /v1/models` 动态模型列表
//! - 现成的可复用件：
//!   - [`provider_stream_llama::recognize_stream`]——通用 SSE 结果流式客户端
//!     （对 `<asr_text>` 标记自动检测，纯 OpenAI 文本也能解析；llama-asr 已在用）
//!   - [`parse_llama_text`] / [`parse_llama_models`]——响应与模型列表解析
//! - 泛化方案备忘（未实施，2026-08 评审预留）：
//!   - 新增固定 id `openai-compatible` 的通用 struct（id 保持 `&'static str`，
//!     trait 无需改；参照 LLM 侧 `lmstudio` 固定 id + 可配 endpoint 的先例）
//!   - llama-asr 保留（默认端点/默认模型/热词有友好默认值，老配置不断），
//!     与通用 struct 共享上述复用件
//!   - 前端 `isLlamaStream()`（useAsrInput.ts）需改为覆盖 SSE 类 provider
//!     的集合判定——llama-asr 与 openai-compatible 都是"整段上传 + SSE
//!     partial"，必须同链路，新增第三个 SSE 类 provider 时同步扩展该判定

use std::sync::Arc;

use async_trait::async_trait;

use super::error::AsrError;

// ============================================================================
// 重导出：保持历史路径 `asr::provider::X` 对既有调用方全部有效
//
// 实现按关注点拆在同目录其它模块（扩展指南见上方模块文档）：
//   provider_types —— 对外类型（结果 / 参数 / 凭证 / 模型元数据）
//   provider_meta  —— UI 配置元数据
//   providers/     —— 各 provider 实现
//   qwen_models    —— qwen 模型目录与协议能力表
//   registry       —— 注册表 / 模型清单 / 流式参数
//   config_fields  —— 设置页表单字段
//
// 下面若干条目当前没有调用方，编译器会报 unused_imports —— `allow` 是刻意保留：
// 不缩窄 provider 模块的对外面，新增 provider 时这些是天然的入口。
// ============================================================================

#[allow(unused_imports)]
pub use super::provider_meta::{
    AsrConfigField, AsrConfigFieldOption, ConfigFieldKind, ProviderInfo,
};
#[allow(unused_imports)]
pub use super::provider_types::{
    AsrOptions, AsrResult, EndpointKind, Hotword, ModelInfo, ProviderCredentials,
};
#[allow(unused_imports)]
pub(crate) use super::providers::llama::{parse_llama_models, parse_llama_text};
#[allow(unused_imports)]
pub use super::providers::{LlamaAsrProvider, QwenAsrProvider};
#[allow(unused_imports)]
pub use super::qwen_models::{
    qwen_default_model, qwen_is_streaming_model, qwen_models, qwen_supports_inline_vocabulary,
    qwen_supports_language_hints,
};
#[allow(unused_imports)]
pub use super::registry::{build_stream_params, get_provider, list_models, list_provider_info};

/// 所有云 ASR provider 必须实现的接口。
#[async_trait]
pub trait AsrProvider: Send + Sync {
    /// provider id（与 `ProviderInfo.id` 一致）。
    fn id(&self) -> &'static str;

    /// UI 显示名。
    fn display_name(&self) -> &'static str;

    /// provider 在 SettingsAsr.vue 中渲染所需的配置字段。
    fn config_fields(&self) -> Vec<AsrConfigField>;

    /// 调用云 API 识别一段 WAV 字节。
    ///
    /// - `wav_bytes`：前端 OfflineAudioContext 重采样后的 16kHz mono WAV。
    /// - `opts`：本次调用的可选参数（语言提示 + 热词）。热词走这里而非
    ///   `ProviderCredentials`，是因为它的来源是**当前角色**，随角色切换而变。
    ///
    /// 错误统一返回 [`AsrError`]。
    async fn recognize(&self, wav_bytes: Vec<u8>, opts: &AsrOptions)
    -> Result<AsrResult, AsrError>;

    /// 是否支持流式协议（WebSocket 实时识别）。默认不支持。
    fn supports_streaming(&self) -> bool {
        false
    }

    /// 结果流式识别（SSE 类协议：音频整段上传、结果增量返回）。默认不支持。
    ///
    /// 与 WS 会话流式（`asr_start_streaming`/`stop_streaming`）独立：llama-asr
    /// 走这里（provider_stream_llama.rs），qwen 走 WebSocket 会话路径。
    /// 默认实现返回 [`AsrError::StreamingNotSupported`]。
    ///
    /// `on_partial`：增量文本回调（整段累积视图，每次整体替换）——由调用方
    /// （session / 命令层）注入，provider 不直接依赖 Tauri 事件发射（展示
    /// 与识别解耦，provider 可脱离 Tauri 环境测试）。
    async fn stream_recognize(
        &self,
        _wav_bytes: Vec<u8>,
        _opts: &AsrOptions,
        _on_partial: Option<Arc<dyn for<'a> Fn(&'a str) + Send + Sync + 'static>>,
    ) -> Result<AsrResult, AsrError> {
        Err(AsrError::StreamingNotSupported(self.id().into()))
    }
}
