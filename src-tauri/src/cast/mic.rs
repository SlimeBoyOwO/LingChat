//! 投屏侧麦克风 → 语音转文字（ASR）。
//!
//! 协议（client → server，经投屏服务的 `/ws`）：
//! ```json
//! { "type": "cast:mic", "action": "start" | "data" | "end",
//!   "format": "pcm16" | "opus", "data": "<base64 音频帧>" }
//! ```
//!
//! server → client 预留事件（远程同步播放音频）：
//! ```json
//! { "type": "cast:audio", "action": "play" | "stop", "kind": "bgm" | "voice", "url": "..." }
//! ```
//!
//! 输入走现有 ASR 会话的一次性识别（[`AsrSession::recognize_wav`]）：
//! - 客户端驱动分段（push-to-talk）：`start` 起缓冲、`data` 累积、`end` 触发识别；
//! - 识别结果由上层（[`super::server`]）以 `cast:mic:recognized` 事件发给投屏窗口，
//!   投屏窗口再走前端 sendMessage 注入对话；
//! - 不接入 VAD / auto 模式，避免与主窗口本地 auto-listen 争抢单会话端点检测。

use base64::Engine;
use base64::engine::general_purpose::STANDARD as BASE64;
use tauri::{AppHandle, Manager};
use tokio::sync::Mutex as AsyncMutex;

use crate::AppState;
use crate::ai_service::asr::settings;

/// 一段待识别的音频缓冲（16kHz mono f32，跨 `data` 帧累积）。
#[derive(Default)]
pub struct MicBuffer {
    pub pcm: Vec<f32>,
    /// `start` 帧协商的音频格式（缺省按 pcm16 处理）。设备端只在 `start` 帧带
    /// `format`，`data` 帧不带——这里记住协商结果，后续 `data` 帧沿用。
    pub format: Option<String>,
}

/// 跨连接串行化识别调用：ASR provider 是共享单例，并发打 API 会互相干扰/重复计费。
/// 用 `try_lock` 让后到的识别立即以「识别中」拒绝，而不是排队等待（客户端可自行稍后重试）。
static RECOGNITION_LOCK: AsyncMutex<()> = AsyncMutex::const_new(());

/// 处理一帧来自投屏客户端的麦克风音频。
///
/// - `start`：重置缓冲，并把帧内 `format` 存入缓冲（协商结果）。
/// - `data`：把 base64 PCM16 帧解码追加到缓冲。生效格式取 `start` 协商值 →
///   帧内值 → 兜底 `pcm16`（唯一支持的输入格式）；显式 `opus` 等仍报错。
/// - `end`：缓冲非空时整段一次性识别，返回 `Ok(Some(text))`；空缓冲返回 `Ok(None)`。
///
/// 其余 action 返回 `Ok(None)`；出错返回 `Err(用户可读错误)`。
pub async fn on_frame(
    app: &AppHandle,
    buf: &mut Option<MicBuffer>,
    action: &str,
    format: Option<&str>,
    data: Option<&str>,
) -> Result<Option<String>, String> {
    match action {
        "start" => {
            *buf = Some(MicBuffer {
                pcm: Vec::new(),
                format: format.map(str::to_owned),
            });
            Ok(None)
        },
        "data" => {
            // 生效格式：start 协商值优先，其次帧内 format，最后兜底 pcm16。
            // 设备端只在 start 帧带 format，缺失/未知一律按唯一支持的 pcm16 处理，
            // 不再因缺少 format 而拒绝整段识别。
            let effective = buf
                .as_ref()
                .and_then(|b| b.format.clone())
                .or_else(|| format.map(str::to_owned))
                .unwrap_or_else(|| "pcm16".to_string());
            let Some(samples) = decode_pcm16_frames(data, &effective)? else {
                return Ok(None);
            };
            let mic = buf.get_or_insert_with(MicBuffer::default);
            mic.pcm.extend_from_slice(&samples);
            Ok(None)
        },
        "end" => {
            let Some(mic) = buf.as_mut() else {
                return Ok(None);
            };
            if mic.pcm.is_empty() {
                return Ok(None);
            }
            let pcm = std::mem::take(&mut mic.pcm);
            let text = recognize(app, &pcm).await?;
            if text.trim().is_empty() {
                return Ok(None);
            }
            Ok(Some(text))
        },
        _ => Ok(None),
    }
}

