//! DashScope 实时语音识别 WebSocket 客户端（paraformer-realtime-v2）。
//!
//! 协议要点（官方 Python SDK 行为）：
//! - 端点 `wss://dashscope.aliyuncs.com/api/v1/services/audio/asr/recognition`，
//!   query 带 model / format / sample_rate / enable_partial_results
//! - 鉴权：`Authorization: Bearer <api_key>`（建连时通过 request header 携带）
//! - 二进制帧：17 字节 header（version=1, header_len=16, message_len, data_type, namespace）
//!   小端；data_type = 0x0B0000(JSON) / 0x0A0000(二进制音频)，namespace = 0x0B0000
//! - 客户端 JSON 帧：start（payload 声明 format=pcm / sample_rate=16000 /
//!   enable_partial_results=true / language_hints）、stop
//! - 服务端事件：sentence_start / transcript（同一句整体累积，partial）/
//!   sentence_end（该句定稿）/ result（stop 后整段汇总，权威 final）/
//!   task_failed（失败）
//!
//! 设计：`start_streaming` 建立连接 + 发 start 事件后 spawn 一个读写分离
//! task 常驻后台。音频块经 `StreamCommand::Audio` 转发为二进制帧；partial
//! 文本经 `asr://stream_partial` 事件实时 emit（整段累积视图 = 已定稿句 +
//! 当前句 partial，前端整体替换输入框的语音追加块）；`StreamCommand::Stop`
//! 发 stop 事件后等服务端 result 事件，整段文本经 oneshot 回传。

use std::fmt::Write as _;

use futures_util::{SinkExt, StreamExt};
use serde_json::json;
use serde_json::Value as JsonValue;
use tauri::{AppHandle, Emitter};
use tokio::sync::{mpsc, oneshot};
use tokio_tungstenite::tungstenite::client::IntoClientRequest;
use tokio_tungstenite::tungstenite::http;
use tokio_tungstenite::tungstenite::Message;
use tracing::{debug, warn};

use super::error::AsrError;

const WS_PATH: &str = "wss://dashscope.aliyuncs.com/api/v1/services/audio/asr/recognition";
const MODEL: &str = "paraformer-realtime-v2";
const DATA_TYPE_JSON: u32 = 0x0B0000;
const DATA_TYPE_AUDIO: u32 = 0x0A0000;
const NAMESPACE_AUDIO: u32 = 0x0B0000;

/// 流式会话命令（由 session 侧转发）。
pub enum StreamCommand {
    /// 待识别 PCM（16kHz mono f32）块，写循环转 PCM16 后发二进制帧。
    Audio(Vec<f32>),
    /// 停止：发 stop JSON 帧，等服务端 result 事件后回传整段文本。
    Stop {
        reply: oneshot::Sender<Result<StreamResult, AsrError>>,
    },
}

/// 流式识别结果（整段 final 文本）。
pub struct StreamResult {
    pub text: String,
}

/// 服务端事件（解析后的结构化形式）。
#[derive(Debug, PartialEq)]
enum ServerEvent {
    SentenceStart {
        index: u32,
    },
    Transcript {
        index: u32,
        text: String,
    },
    SentenceEnd {
        index: u32,
    },
    /// result 事件：整段句子数组拼成的最终文本。
    Result {
        text: String,
    },
    Error {
        message: String,
    },
}

/// 打包一帧：17 字节小端 header + payload。
pub fn pack_frame(payload: &[u8], data_type: u32) -> Vec<u8> {
    let mut frame = Vec::with_capacity(17 + payload.len());
    frame.push(1u8); // version
    frame.extend_from_slice(&16u32.to_le_bytes()); // header_len
    frame.extend_from_slice(&(payload.len() as u32).to_le_bytes()); // message_len
    frame.extend_from_slice(&data_type.to_le_bytes()); // data_type
    frame.extend_from_slice(&NAMESPACE_AUDIO.to_le_bytes()); // namespace
    frame.extend_from_slice(payload);
    frame
}

