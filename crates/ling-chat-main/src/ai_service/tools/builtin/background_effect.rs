//! set_background_effect：背景粒子特效工具（模型自主调用入口）。
//!
//! 特效概念的"正主"在剧本引擎——`script_engine::events::background_effect_event`
//! 与 `known_effects` 特效名单；这里只是把同一能力开放给模型在聊天中调用。
//! 与剧本的 `background_effect` 事件走不同的前端通道（`ambient:effect`），
//! 原因见 `execute` 内注释。

use async_trait::async_trait;
use serde_json::{Value, json};

use crate::ai_service::types::ToolDefinition;

use crate::ai_service::tools::executor::{Tool, ToolContext, ToolError, ToolResult};

/// set_background_effect：设置背景粒子特效。
pub struct SetBackgroundEffect;

#[async_trait]
impl Tool for SetBackgroundEffect {
    fn definition(&self) -> ToolDefinition {
        let effect_names =
            crate::ai_service::game_system::script_engine::events::known_effects::KNOWN_EFFECTS
                .join(" / ");
        ToolDefinition::new(
            "set_background_effect",
            format!(
                "设置背景粒子特效，营造氛围。可选值：{effect_names}，none 为清除当前特效；\
                 名称大小写敏感。拿到天气结果或想配合情绪氛围时可主动使用。\
                 若工具返回 locked（用户手动锁定过特效），任何调用都不会执行——不要再尝试，\
                 可提示用户在设置中重新打开自动切换"
            ),
            json!({
                "type": "object",
                "properties": {
                    "effect": {"type": "string", "description": "特效名，必须是以上列表中的一个"},
                    "duration": {"type": "number", "description": "持续秒数；不填则一直持续"}
                },
                "required": ["effect"],
                "additionalProperties": false
            }),
        )
        .with_rag_examples([
            "加点下雨的效果",
            "换一个氛围特效",
            "来点樱花飘落的感觉",
            "把背景特效关掉",
        ])
    }

    async fn execute(
        &self,
        context: &ToolContext,
        arguments: Value,
    ) -> Result<ToolResult, ToolError> {
        let Some(obj) = arguments.as_object() else {
            return Err(ToolError::InvalidArguments(
                "set_background_effect 参数必须是 JSON object".into(),
            ));
        };
        let effect = obj
            .get("effect")
            .and_then(Value::as_str)
            .map(str::trim)
            .filter(|s| !s.is_empty())
            .ok_or_else(|| ToolError::InvalidArguments("set_background_effect 需要 effect".into()))?
            .to_string();
        let duration = obj.get("duration").and_then(Value::as_f64);
        tracing::info!(
            "[SetBackgroundEffect] 收到模型请求 effect={effect:?} duration={duration:?}"
        );

        let app = context.require_app()?;

        // 手动优先（硬边界）：用户手动选过特效后，AI 的切换请求一律不执行；
        // 恢复的唯一途径是用户在设置页重新打开开关
        let auto_allowed = crate::config::AppConfig::load(&app)
            .map(|c| c.ai_auto_effect)
            .unwrap_or(true);
        if !auto_allowed {
            return Ok(json!({
                "success": false,
                "locked": true,
                "message": "用户已手动选择当前粒子特效并关闭了自动切换，本次及后续的特效调用都不会执行。请自然接受，不要反复尝试；若用户想恢复自动切换，可提示其在设置中重新打开",
            }));
        }

        use crate::ai_service::game_system::script_engine::events::known_effects;

        // 大小写纠错：'rain' → 'Rain'，避免模型随手写出前端渲染不出的变体
        let effect = known_effects::KNOWN_EFFECTS
            .iter()
            .find(|k| k.eq_ignore_ascii_case(&effect))
            .copied()
            .map(str::to_string)
            .unwrap_or(effect);
        let clearing = ["none", "None", ""].contains(&effect.as_str());
        if !clearing && !known_effects::KNOWN_EFFECTS.contains(&effect.as_str()) {
            return Err(ToolError::Execution(format!(
                "未知特效「{effect}」，可用值：{}（或 none 清除）",
                known_effects::KNOWN_EFFECTS.join(", ")
            )));
        }

        // 注意：这里刻意不走剧本引擎的 script:background-effect 通道——那个事件
        // 会被前端放进对话事件队列顺序消费，还会翻转 currentStatus；聊天中的
        // 氛围特效是即时状态，走队列会插队在还没显示的台词前面、把状态机搅乱。
        // 世界动作层广播的是独立的 ambient:effect，前端直达监听只改特效状态。
        crate::ai_service::game_system::world_actions::apply_effect(
            &app,
            crate::ai_service::game_system::world_actions::EffectOrigin::Ai,
            &effect,
            duration,
        )
        .await;
        tracing::info!("[SetBackgroundEffect] 切换特效: {effect}（ambient:effect 已广播）");

        Ok(json!({"ok": true, "effect": effect}))
    }
}
