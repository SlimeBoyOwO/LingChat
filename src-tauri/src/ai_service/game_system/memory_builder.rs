use std::collections::HashSet;

use crate::ai_service::types::{GameLine, LineBase, LlmMessage, ToolCall};
use crate::db::entities::line::LineAttribute;

/// 将 `GameLine` 序列构建成目标角色的 LLM 消息列表。
pub struct MemoryBuilder {
    pub target_role_id: i32,
    /// 窗口内没有任何 user 消息时，是否在裁切后的首条 assistant 前注入一条 user「继续」。
    /// 关闭时只裁切、不注入（首条会是 assistant，OpenAI 兼容端点接受，Gemini 会 400）。
    inject_continue_user: bool,
}

enum BufferKind {
    TargetAssistant,
    OtherBlock,
}

impl MemoryBuilder {
    pub fn new(target_role_id: i32) -> Self {
        Self {
            target_role_id,
            inject_continue_user: false,
        }
    }

    /// 设置「窗口内无 user 时是否注入 user「继续」」。仅用于发给 LLM 的对话上下文；
    /// 记忆压缩取文本时保持默认（false），避免把注入内容写进摘要。
    pub fn with_continue_user(mut self, enabled: bool) -> Self {
        self.inject_continue_user = enabled;
        self
    }

    fn is_target(&self, line: &GameLine) -> bool {
        if line.sender_role_id() == Some(self.target_role_id) {
            return true;
        }
        line.perceived_role_ids.contains(&self.target_role_id)
    }

    /// 格式化内容：【情绪】内容（动作）<TTS>，仅用于 assistant (AI自身) 消息。
    fn format_content_with_extras(&self, line: &LineBase) -> String {
        let mut s = String::new();
        if let Some(emo) = line.original_emotion.as_deref().filter(|v| !v.is_empty()) {
            s.push('【');
            s.push_str(emo);
            s.push('】');
        }
        s.push_str(&line.content);
        s.push('\n');
        if let Some(act) = line.action_content.as_deref().filter(|v| !v.is_empty()) {
            s.push('(');
            s.push_str(act);
            s.push(')');
            s.push('\n');
        }

        if let Some(tts) = line.tts_content.as_deref().filter(|v| !v.is_empty()) {
            s.push('<');
            s.push_str(tts);
            s.push('>');
            s.push('\n');
        }

        s.push('\n');

        s
    }

    /// [修改点 1]：格式化为 context 行：过滤掉情绪和TTS，仅保留 "名称: 内容(动作)"
    fn format_context_line(&self, line: &LineBase) -> String {
        let name = line.display_name.as_deref().unwrap_or("未知");
        let mut s = match name {
            "旁白" | "系统" => line.content.clone(),
            _ => format!("{}: {}", name, line.content),
        };

        // 如果有动作，则追加 (动作)
        if let Some(act) = line.action_content.as_deref().filter(|v| !v.is_empty()) {
            s.push('(');
            s.push_str(act);
            s.push(')');
        }
        s
    }

