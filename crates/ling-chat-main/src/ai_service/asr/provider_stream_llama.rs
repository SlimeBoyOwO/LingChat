//! **通用 OpenAI 兼容 SSE 结果流式客户端**（llama-server 实测起步，名字带 llama 是历史遗留）。
//!
//! 协议：`POST {endpoint}/v1/audio/transcriptions` multipart 上传整段音频 +
//! `stream=true` 让**识别结果**以 SSE 增量返回（OpenAI 兼容语义：每条 `data`
//! 是当前累积的完整转录，非 delta）。
//!
//! 与 [`super::provider_stream`]（DashScope WebSocket 真流式）的本质区别：
//! - **音频仍整段 multipart 上传**——llama-server 没有流式音频输入（Qwen3-ASR
//!   是非因果 encoder，非增量架构）
//! - partial 经 `on_partial` 回调发射（与 qwen WS 流式**同 key 同语义**：
//!   整段累积视图，前端整体替换语音追加块）——前端共用监听零改动。
//!   回调由 session / 命令层注入（事件发射上移，本模块不依赖 Tauri AppHandle）
//!
//! 复用范围：凡 OpenAI 兼容的转写服务（whisper.cpp / faster-whisper-server /
//! Groq / DashScope compatible-mode qwen-audio-asr）都可直接复用本模块——
//! [`super::provider::parse_llama_text`] 对 `<asr_text>` 标记自动检测
//! （无标记的整体当文本），纯 OpenAI 文本响应无需改动。
//!
//! 调用方：`LlamaAsrProvider::stream_recognize`（trait 方法，见 provider.rs 扩展指南）。

use futures_util::StreamExt;
use reqwest::multipart::Form;
use tracing::debug;

use super::error::{AsrError, map_reqwest_error};
use super::provider::{AsrResult, PartialCallback, ProviderCredentials, parse_llama_text};

/// SSE 帧解析结果。
#[derive(Debug, PartialEq)]
pub enum SseEvent {
    /// 一条 `data:` 载荷（原始内容，可能是 JSON 或 `[DONE]`）。
    Text(String),
    /// `data: [DONE]`——流结束标记。
    Done,
}

/// 解析一个 SSE 帧（`\n\n` 分隔的文本块）。
///
/// 逐行找 `data:` 前缀；`[DONE]` 返回 [`SseEvent::Done`]；其余非空 data 返回
/// [`SseEvent::Text`]。无 data 行（注释行/事件名行/空帧）返回 None。
pub fn parse_sse_event(frame: &str) -> Option<SseEvent> {
    for line in frame.lines() {
        // 非 data 行（event 名/注释/空行）跳过，不能提前返回——标准 SSE 帧
        // 是 "event: xxx\ndata: {...}" 两行结构
        let Some(rest) = line.strip_prefix("data:") else {
            continue;
        };
        let data = rest.trim();
        if data.is_empty() {
            continue;
        }
        if data == "[DONE]" {
            return Some(SseEvent::Done);
        }
        return Some(SseEvent::Text(data.to_string()));
    }
    None
}

/// 从字节 buffer 提取所有以 `\n` 结尾的完整行，返回解码后的行列表，
/// 未闭合字节留在 buffer 中等待下一 chunk。
///
/// 必须字节级切分再解码：网络 chunk 可能切断多字节 UTF-8 字符（SSE 转写
/// 文本以中文为主），逐 chunk `from_utf8_lossy` 会把半截字符替换成 � 导致
/// 乱码。`\n` 是 ASCII（多字节字符的字节值 ∈ {0x80..}，永不含 0x0A），
/// 以 `\n` 为界的行内 UTF-8 序列必然完整。
///
/// 行尾 `\r`（CRLF 服务端）在此 trim，`parse_sse_event` 只处理 `data:` 行。
fn extract_lines(buf: &mut Vec<u8>) -> Vec<String> {
    let mut lines = Vec::new();
    while let Some(pos) = buf.iter().position(|&b| b == b'\n') {
        let line = String::from_utf8_lossy(&buf[..pos])
            .trim_end_matches('\r')
            .to_string();
        buf.drain(..=pos);
        lines.push(line);
    }
    lines
}

