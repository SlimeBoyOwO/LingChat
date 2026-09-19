//! Lighting event — 剧本里写死一段剧情用什么光影。
//!
//! YAML 写法（三选一）：
//!
//! ```yaml
//! - type: lighting
//!   preset: warm_window      # 预设 id，见 lighting_store
//!   duration: 0
//!
//! - type: lighting
//!   clear: true              # 回到「跟随场景」
//!
//! - type: lighting
//!   params: { ... }          # 内联完整参数，供微调 / 新玩法
//! ```
//!
//! 与 `background_effect` 一样走前端事件队列（`script:lighting` →
//! `lighting-processor`），所以光影跟着台词节奏切换，而不是剧本一执行就抢跑。
//! 面板与 LLM 工具走即时的 `lighting:change`，两条事件在前端汇到同一个 store。

use anyhow::Result;
use async_trait::async_trait;
use serde_json::Value;

use crate::ai_service::game_system::scene_store::{self, LightingOverride, LightingParams};
use crate::ai_service::game_system::script_engine::events::{
    ScriptContext, ScriptEvent, parse_duration, register_event,
};
use crate::ai_service::game_system::script_engine::responses::event_names::SCRIPT_LIGHTING;
use crate::ai_service::message_system::events::emit;

/// 清空覆盖的惯用写法。`follow_scene` 与设置面板的「跟随场景」同一语义。
const CLEARING_PRESETS: [&str; 4] = ["none", "None", "", "follow_scene"];

pub struct LightingEvent {
    requested: Option<LightingOverride>,
    duration: Option<f64>,
}

impl LightingEvent {
    fn from_event_data(data: &Value) -> Self {
        let duration = parse_duration(data);

        if data.get("clear").and_then(Value::as_bool).unwrap_or(false) {
            return Self {
                requested: None,
                duration,
            };
        }

        let preset = data
            .get("preset")
            .and_then(Value::as_str)
            .map(str::to_string)
            .filter(|s| !CLEARING_PRESETS.contains(&s.as_str()));
        let params = match data.get("params") {
            Some(v) if v.is_object() => {
                let mut v = v.clone();
                scene_store::normalize_legacy_lighting(&mut v);
                serde_json::from_value::<LightingParams>(v).ok()
            },
            _ => None,
        };

        Self {
            requested: match (preset, params) {
                (None, None) => None,
                (preset, params) => Some(LightingOverride { preset, params }),
            },
            duration,
        }
    }
}

#[async_trait]
impl ScriptEvent for LightingEvent {
    async fn execute(&mut self, ctx: &mut ScriptContext<'_>) -> Result<Option<String>> {
        // 解析放在 execute：未知预设要让剧本当场报错，而不是静默沿用上一段的灯光。
        // 这里只锁 ctx.game_status——事件不得碰 ai_service 锁，故不走 api::lighting::broadcast。
        let resolved = match &self.requested {
            None => None,
            Some(requested) => {
                Some(crate::api::lighting::resolve_request(requested).map_err(anyhow::Error::msg)?)
            },
        };

        let (preset, params) = match &resolved {
            Some((preset, params)) => (preset.clone(), Some(params.clone())),
            None => (None, None),
        };

        {
            let mut gs = ctx.game_status.lock().await;
            match resolved {
                Some((preset, params)) => gs.set_lighting_override(
                    LightingOverride {
                        preset,
                        params: Some(params),
                    },
                    "script",
                ),
                None => {
                    gs.clear_lighting_override();
                },
            }
        }

        let payload = crate::api::lighting::LightingChangePayload {
            preset,
            params,
            source: "script".to_string(),
            duration: self.duration,
        };
        let _ = emit(ctx.app, SCRIPT_LIGHTING, &payload);

        Ok(None)
    }

    fn event_type() -> &'static str {
        "lighting"
    }
}

pub fn register() {
    register_event(LightingEvent::event_type(), |data| {
        Box::new(LightingEvent::from_event_data(&data))
    });
}