    pub fn build(&self, lines: &[GameLine]) -> Vec<LlmMessage> {
        let mut memory: Vec<LlmMessage> = Vec::new();
        let mut buffer: Vec<GameLine> = Vec::new();
        let mut buffer_kind: Option<BufferKind> = None;

        let flush = |memory: &mut Vec<LlmMessage>,
                     buffer: &mut Vec<GameLine>,
                     buffer_kind: &mut Option<BufferKind>,
                     this: &MemoryBuilder| {
            if buffer.is_empty() {
                *buffer_kind = None;
                return;
            }
            match buffer_kind {
                Some(BufferKind::TargetAssistant) => {
                    let full: String = buffer
                        .iter()
                        .map(|l| this.format_content_with_extras(&l.base))
                        .collect();
                    if !full.trim().is_empty() {
                        memory.push(LlmMessage::assistant(full));
                    }
                },
                Some(BufferKind::OtherBlock) => {
                    // 从末尾向前找连续的 user 行，切分 context / active_user
                    let mut split_index = buffer.len();
                    for i in (0..buffer.len()).rev() {
                        let is_user = matches!(buffer[i].attribute(), LineAttribute::User);
                        if !is_user {
                            split_index = i + 1;
                            break;
                        }
                        if i == 0 && is_user {
                            split_index = 0;
                        }
                    }
                    let (context_lines, active_user_lines) = buffer.split_at(split_index);

                    let mut parts: Vec<String> = Vec::new();

                    // 记录是否包含上下文（即是否有其他角色发言）
                    let has_context = !context_lines.is_empty();

                    if has_context {
                        let joined: Vec<String> = context_lines
                            .iter()
                            .map(|l| this.format_context_line(&l.base))
                            .collect();
                        parts.push(format!("{{{}}}", joined.join("\n")));
                    }

                    if !active_user_lines.is_empty() {
                        // [修改点 2]：如果存在其他角色台词(has_context)，则强制给 User 台词加上 "主角名称: "
                        let user_text: Vec<String> = active_user_lines
                            .iter()
                            .map(|l| {
                                let name = l.base.display_name.as_deref().unwrap_or("未知");
                                let s = match name {
                                    "旁白" | "系统" => l.base.content.clone(),
                                    _ => format!("{}: {}", name, l.base.content),
                                };
                                s
                            })
                            .collect();
                        // 用换行符拼接多条User台词
                        parts.push(user_text.join("\n"));
                    }

                    let final_content =
                        if !context_lines.is_empty() && !active_user_lines.is_empty() {
                            parts.join("\n")
                        } else {
                            parts.concat()
                        };
                    memory.push(LlmMessage::user(final_content));
                },
                None => {},
            }
            buffer.clear();
            *buffer_kind = None;
        };

        let mut has_system_for_target = false;

        for line in lines {
            // system 消息处理逻辑保持不变...
            if matches!(line.attribute(), LineAttribute::System) {
                if line.sender_role_id() == Some(self.target_role_id) {
                    flush(&mut memory, &mut buffer, &mut buffer_kind, self);
                    if has_system_for_target {
                        tracing::warn!(
                            "[MemoryBuilder] 角色 {} 存在多条 System 台词，已跳过重复项 \
                             (sender_role_id={})",
                            self.target_role_id,
                            line.sender_role_id().unwrap_or(-1)
                        );
                    } else {
                        has_system_for_target = true;
                        memory.push(LlmMessage::system(line.content().to_string()));
                    }
                }
                continue;
            }

            // 工具调用 assistant 行：优先读 tool_call 字段，兼容旧版 \n\n 内嵌格式
            if matches!(line.attribute(), LineAttribute::Assistant) {
                let has_tool_call = line
                    .base
                    .tool_call
                    .as_deref()
                    .map(|s| !s.is_empty())
                    .unwrap_or(false);

                if has_tool_call {
                    // 新版：tool_call 存 JSON，content 纯文本
                    if let Ok(tool_calls) =
                        serde_json::from_str::<Vec<crate::ai_service::types::ToolCall>>(
                            line.base.tool_call.as_deref().unwrap_or(""),
                        )
                    {
                        flush(&mut memory, &mut buffer, &mut buffer_kind, self);
                        memory.push(LlmMessage {
                            role: "assistant".into(),
                            content: line.base.content.clone(),
                            tool_calls: Some(tool_calls),
                            tool_call_id: None,
                            image_data_url: None,
                        });
                        continue;
                    }
                } else if !line.base.content.is_empty() {
                    // 旧版兼容：content = "tool_calls_json\n\ntext"
                    let (tool_calls_json, text) = if let Some(idx) = line.base.content.find("\n\n")
                    {
                        let (head, tail) = line.base.content.split_at(idx);
                        (head, tail.strip_prefix("\n\n").unwrap_or(""))
                    } else {
                        (line.base.content.as_str(), "")
                    };
                    if let Ok(tool_calls) = serde_json::from_str::<
                        Vec<crate::ai_service::types::ToolCall>,
                    >(tool_calls_json)
                    {
                        flush(&mut memory, &mut buffer, &mut buffer_kind, self);
                        memory.push(LlmMessage {
                            role: "assistant".into(),
                            content: text.to_string(),
                            tool_calls: Some(tool_calls),
                            tool_call_id: None,
                            image_data_url: None,
                        });
                        continue;
                    }
                }
            }

            // 工具返回行：content 存 JSON {"tool_call_id":..., "result":...}
            if matches!(line.attribute(), LineAttribute::Tool) {
                flush(&mut memory, &mut buffer, &mut buffer_kind, self);
                let (tool_call_id, result) =
                    serde_json::from_str::<serde_json::Value>(&line.base.content)
                        .ok()
                        .map(|v| {
                            (
                                v.get("tool_call_id")
                                    .and_then(|s| s.as_str())
                                    .map(String::from),
                                v.get("result").map(|r| r.to_string()).unwrap_or_default(),
                            )
                        })
                        .unwrap_or((None, line.base.content.clone()));
                memory.push(LlmMessage {
                    role: "tool".into(),
                    content: result,
                    tool_calls: None,
                    tool_call_id,
                    image_data_url: None,
                });
                continue;
            }

            if !self.is_target(line) {
                continue;
            }

            let is_self_speaking = line.sender_role_id() == Some(self.target_role_id)
                && line.attribute() == &LineAttribute::Assistant;
            if is_self_speaking {
                if matches!(buffer_kind, Some(BufferKind::OtherBlock)) {
                    flush(&mut memory, &mut buffer, &mut buffer_kind, self);
                }
                buffer_kind = Some(BufferKind::TargetAssistant);
                buffer.push(line.clone());
            } else {
                if matches!(buffer_kind, Some(BufferKind::TargetAssistant)) {
                    flush(&mut memory, &mut buffer, &mut buffer_kind, self);
                }
                buffer_kind = Some(BufferKind::OtherBlock);
                buffer.push(line.clone());
            }
        }

        flush(&mut memory, &mut buffer, &mut buffer_kind, self);
        let memory = Self::normalize_window_head(memory, self.inject_continue_user);
        self.sanitize_tool_pairing(memory)
    }

