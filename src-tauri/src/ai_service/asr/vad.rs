//! Silero VAD v5 端点检测。
//!
//! 加载 bundled `silero-vad.onnx` 模型到 ort Session，对 30ms PCM 块连续
//! 推理出语音概率，喂给 [`vad_segmenter`] 纯状态机做端点切分，
//! 事件映射为 `asr://*` 事件推送给前端。

use std::path::PathBuf;
use std::sync::Arc;

use ndarray::Array2;
use ort::session::Session;
use ort::value::Tensor;
use serde::Serialize;
use tauri::{AppHandle, Emitter};
use tokio::sync::Mutex;

use super::error::AsrError;
use super::vad_segmenter::SegmentEvent;

/// Silero VAD v5（snakers4 master ONNX 导出）：
/// - 输入: `input` (1, 576) = 前帧 64 context + 当前帧 512
///   （官方 utils_vad.py: `torch.cat([self._context, x], dim=1)` —— 只传 512
///   会输出恒定 ~0.001 的 prob，VAD 永不触发）
/// - 输入: `state` (2, 1, 128)（state[0]=h, state[1]=c）、`sr` (int64)
/// - 输出: `output` prob、`stateN` (2, 1, 128)
const VAD_STATE_DIM: usize = 128;
const VAD_CONTEXT_SAMPLES: usize = 64;

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

/// 模型隐状态（h/c 合并为 state）+ 帧 context。
struct VadState {
    /// (2, 1, 128)：state[0]=h, state[1]=c
    state: Array2<f32>,
    /// 前帧尾部 64 samples（模型 context 输入）
    context: Vec<f32>,
}

impl VadState {
    fn new() -> Self {
        Self {
            state: Array2::zeros((2, VAD_STATE_DIM)),
            context: vec![0.0; VAD_CONTEXT_SAMPLES],
        }
    }
}

