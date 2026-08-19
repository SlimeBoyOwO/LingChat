//! Silero VAD v5 端点检测。
//!
//! 加载 bundled `silero-vad.onnx` 模型到 ort Session，
//! 对 30ms PCM 块连续推理，按 spec §2.4 三阶段状态机：
//! - Phase 1: 粗粒度 silence 累计 ≥300ms → emit TurnCandidate
//! - Phase 2: 1 秒 confirmation 窗口，speech 概率 > 0.5 取消 SEAL
//! - Phase 3: 终态 SEAL → emit TurnSealed
//!
//! 与情绪识别共用 ort 运行时，各持独立 Session。

use std::path::PathBuf;
use std::sync::Arc;
use std::time::Instant as StdInstant;

use ndarray::Array2;
use ort::session::Session;
use ort::value::Tensor;
use serde::Serialize;
use tauri::{AppHandle, Emitter};
use tokio::sync::Mutex;
use tokio_util::sync::CancellationToken;

use super::error::AsrError;

/// Silero VAD v5 隐状态 shape (2, 1, 64) → ndarray (2, 64)
const VAD_STATE_DIM: usize = 64;
const VAD_THRESHOLD: f32 = 0.5;
const VAD_SILENCE_MS_FOR_CANDIDATE: u128 = 300;
const VAD_CONFIRMATION_WINDOW_MS: u128 = 1000;

#[derive(Serialize, Clone, Debug)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum VadEvent {
    SpeechStarted,
    SilenceStarted { silence_ms: u32 },
    TurnCandidate { silence_ms: u32 },
    TurnSealed,
}

#[derive(Serialize, Clone, Debug)]
pub struct VadProcessResult {
    pub speech_prob: f32,
    pub event: Option<VadEvent>,
}

struct VadState {
    h: Array2<f32>,
    c: Array2<f32>,
    last_speech_ts: Option<StdInstant>,
    silence_started_ts: Option<StdInstant>,
    speech_active: bool,
    confirm_token: Option<CancellationToken>,
}

impl VadState {
    fn new() -> Self {
        Self {
            h: Array2::zeros((2, VAD_STATE_DIM)),
            c: Array2::zeros((2, VAD_STATE_DIM)),
            last_speech_ts: None,
            silence_started_ts: None,
            speech_active: false,
            confirm_token: None,
        }
    }
}

/// Silero VAD wrapper。
pub struct AsrVad {
    session: Mutex<Option<Session>>,
    /// 端点检测状态。Arc 共享给 confirmation timer，
    /// 让 timer 触发时可以查询最新 speech_active 决定是否 SEAL。
    state: Arc<Mutex<VadState>>,
}

impl AsrVad {
    /// 从 bundled 路径加载 Silero VAD 模型。
    /// 失败时返回 Err，由调用方决定是否降级为手动模式。
    pub fn load(app: &AppHandle) -> Result<Self, AsrError> {
        let model_path = resolve_vad_model_path(app)?;
        tracing::info!("[ASR/VAD] loading model from {}", model_path.display());
        let session = Session::builder()
            .map_err(|e| AsrError::EngineLoadFailed(format!("SessionBuilder: {e}")))?
            .with_optimization_level(ort::session::builder::GraphOptimizationLevel::Level3)
            .map_err(|e| AsrError::EngineLoadFailed(format!("optimization level: {e}")))?
            .with_intra_threads(1)
            .map_err(|e| AsrError::EngineLoadFailed(format!("intra threads: {e}")))?
            .commit_from_file(model_path.as_path())
            .map_err(|e| {
                AsrError::EngineLoadFailed(format!("commit_from_file({}): {e}", model_path.display()))
            })?;
        Ok(Self {
            session: Mutex::new(Some(session)),
            state: Arc::new(Mutex::new(VadState::new())),
        })
    }

    /// 重置隐状态（每次新会话开始时调用）。
    pub async fn reset(&self) {
        let mut s = self.state.lock().await;
        if let Some(token) = s.confirm_token.take() {
            token.cancel();
        }
        *s = VadState::new();
    }

