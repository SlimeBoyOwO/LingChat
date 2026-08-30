//! DashScope 实时语音识别 WebSocket 客户端（paraformer-realtime-v2）。
//!
//! 协议（通过官方 Python SDK（dashscope 1.27）debug 日志实证，非猜测）：
//! - 端点默认 `wss://dashscope.aliyuncs.com/api-ws/v1/inference`（**无 query 参数**；
//!   可由设置 `provider_configs[id].endpoint` 覆盖）；
//!   `/api/v1/services/audio/asr/recognition` 是另一个 HTTP API，不是实时端点）
//! - 标准 WebSocket 升级（GET）+ `Authorization: Bearer <api_key>` header
//! - 客户端事件（文本帧）：
//!   - run-task（start）：`{"header":{"streaming":"duplex","task_id":"<32 hex>",
//!     "action":"run-task"},"payload":{"model":"paraformer-realtime-v2",
//!     "parameters":{"sample_rate":16000,"format":"pcm","language_hints":[...]},
//!     "input":{},"task":"asr","task_group":"audio","function":"recognition"}}`
//!   - 音频：**裸 PCM16 二进制帧**（无 DashScope 帧头）
//!   - finish-task（stop）：`{"header":{"streaming":"duplex","task_id":"...",
//!     "action":"finish-task"},"payload":{"input":{}}}`
//! - 服务端事件（文本帧，header.event）：
//!   - task-started / result-generated / task-finished / task-failed
//!   - result-generated：`{"output":{"sentence":{"index":N,"time":..,"text":"..",
//!     "begin_time":..,"end_time":..}}}` —— `end_time` 存在 = 该句定稿（final）
//!
//! 设计：`start_streaming` 建连 + 发 run-task 后 spawn 读写分离 task 常驻后台。
//! 音频块经 `StreamCommand::Audio` 转发为裸二进制帧；partial 文本经
//! `asr://stream_partial` 事件实时 emit（整段累积视图 = 已定稿句 + 当前句
//! partial）；`StreamCommand::Stop` 发 finish-task 后等 task-finished，
//! 整段文本经 oneshot 回传。

use futures_util::{SinkExt, StreamExt};
use serde_json::Value as JsonValue;
use serde_json::json;
use std::sync::Arc;
use tokio::sync::{mpsc, oneshot};
use tokio_tungstenite::tungstenite::Message;
use tokio_tungstenite::tungstenite::client::IntoClientRequest;
use tokio_tungstenite::tungstenite::http;
use tracing::{debug, warn};

use super::error::AsrError;

pub const WS_URL: &str = "wss://dashscope.aliyuncs.com/api-ws/v1/inference";

/// 流式会话命令（由 session 侧转发）。
pub enum StreamCommand {
    /// 待识别 PCM（16kHz mono f32）块，写循环转 PCM16 后发裸二进制帧。
    Audio(Vec<f32>),
    /// 停止：发 finish-task 文本帧，等服务端 task-finished 后回传整段文本。
    Stop {
        reply: oneshot::Sender<Result<StreamResult, AsrError>>,
    },
    /// 丢弃会话（前端切界面等）：发 finish-task 让服务端干净收尾，不等待结果。
    /// 与 Stop 的区别：无 reply、无超时等待；服务端不回时由连接关闭兜底退出。
    Abort,
}

/// 流式识别结果（整段 final 文本）。
pub struct StreamResult {
    pub text: String,
}

/// 服务端事件（解析后的结构化形式）。
#[derive(Debug, PartialEq)]
enum ServerEvent {
    /// task-started：run-task 被接受，可开始发音频。
    Started,
    /// result-generated：同一句 partial 整体累积更新；`is_final` 时该句定稿。
    Transcript {
        index: u32,
        text: String,
        is_final: bool,
    },
    /// task-finished：结束（整段文本以本地 buffer 累积为准）。
    Finished,
    /// task-failed：header.error_code / error_message。
    Error { code: String, message: String },
}