/// Silero VAD wrapper。
pub struct AsrVad {
    session: Mutex<Option<Session>>,
    state: Arc<Mutex<VadState>>,
    /// 语音切分状态机（独立于模型推理，见 vad_segmenter.rs）。
    segmenter: Mutex<super::vad_segmenter::VadSegmenter>,
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
                AsrError::EngineLoadFailed(format!(
                    "commit_from_file({}): {e}",
                    model_path.display()
                ))
            })?;
        Ok(Self {
            session: Mutex::new(Some(session)),
            state: Arc::new(Mutex::new(VadState::new())),
            segmenter: Mutex::new(super::vad_segmenter::VadSegmenter::new()),
        })
    }

    /// 重置隐状态 + 切分器（每次新会话开始时调用）。
    pub async fn reset(&self) {
        *self.state.lock().await = VadState::new();
        self.segmenter.lock().await.reset();
    }

    /// 设置 VAD 静音计时（毫秒）：停止说话后静音该时长才触发「候选结束」。
    /// 设置页可自定义（默认 800ms）；帧率 30ms/帧，毫秒向上取整到帧。
    pub async fn set_silence_timeout_ms(&self, ms: u32) {
        let frames = (ms as u64).div_ceil(30);
        self.segmenter
            .lock()
            .await
            .set_candidate_silence_frames(frames);
        tracing::info!("[ASR/VAD] 静音计时设置为 {ms}ms（{frames} 帧）");
    }

    /// 处理 30ms 块 PCM（512 samples @ 16kHz），返回推理结果 + 可能的事件。
    /// 推理：拼接前帧 context(64) + 当前帧(512) = 576 → Silero v5 ONNX；
    /// 切分：prob 喂给 [`vad_segmenter::VadSegmenter`] 纯状态机，
    /// 事件映射为 [`VadEvent`] 并 emit。
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

        // 取 state + 拼 context（先算完释放 state 锁再推理）
        let (input_samples, state_tensor) = {
            let mut st = self.state.lock().await;
            let mut input = Vec::with_capacity(VAD_CONTEXT_SAMPLES + pcm.len());
            input.extend_from_slice(&st.context);
            input.extend_from_slice(pcm);
            if pcm.len() >= VAD_CONTEXT_SAMPLES {
                st.context = pcm[pcm.len() - VAD_CONTEXT_SAMPLES..].to_vec();
            }
            let state_tensor = st.state.clone().insert_axis(ndarray::Axis(1)); // (2, 1, 128)
            (input, state_tensor)
        };

        let input_len = input_samples.len();
        let input = ndarray::Array::from_shape_vec((1, input_len), input_samples).map_err(|e| {
            tracing::error!("[ASR/VAD] input shape 构造失败 (len={}): {e}", input_len);
            AsrError::EngineLoadFailed(format!("input shape: {e}"))
        })?;
        let input_t = Tensor::from_array(input).map_err(|e| {
            tracing::error!("[ASR/VAD] input Tensor::from_array 失败: {e}");
            AsrError::EngineLoadFailed(format!("input tensor: {e}"))
        })?;
        let state_t = Tensor::from_array(state_tensor).map_err(|e| {
            tracing::error!("[ASR/VAD] state Tensor::from_array 失败: {e}");
            AsrError::EngineLoadFailed(format!("state tensor: {e}"))
        })?;
        let sr_t = Tensor::from_array(ndarray::arr0(16000i64)).map_err(|e| {
            tracing::error!("[ASR/VAD] sr Tensor::from_array 失败: {e}");
            AsrError::EngineLoadFailed(format!("sr tensor: {e}"))
        })?;

        let outputs = session
            .run(ort::inputs![
                "input" => input_t,
                "state" => state_t,
                "sr" => sr_t,
            ])
            .map_err(|e| {
                tracing::error!("[ASR/VAD] session.run 失败: {e}");
                AsrError::EngineLoadFailed(format!("vad forward: {e}"))
            })?;

        // 解析输出：prob + 更新后的 stateN
        let prob = outputs["output"]
            .try_extract_array::<f32>()
            .map_err(|e| {
                tracing::error!("[ASR/VAD] extract output 失败: {e}");
                AsrError::EngineLoadFailed(format!("extract prob: {e}"))
            })?
            .as_slice()
            .and_then(|s| s.first())
            .copied()
            .unwrap_or(0.0);

        if let Ok(state_n) = outputs["stateN"].try_extract_array::<f32>() {
            let data: Vec<f32> = state_n.iter().copied().collect();
            if let Ok(arr) = Array2::from_shape_vec((2, VAD_STATE_DIM), data) {
                self.state.lock().await.state = arr;
            }
        }

        // 切分状态机：prob → 事件 → emit
        let mut seg = self.segmenter.lock().await;
        let frame = seg.current_frame();
        let events = seg.feed(prob);
        drop(seg);

        // 诊断日志（切分链路观测）：前 10 帧 + 每秒 1 条（33 帧 @30ms），
        // 确认前端 VAD 流在走、prob 是否检测到语音；块长异常直接暴露。
        if pcm.len() != 512 {
            tracing::warn!(
                "[ASR/VAD] chunk 长度异常: {} samples（期望 512）",
                pcm.len()
            );
        }
        if frame < 10 || frame % 33 == 0 {
            tracing::info!("[ASR/VAD] frame={frame} prob={prob:.3} len={}", pcm.len());
        }

        let mut emitted = Vec::new();
        for ev in &events {
            let vad_event = match ev {
                SegmentEvent::SpeechStart { .. } => {
                    tracing::info!("[ASR/VAD] 录入开始 (SpeechStarted, prob={prob:.3})");
                    VadEvent::SpeechStarted
                },
                SegmentEvent::SilenceStart { .. } => VadEvent::SilenceStarted { silence_ms: 0 },
                SegmentEvent::TurnCandidate { silence_frames, .. } => {
                    tracing::info!(
                        "[ASR/VAD] TurnCandidate (silence={}ms)",
                        silence_frames * 30
                    );
                    VadEvent::TurnCandidate {
                        silence_ms: (*silence_frames * 30) as u32,
                    }
                },
                SegmentEvent::TurnSealed { .. } => {
                    tracing::info!("[ASR/VAD] 录入结束 (TurnSealed, 静音 1s 确认)");
                    VadEvent::TurnSealed
                },
            };
            let name = match &vad_event {
                VadEvent::SpeechStarted => "asr://speech_started",
                VadEvent::SilenceStarted { .. } => "asr://silence_started",
                VadEvent::TurnCandidate { .. } => "asr://turn_candidate",
                VadEvent::TurnSealed => "asr://turn_sealed",
            };
            let _ = app.emit(name, &vad_event);
            emitted.push(vad_event);
        }

        Ok(Some(VadProcessResult {
            speech_prob: prob,
            event: emitted.into_iter().next(),
        }))
    }
}

/// 解析 VAD 模型路径：`data_dir/third_party/asr_vad/silero-vad.onnx`。
/// （data_dir 按平台由 [`crate::init::static_copy`] 解析，与 emotion 模型同策略。）
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