    /// 规范化窗口头部：各家 provider 都要求（或强烈期望）首条非 system 消息是 user，
    /// 而上下文裁剪的起点可能落在 assistant / tool 上。
    ///
    /// - 窗口内存在 user：丢弃第一条 user 之前的非 system 消息，从该 user 开始；
    /// - 窗口内没有 user（工具轮挤满窗口、剧本/主动对话等本身就没有 user 台词的回合）：
    ///   丢弃第一条 assistant 之前的消息；`inject_continue_user` 打开时在它前面补一条
    ///   user「继续」，让序列仍以 user 开头（Gemini 的 `contents` 首条必须是 user）；
    /// - 连 assistant 都没有（只剩 system / 空）：原样返回，由配对清洗兜底。
    fn normalize_window_head(
        messages: Vec<LlmMessage>,
        inject_continue_user: bool,
    ) -> Vec<LlmMessage> {
        let cut = messages
            .iter()
            .position(|m| m.role == "user")
            .map(|idx| (idx, false))
            .or_else(|| {
                messages
                    .iter()
                    .position(|m| m.role == "assistant")
                    .map(|idx| (idx, inject_continue_user))
            });
        let Some((cut, inject)) = cut else {
            return messages;
        };

        // 首条之前只保留 system（人设等），其余前导消息一概丢弃。
        let mut out: Vec<LlmMessage> = messages[..cut]
            .iter()
            .filter(|m| m.role == "system")
            .cloned()
            .collect();
        if inject {
            out.push(LlmMessage::user("继续"));
        }
        out.extend(messages[cut..].iter().cloned());
        out
    }

