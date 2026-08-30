//! Background effect event — sets `game_status.background_effect`.

use std::sync::OnceLock;

use anyhow::Result;
use async_trait::async_trait;
use serde::Deserialize;
use serde_json::Value;

use crate::ai_service::game_system::script_engine::events::{
    parse_duration, register_event, ScriptContext, ScriptEvent,
};
use crate::ai_service::game_system::script_engine::responses::{
    event_names::SCRIPT_BACKGROUND_EFFECT, BackgroundEffectPayload,
};
use crate::ai_service::message_system::events::emit;

#[derive(Deserialize)]
struct EffectManifestEntry {
    key: String,
}

static KNOWN_EFFECTS: OnceLock<Vec<String>> = OnceLock::new();

/// Rust 与 Vue 共用 `shared/script-effects.json`，避免编辑器、校验器和渲染层名单漂移。
pub fn known_effects() -> &'static [String] {
    KNOWN_EFFECTS.get_or_init(|| {
        serde_json::from_str::<Vec<EffectManifestEntry>>(include_str!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../shared/script-effects.json"
        )))
        .expect("shared/script-effects.json must be valid")
        .into_iter()
        .map(|entry| entry.key)
        .collect()
    })
}

/// Names that explicitly mean "no effect" and therefore must not be warned about.
const CLEARING_EFFECTS: [&str; 3] = ["none", "None", ""];

pub struct BackgroundEffectEvent {
    effect: String,
    duration: Option<f64>,
    text: Option<String>,
    echo: Option<String>,
}

impl BackgroundEffectEvent {
    fn from_event_data(data: &Value) -> Self {
        let effect = data
            .get("effect")
            .and_then(|v| v.as_str())
            .unwrap_or("none")
            .to_string();

        // Behaviour is unchanged — the value is still passed through verbatim.
        // The warning exists because the failure was previously completely
        // silent: two of the shipped scripts write `starfield` / `Starfield`
        // and get no particles at all with no diagnostic anywhere.
        // 支持 '+' 组合叠加（如 "Glitch+BloodDrip"），逐段校验
        let all_known = effect.split('+').map(|p| p.trim()).all(|p| {
            CLEARING_EFFECTS.contains(&p) || known_effects().iter().any(|known| known == p)
        });
        if !all_known && !CLEARING_EFFECTS.contains(&effect.as_str()) {
            let hint = known_effects()
                .iter()
                .find(|k| k.eq_ignore_ascii_case(&effect));
            match hint {
                Some(correct) => tracing::warn!(
                    "[BackgroundEffectEvent] 特效名 '{}' 大小写不匹配，前端不会渲染；应为 '{}'",
                    effect,
                    correct
                ),
                None => tracing::warn!(
                    "[BackgroundEffectEvent] 未知特效 '{}'，将清空当前特效；可用值: {:?}",
                    effect,
                    known_effects()
                ),
            }
        }

        Self {
            effect,
            duration: parse_duration(data),
            text: data
                .get("text")
                .and_then(|v| v.as_str())
                .map(|s| s.to_string()),
            echo: data
                .get("echo")
                .and_then(|v| v.as_str())
                .map(|s| s.to_string()),
        }
    }
}

#[async_trait]
impl ScriptEvent for BackgroundEffectEvent {
    async fn execute(&mut self, ctx: &mut ScriptContext<'_>) -> Result<Option<String>> {
        // 限时特效只是一层瞬时演出，由前端计时并还原；不要把它写进存档快照，
        // 否则恰好在闪烁期间自动保存，重载后会把血色 UI / Tear 永久恢复出来。
        if self.duration.unwrap_or(0.0) <= 0.0 {
            ctx.game_status.lock().await.background_effect = self.effect.clone();
        }

        let payload = BackgroundEffectPayload {
            effect: self.effect.clone(),
            duration: self.duration,
            text: self.text.clone(),
            echo: self.echo.clone(),
        };
        let _ = emit(ctx.app, SCRIPT_BACKGROUND_EFFECT, &payload);

        Ok(None)
    }

    fn event_type() -> &'static str {
        "background_effect"
    }
}

pub fn register() {
    register_event(BackgroundEffectEvent::event_type(), |data| {
        Box::new(BackgroundEffectEvent::from_event_data(&data))
    });
}

#[cfg(test)]
mod tests {
    use super::{known_effects, BackgroundEffectEvent};
    use serde_json::json;

    /// The value must keep passing through untouched — PR1 only adds a warning,
    /// it deliberately does not "helpfully" correct the author's data.
    #[test]
    fn effect_is_passed_through_verbatim() {
        for raw in ["StarField", "starfield", "Starfield", "None", "Nonsense"] {
            let e = BackgroundEffectEvent::from_event_data(&json!({ "effect": raw }));
            assert_eq!(e.effect, raw);
        }
    }

    #[test]
    fn missing_effect_defaults_to_none() {
        let e = BackgroundEffectEvent::from_event_data(&json!({}));
        assert_eq!(e.effect, "none");
    }

    #[test]
    fn shared_effect_manifest_has_unique_composable_keys() {
        let effects = known_effects();
        assert!(!effects.is_empty());
        let unique: std::collections::HashSet<&str> =
            effects.iter().map(String::as_str).collect();
        assert_eq!(unique.len(), effects.len());
        assert!(effects.iter().all(|key| !key.is_empty()));
        assert!(effects.iter().all(|key| !key.contains('+')));
        assert!(effects.iter().all(|key| !key.eq_ignore_ascii_case("none")));
    }
}