/// f32 PCM（-1..1）→ 16-bit PCM 小端字节（clamp 越界）。
pub fn pcm_f32_to_i16(pcm: &[f32]) -> Vec<u8> {
    let mut out = Vec::with_capacity(pcm.len() * 2);
    for &s in pcm {
        let clamped = s.clamp(-1.0, 1.0);
        let v = (clamped * 32767.0).round() as i16;
        out.extend_from_slice(&v.to_le_bytes());
    }
    out
}

/// 解析服务端 JSON 文本帧。非事件 / 无法解析返回 None。
fn parse_server_event(text: &str) -> Option<ServerEvent> {
    let v: JsonValue = serde_json::from_str(text).ok()?;
    let action = v.get("header")?.get("action")?.as_str()?;
    match action {
        "sentence_start" => {
            let index = v
                .get("payload")
                .and_then(|p| p.get("index"))
                .and_then(|i| i.as_u64())
                .unwrap_or(0) as u32;
            Some(ServerEvent::SentenceStart { index })
        }
        "transcript" => {
            let index = v
                .get("payload")
                .and_then(|p| p.get("index"))
                .and_then(|i| i.as_u64())
                .unwrap_or(0) as u32;
            let text = v
                .get("payload")
                .and_then(|p| p.get("text"))
                .and_then(|t| t.as_str())?;
            Some(ServerEvent::Transcript {
                index,
                text: text.to_string(),
            })
        }
        "sentence_end" => {
            let index = v
                .get("payload")
                .and_then(|p| p.get("index"))
                .and_then(|i| i.as_u64())
                .unwrap_or(0) as u32;
            Some(ServerEvent::SentenceEnd { index })
        }
        "result" => {
            // payload.sentence 数组 → 按顺序拼接各句 text
            let sentences = v
                .get("payload")
                .and_then(|p| p.get("sentence"))
                .and_then(|s| s.as_array());
            let mut text = String::new();
            if let Some(arr) = sentences {
                for s in arr {
                    if let Some(t) = s.get("text").and_then(|t| t.as_str()) {
                        text.push_str(t);
                    }
                }
            } else if let Some(t) = v
                .get("payload")
                .and_then(|p| p.get("text"))
                .and_then(|t| t.as_str())
            {
                text.push_str(t);
            }
            if text.is_empty() {
                return None;
            }
            Some(ServerEvent::Result { text })
        }
        "task_failed" => {
            let message = v
                .get("error")
                .and_then(|e| e.get("message"))
                .and_then(|m| m.as_str())
                .unwrap_or("task_failed")
                .to_string();
            Some(ServerEvent::Error { message })
        }
        _ => None,
    }
}

/// 构造 start 事件 payload（JSON）。
fn build_start_payload(language_hint: Option<&str>) -> Vec<u8> {
    let mut payload = json!({
        "header": {
            "message_id": "msg_1",
            "task_id": "task_1",
            "action": "start",
            "streaming": "duplex"
        },
        "payload": {
            "format": "pcm",
            "sample_rate": 16000,
            "enable_partial_results": true,
        }
    });
    if let Some(lang) = language_hint {
        payload["payload"]["language_hints"] = json!([lang]);
    }
    serde_json::to_vec(&payload).expect("start payload 序列化不应失败")
}

