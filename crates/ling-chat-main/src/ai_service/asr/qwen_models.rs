//! DashScope qwen 模型目录与协议能力表。
//!
//! 模型清单、流式判定、body 格式、热词门控、端点预设全部从 [`QWEN_MODELS`]
//! 派生 —— 单一真相，避免「加了模型忘了改 `matches!`」导致静默走错链路。

use crate::ai_service::asr::provider_types::{EndpointKind, ModelInfo};
use crate::ai_service::asr::region::DashScopeRegion;

/// 单个 DashScope 模型的协议能力描述。
///
/// 模型清单、流式判定、body 格式、热词门控、端点预设**全部从这张表派生**
/// —— 单一真相，避免「加了模型忘了改 `matches!`」导致静默走错链路。
struct QwenModelSpec {
    id: &'static str,
    display: &'static str,
    /// 是否出现在设置页的模型列表里。
    ///
    /// `false` = 隐藏的历史模型：不出现在清单里，但 [`qwen_model_spec`] 仍能查到它，
    /// 因此老配置（`model: "fun-asr-realtime"`）的协议行为完全不变。
    listed: bool,
    /// 走 WebSocket 实时端点（`false` = 同步 HTTP 端点）。
    ws_realtime: bool,
    /// 支持即时热词 `parameters.vocabulary`（文档明确**仅 qwen-audio-3.x 系**）。
    ///
    /// 预编译热词（`parameters.vocabulary_id`）随 ASR 设置页的热词字段一并移除，
    /// 故这里不再有对应的能力标记。
    supports_inline_vocabulary: bool,
    /// 支持 `language_hints`。
    supports_language_hints: bool,
    /// 同步 body 是否沿用旧格式 `content:[{"audio": "data:..."}]`。
    ///
    /// 新的 qwen-audio-3.x 系要求 `content:[{"type":"input_audio","input_audio":{"data":...}}]`；
    /// 历史 Fun-ASR-Realtime 走的是旧格式。一刀切替换会让老配置从「能用」变成
    /// 「未知错误」，故按模型分流。
    use_legacy_audio_content: bool,
    /// 该模型可用的地域。
    regions: &'static [DashScopeRegion],
    /// 所属地域是否以它作为**同步**默认模型。
    is_default_batch: bool,
    /// 前端 `find(supports_streaming)` 挑流式模型时的**首选**。
    is_default_stream: bool,
}

const BOTH_REGIONS: &[DashScopeRegion] =
    &[DashScopeRegion::CnBeijing, DashScopeRegion::ApSoutheast1];
const CN_ONLY: &[DashScopeRegion] = &[DashScopeRegion::CnBeijing];

/// DashScope 语音识别模型清单（按协议实接情况维护）。
///
/// 异步任务类（`*-filetrans`、`paraformer-v2`）协议未接入，不列出。
const QWEN_MODELS: &[QwenModelSpec] = &[
    QwenModelSpec {
        id: "qwen-audio-3.0-asr-flash",
        display: "Qwen-Audio-3.0-ASR-Flash（非实时）",
        listed: true,
        ws_realtime: false,
        supports_inline_vocabulary: true,
        supports_language_hints: true,
        use_legacy_audio_content: false,
        regions: BOTH_REGIONS,
        is_default_batch: true,
        is_default_stream: false,
    },
    QwenModelSpec {
        id: "qwen-audio-3.1-asr-flash",
        display: "Qwen-Audio-3.1-ASR-Flash（非实时）",
        listed: true,
        ws_realtime: false,
        supports_inline_vocabulary: true,
        supports_language_hints: true,
        use_legacy_audio_content: false,
        regions: BOTH_REGIONS,
        // 保留 3.0 作默认：新增模型不改既有用户的行为。想改用 3.1 作默认为改此行
        is_default_batch: false,
        is_default_stream: false,
    },
    // 历史默认非流式模型。DashScope 已把它归入实时（WebSocket）族，但本项目的
    // 同步路径一直在用它且实测可用，故按旧格式保留规格，避免老配置失效。
    // **不在设置页列出**（listed: false）：它与 fun-asr-realtime-2026-02-28 同名
    // 不同协议，两条"Fun-ASR-Realtime"摆在列表里会让人误选；但老配置里存着它的
    // model 名，规格必须仍能查到，否则会静默换 body 格式（它走旧 content 格式）。
    QwenModelSpec {
        id: "fun-asr-realtime",
        display: "Fun-ASR-Realtime（非实时·历史协议）",
        listed: false,
        ws_realtime: false,
        supports_inline_vocabulary: false,
        supports_language_hints: false,
        use_legacy_audio_content: true,
        regions: BOTH_REGIONS,
        is_default_batch: false,
        is_default_stream: false,
    },
    // 流式首选仍是它：现有 WS 客户端是对着这个模型实证写出来的，
    // fun-asr-realtime-2026-02-28 是否复用同一协议尚未实测（见计划 V2）。
    QwenModelSpec {
        id: "paraformer-realtime-v2",
        display: "Paraformer-Realtime-V2",
        listed: true,
        ws_realtime: true,
        supports_inline_vocabulary: false,
        supports_language_hints: true,
        use_legacy_audio_content: false,
        regions: BOTH_REGIONS,
        is_default_batch: false,
        is_default_stream: true,
    },
    // 文档的热词支持表只在北京地域列出它（新加坡只列 fun-asr-realtime 与
    // fun-asr-realtime-2025-11-07），故保守标为北京独有。
    //
    // **不再在设置页列出**（listed: false）：它同样是"Fun-ASR-Realtime"，与上面
    // 那条靠 display 里的一段后缀区分，实测中确实被误认成同一个模型。规格同样
    // 保留——老配置里可能存着它，查不到就会按非流式处理、发到同步端点直接 400
    // （它是 ws_realtime 模型）。
    QwenModelSpec {
        id: "fun-asr-realtime-2026-02-28",
        display: "Fun-ASR-Realtime（2026-02-28·历史）",
        listed: false,
        ws_realtime: true,
        supports_inline_vocabulary: false,
        supports_language_hints: true,
        use_legacy_audio_content: false,
        regions: CN_ONLY,
        is_default_batch: false,
        is_default_stream: false,
    },
    // 文档里实时识别的首推模型。与 fun-asr-realtime 共用同一个 WebSocket 接入
    // 协议（同一份接入文档），因此不需要新的客户端代码——但它和
    // fun-asr-realtime-2026-02-28 一样，**是否与现有客户端完全兼容尚未实测**
    // （现有客户端是对着 paraformer-realtime-v2 实证写的）。
    QwenModelSpec {
        id: "qwen-audio-3.1-asr-flash-streaming",
        display: "Qwen-Audio-3.1-ASR-Flash-Streaming（实时）",
        listed: true,
        ws_realtime: true,
        supports_inline_vocabulary: true,
        supports_language_hints: true,
        use_legacy_audio_content: false,
        regions: BOTH_REGIONS,
        is_default_batch: false,
        is_default_stream: false,
    },
];

