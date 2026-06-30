//! 环境音事件 —— 循环持续的场景音效（雨声、风声、人群嘈杂声），与 BGM 同时共存。

use anyhow::Result;
use async_trait::async_trait;
use serde_json::Value;

use crate::ai_service::game_system::script_engine::events::{register_event, ScriptContext, ScriptEvent};
use crate::ai_service::game_system::script_engine::responses::{
    event_names::SCRIPT_AMBIENT, AmbientPayload,
};
use crate::ai_service::game_system::script_engine::utils::media::{resolve_script_media, MediaType};
use crate::ai_service::message_system::events::emit;

pub struct AmbientEvent {
    ambient_path: String,
    volume: f64,
    is_loop: bool,
    stop: bool,
    fade: bool,
}

impl AmbientEvent {
    fn from_event_data(data: &Value) -> Self {
        Self {
            ambient_path: data
                .get("ambientPath")
                .and_then(|v| v.as_str())
                .unwrap_or("")
                .to_string(),
            // 音量字段，默认 100.0
            volume: data
                .get("volume")
                .and_then(|v| v.as_f64())
                .unwrap_or(100.0),
            // 循环字段，默认 true
            is_loop: data
                .get("loop")
                .and_then(|v| v.as_bool())
                .unwrap_or(true),
            // 停止字段，默认 false
            stop: data
                .get("stop")
                .and_then(|v| v.as_bool())
                .unwrap_or(false),
            // 淡入淡出字段，默认 true
            fade: data
                .get("fade")
                .and_then(|v| v.as_bool())
                .unwrap_or(true),
        }
    }
}

#[async_trait]
impl ScriptEvent for AmbientEvent {
    async fn execute(&mut self, ctx: &mut ScriptContext<'_>) -> Result<Option<String>> {
        // 如果是停止指令，不需要解析路径
        if self.stop {
            let payload = AmbientPayload {
                ambient_path: self.ambient_path.clone(),
                volume: self.volume,
                is_loop: self.is_loop,
                stop: true,
                fade: self.fade,
            };
            let _ = emit(ctx.app, SCRIPT_AMBIENT, &payload);
            tracing::info!("[AmbientEvent] 停止环境音: {}", self.ambient_path);
            return Ok(None);
        }

        let script_path = ctx
            .game_status
            .lock()
            .await
            .script_status
            .as_ref()
            .map(|ss| ss.script_path.clone());

        let resolved = resolve_script_media(
            ctx.data_dir,
            script_path.as_deref(),
            &self.ambient_path,
            MediaType::Ambient,
        )
        .unwrap_or_default();

        let payload = AmbientPayload {
            ambient_path: resolved,
            volume: self.volume,
            is_loop: self.is_loop,
            stop: false,
            fade: self.fade,
        };
        let _ = emit(ctx.app, SCRIPT_AMBIENT, &payload);

        tracing::info!(
            "[AmbientEvent] 环境音: {} (音量: {}, 循环: {})",
            self.ambient_path,
            self.volume,
            self.is_loop
        );
        Ok(None)
    }

    fn event_type() -> &'static str {
        "ambient"
    }
}

pub fn register() {
    register_event(AmbientEvent::event_type(), |data| {
        Box::new(AmbientEvent::from_event_data(&data))
    });
}
