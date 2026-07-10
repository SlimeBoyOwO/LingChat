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

        let allowed = match intent_type {
            // 明确的日程闹钟优先级最高，只要前端允许即可投递。
            IntentType::Alarm => true,
            // 重要日子不应在全屏游戏时打断用户。
            IntentType::ImportantDay => perception.state != UserState::GAME,
            // TODO 在工作状态下也可能有价值，但游戏中不应弹出。
            IntentType::Todo => perception.state != UserState::GAME,
            // 屏幕评论只在轻度活动/浏览时投递；工作、游戏和真正挂机时容易过时或打扰。
            IntentType::Screen => {
                matches!(perception.state, UserState::BROWSING | UserState::CASUAL)
            }
            // 无明确事项的闲聊只在用户空闲或轻度活动时发起。
            IntentType::Topic => {
                matches!(perception.state, UserState::IDLE | UserState::CASUAL)
            }
        };

        if !allowed {
            tracing::debug!(
                "[DeliveryEval] {:?} suppressed for user state {:?}",
                intent_type,
                perception.state
            );
        }
        allowed
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn perception(state: UserState) -> PerceptionResult {
        PerceptionResult {
            state,
            description: String::new(),
            interest_modifier: 0,
            visual_change_detected: false,
            current_screen_text: String::new(),
        }
    }

    #[test]
    fn frontend_gate_always_wins() {
        assert!(!DeliveryEvaluator::can_deliver(
            IntentType::Alarm,
            &perception(UserState::IDLE),
            false,
        ));
    }

    #[test]
    fn topic_does_not_interrupt_work_or_game() {
        assert!(!DeliveryEvaluator::can_deliver(
            IntentType::Topic,
            &perception(UserState::WORK),
            true,
        ));
        assert!(!DeliveryEvaluator::can_deliver(
            IntentType::Topic,
            &perception(UserState::GAME),
            true,
        ));
        assert!(DeliveryEvaluator::can_deliver(
            IntentType::Topic,
            &perception(UserState::CASUAL),
            true,
        ));
    }

    #[test]
    fn alarms_are_allowed_in_busy_states() {
        assert!(DeliveryEvaluator::can_deliver(
            IntentType::Alarm,
            &perception(UserState::GAME),
            true,
        ));
    }
}