    /// 工具调用配对清洗（上游 issue #774）：窗口裁剪可能把 `assistant(tool_calls) → tool`
    /// 拦腰切断，存档对话链断裂时也会静默丢行，留下没有声明者的孤儿 tool 消息，provider
    /// 校验失败直接返回 HTTP 400（genai 不做任何规范化，原样下发）。
    ///
    /// - assistant 的 `tool_calls` 只保留确实有 tool 结果回应的项；全被剪掉时，正文也为空
    ///   的纯工具占位行整条丢弃，有正文的降级为普通 assistant；
    /// - tool 消息只有其 `tool_call_id` 被前面 assistant 声明过才保留，孤儿一律丢弃。
    fn sanitize_tool_pairing(&self, messages: Vec<LlmMessage>) -> Vec<LlmMessage> {
        let answered: HashSet<String> = messages
            .iter()
            .filter(|m| m.role == "tool")
            .filter_map(|m| m.tool_call_id.clone())
            .collect();

        let mut declared: HashSet<String> = HashSet::new();
        let mut out: Vec<LlmMessage> = Vec::with_capacity(messages.len());
        for mut message in messages {
            match message.role.as_str() {
                "assistant" => {
                    if let Some(calls) = message.tool_calls.take() {
                        let kept: Vec<ToolCall> = calls
                            .into_iter()
                            .filter(|call| answered.contains(call.id.as_str()))
                            .collect();
                        declared.extend(kept.iter().map(|call| call.id.clone()));
                        if kept.is_empty() {
                            if message.content.trim().is_empty() {
                                continue;
                            }
                        } else {
                            message.tool_calls = Some(kept);
                        }
                    }
                    out.push(message);
                },
                "tool" => {
                    let matched = message
                        .tool_call_id
                        .as_deref()
                        .is_some_and(|id| declared.remove(id));
                    if matched {
                        out.push(message);
                    } else {
                        tracing::debug!(
                            "[MemoryBuilder] 角色 {} 丢弃孤儿 tool 消息（无前置 tool_calls 声明）",
                            self.target_role_id
                        );
                    }
                },
                _ => out.push(message),
            }
        }
        out
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ai_service::types::{FunctionCall, LineAttributeExt};

    const ROLE: i32 = 7;

    fn assistant_with_calls(ids: &[&str]) -> LlmMessage {
        LlmMessage {
            role: "assistant".into(),
            content: String::new(),
            tool_calls: Some(
                ids.iter()
                    .map(|id| ToolCall {
                        id: (*id).to_string(),
                        type_: "function".into(),
                        function: FunctionCall {
                            name: "clock".into(),
                            arguments: "{}".into(),
                        },
                    })
                    .collect(),
            ),
            tool_call_id: None,
            image_data_url: None,
        }
    }

    fn tool_msg(id: &str) -> LlmMessage {
        LlmMessage {
            role: "tool".into(),
            content: "{}".into(),
            tool_calls: None,
            tool_call_id: Some(id.to_string()),
            image_data_url: None,
        }
    }

    fn builder(lines: Vec<GameLine>, inject: bool) -> Vec<LlmMessage> {
        MemoryBuilder::new(ROLE)
            .with_continue_user(inject)
            .build(&lines)
    }

    fn line(base: LineBase) -> GameLine {
        GameLine::from_base(base, vec![ROLE])
    }

    fn line_with(attribute: LineAttribute, content: &str) -> GameLine {
        line(LineBase {
            content: content.to_string(),
            attribute: LineAttributeExt(attribute),
            ..Default::default()
        })
    }

    /// 目标角色自己说的话（会被 build 成 assistant 消息）。
    fn self_line(content: &str) -> GameLine {
        line(LineBase {
            content: content.to_string(),
            attribute: LineAttributeExt(LineAttribute::Assistant),
            sender_role_id: Some(ROLE),
            ..Default::default()
        })
    }

    fn tool_call_line(ids: &[&str]) -> GameLine {
        let calls: Vec<ToolCall> = ids
            .iter()
            .map(|id| ToolCall {
                id: (*id).to_string(),
                type_: "function".into(),
                function: FunctionCall {
                    name: "clock".into(),
                    arguments: "{}".into(),
                },
            })
            .collect();
        line(LineBase {
            tool_call: Some(serde_json::to_string(&calls).unwrap()),
            attribute: LineAttributeExt(LineAttribute::Assistant),
            ..Default::default()
        })
    }

    fn tool_line(id: &str) -> GameLine {
        line_with(
            LineAttribute::Tool,
            &format!(r#"{{"tool_call_id":"{id}","result":"12:00"}}"#),
        )
    }

    fn player_line(content: &str) -> GameLine {
        line(LineBase {
            content: content.to_string(),
            attribute: LineAttributeExt(LineAttribute::User),
            sender_role_id: Some(0),
            display_name: Some("玩家".into()),
            ..Default::default()
        })
    }

    #[test]
    fn window_with_user_drops_leading_assistant_and_tool() {
        let out = builder(vec![self_line("上一句"), player_line("你好")], true);
        assert_eq!(out.len(), 1);
        assert_eq!(out[0].role, "user");
        assert!(out[0].content.contains("你好"));
    }

    #[test]
    fn window_without_user_cuts_to_assistant_and_injects_continue() {
        let out = builder(
            vec![
                tool_line("call_0"),
                tool_call_line(&["call_1"]),
                tool_line("call_1"),
            ],
            true,
        );
        assert_eq!(out[0].role, "user");
        assert_eq!(out[0].content, "继续");
        assert_eq!(out[1].role, "assistant");
        assert_eq!(out[1].tool_calls.as_ref().map(|c| c.len()), Some(1));
        assert_eq!(out[2].role, "tool");
    }

    #[test]
    fn window_without_user_leaves_assistant_first_when_injection_disabled() {
        let out = builder(
            vec![tool_call_line(&["call_1"]), tool_line("call_1")],
            false,
        );
        assert_eq!(out[0].role, "assistant");
        assert_eq!(out[1].role, "tool");
    }

    #[test]
    fn trailing_orphan_tool_is_dropped() {
        let out = builder(
            vec![
                player_line("你好"),
                tool_call_line(&["call_1"]),
                tool_line("call_1"),
                tool_line("orphan_z"),
            ],
            true,
        );
        assert_eq!(out.len(), 3);
        assert_eq!(out[0].role, "user");
        assert_eq!(out[2].role, "tool");
        assert_eq!(out[2].tool_call_id.as_deref(), Some("call_1"));
    }

    #[test]
    fn unanswered_tool_calls_are_pruned() {
        let out = builder(
            vec![
                player_line("你好"),
                tool_call_line(&["call_0", "call_1"]),
                tool_line("call_1"),
            ],
            true,
        );
        let assistant = out.iter().find(|m| m.role == "assistant").unwrap();
        let calls = assistant.tool_calls.as_ref().unwrap();
        assert_eq!(calls.len(), 1);
        assert_eq!(calls[0].id, "call_1");
        assert!(out.iter().any(|m| m.role == "tool"));
    }

    #[test]
    fn assistant_with_all_calls_pruned_and_empty_content_is_dropped() {
        let out = builder(vec![player_line("你好"), tool_call_line(&["call_0"])], true);
        assert!(!out.iter().any(|m| m.role == "assistant"));
        assert_eq!(out[0].role, "user");
    }

    #[test]
    fn valid_tool_pair_is_kept_intact() {
        let out = builder(
            vec![tool_call_line(&["call_1"]), tool_line("call_1")],
            false,
        );
        assert_eq!(out.len(), 2);
        assert_eq!(out[0].role, "assistant");
        assert_eq!(out[1].role, "tool");
        assert_eq!(out[1].tool_call_id.as_deref(), Some("call_1"));
    }
}
