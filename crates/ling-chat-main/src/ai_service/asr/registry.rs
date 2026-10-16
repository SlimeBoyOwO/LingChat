//! provider 注册表、模型清单分发与流式参数构造。
//!
//! 「有哪些 provider」「某 provider 有哪些模型」的单一入口。具体实现见
//! `crate::ai_service::asr::providers`，模型能力表见
//! `crate::ai_service::asr::qwen_models`。

use std::sync::Arc;

use tracing::{debug, warn};

use crate::ai_service::asr::config_fields::{llama_asr_config_fields, qwen_asr_config_fields};
use crate::ai_service::asr::error::{AsrError, map_reqwest_error};
use crate::ai_service::asr::provider::AsrProvider;
use crate::ai_service::asr::provider_meta::ProviderInfo;
use crate::ai_service::asr::provider_types::{AsrOptions, ModelInfo, ProviderCredentials};
use crate::ai_service::asr::providers::llama::{LlamaAsrProvider, parse_llama_models};
use crate::ai_service::asr::providers::qwen::{QwenAsrProvider, hotwords_to_vocabulary_json};
use crate::ai_service::asr::qwen_models::{
    qwen_models, qwen_supports_inline_vocabulary, qwen_supports_language_hints,
};
use crate::ai_service::asr::region::DashScopeRegion;

pub fn list_provider_info() -> Vec<ProviderInfo> {
    vec![
        ProviderInfo {
            id: QwenAsrProvider::ID,
            display_name: QwenAsrProvider::DISPLAY,
            description: "阿里云百炼 ASR（实时 / 非实时）",
            supports_streaming: true,
            config_fields: qwen_asr_config_fields(),
            regions: DashScopeRegion::ALL
                .iter()
                .copied()
                .map(Into::into)
                .collect(),
        },
        ProviderInfo {
            id: LlamaAsrProvider::ID,
            display_name: LlamaAsrProvider::DISPLAY,
            description: "本地 llama-server Qwen3-ASR（整句识别）",
            supports_streaming: false,
            config_fields: llama_asr_config_fields(),
            // 本地服务无地域概念：空数组 → 前端不渲染地域下拉
            regions: Vec::new(),
        },
    ]
}

/// 按模型能力构造 WS run-task 的可选参数。
///
/// **热词门控集中在这里**：模型不支持即时热词时**不发** `parameters.vocabulary`
/// —— 对不支持的模型发这个字段很可能直接 400。静默丢弃会让用户困惑，所以同时
/// warn 一条说明原因。
pub fn build_stream_params(model: &str, opts: &AsrOptions) -> super::provider_stream::StreamParams {
    let hotwords = &opts.hotwords;
    let vocabulary = if hotwords.is_empty() {
        None
    } else if qwen_supports_inline_vocabulary(model) {
        Some(hotwords_to_vocabulary_json(hotwords))
    } else {
        warn!(
            "[ASR] 流式模型 {model} 不支持即时热词，已忽略 {} 条热词\
             （该模型族不支持 parameters.vocabulary）",
            hotwords.len()
        );
        None
    };

    super::provider_stream::StreamParams {
        // 文档：qwen-audio-3.x 最多 4 个、fun-asr-realtime 系只取第一个。
        // 这里只做能力门控，个数由服务端按各自规则处理
        language_hint: if qwen_supports_language_hints(model) {
            opts.language_hint.clone()
        } else {
            None
        },
        vocabulary,
    }
}

/// 按 provider id 返回模型清单。
///
/// qwen 返回静态清单；llama-asr 动态请求服务端 `/v1/models`（endpoint 取
/// 当前设置，默认 `http://127.0.0.1:8080`；服务未启动/模型列表为空时返回
/// `ProviderApiError`，前端展示错误并回退为模型文本输入）。未接入模型选择
/// 的 provider 返回空数组（前端据此隐藏模型下拉）。
///
/// `region_override` 是调用方（设置页表单）当前选中的地域，优先于持久化配置
/// ——理由见 [`qwen_region_override`]。只有 qwen 用得上。
pub async fn list_models(
    provider_id: &str,
    region_override: Option<&str>,
    app: &tauri::AppHandle,
    http: &reqwest::Client,
) -> Result<Vec<ModelInfo>, AsrError> {
    match provider_id {
        // unwrap_or_else 而非 unwrap_or：被覆盖时不必白读一次磁盘配置
        QwenAsrProvider::ID => Ok(qwen_models(
            qwen_region_override(region_override).unwrap_or_else(|| qwen_region(app)),
        )),
        // llama-asr 的清单来自服务端 /v1/models，与地域无关，忽略该参数
        LlamaAsrProvider::ID => llama_models(app, http).await,
        _ => Ok(Vec::new()),
    }
}