/// 按 id 查模型规格。未知模型返回 `None`。
fn qwen_model_spec(model: &str) -> Option<&'static QwenModelSpec> {
    QWEN_MODELS.iter().find(|s| s.id == model)
}

/// qwen（DashScope）语音识别模型清单，按地域过滤，**不含隐藏的历史模型**。
pub fn qwen_models(region: DashScopeRegion) -> Vec<ModelInfo> {
    QWEN_MODELS
        .iter()
        .filter(|s| s.listed && s.regions.contains(&region))
        .map(|s| ModelInfo {
            id: s.id.to_string(),
            display_name: s.display.to_string(),
            supports_streaming: s.ws_realtime,
            is_default: s.is_default_batch,
            endpoint_kind: Some(if s.ws_realtime {
                EndpointKind::Ws
            } else {
                EndpointKind::Http
            }),
        })
        .collect()
}

/// 该模型是否走 WebSocket 实时端点。
///
/// 流式模型只能走 WebSocket；非流式端点（multimodal-generation）不认识它们，
/// DashScope 会返回 HTTP 400 "url error"（模型名与端点不匹配）。
pub fn qwen_is_streaming_model(model: &str) -> bool {
    qwen_model_spec(model).is_some_and(|s| s.ws_realtime)
}

/// 该模型是否支持即时热词 `parameters.vocabulary`。
///
/// 文档明确仅 qwen-audio-3.x 系支持；对 fun-asr-realtime / paraformer 系发这个
/// 参数很可能直接 400，所以必须门控而不是「有热词就发」。
pub fn qwen_supports_inline_vocabulary(model: &str) -> bool {
    qwen_model_spec(model).is_some_and(|s| s.supports_inline_vocabulary)
}

/// 该模型是否支持 `language_hints`。
pub fn qwen_supports_language_hints(model: &str) -> bool {
    qwen_model_spec(model).is_some_and(|s| s.supports_language_hints)
}

/// 该模型的同步 body 是否用旧格式（`{"audio": ...}` 而非 `input_audio`）。
pub(crate) fn qwen_uses_legacy_audio_content(model: &str) -> bool {
    qwen_model_spec(model).is_some_and(|s| s.use_legacy_audio_content)
}

/// 给定地域下该走哪种协议的默认模型。
///
/// `ws = true` 取流式首选，`false` 取同步默认。地域内无对应模型时回退到
/// 清单里第一个同协议模型；清单为空（不可能）时回退第一个模型。
///
/// 取代了两处硬编码回退：`QwenAsrProvider::MODEL` 常量与
/// `asr_start_streaming` 里的 `"paraformer-realtime-v2"` 字面量。
pub fn qwen_default_model(ws: bool, region: DashScopeRegion) -> &'static str {
    // 只在「已列出」的模型里挑默认：隐藏的历史模型不该被选为当前模型
    let avail = |s: &QwenModelSpec| s.listed && s.regions.contains(&region) && s.ws_realtime == ws;
    QWEN_MODELS
        .iter()
        .find(|s| {
            avail(s)
                && if ws {
                    s.is_default_stream
                } else {
                    s.is_default_batch
                }
        })
        .or_else(|| QWEN_MODELS.iter().find(|s| avail(s)))
        .or_else(|| QWEN_MODELS.first())
        .map(|s| s.id)
        .unwrap_or("qwen-audio-3.0-asr-flash")
}
