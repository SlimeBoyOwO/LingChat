//! 投屏 MJPEG 串流 HTTP 服务（axum）。
//!
//! 路由：
//! - `GET /`        简易预览页（树莓派 / 普通浏览器直接看）
//! - `GET /stream`  MJPEG 流（ESP32 / HoloCubic / ffmpeg / VLC 拉流）
//!   `?w=&h=` 指定输出分辨率（缺省 = 按投屏窗口当前尺寸）
//!   `?fps=&quality=` 覆盖帧率与 JPEG 质量
//! - `GET /ws`      预留：远程音频 / 麦克风 → 语音转文字接口（v1 仅桩，见 [super::mic]）
//!
//! 帧封装与 keepalive 语义复刻 `temp/cast_sender.py`：
//! - boundary `screen_cast_frame`，`multipart/x-mixed-replace`
//! - 每帧 `--screen_cast_frame\r\nContent-Type: image/jpeg\r\nContent-Length: N\r\n\r\n<JPEG>\r\n`
//! - 无新帧 2 秒重发最近一帧保活

use std::time::Duration;

use axum::{
    Router,
    body::Body,
    extract::ws::{Message, WebSocket, WebSocketUpgrade},
    extract::{Path, Query, State as AxumState},
    http::{StatusCode, header},
    response::{IntoResponse, Response},
    routing::get,
};
use futures_util::StreamExt;
use serde::Deserialize;
use tauri::{AppHandle, Emitter};
use tokio::net::TcpListener;
use tokio::sync::{broadcast, mpsc, oneshot};
use tracing::{info, warn};

use super::mic;

pub const DEFAULT_PORT: u16 = 1470;
pub const DEFAULT_FPS: u8 = 15;
pub const DEFAULT_QUALITY: u8 = 80;

const BOUNDARY: &str = "screen_cast_frame";

/// axum 共享状态。
#[derive(Clone)]
struct CastServerState {
    app: tauri::AppHandle,
    /// 串流默认帧率（可被 `?fps=` 覆盖）
    default_fps: u8,
    /// 串流默认 JPEG 质量（可被 `?quality=` 覆盖）
    default_quality: u8,
}

/// 全局服务关闭信号（一次只跑一个投屏服务）。
static SHUTDOWN_TX: std::sync::Mutex<Option<oneshot::Sender<()>>> = std::sync::Mutex::new(None);

/// 投屏 WS 客户端广播通道（服务端 → 客户端，如 `cast:audio` 语音播放帧）。
/// 与 SHUTDOWN_TX 同生命周期：`start_server` 创建、`stop_server` 清空；
/// 清空后已订阅连接的 rx 收到 `RecvError::Closed` 自动断开发送支路。
static BROADCAST_TX: std::sync::Mutex<Option<broadcast::Sender<String>>> =
    std::sync::Mutex::new(None);

/// 启动投屏 HTTP 服务，绑定指定端口，返回实际绑定的端口号。
pub async fn start_server(
    app: tauri::AppHandle,
    port: u16,
    default_fps: u8,
    default_quality: u8,
) -> Result<u16, String> {
    let state = CastServerState {
        app,
        default_fps,
        default_quality,
    };

    let router = Router::new()
        .route("/", get(index_handler))
        .route("/stream", get(stream_handler))
        .route("/voice/:file", get(voice_handler))
        .route("/ws", get(ws_handler))
        .with_state(state);

    let listener = TcpListener::bind(("0.0.0.0", port))
        .await
        .map_err(|e| format!("无法监听端口 {port}（可能已被占用）：{e}"))?;
    let actual = listener
        .local_addr()
        .map_err(|e| format!("获取端口失败: {e}"))?
        .port();

    let (tx, rx) = oneshot::channel::<()>();
    {
        let mut guard = SHUTDOWN_TX.lock().map_err(|e| format!("锁失败: {e}"))?;
        *guard = Some(tx);
    }
    // 建立服务端 → 客户端广播通道（cast:audio 等，均为 JSON 文本帧）
    let (broadcast_tx, _) = broadcast::channel::<String>(32);
    {
        let mut guard = BROADCAST_TX.lock().map_err(|e| format!("锁失败: {e}"))?;
        *guard = Some(broadcast_tx);
    }

    tauri::async_runtime::spawn(async move {
        info!("[Cast] MJPEG 服务已启动在端口 {}", actual);
        axum::serve(listener, router)
            .with_graceful_shutdown(async move {
                let _ = rx.await;
            })
            .await
            .unwrap_or_else(|e| warn!("[Cast] axum 服务错误: {e}"));
    });

    Ok(actual)
}

