//! Random boolean variable event for low-frequency story anomalies.
//!
//! YAML example:
//! `- type: random_var`
//! `  variable: rare_glimpse`
//! `  chance: 0.0625`

use anyhow::{anyhow, Result};
use async_trait::async_trait;
use rand::Rng;
use serde_json::Value;

use crate::ai_service::game_system::script_engine::events::{
    register_event, ScriptContext, ScriptEvent,
};

pub struct RandomVarEvent {
    variable: String,
    chance: f64,
}

impl RandomVarEvent {
    fn from_event_data(data: &Value) -> Self {
        let variable = data
            .get("variable")
            .and_then(Value::as_str)
            .unwrap_or("random_result")
            .trim()
            .to_string();
        let raw_chance = data.get("chance").and_then(Value::as_f64).unwrap_or(0.5);
        let chance = if raw_chance.is_finite() {
            raw_chance.clamp(0.0, 1.0)
        } else {
            0.0
        };

        if raw_chance != chance {
            tracing::warn!(
                "[RandomVarEvent] chance={} 无效，已限制为 {}（0-1）",
                raw_chance,
                chance
            );
        }

        Self { variable, chance }
    }
}

#[async_trait]
impl ScriptEvent for RandomVarEvent {
    async fn execute(&mut self, ctx: &mut ScriptContext<'_>) -> Result<Option<String>> {
        if self.variable.is_empty() {
            return Err(anyhow!("random_var 的 variable 不能为空"));
        }

        // ThreadRng is not held across the await point.
        let rolled = {
            let mut rng = rand::thread_rng();
            rng.gen_bool(self.chance)
        };

        let mut gs = ctx.game_status.lock().await;
        let script_status = gs
            .script_status
            .as_mut()
            .ok_or_else(|| anyhow!("ScriptStatus 未设置，无法写入随机变量"))?;
        script_status.set_variable(self.variable.clone(), Value::Bool(rolled));
        tracing::info!(
            "[RandomVarEvent] {}={}（chance={}）",
            self.variable,
            rolled,
            self.chance
        );
        Ok(None)
    }

    fn event_type() -> &'static str {
        "random_var"
    }
}

pub fn register() {
    register_event(RandomVarEvent::event_type(), |data| {
        Box::new(RandomVarEvent::from_event_data(&data))
    });
}
