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
        /*
        Tips: 此处可以添加更多判断逻辑，比如过滤掉无意义对话等
         */
        true
    }
}