/// 停止投屏 HTTP 服务。
pub async fn stop_server() -> Result<(), String> {
    let tx = {
        let mut guard = SHUTDOWN_TX.lock().map_err(|e| format!("锁失败: {e}"))?;
        guard.take()
    };
    if let Some(tx) = tx {
        let _ = tx.send(());
        info!("[Cast] 已发送 MJPEG 服务关闭信号");
    }
    // 关闭广播通道：已订阅连接的 rx 收到 Closed 自动断开发送支路
    {
        let mut guard = BROADCAST_TX.lock().map_err(|e| format!("锁失败: {e}"))?;
        *guard = None;
    }
    Ok(())
}

// ─── 查询参数 ──────────────────────────────────────────────

#[derive(Deserialize)]
struct StreamQuery {
    w: Option<u32>,
    h: Option<u32>,
    fps: Option<u8>,
    quality: Option<u8>,
}

// ─── 端点 ──────────────────────────────────────────────────

/// GET / — 简易预览页。
async fn index_handler() -> Response {
    let html = r#"<!doctype html>
<html lang="zh-CN"><head><meta charset="utf-8">
<meta name="viewport" content="width=device-width,initial-scale=1">
<title>LingChat 投屏</title>
<style>
 body{margin:0;background:#0b1020;color:#dbe7f5;font:14px/1.5 system-ui,"Segoe UI",sans-serif;text-align:center}
 h1{font-size:18px;margin:14px 0 4px}
 p{color:#8a94a6;margin:4px 0}
 img{width:min(96vw,640px);border:1px solid #22334a;border-radius:8px;display:block;margin:14px auto}
 .addr{font:13px ui-monospace,Consolas,monospace;color:#71f59b}
</style></head><body>
 <h1>LingChat 投屏</h1>
 <p>MJPEG 流：<span class="addr">/stream</span>
    （可加 <span class="addr">?w=320&amp;h=240&amp;fps=15&amp;quality=80</span>）</p>
 <p>小屏设备建议先按自家屏幕比例调整好电脑上的投屏窗口再拉流。</p>
 <img src="/stream" alt="MJPEG 预览">
</body></html>"#;
    Response::builder()
        .header(header::CONTENT_TYPE, "text/html; charset=utf-8")
        .header(header::CACHE_CONTROL, "no-store")
        .body(Body::from(html))
        .unwrap_or_else(|_| StatusCode::INTERNAL_SERVER_ERROR.into_response())
}

/// GET /stream — MJPEG 流。
async fn stream_handler(
    AxumState(state): AxumState<CastServerState>,
    Query(query): Query<StreamQuery>,
) -> Response {
    let fps = query.fps.unwrap_or(state.default_fps).clamp(1, 30);
    let quality = query.quality.unwrap_or(state.default_quality).clamp(1, 100);
    // 给了 w/h 则 letterbox 到目标尺寸；没给时读设置里的默认输出分辨率
    // （cast.width/height，0 = 按投屏窗口当前原生尺寸）。每路连接读一次，
    // 这样在设置页改分辨率后下一路连接即生效，无需重启串流服务。
    let target = match (query.w, query.h) {
        (Some(w), Some(h)) => Some((w.clamp(160, 1920), h.clamp(160, 1920))),
        _ => {
            let w = super::read_u64(&state.app, crate::config::keys::CAST_WIDTH, 0);
            let h = super::read_u64(&state.app, crate::config::keys::CAST_HEIGHT, 0);
            if w > 0 && h > 0 {
                Some((w.clamp(160, 1920) as u32, h.clamp(160, 1920) as u32))
            } else {
                None
            }
        },
    };
    let interval = Duration::from_millis(1000 / fps as u64);
    // vivid 色彩增强（复刻 cast_sender.py 的 --vivid 预设）。按连接读取设置，
    // 设置页切换后下一路连接即生效，无需重启串流服务。
    let vivid = super::read_bool(&state.app, crate::config::keys::CAST_VIVID, false);

    // 每路连接一个后台抓帧任务（正常场景只有 1~2 个客户端）
    let (tx, rx) = mpsc::channel::<Vec<u8>>(2);
    let state_clone = state.clone();
    tauri::async_runtime::spawn(async move {
        capture_loop(state_clone, tx, target, quality, vivid, interval).await;
    });

    // 消费端：组装 MJPEG 帧；2 秒无新帧则重发最近一帧保活
    let body_stream =
        futures_util::stream::unfold((rx, None::<Vec<u8>>), |(mut rx, mut last)| async move {
            let frame_bytes: Vec<u8> = loop {
                let got = tokio::select! {
                    frame = rx.recv() => frame,
                    _ = tokio::time::sleep(Duration::from_secs(2)) => None,
                };
                match got {
                    Some(bytes) => {
                        last = Some(bytes.clone());
                        break bytes;
                    },
                    None => {
                        if let Some(bytes) = &last {
                            break bytes.clone();
                        }
                        // 还没任何帧：继续等，不结束流
                        continue;
                    },
                }
            };
            let mut part = Vec::with_capacity(frame_bytes.len() + 64);
            part.extend_from_slice(
                format!(
                    "--{BOUNDARY}\r\nContent-Type: image/jpeg\r\nContent-Length: {}\r\n\r\n",
                    frame_bytes.len()
                )
                .as_bytes(),
            );
            part.extend_from_slice(&frame_bytes);
            part.extend_from_slice(b"\r\n");
            Some((
                Ok::<_, std::io::Error>(bytes::Bytes::from(part)),
                (rx, last),
            ))
        });

    Response::builder()
        .header(
            header::CONTENT_TYPE,
            "multipart/x-mixed-replace; boundary=screen_cast_frame",
        )
        .header(header::CACHE_CONTROL, "no-cache, no-store")
        .header(header::CONNECTION, "keep-alive")
        .body(Body::from_stream(body_stream))
        .unwrap_or_else(|_| StatusCode::INTERNAL_SERVER_ERROR.into_response())
}

/// GET /ws — 投屏双向音频通道：
/// - client → server：`cast:mic`（麦克风 → ASR → 前端 sendMessage，见 [super::mic]）
/// - server → client：`cast:audio`（前端触发 AI 回复语音播放时广播给设备）
async fn ws_handler(
    ws: WebSocketUpgrade,
    AxumState(state): AxumState<CastServerState>,
) -> impl IntoResponse {
    let app = state.app.clone();
    ws.on_upgrade(move |socket| handle_ws(app, socket))
}

async fn handle_ws(app: AppHandle, mut socket: WebSocket) {
    info!("[Cast] 投屏 WebSocket 客户端已连接");
    // 订阅服务端广播（cast:audio）；服务未运行时为 None，仅保留 mic 上行
    let mut broadcast_rx = BROADCAST_TX
        .lock()
        .ok()
        .and_then(|g| g.as_ref().map(broadcast::Sender::subscribe));
    // 每连接一段麦克风缓冲（start/data/end 协议）
    let mut mic_buf: Option<mic::MicBuffer> = None;

    loop {
        tokio::select! {
            incoming = socket.next() => {
                match incoming {
                    Some(Ok(Message::Text(text))) => {
                        handle_client_frame(&app, &mut socket, &mut mic_buf, &text).await;
                    }
                    Some(Ok(Message::Close(_))) => break,
                    Some(Err(e)) => {
                        tracing::debug!("[Cast][ws] 读错误: {e}");
                        break;
                    }
                    _ => {}
                }
            }
            broadcast = async {
                match broadcast_rx.as_mut() {
                    Some(rx) => Some(rx.recv().await),
                    None => None,
                }
            } => {
                match broadcast {
                    Some(Ok(text)) => {
                        let _ = socket.send(Message::Text(text.into())).await;
                    }
                    // 服务停止（通道关闭）或未运行：停掉发送支路，连接仅保留 mic 上行
                    _ => broadcast_rx = None,
                }
            }
        }
    }
    info!("[Cast] 投屏 WebSocket 客户端已断开");
}

/// 处理一帧客户端 `cast:mic` 消息：累积音频，`end` 时识别并回执。
async fn handle_client_frame(
    app: &AppHandle,
    socket: &mut WebSocket,
    mic_buf: &mut Option<mic::MicBuffer>,
    text: &str,
) {
    let Some(v) = serde_json::from_str::<serde_json::Value>(text).ok() else {
        return;
    };
    if v.get("type").and_then(|t| t.as_str()) != Some("cast:mic") {
        return;
    }
    let action = v
        .get("action")
        .and_then(|a| a.as_str())
        .unwrap_or("data")
        .to_string();
    // format 按 Option 读取：`start` 帧协商后由 mic::on_frame 存入连接缓冲，
    // `data` 帧缺失时沿用协商值（兜底 pcm16）。不再默认 "unknown" 拒帧。
    let format = v.get("format").and_then(|f| f.as_str());
    let data = v.get("data").and_then(|d| d.as_str());

    match mic::on_frame(app, mic_buf, &action, format, data).await {
        Ok(Some(recognized)) => {
            // 识别成功：发给投屏窗口前端 → dispatch asr-send → sendMessage
            let _ = app.emit(
                "cast:mic:recognized",
                serde_json::json!({ "text": recognized }),
            );
            send_ack(socket).await;
        },
        Ok(None) => send_ack(socket).await,
        Err(e) => {
            tracing::warn!("[Cast][mic] 处理失败: {e}");
            let err = serde_json::json!({"type": "cast:mic", "error": e}).to_string();
            let _ = socket.send(Message::Text(err.into())).await;
        },
    }
}

/// 回执（预留协议：客户端可据此判断服务端已收到）。
async fn send_ack(socket: &mut WebSocket) {
    let ack = serde_json::json!({"type": "cast:mic", "ack": true}).to_string();
    let _ = socket.send(Message::Text(ack.into())).await;
}

/// 广播一条 `cast:audio`(play, voice) 给所有投屏 WS 客户端，供设备同步播放。
/// 由前端在触发对话/播放回复音频时经 `cast_play_voice` 命令调用。
/// 服务未运行 / 无订阅者时安全 no-op。
///
/// URL 发送**相对路径** `/voice/{file}`，由设备按自身配置补全主机端口
/// （stream.lua `absolute_url`，用设备配置里 MJPEG/WS 已在用的 host:port）。
/// 不在这里拼 `http://{host}:{port}`：`get_local_ips()` 只按 192.168/10/172 优先取
/// 第一块网卡，多网卡 / VPN（如 Radmin 26.x）/ 设备在不同网段时可能选出设备
/// 到不了的主机 → 设备 http.get 在连接层即失败（code=-1，`http init failed`）。
/// 由设备解析可保证 URL 与设备正在用的投屏地址一致。
pub async fn broadcast_voice_play(file: &str) -> Result<(), String> {
    let tx = BROADCAST_TX
        .lock()
        .map_err(|e| format!("锁失败: {e}"))?
        .clone();
    let Some(tx) = tx else {
        return Ok(()); // 投屏服务未运行
    };
    let frame = serde_json::json!({
        "type": "cast:audio",
        "action": "play",
        "kind": "voice",
        "url": format!("/voice/{file}"),
    })
    .to_string();
    let _ = tx.send(frame);
    info!("[Cast] 广播语音播放: {file}");
    Ok(())
}

/// GET /voice/{file} — 提供 TTS 语音文件（`<data_dir>/voice/`）供投屏设备拉流播放。
///
/// 设备出声延迟优化：PCM WAV 统一降采样为 16kHz mono 16-bit（[`normalize_voice_wav`]），
/// 下载体积约减半（32k）~减 2.7 倍（44.1k），配合设备端按真实采样率播放；
/// 本地桌面端播放不受影响（放的是原始文件，只改这个 HTTP 出口）。
async fn voice_handler(Path(file): Path<String>) -> Response {
    let base = crate::api::voice_dir();
    let resolved = base.join(&file);
    if !resolved.is_file() {
        return StatusCode::NOT_FOUND.into_response();
    }
    // 路径穿越防护：resolved 必须落在 voice_dir 内（validate 会 canonicalize 解析软链）
    if crate::utils::path::validate_path_in_base(&resolved, &base).is_err() {
        return StatusCode::FORBIDDEN.into_response();
    }
    let raw = match tokio::fs::read(&resolved).await {
        Ok(b) => b,
        Err(_) => return StatusCode::NOT_FOUND.into_response(),
    };
    // mp3 由设备原生解码、audio.info() 能正确探测采样率，透传；WAV 降采样
    let (bytes, mime) = match std::path::Path::new(&file)
        .extension()
        .and_then(|e| e.to_str())
    {
        Some("mp3") => (raw, "audio/mpeg"),
        _ => (normalize_voice_wav(&raw).unwrap_or(raw), "audio/wav"),
    };
    Response::builder()
        .header(header::CONTENT_TYPE, mime)
        .header(header::CONTENT_LENGTH, bytes.len())
        .header(header::CACHE_CONTROL, "no-store")
        .body(Body::from(bytes))
        .unwrap_or_else(|_| StatusCode::INTERNAL_SERVER_ERROR.into_response())
}

/// 若 WAV 非 16kHz mono 16-bit PCM，线性重采样为 16kHz mono 16-bit PCM。
///
/// 返回 `Some(新字节)` 表示已转换；返回 `None` 表示原样透传（已是目标格式、
/// 非 WAV、或非 PCM16 WAV——如 24-bit / IEEE float，无法无损线性换算）。
/// 按 RIFF chunk 遍历解析 `fmt `（format/channels/sample_rate/bits）与 `data` 段，
/// 跳过 `LIST`/`fact` 等多余 chunk；非 mono 先平均混音再重采样。
fn normalize_voice_wav(bytes: &[u8]) -> Option<Vec<u8>> {
    if bytes.len() < 44 || &bytes[0..4] != b"RIFF" || &bytes[8..12] != b"WAVE" {
        return None;
    }
    const FMT: &[u8; 4] = b"fmt ";
    const DATA: &[u8; 4] = b"data";

    let mut fmt: Option<(u16, u16, u32, u16)> = None; // (format, channels, rate, bits)
    let mut data: Option<(usize, usize)> = None; // (offset, len)
    let mut off = 12usize;
    while off + 8 <= bytes.len() {
        let id = &bytes[off..off + 4];
        let size = u32::from_le_bytes([
            bytes[off + 4],
            bytes[off + 5],
            bytes[off + 6],
            bytes[off + 7],
        ]) as usize;
        if id == FMT {
            if off + 8 + 16 <= bytes.len() {
                let audio_format = u16::from_le_bytes([bytes[off + 8], bytes[off + 9]]);
                let channels = u16::from_le_bytes([bytes[off + 10], bytes[off + 11]]);
                let rate = u32::from_le_bytes([
                    bytes[off + 12],
                    bytes[off + 13],
                    bytes[off + 14],
                    bytes[off + 15],
                ]);
                // fmt 体：format(0-1) channels(2-3) rate(4-7) byte_rate(8-11)
                // block_align(12-13) bits_per_sample(14-15)
                let bits = u16::from_le_bytes([bytes[off + 22], bytes[off + 23]]);
                fmt = Some((audio_format, channels, rate, bits));
            }
        } else if id == DATA {
            let data_off = off + 8;
            let data_len = size.min(bytes.len().saturating_sub(data_off));
            data = Some((data_off, data_len));
            break; // data 是最后一个相关 chunk
        }
        off += 8 + size + (size & 1); // chunk 按偶数对齐
    }

    let (audio_format, channels, rate, bits) = fmt?;
    let (data_off, data_len) = data?;
    // 非 16-bit PCM 透传（不引入解码依赖）
    if audio_format != 1 || bits != 16 || data_len % 2 != 0 {
        return None;
    }
    if rate == 16_000 && channels == 1 {
        return None; // 已是目标格式
    }

    let frame_bytes = 2 * channels as usize;
    let in_frames = data_len / frame_bytes;
    if in_frames == 0 {
        return None;
    }

    // 1) 去交织 + 平均混音 → mono f32
    let mono: Vec<f32> = (0..in_frames)
        .map(|f| {
            let base = data_off + f * frame_bytes;
            let sum: i32 = (0..channels as usize)
                .map(|c| {
                    let i = base + c * 2;
                    i16::from_le_bytes([bytes[i], bytes[i + 1]]) as i32
                })
                .sum();
            sum as f32 / (channels as f32 * 32768.0)
        })
        .collect();

    // 2) 线性重采样到 16000（浮点源位置 + 相邻两采样插值）
    let out_frames = ((in_frames as u64 * 16_000 + rate as u64 / 2) / rate as u64) as usize;
    let ratio = rate as f64 / 16_000.0;
    let mut out_i16: Vec<u8> = Vec::with_capacity(out_frames * 2);
    for i in 0..out_frames {
        let src = i as f64 * ratio;
        let idx = src.floor() as usize;
        let frac = (src - idx as f64) as f32;
        let a = mono.get(idx).copied().unwrap_or(0.0);
        let b = mono.get(idx + 1).copied().unwrap_or(a);
        let v = a + (b - a) * frac;
        // round 而非截断：消除 ÷32768×32767 的浮点往返误差（如 2000→1999.8）
        let s = (v.clamp(-1.0, 1.0) * 32767.0).round() as i16;
        out_i16.extend_from_slice(&s.to_le_bytes());
    }

    let mut out = Vec::with_capacity(44 + out_i16.len());
    out.extend_from_slice(&mic::build_wav_header(out_i16.len() as u32, 16_000, 1, 16));
    out.extend_from_slice(&out_i16);
    Some(out)
}

// ─── 抓帧任务 ──────────────────────────────────────────────

/// 后台抓帧：捕获投屏窗口 → letterbox → JPEG，以固定间隔送入通道。
///
/// 捕获失败（窗口未开 / 最小化 / 非 Windows）时发送最近一帧或纯黑帧保活。
async fn capture_loop(
    state: CastServerState,
    tx: mpsc::Sender<Vec<u8>>,
    target: Option<(u32, u32)>,
    quality: u8,
    vivid: bool,
    interval: Duration,
) {
    let mut last: Option<Vec<u8>> = None;
    loop {
        let start = tokio::time::Instant::now();
        // 阻塞式 GDI 抓屏 + JPEG 编码放到阻塞池，避免卡住 tokio 线程
        let frame = {
            let app = state.app.clone();
            tokio::task::spawn_blocking(move || -> Result<Vec<u8>, String> {
                let img = super::capture::capture_cast_window(&app)?;
                let img = match target {
                    Some((w, h)) => super::capture::resize_letterbox(&img, w, h),
                    None => img,
                };
                // vivid 色彩预设：在 resize 之后、JPEG 编码之前（与 cast_sender.py 同序）
                let img = if vivid {
                    super::capture::apply_vivid(&img, 1.4, 1.15)
                } else {
                    img
                };
                super::capture::encode_jpeg(&img, quality)
            })
            .await
            .ok()
            .and_then(|r| r.ok())
        };
        let to_send = match frame {
            Some(bytes) => {
                last = Some(bytes.clone());
                bytes
            },
            None => match &last {
                Some(bytes) => bytes.clone(),
                None => black_jpeg(target, quality),
            },
        };
        if tx.send(to_send).await.is_err() {
            // 客户端断开 / 服务关闭
            break;
        }
        let elapsed = start.elapsed();
        if elapsed < interval {
            tokio::time::sleep(interval - elapsed).await;
        }
    }
}

/// 无任何帧可发时的纯黑兜底帧。
fn black_jpeg(target: Option<(u32, u32)>, quality: u8) -> Vec<u8> {
    let (w, h) = target.unwrap_or((320, 240));
    let img = image::RgbaImage::from_pixel(w, h, image::Rgba([0, 0, 0, 255]));
    super::capture::encode_jpeg(&img, quality).unwrap_or_default()
}

#[cfg(test)]
mod tests {
    use super::*;

    /// 构造一个 PCM WAV（可交错多声道）。
    fn make_wav(sample_rate: u32, channels: u16, bits: u16, samples: &[i16]) -> Vec<u8> {
        let mut out =
            mic::build_wav_header((samples.len() * 2) as u32, sample_rate, channels, bits);
        for s in samples {
            out.extend_from_slice(&s.to_le_bytes());
        }
        out
    }

    fn read_i16(bytes: &[u8], data_off: usize, pos: usize) -> i16 {
        i16::from_le_bytes([bytes[data_off + pos * 2], bytes[data_off + pos * 2 + 1]])
    }

    #[test]
    fn wav_already_16k_mono_passthrough() {
        let wav = make_wav(16_000, 1, 16, &[0, 100, -100, 300]);
        assert!(normalize_voice_wav(&wav).is_none());
    }

    #[test]
    fn wav_32k_downsample_16k() {
        // 32kHz 8 采样 → 16kHz 4 采样；ratio=2，取源 idx 0/2/4/6
        let wav = make_wav(
            32_000,
            1,
            16,
            &[0, 1000, 2000, 3000, 4000, 5000, 6000, 7000],
        );
        let out = normalize_voice_wav(&wav).expect("应重采样");
        assert_eq!(out.len(), 44 + 4 * 2);
        assert_eq!(&out[0..4], b"RIFF");
        assert_eq!(read_i16(&out, 44, 0), 0);
        assert_eq!(read_i16(&out, 44, 1), 2000);
        assert_eq!(read_i16(&out, 44, 2), 4000);
        assert_eq!(read_i16(&out, 44, 3), 6000);
    }

    #[test]
    fn wav_stereo_32k_mixes_then_downsample() {
        // 32kHz 立体声 4 帧 → 16kHz mono 2 帧；帧内左右平均后再抽 idx 0/2
        let wav = make_wav(
            32_000,
            2,
            16,
            &[0, 1000, 2000, 3000, 4000, 5000, 6000, 7000],
        );
        let out = normalize_voice_wav(&wav).expect("应重采样");
        assert_eq!(out.len(), 44 + 2 * 2);
        assert_eq!(read_i16(&out, 44, 0), 500); // avg(0,1000)
        assert_eq!(read_i16(&out, 44, 1), 4500); // avg(4000,5000)
    }

    #[test]
    fn wav_44100_downsample_16k_len() {
        // 44.1kHz 4410 帧 → 16kHz ≈1600 帧；仅校验长度与 RIFF
        let samples: Vec<i16> = (0..4410)
            .map(|i| (((i as i32 * 37) % 32000) - 16000) as i16)
            .collect();
        let wav = make_wav(44_100, 1, 16, &samples);
        let out = normalize_voice_wav(&wav).expect("应重采样");
        let out_frames = (out.len() - 44) / 2;
        let expect_frames = ((4410u64 * 16_000 + 44_100 / 2) / 44_100) as usize;
        assert_eq!(out_frames, expect_frames);
        assert_eq!(&out[24..28], &16_000u32.to_le_bytes());
        assert_eq!(&out[22..24], &1u16.to_le_bytes());
    }

    #[test]
    fn wav_ieee_float_passthrough() {
        // fmt audio_format=3（IEEE float）→ 无法线性换算，原样透传
        let mut wav = Vec::new();
        wav.extend_from_slice(b"RIFF");
        wav.extend_from_slice(&36u32.to_le_bytes());
        wav.extend_from_slice(b"WAVE");
        wav.extend_from_slice(b"fmt ");
        wav.extend_from_slice(&16u32.to_le_bytes());
        wav.extend_from_slice(&3u16.to_le_bytes());
        wav.extend_from_slice(&1u16.to_le_bytes());
        wav.extend_from_slice(&16_000u32.to_le_bytes());
        wav.extend_from_slice(&64_000u32.to_le_bytes());
        wav.extend_from_slice(&2u16.to_le_bytes());
        wav.extend_from_slice(&32u16.to_le_bytes());
        wav.extend_from_slice(b"data");
        wav.extend_from_slice(&8u32.to_le_bytes());
        wav.extend_from_slice(&[0u8; 8]);
        assert!(normalize_voice_wav(&wav).is_none());
    }
}
