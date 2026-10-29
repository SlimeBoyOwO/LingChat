use std::collections::HashSet;

use crate::ai_service::message_system::processor::truncate_at_turn_end;
use crate::ai_service::types::{GameLine, LineBase, LlmMessage, ToolCall};
use crate::db::entities::line::LineAttribute;

/// 将 `GameLine` 序列构建成目标角色的 LLM 消息列表。
pub struct MemoryBuilder {
    pub target_role_id: i32,
    /// 窗口内没有任何 user 消息时，是否在裁切后的首条 assistant 前注入一条 user「继续」。
    /// 关闭时只裁切、不注入（首条会是 assistant，OpenAI 兼容端点接受，Gemini 会 400）。
    inject_continue_user: bool,
    player_alias: Option<String>,
    player_identity_id: Option<String>,
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
            player_alias: None,
            player_identity_id: None,
        }
    }

    pub fn with_player_alias(mut self, alias: Option<String>) -> Self {
        self.player_alias = alias;
        self
    }

    pub fn with_player_identity(mut self, identity_id: Option<String>) -> Self {
        self.player_identity_id = identity_id;
        self
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
        s.push_str(truncate_at_turn_end(&line.content).trim_end());
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
                        // 回合末尾留两个空行（台词之间只有一个空行），给模型一个能照抄的
                        // 「本轮到此为止」形状；流式侧也按这个边界截断越界内容。
                        memory.push(LlmMessage::assistant(format!("{full}\n")));
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
                        // 玩家行一律带名（"永远带名"）：换身份后历史里并存多个身份的台词，
                        // 只靠 role=user 无法区分是谁说的；旁白/系统行保持裸内容。
                        let user_text: Vec<String> = active_user_lines
                            .iter()
                            .map(|l| {
                                let name = match l.sender_role_id() {
                                    Some(0)
                                        if self
                                            .player_alias
                                            .as_deref()
                                            .is_some_and(|s| !s.is_empty())
                                            && l.base.identity_id.is_some()
                                            && l.base.identity_id == self.player_identity_id =>
                                    {
                                        self.player_alias.as_deref().unwrap_or_default()
                                    },
                                    _ => l.base.display_name.as_deref().unwrap_or("未知"),
                                };
                                match name {
                                    "旁白" | "系统" => l.base.content.clone(),
                                    _ => format!("{}: {}", name, l.base.content),
                                }
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
            // 只有目标角色自己的第一条人设会成为 system 消息；
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
                    // 新版：tool_call 存 JSON，content 纯文本。
                    // 带 round_id 的载体行只合并同轮标识的前导台词；标识之前的行
                    // 属于上一轮回复，先按普通 assistant 消息 flush。
                    if let Ok(tool_calls) =
                        serde_json::from_str::<Vec<crate::ai_service::types::ToolCall>>(
                            line.base.tool_call.as_deref().unwrap_or(""),
                        )
                    {
                        let split_at = line.base.round_id.as_deref().and_then(|round_id| {
                            if matches!(buffer_kind, Some(BufferKind::TargetAssistant)) {
                                buffer
                                    .iter()
                                    .position(|l| l.base.round_id.as_deref() == Some(round_id))
                            } else {
                                None
                            }
                        });
                        let content = if let Some(split_at) = split_at {
                            let suffix = buffer.split_off(split_at);
                            flush(&mut memory, &mut buffer, &mut buffer_kind, self);
                            buffer = suffix;
                            let merged: String = buffer
                                .iter()
                                .map(|l| self.format_content_with_extras(&l.base))
                                .collect();
                            buffer.clear();
                            buffer_kind = None;
                            merged.trim_end().to_string()
                        } else {
                            flush(&mut memory, &mut buffer, &mut buffer_kind, self);
                            line.base.content.clone()
                        };
                        memory.push(LlmMessage {
                            role: "assistant".into(),
                            content,
                            tool_calls: Some(tool_calls),
                            tool_call_id: None,
                            image_data_url: None,
                        });
                        continue;
                    }
                    // 空载体行解析失败时直接跳过，避免被当成 OtherBlock 混进上下文
                    if line.base.content.trim().is_empty() {
                        tracing::warn!("工具调用载体行的 tool_call 无法解析，已跳过");
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

    /// 工具调用配对清洗
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
