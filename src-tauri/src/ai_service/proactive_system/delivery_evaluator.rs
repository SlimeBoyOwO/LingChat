//! 投放时机评估器。
//!
//! 唯一闸门：所有意图（新生成/暂存）投放前必经此处。

use super::types::{IntentType, PerceptionResult, UserState};

pub struct DeliveryEvaluator;

impl DeliveryEvaluator {
    /// 判断是否可投放。`can_deliver` 由前端上报（在聊天界面 + 无设置面板 + 输入为空）。
    /// 软条件根据意图类型进一步过滤用户活动状态。
    pub fn can_deliver(
        intent_type: IntentType,
        perception: &PerceptionResult,
        can_deliver: bool,
    ) -> bool {
        if !can_deliver {
            tracing::debug!("[DeliveryEval] can_deliver=false (frontend report)");
            return false;
        }

        if perception.state == UserState::GAME {
            // Alarm 例外：日程闹钟连游戏时也允许投放
            if intent_type != IntentType::Alarm {
                tracing::debug!("[DeliveryEval] User is GAME -> deny {:?}", intent_type);
                return false;
            }
        }

        // 各意图类型允许的 UserState
        let allowed = match intent_type {
            // Alarm：任何时候都可以
            IntentType::Alarm => true,
            // 重要提醒：除 GAME（上面已拦截）外均可
            IntentType::ImportantDay | IntentType::Todo => true,
            // 屏幕感知：直接允许
            IntentType::Screen => true,
            // 闲聊：仅真正空闲时
            IntentType::Topic => matches!(
                perception.state,
                UserState::IDLE | UserState::CASUAL
            ),
        };

            if !allowed {
            tracing::debug!(
                "[DeliveryEval] State={:?} denies intent {:?}",
                perception.state,
                intent_type
            );
        }
        allowed
    }
}