/// f32 PCM（-1..1）→ 16-bit PCM 小端字节（clamp 越界）。
/// 协议是裸 PCM16 二进制帧（format=pcm），无容器头。
pub fn pcm_f32_to_i16(pcm: &[f32]) -> Vec<u8> {
    let mut out = Vec::with_capacity(pcm.len() * 2);
    for &s in pcm {
        let clamped = s.clamp(-1.0, 1.0);
        let v = (clamped * 32767.0).round() as i16;
        out.extend_from_slice(&v.to_le_bytes());
    }
    out
}

/// 解析服务端文本帧。非事件 / 无法解析返回 None。
fn parse_server_event(text: &str) -> Option<ServerEvent> {
    let v: JsonValue = serde_json::from_str(text).ok()?;
    let event = v.get("header")?.get("event")?.as_str()?;
    match event {
        "task-started" => Some(ServerEvent::Started),
        "result-generated" => {
            let s = v.get("payload")?.get("output")?.get("sentence")?;
            let index = s.get("index").and_then(|i| i.as_u64()).unwrap_or(0) as u32;
            let text = s
                .get("text")
                .and_then(|t| t.as_str())
                .unwrap_or("")
                .to_string();
            // 句子定稿标志：end_time 存在（官方 SDK RecognitionResult.is_sentence_end）
            let is_final = s.get("end_time").and_then(|t| t.as_u64()).is_some();
            Some(ServerEvent::Transcript {
                index,
                text,
                is_final,
            })
        },
        "task-finished" => Some(ServerEvent::Finished),
        "task-failed" => {
            let header = v.get("header")?;
            let code = header
                .get("error_code")
                .and_then(|c| c.as_str())
                .unwrap_or("?")
                .to_string();
            let message = header
                .get("error_message")
                .and_then(|m| m.as_str())
                .unwrap_or("?")
                .to_string();
            Some(ServerEvent::Error { code, message })
        },
        _ => None,
    }
}

/// 构造 run-task（start）事件 JSON。返回 (task_id, body)——
/// task_id 需在 finish-task 时复用（服务端按 task_id 关联任务）。
fn build_run_task_payload(model: &str, language_hint: Option<&str>) -> (String, Vec<u8>) {
    // task_id：32 位 hex（官方 SDK uuid4().hex）
    let task_id = uuid::Uuid::new_v4().to_string().replace('-', "");
    let mut payload = json!({
        "header": {
            "streaming": "duplex",
            "task_id": task_id,
            "action": "run-task"
        },
        "payload": {
            "model": model,
            "parameters": {
                "sample_rate": 16000,
                "format": "pcm"
            },
            "input": {},
            "task": "asr",
            "task_group": "audio",
            "function": "recognition"
        }
    });
    if let Some(lang) = language_hint {
        payload["payload"]["parameters"]["language_hints"] = json!([lang]);
    }
    (
        task_id,
        serde_json::to_vec(&payload).expect("run-task payload 序列化不应失败"),
    )
}

/// 构造 finish-task（stop）事件 JSON（复用 run-task 的 task_id）。
fn build_finish_task_payload(task_id: &str) -> Vec<u8> {
    serde_json::to_vec(&json!({
        "header": {
            "streaming": "duplex",
            "task_id": task_id,
            "action": "finish-task"
        },
        "payload": {
            "input": {}
        }
    }))
    .expect("finish-task payload 序列化不应失败")
}