/// 发起结果流式识别：整段 WAV multipart 上传 + SSE 增量 partial + 返回 final。
///
/// - 每个 partial 以累积完整文本经 `on_partial` 回调发射（`parse_llama_text`
///   切 `<asr_text>` 取文本，与整句识别同一解析）——事件发射由调用方负责
///   （session / 命令层注入回调），本模块不依赖 Tauri AppHandle
/// - 热词接口复用：`prompt` 非空时带 `prompt` 字段。偏置文本的构造（去重、
///   长度上限）是 provider 的策略，本模块只负责发送——保持「通用 SSE 客户端」
///   的定位，不依赖热词的具体形态
/// - 无 `[DONE]` 正常断开时以最后一条 partial 为 final
pub async fn recognize_stream(
    http: &reqwest::Client,
    cred: &ProviderCredentials,
    endpoint: &str,
    model: &str,
    prompt: Option<String>,
    wav_bytes: Vec<u8>,
    on_partial: Option<PartialCallback>,
) -> Result<AsrResult, AsrError> {
    let url = format!("{endpoint}/v1/audio/transcriptions");
    debug!("[ASR/llama-stream] 上传整段 WAV 流式转写: {url} (model={model})");

    let mut form = Form::new()
        .text("model", model.to_string())
        .text("response_format", "json")
        .text("stream", "true")
        .part(
            "file",
            reqwest::multipart::Part::bytes(wav_bytes)
                .file_name("audio.wav")
                .mime_str("audio/wav")
                .map_err(|e| AsrError::ProviderApiError {
                    provider: "llama-asr".into(),
                    message: format!("构造 multipart 失败: {e}"),
                })?,
        );
    // 热词接口：prompt 上下文偏置（与整句识别同一构造，见
    // provider::llama_prompt_from_hotwords）
    if let Some(p) = prompt {
        debug!(
            "[ASR/llama-stream] prompt 偏置（{} 字符）: {p}",
            p.chars().count()
        );
        form = form.text("prompt", p);
    }

    let mut req = http.post(&url).multipart(form);
    if cred.has_api_key() {
        req = req.bearer_auth(&cred.api_key);
    }
    let resp = req.send().await.map_err(map_reqwest_error)?;

    let status = resp.status();
    if status == reqwest::StatusCode::REQUEST_TIMEOUT
        || status == reqwest::StatusCode::GATEWAY_TIMEOUT
    {
        return Err(AsrError::ProviderTimeout("llama-asr".into()));
    }
    if !status.is_success() {
        let body = resp.text().await.unwrap_or_default();
        return Err(AsrError::ProviderApiError {
            provider: "llama-asr".into(),
            message: format!("HTTP {status}: {body}"),
        });
    }

    // SSE 解析：字节级逐行切分（见 extract_lines 注释——UTF-8 chunk 边界安全）。
    // 逐行处理对 `\n` 与 `\n\n` 分隔都鲁棒（llama-server 实测两条 data 相邻、
    // 无空行分隔；`\n\n` 帧切分会把 [DONE] 粘进上一帧导致丢失）。
    let mut buf: Vec<u8> = Vec::new();
    let mut bytes = resp.bytes_stream();
    let mut final_text = String::new();
    let mut final_lang: Option<String> = None;
    while let Some(chunk) = bytes.next().await {
        let chunk = chunk.map_err(map_reqwest_error)?;
        buf.extend_from_slice(&chunk);
        for line in extract_lines(&mut buf) {
            match parse_sse_event(&line) {
                Some(SseEvent::Done) => {
                    debug!(
                        "[ASR/llama-stream] [DONE]，final 文本 {} 字",
                        final_text.chars().count()
                    );
                    return Ok(AsrResult {
                        text: final_text,
                        language: final_lang,
                        confidence: None,
                        provider_id: "llama-asr".into(),
                    });
                },
                Some(SseEvent::Text(raw)) => {
                    // OpenAI 兼容语义：data 为当前累积完整转录 → 整体替换视图
                    if let Some((text, lang)) = parse_llama_text(&raw) {
                        final_text = text.clone();
                        if lang.is_some() {
                            final_lang = lang;
                        }
                        // 空文本（无语音 `language None<asr_text>`）不回调——
                        // 空 partial 会清空输入框的语音追加块
                        if !text.is_empty() {
                            if let Some(cb) = &on_partial {
                                cb(&final_text);
                            }
                        }
                    }
                },
                None => {},
            }
        }
    }

    // 连接正常关闭但无 [DONE]（llama-server 行为差异）→ 以最后 partial 为 final
    debug!(
        "[ASR/llama-stream] 连接关闭（无 [DONE]），final 文本 {} 字",
        final_text.chars().count()
    );
    Ok(AsrResult {
        text: final_text,
        language: final_lang,
        confidence: None,
        provider_id: "llama-asr".into(),
    })
}
