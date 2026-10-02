//! Voice shift event — sets the playback rate of subsequent character voice
//! (TTS) audio. Lower values = deeper "demon voice" (HTML playbackRate also
//! lowers pitch because preservesPitch defaults to false).
//!
//! The rate is persisted in `script_status.vars["voice_rate"]` so it survives
//! chapter changes; scripts should reset it with `rate: 1.0` during teardown.

use anyhow::Result;
use async_trait::async_trait;
use serde_json::{json, Value};

use crate::ai_service::game_system::script_engine::events::{
    parse_duration, register_event, ScriptContext, ScriptEvent,
};
use crate::ai_service::game_system::script_engine::responses::{
    event_names::SCRIPT_VOICE_SHIFT, VoiceShiftPayload,
};
use crate::ai_service::message_system::events::emit;

/// 变速合法区间：过慢会听不懂，过快花栗鼠；限制在恐怖演出可用范围
const MIN_RATE: f64 = 0.5;
const MAX_RATE: f64 = 1.5;
/// 音调偏移合法区间（半音数）：±12 即一个八度，恐怖演出一般用 -2 ~ -6
const MIN_PITCH: f64 = -12.0;
const MAX_PITCH: f64 = 12.0;

pub struct VoiceShiftEvent {
    rate: f64,
    pitch: f64,
    duration: Option<f64>,
}

impl VoiceShiftEvent {
    fn from_event_data(data: &Value) -> Self {
        let raw = data
            .get("rate")
            .and_then(|v| v.as_f64())
            .unwrap_or(1.0);
        let rate = raw.clamp(MIN_RATE, MAX_RATE);
        if (raw - rate).abs() > f64::EPSILON {
            tracing::warn!(
                "[VoiceShiftEvent] rate {} 超出范围，已截断为 {}（合法区间 {}-{}）",
                raw,
                rate,
                MIN_RATE,
                MAX_RATE
            );
        }
        let raw_pitch = data
            .get("pitch")
            .and_then(|v| v.as_f64())
            .unwrap_or(0.0);
        let pitch = raw_pitch.clamp(MIN_PITCH, MAX_PITCH);
        if (raw_pitch - pitch).abs() > f64::EPSILON {
            tracing::warn!(
                "[VoiceShiftEvent] pitch {} 超出范围，已截断为 {}（合法区间 {}-{}）",
                raw_pitch,
                pitch,
                MIN_PITCH,
                MAX_PITCH
            );
        }
        Self {
            rate,
            pitch,
            duration: parse_duration(data),
        }
    }
}

#[async_trait]
impl ScriptEvent for VoiceShiftEvent {
    async fn execute(&mut self, ctx: &mut ScriptContext<'_>) -> Result<Option<String>> {
        // 存进剧本变量：跨章节保留，也可被 condition 引用（如 voice_rate != 1）
        if let Some(script_status) = ctx.game_status.lock().await.script_status.as_mut() {
            script_status.set_variable("voice_rate", json!(self.rate));
            script_status.set_variable("voice_pitch", json!(self.pitch));
        }

        emit(
            ctx.app,
            SCRIPT_VOICE_SHIFT,
            &VoiceShiftPayload {
                rate: self.rate,
                pitch: self.pitch,
                duration: self.duration,
            },
        )?;

        Ok(None)
    }

    fn event_type() -> &'static str {
        "voice_shift"
    }
}

pub fn register() {
    register_event(VoiceShiftEvent::event_type(), |data| {
        Box::new(VoiceShiftEvent::from_event_data(&data))
    });
}