    /// 处理 30ms 块 PCM（512 samples @ 16kHz），返回推理结果 + 可能的事件。
    /// 端点检测状态机（spec §2.4）：
    /// - prob > 0.5 → speech_active = true；silence_started_ts = None；emit SpeechStarted
    /// - prob ≤ 0.5 且 speech_active：
    ///   - silence_started_ts = Some(now())
    ///   - elapsed ≥ 300ms 且无 confirm_token：emit TurnCandidate + spawn 1s confirmation timer
    /// - confirmation timer 触发：
    ///   - speech_active 仍 true → 取消 SEAL，回到 Listening
    ///   - 否则 → emit TurnSealed
    pub async fn process_chunk(
        &self,
        app: &AppHandle,
        pcm: &[f32],
    ) -> Result<Option<VadProcessResult>, AsrError> {
        let mut session_guard = self.session.lock().await;
        let session = match session_guard.as_mut() {
            Some(s) => s,
            None => return Ok(None), // fail-open: 模型未加载返回 None
        };

        // 取当前 state
        let mut state_guard = self.state.lock().await;
        let state = &mut *state_guard;

        // 构造输入 tensor。Silero VAD 期望输入 shape: [batch=1, samples=512]
        let input = ndarray::Array::from_shape_vec((1, pcm.len()), pcm.to_vec())
            .map_err(|e| AsrError::EngineLoadFailed(format!("input shape: {e}")))?;
        let c_tensor = state.c.clone().insert_axis(ndarray::Axis(1));
        let h_tensor = state.h.clone().insert_axis(ndarray::Axis(1));

        let input_t = Tensor::from_array(input)
            .map_err(|e| AsrError::EngineLoadFailed(format!("input tensor: {e}")))?;
        let h_t = Tensor::from_array(h_tensor)
            .map_err(|e| AsrError::EngineLoadFailed(format!("h tensor: {e}")))?;
        let c_t = Tensor::from_array(c_tensor)
            .map_err(|e| AsrError::EngineLoadFailed(format!("c tensor: {e}")))?;

        let outputs = session
            .run(ort::inputs![
                "input" => input_t,
                "h" => h_t,
                "c" => c_t,
            ])
            .map_err(|e| AsrError::EngineLoadFailed(format!("vad forward: {e}")))?;

        // 解析输出：prob + 更新后的 h, c
        let prob = outputs["output"]
            .try_extract_array::<f32>()
            .map_err(|e| AsrError::EngineLoadFailed(format!("extract prob: {e}")))?
            .as_slice()
            .and_then(|s| s.first())
            .copied()
            .unwrap_or(0.0);

        // 更新 h/c
        if let (Ok(h_view), Ok(c_view)) = (
            outputs["hn"].try_extract_array::<f32>(),
            outputs["cn"].try_extract_array::<f32>(),
        ) {
            let h_data: Vec<f32> = h_view.iter().copied().collect();
            let c_data: Vec<f32> = c_view.iter().copied().collect();
            if let (Ok(h_arr), Ok(c_arr)) = (
                ndarray::Array2::from_shape_vec((2, VAD_STATE_DIM), h_data),
                ndarray::Array2::from_shape_vec((2, VAD_STATE_DIM), c_data),
            ) {
                state.h = h_arr;
                state.c = c_arr;
            }
        }

        let now = StdInstant::now();
        let mut emitted = Vec::new();

        if prob > VAD_THRESHOLD {
            // speech
            state.last_speech_ts = Some(now);
            state.silence_started_ts = None;
            // 取消 confirmation timer
            if let Some(token) = state.confirm_token.take() {
                token.cancel();
            }
            if !state.speech_active {
                state.speech_active = true;
                emitted.push(VadEvent::SpeechStarted);
            }
        } else if state.speech_active {
            // silence after speech
            let silence_start = *state.silence_started_ts.get_or_insert(now);
            let elapsed_ms = now.duration_since(silence_start).as_millis();

            if elapsed_ms >= VAD_SILENCE_MS_FOR_CANDIDATE && state.confirm_token.is_none() {
                emitted.push(VadEvent::SilenceStarted { silence_ms: elapsed_ms as u32 });
                emitted.push(VadEvent::TurnCandidate { silence_ms: elapsed_ms as u32 });

                // spawn 1s confirmation timer
                let token = CancellationToken::new();
                state.confirm_token = Some(token.clone());
                drop(state_guard); // 释放锁，让 timer 可以获取

                let app_clone = app.clone();
                let state_for_timer = self.state.clone();
                tokio::spawn(async move {
                    tokio::time::sleep(std::time::Duration::from_millis(
                        VAD_CONFIRMATION_WINDOW_MS as u64,
                    ))
                    .await;
                    if token.is_cancelled() {
                        return;
                    }
                    // 1 秒到时再判一次 speech_active
                    let still_speech = {
                        let s = state_for_timer.lock().await;
                        s.speech_active
                    };
                    if !still_speech {
                        let _ = app_clone.emit("asr://turn_sealed", &VadEvent::TurnSealed);
                        // 清理 state 中的 confirm_token
                        let mut s = state_for_timer.lock().await;
                        s.confirm_token = None;
                        s.silence_started_ts = None;
                        s.speech_active = false; // 进入下一轮 listening 准备
                    }
                    // else: speech 重启，取消 SEAL，回到 Listening（什么都不 emit）
                });
            }
        }

        for event in &emitted {
            let name = match event {
                VadEvent::SpeechStarted => "asr://speech_started",
                VadEvent::SilenceStarted { .. } => "asr://silence_started",
                VadEvent::TurnCandidate { .. } => "asr://turn_candidate",
                VadEvent::TurnSealed => "asr://turn_sealed",
            };
            let _ = app.emit(name, event);
        }

        Ok(Some(VadProcessResult {
            speech_prob: prob,
            event: emitted.into_iter().next(),
        }))
    }
}

/// 解析 VAD 模型 bundled 路径（与 emotion 同款策略）：
/// - 桌面 release: `app.path().resource_dir()/data/third_party/asr_vad/silero-vad.onnx`
/// - 桌面 debug: `CARGO_MANIFEST_DIR/../data/third_party/asr_vad/silero-vad.onnx`
/// - Android: `static_copy::get_data_dir()/third_party/asr_vad/silero-vad.onnx`
fn resolve_vad_model_path(_app: &AppHandle) -> Result<PathBuf, AsrError> {
    let data_dir = crate::init::static_copy::get_data_dir().clone();
    let path = data_dir
        .join("third_party")
        .join("asr_vad")
        .join("silero-vad.onnx");
    if path.exists() {
        Ok(path)
    } else {
        Err(AsrError::ModelNotFound(path))
    }
}