/// 建立 WebSocket 连接 + 发 start 事件 + spawn 读写分离 task。
///
/// 返回命令通道发送端。连接与读写完全在后台 task，不占用调用方。
/// partial 文本通过 `asr://stream_partial` 事件实时 emit（整段累积视图：
/// 已定稿句 + 当前句 partial，前端整体替换输入框的语音追加块）。
pub async fn start_streaming(
    app: AppHandle,
    api_key: String,
    language_hint: Option<String>,
) -> Result<mpsc::UnboundedSender<StreamCommand>, AsrError> {
    // query 参数（language_hints 可选，URL 编码的 JSON 数组字面量）
    let mut url =
        format!("{WS_PATH}?model={MODEL}&format=pcm&sample_rate=16000&enable_partial_results=true");
    if let Some(lang) = language_hint.as_deref() {
        let _ = write!(url, "&language_hints=%5B%22{}%22%5D", lang);
    }
    debug!("[ASR/stream] 连接 DashScope 实时识别: {url}");

    // 鉴权必须在建连时通过 request header 携带（connect_async 之后补 header 无效）
    let mut request = url
        .into_client_request()
        .map_err(|e| AsrError::EngineLoadFailed(format!("构建 WebSocket 请求失败: {e}")))?;
    request.headers_mut().insert(
        http::header::AUTHORIZATION,
        format!("Bearer {api_key}")
            .parse()
            .map_err(|e| AsrError::EngineLoadFailed(format!("header parse: {e}")))?,
    );
    let (ws, _resp) = tokio_tungstenite::connect_async(request)
        .await
        .map_err(|e| AsrError::ProviderApiError {
            provider: "qwen-asr".into(),
            message: format!("WebSocket 连接失败: {e}"),
        })?;
    let mut ws = ws;
    ws.send(Message::Binary(
        pack_frame(
            &build_start_payload(language_hint.as_deref()),
            DATA_TYPE_JSON,
        )
        .into(),
    ))
    .await
    .map_err(|e| AsrError::ProviderApiError {
        provider: "qwen-asr".into(),
        message: format!("发送 start 失败: {e}"),
    })?;

    let (mut write, mut read) = ws.split();
    let (tx, mut rx) = mpsc::unbounded_channel::<StreamCommand>();

    tokio::spawn(async move {
        let mut buffer = String::new(); // 已定稿句子累积
        let mut current = String::new(); // 当前句 partial
        let mut pending_reply: Option<oneshot::Sender<Result<StreamResult, AsrError>>> = None;
        let mut stopped = false;

        loop {
            tokio::select! {
                Some(cmd) = rx.recv() => {
                    match cmd {
                        StreamCommand::Audio(pcm) => {
                            if stopped {
                                continue;
                            }
                            let bytes = pcm_f32_to_i16(&pcm);
                            let frame = pack_frame(&bytes, DATA_TYPE_AUDIO);
                            if write.send(Message::Binary(frame.into())).await.is_err() {
                                break;
                            }
                        }
                        StreamCommand::Stop { reply } => {
                            pending_reply = Some(reply);
                            let frame = pack_frame(
                                b"{\"header\":{\"action\":\"stop\"}}",
                                DATA_TYPE_JSON,
                            );
                            if write.send(Message::Binary(frame.into())).await.is_err() {
                                if let Some(r) = pending_reply.take() {
                                    let _ = r.send(Err(AsrError::ProviderApiError {
                                        provider: "qwen-asr".into(),
                                        message: "发送 stop 失败".into(),
                                    }));
                                }
                                break;
                            }
                            stopped = true;
                        }
                    }
                }
                msg = read.next() => {
                    match msg {
                        Some(Ok(Message::Text(t))) => {
                            match parse_server_event(&t) {
                                Some(ServerEvent::Transcript { text, .. }) => {
                                    current = text;
                                    // 整段累积视图 → partial 字幕
                                    let partial = format!("{buffer}{current}");
                                    let _ = app.emit("asr://stream_partial", partial);
                                }
                                Some(ServerEvent::SentenceEnd { .. }) => {
                                    buffer.push_str(&current);
                                    current.clear();
                                }
                                Some(ServerEvent::Result { text }) => {
                                    buffer = text;
                                    if let Some(r) = pending_reply.take() {
                                        let _ = r.send(Ok(StreamResult { text: buffer.clone() }));
                                    }
                                    let _ = app.emit("asr://stream_partial", buffer.clone());
                                }
                                Some(ServerEvent::Error { message }) => {
                                    warn!("[ASR/stream] 服务端错误: {message}");
                                    if let Some(r) = pending_reply.take() {
                                        let _ = r.send(Err(AsrError::ProviderApiError {
                                            provider: "qwen-asr".into(),
                                            message,
                                        }));
                                    }
                                }
                                Some(ServerEvent::SentenceStart { .. }) => {}
                                None => debug!("[ASR/stream] 未识别事件: {t}"),
                            }
                        }
                        Some(Ok(Message::Close(_))) => break,
                        Some(Ok(Message::Ping(p))) => {
                            let _ = write.send(Message::Pong(p)).await;
                        }
                        Some(Ok(Message::Pong(_))) => {}
                        Some(Ok(_)) => {}
                        Some(Err(e)) => {
                            warn!("[ASR/stream] 连接错误: {e}");
                            if let Some(r) = pending_reply.take() {
                                let _ = r.send(Err(AsrError::ProviderApiError {
                                    provider: "qwen-asr".into(),
                                    message: format!("连接错误: {e}"),
                                }));
                            }
                            break;
                        }
                        None => break,
                    }
                }
            }
            // 命令通道关闭且还有挂起的 stop reply → 兜底为取消
            if rx.is_closed() && pending_reply.is_some() {
                let _ = pending_reply
                    .take()
                    .map(|r| r.send(Err(AsrError::Canceled)));
            }
        }
    });

    Ok(tx)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn pack_frame_header_layout() {
        let payload = b"hello";
        let frame = pack_frame(payload, 0x0A0000);
        assert_eq!(frame.len(), 17 + payload.len());
        assert_eq!(frame[0], 1); // version
        assert_eq!(u32::from_le_bytes(frame[1..5].try_into().unwrap()), 16); // header_len
        assert_eq!(u32::from_le_bytes(frame[5..9].try_into().unwrap()), 5); // message_len
        assert_eq!(
            u32::from_le_bytes(frame[9..13].try_into().unwrap()),
            0x0A0000
        ); // data_type
        assert_eq!(
            u32::from_le_bytes(frame[13..17].try_into().unwrap()),
            0x0B0000
        ); // namespace
        assert_eq!(&frame[17..], payload);
    }

    #[test]
    fn pcm_f32_to_i16_roundtrip() {
        // 1.0 → 32767, -1.0 → -32768, 0.0 → 0（小端）
        let bytes = pcm_f32_to_i16(&[1.0, -1.0, 0.0]);
        assert_eq!(bytes, vec![0xFF, 0x7F, 0x00, 0x80, 0x00, 0x00]);
        // 超出范围被 clamp
        let clamped = pcm_f32_to_i16(&[2.0]);
        assert_eq!(clamped, vec![0xFF, 0x7F]);
    }

    #[test]
    fn parse_transcript_event() {
        let body = r#"{"header":{"action":"transcript"},"payload":{"index":0,"text":"你好世界"},"error":null}"#;
        assert!(matches!(
            parse_server_event(body),
            Some(ServerEvent::Transcript { text, .. }) if text == "你好世界"
        ));
    }

    #[test]
    fn parse_sentence_boundary_events() {
        assert!(matches!(
            parse_server_event(
                r#"{"header":{"action":"sentence_start"},"payload":{"index":0,"time":100}}"#
            ),
            Some(ServerEvent::SentenceStart { .. })
        ));
        assert!(matches!(
            parse_server_event(
                r#"{"header":{"action":"sentence_end"},"payload":{"index":0,"time":100,"time_begin":50}}"#
            ),
            Some(ServerEvent::SentenceEnd { .. })
        ));
    }

    #[test]
    fn parse_result_event_joins_sentences() {
        let body = r#"{"header":{"action":"result"},"payload":{"sentence":[{"text":"第一句","sentence_id":0,"begin_time":0,"end_time":100,"words":[]},{"text":"第二句","sentence_id":1,"begin_time":100,"end_time":200,"words":[]}]}}"#;
        assert!(matches!(
            parse_server_event(body),
            Some(ServerEvent::Result { text }) if text == "第一句第二句"
        ));
    }

    #[test]
    fn parse_task_failed() {
        let body = r#"{"header":{"action":"task_failed"},"payload":{},"error":{"code":"SomethingWrong","message":"识别失败"}}"#;
        assert!(matches!(
            parse_server_event(body),
            Some(ServerEvent::Error { message }) if message.contains("识别失败")
        ));
    }

    #[test]
    fn parse_garbage_returns_none() {
        assert!(parse_server_event("not json").is_none());
        assert!(parse_server_event(r#"{"foo":1}"#).is_none());
    }
}
