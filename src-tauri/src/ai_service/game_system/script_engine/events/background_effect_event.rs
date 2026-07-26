//! Background effect event — sets `game_status.background_effect`.

use anyhow::Result;
use async_trait::async_trait;
use serde_json::Value;

use crate::ai_service::game_system::script_engine::events::{
    register_event, ScriptContext, ScriptEvent,
};
use crate::ai_service::game_system::script_engine::responses::{
    event_names::SCRIPT_BACKGROUND_EFFECT, BackgroundEffectPayload,
};
use crate::ai_service::message_system::events::emit;

/// Effect names the frontend actually renders.
///
/// `GameBackground.vue` compares with `===`, so these are **case sensitive**:
/// `starfield` and `Starfield` both silently render nothing. Anything not in
/// this list (including the conventional `None`) clears the current effect.
pub const KNOWN_EFFECTS: [&str; 5] = ["StarField", "Rain", "Sakura", "Snow", "Fireworks"];

/// Names that explicitly mean "no effect" and therefore must not be warned about.
const CLEARING_EFFECTS: [&str; 3] = ["none", "None", ""];

pub struct BackgroundEffectEvent {
    effect: String,
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
        if !CLEARING_EFFECTS.contains(&effect.as_str()) && !KNOWN_EFFECTS.contains(&effect.as_str())
        {
            let hint = KNOWN_EFFECTS
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
                    KNOWN_EFFECTS
                ),
            }
        }

        Self { effect }
    }
}

#[async_trait]
impl ScriptEvent for BackgroundEffectEvent {
    async fn execute(&mut self, ctx: &mut ScriptContext<'_>) -> Result<Option<String>> {
        ctx.game_status.lock().await.background_effect = self.effect.clone();

        let payload = BackgroundEffectPayload {
            effect: self.effect.clone(),
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
    use super::{BackgroundEffectEvent, KNOWN_EFFECTS};
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
    fn known_effect_list_matches_the_frontend() {
        // Mirrors the `v-if` chain in src/components/game/standard/GameBackground.vue.
        assert_eq!(
            KNOWN_EFFECTS,
            ["StarField", "Rain", "Sakura", "Snow", "Fireworks"]
        );
    }
}
