use crate::ai_service::types::{GameLine, LineBase, LlmMessage};
use crate::db::entities::line::LineAttribute;

/// 将 `GameLine` 序列构建成目标角色的 LLM 消息列表。
pub struct MemoryBuilder {
    pub target_role_id: i32,
}

enum BufferKind {
    TargetAssistant,
    OtherBlock,
}

impl MemoryBuilder {
    pub fn new(target_role_id: i32) -> Self {
        Self { target_role_id }
    }

    fn is_target(&self, line: &GameLine) -> bool {
        if line.sender_role_id() == Some(self.target_role_id) {
            return true;
        }
        line.perceived_role_ids.contains(&self.target_role_id)
    }

    /// 格式化内容：【情绪】内容<TTS>（动作），仅用于 assistant (AI自身) 消息。
    fn format_content_with_extras(&self, line: &LineBase) -> String {
        let mut s = String::new();
        if let Some(emo) = line.original_emotion.as_deref().filter(|v| !v.is_empty()) {
            s.push('【');
            s.push_str(emo);
            s.push('】');
        }
        s.push('\n');
        s.push_str(&line.content);
        s.push('\n');
        if let Some(tts) = line.tts_content.as_deref().filter(|v| !v.is_empty()) {
            s.push('<');
            s.push_str(tts);
            s.push('>');
        }
        s.push('\n');
        if let Some(act) = line.action_content.as_deref().filter(|v| !v.is_empty()) {
            s.push('(');
            s.push_str(act);
            s.push(')');
        }
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
                }
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
                                let mut s = match name {
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
                }
                None => {}
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
                let has_tool_call = line.base.tool_call.as_deref().map(|s| !s.is_empty()).unwrap_or(false);

                if has_tool_call {
                    // 新版：tool_call 存 JSON，content 纯文本
                    if let Ok(tool_calls) = serde_json::from_str::<Vec<crate::ai_service::types::ToolCall>>(
                        line.base.tool_call.as_deref().unwrap_or(""),
                    ) {
                        flush(&mut memory, &mut buffer, &mut buffer_kind, self);
                        memory.push(LlmMessage {
                            role: "assistant".into(),
                            content: line.base.content.clone(),
                            tool_calls: Some(tool_calls),
                            tool_call_id: None,
                        });
                        continue;
                    }
                } else if !line.base.content.is_empty() {
                    // 旧版兼容：content = "tool_calls_json\n\ntext"
                    let (tool_calls_json, text) = if let Some(idx) = line.base.content.find("\n\n") {
                        let (head, tail) = line.base.content.split_at(idx);
                        (head, tail.strip_prefix("\n\n").unwrap_or(""))
                    } else {
                        (line.base.content.as_str(), "")
                    };
                    if let Ok(tool_calls) = serde_json::from_str::<Vec<crate::ai_service::types::ToolCall>>(tool_calls_json) {
                        flush(&mut memory, &mut buffer, &mut buffer_kind, self);
                        memory.push(LlmMessage {
                            role: "assistant".into(),
                            content: text.to_string(),
                            tool_calls: Some(tool_calls),
                            tool_call_id: None,
                        });
                        continue;
                    }
                }
            }

            // 工具返回行：content 存 JSON {"tool_call_id":..., "result":...}
            if matches!(line.attribute(), LineAttribute::Tool) {
                flush(&mut memory, &mut buffer, &mut buffer_kind, self);
                let (tool_call_id, result) = serde_json::from_str::<serde_json::Value>(&line.base.content)
                    .ok()
                    .map(|v| (
                        v.get("tool_call_id").and_then(|s| s.as_str()).map(String::from),
                        v.get("result").map(|r| r.to_string()).unwrap_or_default(),
                    ))
                    .unwrap_or((None, line.base.content.clone()));
                memory.push(LlmMessage {
                    role: "tool".into(),
                    content: result,
                    tool_calls: None,
                    tool_call_id,
                });
                continue;
            }

            if !self.is_target(line) {
                continue;
            }

            let is_self_speaking = line.sender_role_id() == Some(self.target_role_id);
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
        memory
    }
}