/// 建立 WebSocket 连接 + 发 run-task 事件 + spawn 读写分离 task。
///
/// 返回命令通道发送端。连接与读写完全在后台 task，不占用调用方。
/// partial 文本经 `on_partial` 回调实时发射（整段累积视图：已定稿句 + 当前句
/// partial，前端整体替换输入框的语音追加块）——事件发射由调用方
/// （session 层）注入回调，本模块不依赖 Tauri AppHandle。
pub async fn start_streaming(
    on_partial: Arc<dyn Fn(&str) + Send + Sync>,
    endpoint: String,
    api_key: String,
    model: String,
    language_hint: Option<String>,
) -> Result<mpsc::UnboundedSender<StreamCommand>, AsrError> {
    // 端点可配置：设置为空时用默认 WS_URL
    let ws_url = if endpoint.trim().is_empty() {
        WS_URL.to_string()
    } else {
        endpoint
    };
    debug!("[ASR/stream] 连接 DashScope 实时识别: {ws_url} (model={model})");

    // 标准 GET 升级 + Bearer 鉴权
    let mut request = ws_url
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
    let (task_id, run_task_body) = build_run_task_payload(&model, language_hint.as_deref());
    ws.send(Message::Text(
        String::from_utf8(run_task_body)
            .expect("run-task 是合法 UTF-8")
            .into(),
    ))
    .await
    .map_err(|e| AsrError::ProviderApiError {
        provider: "qwen-asr".into(),
        message: format!("发送 run-task 失败: {e}"),
    })?;

    let (mut write, mut read) = ws.split();
    let (tx, mut rx) = mpsc::unbounded_channel::<StreamCommand>();

    tokio::spawn(async move {
        let mut buffer = String::new(); // 已定稿句子累积
        let mut current = String::new(); // 当前句 partial
        let mut pending_reply: Option<oneshot::Sender<Result<StreamResult, AsrError>>> = None;
        let mut stopped = false;
        let mut aborted = false;

        loop {
            tokio::select! {
                Some(cmd) = rx.recv() => {
                    match cmd {
                        StreamCommand::Audio(pcm) => {
                            if stopped {
                                continue;
                            }
                            // 裸 PCM16 二进制帧（无容器头）
                            let bytes = pcm_f32_to_i16(&pcm);
                            if write.send(Message::Binary(bytes.into())).await.is_err() {
                                break;
                            }
                        }
                        StreamCommand::Stop { reply } => {
                            pending_reply = Some(reply);
                            let frame = build_finish_task_payload(&task_id);
                            if write
                                .send(Message::Text(
                                    String::from_utf8(frame)
                                        .expect("finish-task 是合法 UTF-8")
                                        .into(),
                                ))
                                .await
                                .is_err()
                            {
                                if let Some(r) = pending_reply.take() {
                                    let _ = r.send(Err(AsrError::ProviderApiError {
                                        provider: "qwen-asr".into(),
                                        message: "发送 finish-task 失败".into(),
                                    }));
                                }
                                break;
                            }
                            stopped = true;
                        }
                        StreamCommand::Abort => {
                            // 干净收尾：发 finish-task（无 reply），服务端正常结束任务，
                            // 不报 NO_VALID_AUDIO_ERROR；等 Finished 或连接关闭后退出。
                            // aborted 抑制残余 emit：旧任务 Finished 的 final buffer
                            // 若此时前端已开启新会话（phase 回到 recording），
                            // 会被误当新会话的 partial 写入输入框。
                            let frame = build_finish_task_payload(&task_id);
                            if write
                                .send(Message::Text(
                                    String::from_utf8(frame)
                                        .expect("finish-task 是合法 UTF-8")
                                        .into(),
                                ))
                                .await
                                .is_err()
                            {
                                break;
                            }
                            stopped = true;
                            aborted = true;
                        }
                    }
                }
                msg = read.next() => {
                    match msg {
                        Some(Ok(Message::Text(t))) => {
                            match parse_server_event(&t) {
                                Some(ServerEvent::Started) => {
                                    debug!("[ASR/stream] task-started");
                                }
                                Some(ServerEvent::Transcript { text, is_final, .. }) => {
                                    if is_final {
                                        // 句子定稿：并入 buffer，partial 显示完整定稿内容
                                        buffer.push_str(&text);
                                        current.clear();
                                        on_partial(&buffer);
                                    } else {
                                        // 同一句 partial 整体更新
                                        current = text;
                                        let partial = format!("{buffer}{current}");
                                        on_partial(&partial);
                                    }
                                }
                                Some(ServerEvent::Finished) => {
                                    if let Some(r) = pending_reply.take() {
                                        let _ = r.send(Ok(StreamResult { text: buffer.clone() }));
                                    }
                                    if !aborted {
                                        on_partial(&buffer);
                                    }
                                    break;
                                }
                                Some(ServerEvent::Error { code, message }) => {
                                    warn!("[ASR/stream] 服务端错误: {code} {message}");
                                    if let Some(r) = pending_reply.take() {
                                        let _ = r.send(Err(AsrError::ProviderApiError {
                                            provider: "qwen-asr".into(),
                                            message: format!("{code}: {message}"),
                                        }));
                                    }
                                }
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
    fn pcm_f32_to_i16_roundtrip() {
        // 32767 缩放（±1.0 → ±32767，不取 32768——32768 as i16 会溢出 wrap
        // 成 -32768，正负满幅不对称）。-1.0 → -32767 = 0x8001 小端 [01, 80]。
        let bytes = pcm_f32_to_i16(&[1.0, -1.0, 0.0]);
        assert_eq!(bytes, vec![0xFF, 0x7F, 0x01, 0x80, 0x00, 0x00]);
        // 超出范围被 clamp
        let clamped = pcm_f32_to_i16(&[2.0]);
        assert_eq!(clamped, vec![0xFF, 0x7F]);
    }

    #[test]
    fn parse_task_started() {
        let body = r#"{"header":{"streaming":"duplex","task_id":"abc","action":"run-task","event":"task-started"},"payload":{}}"#;
        assert_eq!(parse_server_event(body), Some(ServerEvent::Started));
    }

    #[test]
    fn parse_partial_sentence() {
        // 无 end_time → partial
        let body = r#"{"header":{"event":"result-generated"},"payload":{"output":{"sentence":{"index":0,"time":100,"text":"你好"}}}}"#;
        assert!(matches!(
            parse_server_event(body),
            Some(ServerEvent::Transcript { text, is_final: false, .. }) if text == "你好"
        ));
    }

    #[test]
    fn parse_final_sentence() {
        // 有 end_time → 定稿
        let body = r#"{"header":{"event":"result-generated"},"payload":{"output":{"sentence":{"index":0,"time":100,"text":"你好世界","begin_time":0,"end_time":100}}}}"#;
        assert!(matches!(
            parse_server_event(body),
            Some(ServerEvent::Transcript { text, is_final: true, .. }) if text == "你好世界"
        ));
    }

    #[test]
    fn parse_task_finished() {
        let body = r#"{"header":{"event":"task-finished"},"payload":{"output":{},"usage":{}}}"#;
        assert_eq!(parse_server_event(body), Some(ServerEvent::Finished));
    }

    #[test]
    fn parse_task_failed() {
        let body = r#"{"header":{"event":"task-failed","error_code":"SomethingWrong","error_message":"识别失败"},"payload":{}}"#;
        assert!(matches!(
            parse_server_event(body),
            Some(ServerEvent::Error { code, message }) if code == "SomethingWrong" && message.contains("识别失败")
        ));
    }

    #[test]
    fn parse_garbage_returns_none() {
        assert!(parse_server_event("not json").is_none());
        assert!(parse_server_event(r#"{"foo":1}"#).is_none());
    }

    #[test]
    fn run_task_payload_has_required_fields() {
        let (task_id, body) = build_run_task_payload("paraformer-realtime-v2", Some("zh"));
        let v: JsonValue = serde_json::from_slice(&body).expect("合法 JSON");
        assert_eq!(v["header"]["action"], "run-task");
        assert_eq!(v["header"]["streaming"], "duplex");
        assert_eq!(v["header"]["task_id"], task_id);
        assert_eq!(v["payload"]["model"], "paraformer-realtime-v2");
        assert_eq!(v["payload"]["task"], "asr");
        assert_eq!(v["payload"]["task_group"], "audio");
        assert_eq!(v["payload"]["function"], "recognition");
        assert_eq!(v["payload"]["parameters"]["format"], "pcm");
        assert_eq!(v["payload"]["parameters"]["sample_rate"], 16000);
        assert_eq!(v["payload"]["parameters"]["language_hints"][0], "zh");
        // task_id 为 32 位 hex
        assert_eq!(task_id.len(), 32);
        assert!(task_id.chars().all(|c| c.is_ascii_hexdigit()));
        // finish-task 复用同一 task_id
        let finish: JsonValue =
            serde_json::from_slice(&build_finish_task_payload(&task_id)).expect("合法 JSON");
        assert_eq!(finish["header"]["action"], "finish-task");
        assert_eq!(finish["header"]["task_id"], task_id);
    }
}