/// base64 → PCM16 小端字节 → f32 采样（÷32768，16kHz mono，按 ASR 契约）。
fn decode_pcm16_frames(data: Option<&str>, format: &str) -> Result<Option<Vec<f32>>, String> {
    if format != "pcm16" {
        return Err(format!(
            "暂不支持 {format} 格式，请发送 pcm16（16kHz mono）"
        ));
    }
    let Some(b64) = data.filter(|s| !s.is_empty()) else {
        return Ok(None);
    };
    let bytes = BASE64
        .decode(b64)
        .map_err(|e| format!("音频 base64 解码失败: {e}"))?;
    if bytes.is_empty() {
        return Ok(None);
    }
    // 容忍客户端分帧对齐问题：非 2 的倍数截断到最近偶数
    let bytes = &bytes[..bytes.len() / 2 * 2];
    let samples: Vec<f32> = bytes
        .chunks_exact(2)
        .map(|c| {
            let s = i16::from_le_bytes([c[0], c[1]]);
            s as f32 / 32768.0
        })
        .collect();
    Ok(Some(samples))
}

/// 对整段缓冲做一次性识别，返回识别文本。
async fn recognize(app: &AppHandle, pcm: &[f32]) -> Result<String, String> {
    let session = {
        let state = app.state::<AppState>();
        let guard = state.asr_state.session.lock().await;
        guard
            .as_ref()
            .cloned()
            .ok_or_else(|| "ASR 未初始化，请在设置中配置语音识别".to_string())?
    };

    let provider_id = settings::load(app)
        .map(|s| s.active_provider)
        .map_err(|e| format!("读取 ASR 设置失败: {e}"))?;

    let wav = build_wav_pcm16(pcm, 16_000);

    // 串行化识别：有别的连接正在识别则立即拒绝（客户端可稍后重试）
    let _guard = RECOGNITION_LOCK
        .try_lock()
        .map_err(|_| "正在识别中，请稍后再试".to_string())?;

    let result = session
        .recognize_wav(provider_id.clone(), wav, None)
        .await
        .map_err(|e| format!("语音识别失败: {e}"))?;
    tracing::info!(
        "[Cast][mic] 识别完成（provider={provider_id}）: {:?}",
        result.text
    );
    Ok(result.text)
}

/// 标准 PCM WAV 44 字节文件头（`/voice` 降采样与 ASR 打包共用，见 [super::server]）。
pub fn build_wav_header(data_len: u32, sample_rate: u32, channels: u16, bits: u16) -> Vec<u8> {
    let block_align = channels * (bits / 8);
    let byte_rate = sample_rate * block_align as u32;
    let mut buf = Vec::with_capacity(44);
    buf.extend_from_slice(b"RIFF");
    buf.extend_from_slice(&(36 + data_len).to_le_bytes());
    buf.extend_from_slice(b"WAVE");
    buf.extend_from_slice(b"fmt ");
    buf.extend_from_slice(&16u32.to_le_bytes());
    buf.extend_from_slice(&1u16.to_le_bytes()); // PCM
    buf.extend_from_slice(&channels.to_le_bytes());
    buf.extend_from_slice(&sample_rate.to_le_bytes());
    buf.extend_from_slice(&byte_rate.to_le_bytes());
    buf.extend_from_slice(&block_align.to_le_bytes());
    buf.extend_from_slice(&bits.to_le_bytes());
    buf.extend_from_slice(b"data");
    buf.extend_from_slice(&data_len.to_le_bytes());
    buf
}

/// 把 16kHz mono f32 采样打包成标准 PCM16 WAV（与前端 `pcmToWavPcm16` / `synth_silence_wav` 同格式）。
fn build_wav_pcm16(samples: &[f32], sample_rate: u32) -> Vec<u8> {
    let mut buf = build_wav_header((samples.len() * 2) as u32, sample_rate, 1, 16);
    for s in samples {
        let i = (s.clamp(-1.0, 1.0) * 32767.0) as i16;
        buf.extend_from_slice(&i.to_le_bytes());
    }
    buf
}
