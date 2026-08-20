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

pub struct VoiceShiftEvent {
    rate: f64,
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
        Self {
            rate,
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
        }

        let _ = emit(
            ctx.app,
            SCRIPT_VOICE_SHIFT,
            &VoiceShiftPayload { rate: self.rate },
        );

        Ok(None)
    }

    fn event_type() -> &'static str {
        "voice_shift"
    }

    fn duration(&self) -> Option<f64> {
        self.duration
    }
}

pub fn register() {
    register_event(VoiceShiftEvent::event_type(), |data| {
        Box::new(VoiceShiftEvent::from_event_data(&data))
    });
}

#[cfg(test)]
mod tests {
    use super::VoiceShiftEvent;
    use serde_json::json;

    #[test]
    fn rate_defaults_to_one() {
        let e = VoiceShiftEvent::from_event_data(&json!({}));
        assert_eq!(e.rate, 1.0);
    }

    #[test]
    fn rate_is_clamped() {
        assert_eq!(VoiceShiftEvent::from_event_data(&json!({"rate": 0.1})).rate, 0.5);
        assert_eq!(VoiceShiftEvent::from_event_data(&json!({"rate": 9.9})).rate, 1.5);
        assert_eq!(VoiceShiftEvent::from_event_data(&json!({"rate": 0.8})).rate, 0.8);
    }
}
