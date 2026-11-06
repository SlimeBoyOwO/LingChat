//! 请求级注入：只对下一次请求有效，取走即弃，不写台词表。
//!
//! 两类数据形状不同但生命周期一样——generator 构建一次请求时取走，用完就没有：
//! - 临时记忆：并入本轮上下文（[`apply_temporary`]）；
//! - 工具推荐名字：由调用方渲染成一条 `{旁白: （系统提示：…）}` 提示文本后同样入队。

use crate::ai_service::types::LlmMessage;

/// 只对本次请求有效的临时记忆。
#[derive(Clone, Debug)]
pub struct TemporaryMemory {
    pub text: String,
    pub placement: TemporaryPlacement,
}

/// 临时记忆的落点。
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TemporaryPlacement {
    /// 并入最后一条 user 消息。
    IntoUser,
    /// 作为末尾独立的 user 消息。
    TrailingUser,
}

/// 下一轮请求的暂存。
#[derive(Default)]
pub struct RequestScoped {
    pub(super) temporary_memories: Vec<TemporaryMemory>,
    /// 回复后工具推荐的暂存：下一轮请求取走并渲染成提示文本。
    pub(super) tool_hint: Option<Vec<String>>,
}

impl RequestScoped {
    /// 入队一条临时记忆，文本重复时不再入队。
    pub(super) fn queue_temporary(&mut self, memory: TemporaryMemory) {
        if self
            .temporary_memories
            .iter()
            .any(|pending| pending.text == memory.text)
        {
            return;
        }
        self.temporary_memories.push(memory);
    }

    /// 取走全部临时记忆，只用于构建一次请求上下文。
    pub(super) fn take_temporary(&mut self) -> Vec<TemporaryMemory> {
        std::mem::take(&mut self.temporary_memories)
    }

    pub(super) fn clear_temporary(&mut self) {
        self.temporary_memories.clear();
    }

    /// 暂存回复后推荐的工具名，覆盖上一份；空列表视为没有推荐。
    pub(super) fn set_tool_hint(&mut self, names: Vec<String>) {
        self.tool_hint = (!names.is_empty()).then_some(names);
    }

    /// 取走暂存的推荐工具名，只用于构建一次请求上下文。
    pub(super) fn take_tool_hint(&mut self) -> Option<Vec<String>> {
        self.tool_hint.take()
    }

    /// 查看暂存的推荐工具名（调试指令用，不取走）。
    pub(super) fn tool_hint(&self) -> Option<&[String]> {
        self.tool_hint.as_deref()
    }

    pub(super) fn clear_tool_hint(&mut self) {
        self.tool_hint = None;
    }
}

/// 把请求级临时记忆应用到本轮上下文。
pub fn apply_temporary(context: &mut Vec<LlmMessage>, memories: &[TemporaryMemory]) {
    for memory in memories {
        match memory.placement {
            TemporaryPlacement::IntoUser => match context.last_mut() {
                Some(last) if last.role == "user" => {
                    if !last.content.is_empty() {
                        last.content.push_str("\n\n");
                    }
                    last.content.push_str(&memory.text);
                },
                _ => context.push(LlmMessage::user(memory.text.clone())),
            },
            TemporaryPlacement::TrailingUser => {
                context.push(LlmMessage::user(memory.text.clone()));
            },
        }
    }
}