/// 调用方显式指定的地域。`None` = 未指定，由 [`qwen_region`] 读持久化配置。
///
/// **为什么需要它**：`qwen_region` 读的是**已落盘**的 settings.json，而设置页
/// 改地域后要等 500ms debounce 才写盘，却在那之前就重拉模型（切地域要立刻刷新
/// 列表）。不传覆盖值的话后端会按旧地域返回，列表要等下次打开设置页才更新
/// ——表现为"切地域后少/多一个模型"，且 `ensureModelValidForRegion` 会拿着
/// 旧清单做回退判断。传了覆盖值，清单就只取决于用户当前选的地域，与磁盘无关。
///
/// 空白串按"未指定"处理：调用方拿到空配置时不该把地域意外重置成默认值。
fn qwen_region_override(region_override: Option<&str>) -> Option<DashScopeRegion> {
    match region_override {
        Some(r) if !r.trim().is_empty() => Some(DashScopeRegion::parse(r)),
        _ => None,
    }
}

/// 读 qwen provider 当前配置的地域（模型清单按地域过滤）。
///
/// 读设置失败时回退默认地域而非报错：模型下拉拉不出来是可用性问题，
/// 不该让整个设置页变成一个错误弹窗。
fn qwen_region(app: &tauri::AppHandle) -> DashScopeRegion {
    match super::settings::load(app) {
        Ok(s) => DashScopeRegion::parse(
            &s.provider_configs
                .get(QwenAsrProvider::ID)
                .map(|c| c.region.clone())
                .unwrap_or_default(),
        ),
        Err(e) => {
            warn!("[ASR] 读取地域配置失败，回退默认地域: {e}");
            DashScopeRegion::DEFAULT
        },
    }
}

/// 请求 llama-server `/v1/models`，映射为 ModelInfo 列表（llama-asr 全部非流式）。
///
/// 显示名取模型文件名的最后一段（`models/Qwen3-ASR-1.7B-Q8_0.gguf` →
/// `Qwen3-ASR-1.7B-Q8_0.gguf`），id 保留全名（服务端按全名匹配模型）。
async fn llama_models(
    app: &tauri::AppHandle,
    http: &reqwest::Client,
) -> Result<Vec<ModelInfo>, AsrError> {
    let settings = super::settings::load(app)?;
    let cred = settings
        .provider_configs
        .get(LlamaAsrProvider::ID)
        .cloned()
        .unwrap_or_default();
    let endpoint = if cred.endpoint.trim().is_empty() {
        LlamaAsrProvider::DEFAULT_ENDPOINT.to_string()
    } else {
        cred.endpoint.trim_end_matches('/').to_string()
    };
    let url = format!("{endpoint}/v1/models");
    debug!("[ASR] 拉取模型列表: {url}");
    let resp = http.get(&url).send().await.map_err(map_reqwest_error)?;
    let status = resp.status();
    if !status.is_success() {
        let body = resp.text().await.unwrap_or_default();
        return Err(AsrError::ProviderApiError {
            provider: LlamaAsrProvider::ID.into(),
            message: format!("HTTP {status}: {body}"),
        });
    }
    let body_text = resp.text().await.map_err(map_reqwest_error)?;
    let names = parse_llama_models(&body_text);
    if names.is_empty() {
        return Err(AsrError::ProviderApiError {
            provider: LlamaAsrProvider::ID.into(),
            message: "模型列表为空（/v1/models 未返回任何模型）".into(),
        });
    }
    Ok(names
        .into_iter()
        .enumerate()
        .map(|(i, id)| ModelInfo {
            display_name: id.rsplit('/').next().unwrap_or(&id).to_string(),
            // 结果流式（SSE）：音频整段上传、结果增量返回。与 qwen WS 真流式
            // 语义不同，但前端流式开关可用（录音结束出 partial 而非边录边出）
            supports_streaming: true,
            is_default: i == 0,
            // 端点与模型无关（本地服务地址固定）
            endpoint_kind: None,
            id,
        })
        .collect())
}

/// 按 id 创建 provider 实例。
///
/// 找不到 id 时返回 [`AsrError::ProviderNotFound`]。
pub async fn get_provider(
    id: &str,
    cred: &ProviderCredentials,
    http: &reqwest::Client,
) -> Result<Arc<dyn AsrProvider>, AsrError> {
    debug!("创建 ASR provider: {id}");
    let provider: Arc<dyn AsrProvider> = match id {
        QwenAsrProvider::ID => Arc::new(QwenAsrProvider::new(http.clone(), cred.clone())?),
        LlamaAsrProvider::ID => Arc::new(LlamaAsrProvider::new(http.clone(), cred.clone())),
        other => {
            return Err(AsrError::ProviderNotFound(other.into()));
        },
    };
    Ok(provider)
}
