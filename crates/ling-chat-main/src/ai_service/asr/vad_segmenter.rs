//! 语音切分器（VAD 纯状态机）。
//!
//! 与 ort 推理解耦：本模块只消费「每帧语音概率」（prob 0~1），输出语音段
//! 边界事件。调用方（[`super::vad::AsrVad`]）负责跑 Silero 模型拿 prob、
//! 把事件映射为 `asr://*` 推送。
//!
//! 纯逻辑、无锁无 IO —— 可脱离 DirectML / ONNX Runtime 环境单测。
//! 帧率约定：每帧 30ms（16kHz × 512 samples），块率必须稳定，帧计数即时间。

/// 切分参数。
#[derive(Debug, Clone, Copy)]
pub struct SegmenterConfig {
    /// 语音概率阈值（prob > 阈值视为语音帧）。
    pub threshold: f32,
    /// 语音段最大长度（帧数），超长强制切段（默认 2000 帧 = 60s）。
    pub max_segment_frames: u64,
    /// 静音多少帧后触发「候选结束」（默认 27 帧 ≈ 800ms，设置页可自定义）。
    pub candidate_silence_frames: u64,
    /// 候选后仍静音多少帧 → 确认结束（默认 33 帧 ≈ 1s）。
    pub confirm_frames: u64,
}

impl Default for SegmenterConfig {
    fn default() -> Self {
        Self {
            threshold: 0.5,
            max_segment_frames: 2000,
            candidate_silence_frames: 27,
            confirm_frames: 33,
        }
    }
}

/// 切分事件（带帧号，供调用方换算样本起止做音频裁剪）。
#[derive(Debug, Clone, PartialEq)]
pub enum SegmentEvent {
    /// 检测到语音开始（新一轮语音段起点）。
    SpeechStart { frame: u64 },
    /// 语音后首次静音。
    SilenceStart { frame: u64 },
    /// 静音达到候选阈值（"可能说完了"），silence_frames = 已连续静音帧数。
    TurnCandidate { frame: u64, silence_frames: u64 },
    /// 确认一轮语音结束（含起止帧：end_frame 为最后一个语音帧）。
    TurnSealed { start_frame: u64, end_frame: u64 },
}

/// 语音切分状态机。
pub struct VadSegmenter {
    cfg: SegmenterConfig,
    /// 内部帧计数（reset 归零，单调递增）。
    frame: u64,
    /// 当前处于语音段内。
    speech_active: bool,
    /// 本轮语音段起始帧。
    segment_start: u64,
    /// 最近一个语音帧。
    last_speech_frame: u64,
    /// 本轮静音起始帧（None = 尚未静音）。
    silence_start_frame: Option<u64>,
    /// 候选已触发标记（防重复触发）。
    candidate_fired: bool,
    /// 候选触发帧（确认窗口基准）。
    candidate_frame: u64,
}

impl VadSegmenter {
    pub fn new() -> Self {
        Self::with_config(SegmenterConfig::default())
    }

    pub fn with_config(cfg: SegmenterConfig) -> Self {
        Self {
            cfg,
            frame: 0,
            speech_active: false,
            segment_start: 0,
            last_speech_frame: 0,
            silence_start_frame: None,
            candidate_fired: false,
            candidate_frame: 0,
        }
    }

    /// 清空状态（新会话 / 新录音开始时调用）。
    pub fn reset(&mut self) {
        *self = Self::with_config(self.cfg);
    }

    /// 当前帧号（可用于与外部帧计数对齐）。
    pub fn current_frame(&self) -> u64 {
        self.frame
    }

    /// 设置候选静音帧数（静音计时可自定义；由 [`super::vad::AsrVad::set_silence_timeout_ms`]
    /// 把毫秒换算成帧后调用）。
    pub fn set_candidate_silence_frames(&mut self, frames: u64) {
        self.cfg.candidate_silence_frames = frames;
    }

    /// 喂一帧语音概率，返回本帧产生的事件（0~2 个）。
    ///
    /// 状态机（30ms/帧）：
    /// - prob > threshold → 语音帧；首次进入语音 → SpeechStart；段长超限 → 强制切段
    /// - prob ≤ threshold 且语音中 → 静音帧；静音起始 → SilenceStart；
    ///   静音 ≥ candidate 帧 → TurnCandidate；候选后仍静音 ≥ confirm 帧 → TurnSealed
    /// - 语音恢复（prob > threshold）→ 取消候选/确认，回到语音态（短停顿不断句）
    pub fn feed(&mut self, prob: f32) -> Vec<SegmentEvent> {
        let frame = self.frame;
        self.frame += 1;
        let mut events = Vec::new();

        if prob > self.cfg.threshold {
            // ── 语音帧 ──
            self.last_speech_frame = frame;
            self.silence_start_frame = None;
            self.candidate_fired = false;
            self.candidate_frame = 0;

            if !self.speech_active {
                self.speech_active = true;
                self.segment_start = frame;
                events.push(SegmentEvent::SpeechStart { frame });
            } else if frame - self.segment_start >= self.cfg.max_segment_frames {
                // 超长段强制切分：先 seal 上一段，再原地开新段
                events.push(SegmentEvent::TurnSealed {
                    start_frame: self.segment_start,
                    end_frame: self.last_speech_frame,
                });
                self.segment_start = frame;
                events.push(SegmentEvent::SpeechStart { frame });
            }
        } else if self.speech_active {
            // ── 静音帧（语音后）──
            let silence_start = *self.silence_start_frame.get_or_insert(frame);
            // 已静音帧数（含当前帧）：第 1 个静音帧 = 静音 1 帧。
            // 之前用 `frame - silence_start` 少算 1（第 N 个静音帧报 N-1），
            // 触发点整体推迟 1 帧（800ms 设置实际 ~830ms 才切段）。
            let silence_frames = frame - silence_start + 1;

            if frame == silence_start {
                events.push(SegmentEvent::SilenceStart { frame });
            }

            if !self.candidate_fired && silence_frames >= self.cfg.candidate_silence_frames {
                self.candidate_fired = true;
                self.candidate_frame = frame;
                events.push(SegmentEvent::TurnCandidate {
                    frame,
                    silence_frames,
                });
            }

            if self.candidate_fired && frame - self.candidate_frame >= self.cfg.confirm_frames {
                // 确认一轮结束
                self.speech_active = false;
                self.silence_start_frame = None;
                self.candidate_fired = false;
                events.push(SegmentEvent::TurnSealed {
                    start_frame: self.segment_start,
                    end_frame: self.last_speech_frame,
                });
            }
        }

        events
    }
}
