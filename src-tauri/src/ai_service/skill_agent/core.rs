//! Skill Agent 核心：多轮工具调用循环。
//!
//! 复刻 ling_chat_agent `llm.rs` 的循环结构（`parse_tool_args` / 逐轮回填 /
//! 轮数上限，-1 为无上限），但把 DeepSeek 直连 SSE 替换为 LingChat 的 `LlmClient`
//! （流式 provider 走 `complete_stream_with_tools`，非流式走 `complete_with_tools`）。
//! 历史与工具结果完整保留、不做裁剪：保证模型看到全部上下文。
//! 仅对从 DB 重载的历史做 `sanitize_history` 规整，修复上一轮中断遗留的
//! 「assistant(tool_calls) 缺 tool 回应」畸形轮次（否则 OpenAI 校验直接 400）。

use std::collections::HashMap;
use std::path::Path;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;

use futures_util::StreamExt;
use sea_orm::DatabaseConnection;
use serde_json::Value;

use crate::ai_service::llm::{LlmChunk, LlmClient};
use crate::ai_service::skill_agent::command_executor::ApprovalMap;
use crate::ai_service::skill_agent::config::SkillAgentConfig;
use crate::ai_service::skill_agent::events::{SkillAgentEvent, Usage};
use crate::ai_service::skill_agent::{db, skills, tools};
use crate::ai_service::types::{FunctionCall, LlmMessage, ToolCall, ToolDefinition};

/// 取消标志，跨 chat 运行共享。
pub type CancelFlag = Arc<AtomicBool>;

/// 单次对话运行上下文。
pub struct SkillAgentRunContext {
    pub conversation_id: i32,
    /// 流式事件推送通道。
    pub channel: tauri::ipc::Channel<SkillAgentEvent>,
    pub approvals: ApprovalMap,
    pub db: DatabaseConnection,
    pub llm: Arc<LlmClient>,
    pub config: SkillAgentConfig,
    pub sandbox_dir: std::path::PathBuf,
    pub skills_dir: std::path::PathBuf,
    /// 会话绑定的剧本 key（运行时解析为路径注入系统提示）。
    pub script_key: Option<String>,
}

/// 累积中的工具调用（流式分片拼接）。
#[derive(Debug, Clone)]
struct AccumToolCall {
    index: usize,
    id: String,
    name: String,
    arguments: String,
}

// ---------- 系统提示 ----------

/// 构建「当前剧本」段：给出 key/路径，并指示 agent 先看已有内容（实时读取，不注入静态快照）。
fn build_script_block(sandbox_dir: &Path, script_key: Option<&str>) -> String {
    let Some(key) = script_key else {
        return String::new();
    };
    match crate::utils::script_paths::resolve_script_dir(key) {
        Ok(dir) => {
            let rel = dir.strip_prefix(sandbox_dir).unwrap_or(&dir);
            format!(
                "\n\n【当前剧本上下文】\n剧本 key：{}\n剧本目录：{}（相对于文件沙箱根 {}）\n\n工作之前，请先用 list_files / read_file 查看剧本中已有的内容，再决定如何编写或修改。",
                key,
                rel.display(),
                sandbox_dir.display()
            )
        }
        Err(_) => String::new(), // 剧本缺失/失效 → 降级，不阻断对话
    }
}

fn build_system_prompt(
    config: &SkillAgentConfig,
    skills_block: &str,
    script_block: &str,
    sandbox_dir: &Path,
    skills_dir: &Path,
) -> String {
    let tool_names = tools::tool_names();
    let default = format!(
        "你是运行在本机 LingChat 桌面应用里的 AI 剧本创作助手。你拥有以下能力：\
\n- 调用工具完成真实操作：{tool_names}\
\n- 通过 read_skill 加载技能指令后再执行任务\
\n- 文件路径默认相对于文件沙箱根目录（{sandbox}）\
\n- 技能目录：{skills_dir}（技能文件以 SKILL.md 存放，需要时可用 list_files / read_file 直接查看）\
\n- execute_command 可能需要用户确认\
\n使用规则：\
\n1. 当任务匹配某个技能的描述时，先调用 read_skill 加载该技能，再按指令执行；已读取过的技能不要重复读取\
\n2. 需要操作文件时使用 list_files / read_file / write_file / delete_file\
\n3. 需要运行本地命令时使用 execute_command；命令由 cmd 执行，带空格的参数请用引号包裹（引号会原样传递）\
\n4. 任务必须完成到产出物为止：读取技能、查询配色、运行搜索都只是中间步骤，最终必须调用 write_file 实际写出用户要求的文件，才算完成任务\
\n5. 未写出文件之前禁止总结收尾，禁止以「已获取到所需信息」「以上就是设计建议」之类的说法结束回答；继续调用工具，直到文件真正创建成功\
\n6. 写文件时一次性用 write_file 写完整内容，不要提前分段；只有当一次写入因参数过长而失败（报错会附带 [诊断] 提示）时，才改用 write_file（append=true）分段补齐\
\n7. 文件范围受限时如实说明，不要编造文件内容",
        tool_names = tool_names,
        sandbox = sandbox_dir.display(),
        skills_dir = skills_dir.display(),
    );

    let base = match &config.system_prompt {
        Some(custom) if !custom.trim().is_empty() => custom.clone(),
        _ => default,
    };
    format!("{}{}{}", base, script_block, skills_block)
}

// ---------- 参数归一化 ----------

/// 归一化工具调用的 `arguments`。有些模型输出非标准形态（嵌套 `{"arguments": {...}}`
/// / `{"params": {...}}`、双编码 JSON 字符串、甚至非法 JSON）。统一成普通对象，
/// 绝不返回 `null` 让 UI 崩溃。
fn parse_tool_args(raw: &str) -> Value {
    let mut args: Value = serde_json::from_str(raw).unwrap_or(Value::Null);

    if let Some(s) = args.as_str() {
        if let Ok(parsed) = serde_json::from_str::<Value>(s) {
            args = parsed;
        }
    }

    if let Value::Object(map) = &args {
        if map.len() == 1 {
            if let Some(inner) = map.get("arguments").or_else(|| map.get("params")) {
                if inner.is_object() {
                    args = inner.clone();
                }
            }
        }
    }

    if !args.is_object() {
        args = serde_json::json!({});
    }
    args
}

// ---------- 历史规整 ----------

/// 规整从 DB 加载的历史，修复/丢弃不完整的工具轮次。
///
/// 背景：某轮生成 `assistant(tool_calls)` 后、对应 `tool` 结果全部落库前，若运行被
/// 中断（用户点停止、崩溃、DB 写失败），DB 里会留下「带 tool_calls 却没有工具回应」
/// 的孤立 assistant。OpenAI 接口校验会直接 400：带 `tool_calls` 的 assistant 消息
/// 必须被紧随的 tool 消息逐一回应（insufficient tool messages following tool_calls）。
///
/// 这里把这种残缺轮次降级为纯文本 assistant（保留正文、去掉 tool_calls），并丢弃
/// 无主的孤立 tool 消息，保证任何一次重载后的请求都合法。完整轮次原样保留。
pub fn sanitize_history(history: Vec<LlmMessage>) -> Vec<LlmMessage> {
    let mut out = Vec::with_capacity(history.len());
    let mut i = 0usize;
    while i < history.len() {
        let msg = &history[i];

        if msg.role == "assistant" && msg.tool_calls.is_some() {
            let expected: Vec<String> = msg
                .tool_calls
                .as_ref()
                .map(|tcs| tcs.iter().map(|tc| tc.id.clone()).collect())
                .unwrap_or_default();

            // 从下一条起连续收集紧随其后的 tool 回应
            let mut j = i + 1;
            let mut actual: Vec<String> = Vec::new();
            while j < history.len() && history[j].role == "tool" {
                if let Some(id) = history[j].tool_call_id.clone() {
                    actual.push(id);
                }
                j += 1;
            }

            // 完整：每个期望的 tool_call_id 都有回应
            let complete = !expected.is_empty() && expected.iter().all(|id| actual.contains(id));

            if complete {
                out.push(msg.clone());
                // 只搬运匹配期望 id 的回应，多出来的孤立 tool 一并丢弃
                for k in (i + 1)..j {
                    let t = &history[k];
                    if let Some(id) = t.tool_call_id.as_ref() {
                        if expected.contains(id) {
                            out.push(t.clone());
                        }
                    }
                }
            } else {
                // 残缺轮次：降级为纯文本 assistant，保留正文
                let mut fixed = msg.clone();
                fixed.tool_calls = None;
                out.push(fixed);
            }
            i = j;
        } else if msg.role == "tool" {
            // 无主 tool（前面没有待回应的 assistant）→ 丢弃
            i += 1;
        } else {
            out.push(msg.clone());
            i += 1;
        }
    }
    out
}

// ---------- 主循环 ----------

/// 运行一次对话（一次用户消息 = 一次调用）。历史由 `history` 传入（不含 system）。
pub async fn run_chat(
    ctx: SkillAgentRunContext,
    history: Vec<LlmMessage>,
    cancelled: CancelFlag,
) -> Result<(), String> {
    let approval_mode = if ctx.config.auto_approve_commands {
        "命令自动执行（无需确认）"
    } else {
        "命令需手动确认"
    };
    let _ = ctx.channel.send(SkillAgentEvent::Status {
        content: format!("思考中…（{}）", approval_mode),
    });

    let skill_list = skills::find_all_skills(&ctx.skills_dir);
    let skills_block = skills::build_skills_xml(&skill_list);
    let script_block = build_script_block(&ctx.sandbox_dir, ctx.script_key.as_deref());
    let system_prompt = build_system_prompt(
        &ctx.config,
        &skills_block,
        &script_block,
        &ctx.sandbox_dir,
        &ctx.skills_dir,
    );

    let mut messages: Vec<LlmMessage> = Vec::with_capacity(history.len() + 1);
    messages.push(LlmMessage::system(system_prompt));
    // 历史先规整再并入：DB 里可能残留上一轮中断产生的「无 tool 回应的 assistant
    // (tool_calls)」，不处理会触发 OpenAI 400（insufficient tool messages）。
    messages.extend(sanitize_history(history));

    // -1 表示无上限（保留全部上下文与工具轮次）；否则为有限轮数，至少 1 轮。
    // 无上限时用 usize::MAX 作区间上界，`round == max_rounds - 1` 的下限检查永不触发。
    let max_rounds: usize = if ctx.config.max_tool_rounds < 0 {
        usize::MAX
    } else {
        (ctx.config.max_tool_rounds as usize).max(1)
    };
    let mut turn_prompt_tokens: u64 = 0;
    let mut turn_completion_tokens: u64 = 0;

    for round in 0..max_rounds {
        if cancelled.load(Ordering::SeqCst) {
            let _ = ctx.channel.send(SkillAgentEvent::Status {
                content: "已停止生成".into(),
            });
            return Ok(());
        }

        let defs = tools::tool_definitions();
        let (assistant_text, tool_calls, usage) =
            match stream_completion(&ctx, &messages, &defs, &cancelled).await {
                Ok(r) => r,
                Err(e) => {
                    let _ = ctx.channel.send(SkillAgentEvent::Error { message: e.clone() });
                    return Err(e);
                }
            };
        turn_prompt_tokens += usage.prompt_tokens;
        turn_completion_tokens += usage.completion_tokens;

        // 无工具调用 → 完成
        if tool_calls.is_empty() {
            let final_msg = LlmMessage::assistant(&assistant_text);
            let _ = db::insert_message(&ctx.db, ctx.conversation_id, &final_msg).await;
            let usage = if turn_prompt_tokens + turn_completion_tokens > 0 {
                Some(Usage {
                    prompt_tokens: turn_prompt_tokens,
                    completion_tokens: turn_completion_tokens,
                    total_tokens: turn_prompt_tokens + turn_completion_tokens,
                })
            } else {
                None
            };
            let _ = ctx.channel.send(SkillAgentEvent::Done {
                final_text: assistant_text,
                usage,
            });
            return Ok(());
        }

        // 有工具调用：回填 assistant(tool_calls) 并持久化
        let assistant_msg = LlmMessage {
            role: "assistant".into(),
            content: assistant_text.clone(),
            tool_calls: Some(
                tool_calls
                    .iter()
                    .map(|tc| ToolCall {
                        id: tc.id.clone(),
                        type_: "function".into(),
                        function: FunctionCall {
                            name: tc.name.clone(),
                            arguments: tc.arguments.clone(),
                        },
                    })
                    .collect(),
            ),
            tool_call_id: None,
        };
        messages.push(assistant_msg.clone());
        let _ = db::insert_message(&ctx.db, ctx.conversation_id, &assistant_msg).await;

        // 逐个执行工具，回填 tool 结果并持久化
        for tc in &tool_calls {
            let call_id = if tc.id.is_empty() {
                format!("call-{}-{}", std::process::id(), tc.index)
            } else {
                tc.id.clone()
            };
            let args = parse_tool_args(&tc.arguments);
            let _ = ctx.channel.send(SkillAgentEvent::ToolCall {
                call_id: call_id.clone(),
                tool: tc.name.clone(),
                args: args.clone(),
                raw_args: tc.arguments.clone(),
            });

            let (ok, mut output) = tools::execute_tool(&ctx, &tc.name, &args).await;

            // 参数不是有效 JSON → 大概率生成被截断，附上原文片段便于模型/user 定位
            if !ok
                && !tc.arguments.trim().is_empty()
                && serde_json::from_str::<Value>(&tc.arguments).is_err()
            {
                let snippet: String = tc.arguments.chars().take(400).collect();
                output = format!(
                    "{}\n\n[诊断] 本次工具调用的参数不是有效 JSON，可能是内容过长被截断。收到的参数开头：\n{}",
                    output, snippet
                );
            }

            let _ = ctx.channel.send(SkillAgentEvent::ToolResult {
                call_id: call_id.clone(),
                tool: tc.name.clone(),
                ok,
                output: output.clone(),
                error: None,
            });

            let tool_msg = LlmMessage::tool_result(tc.id.clone(), &output);
            messages.push(tool_msg.clone());
            let _ = db::insert_message(&ctx.db, ctx.conversation_id, &tool_msg).await;
        }

        if round == max_rounds - 1 {
            let _ = ctx.channel.send(SkillAgentEvent::Error {
                message: format!("已达到最大工具调用轮数（{}），已停止", max_rounds),
            });
            return Ok(());
        }
    }

    Ok(())
}

// ---------- LLM 调用（双路径） ----------

async fn stream_completion(
    ctx: &SkillAgentRunContext,
    messages: &[LlmMessage],
    defs: &[ToolDefinition],
    cancelled: &CancelFlag,
) -> Result<(String, Vec<AccumToolCall>, Usage), String> {
    let llm = &ctx.llm;
    let mut text_out = String::new();
    let usage = Usage::default();
    let mut tool_map: HashMap<usize, AccumToolCall> = HashMap::new();

    if llm.supports_streaming_tools() {
        let mut stream = llm
            .complete_stream_with_tools(messages, defs, Some("auto"))
            .await
            .map_err(|e| e.to_string())?;
        while let Some(chunk) = stream.next().await {
            if cancelled.load(Ordering::SeqCst) {
                break;
            }
            let chunk = chunk.map_err(|e| e.to_string())?;
            match chunk {
                LlmChunk::Content(c) => {
                    text_out.push_str(&c);
                    let _ = ctx.channel.send(SkillAgentEvent::MessageDelta { content: c });
                }
                LlmChunk::Reasoning(r) => {
                    let _ = ctx.channel.send(SkillAgentEvent::Reasoning { content: r });
                }
                LlmChunk::ToolCalls(calls) => {
                    for tc in calls {
                        let idx = tool_map.len();
                        tool_map.insert(
                            idx,
                            AccumToolCall {
                                index: idx,
                                id: tc.id,
                                name: tc.function.name,
                                arguments: tc.function.arguments,
                            },
                        );
                    }
                }
            }
        }
    } else {
        let resp = llm
            .complete_with_tools(messages, defs, Some("auto"))
            .await
            .map_err(|e| e.to_string())?;
        if let Some(c) = resp.content {
            if !c.is_empty() {
                text_out.push_str(&c);
                let _ = ctx.channel.send(SkillAgentEvent::MessageDelta { content: c });
            }
        }
        if let Some(calls) = resp.tool_calls {
            for tc in calls {
                let idx = tool_map.len();
                tool_map.insert(
                    idx,
                    AccumToolCall {
                        index: idx,
                        id: tc.id,
                        name: tc.function.name,
                        arguments: tc.function.arguments,
                    },
                );
            }
        }
    }

    let mut tool_calls: Vec<AccumToolCall> = tool_map.into_values().collect();
    tool_calls.sort_by_key(|t| t.index);
    for tc in &mut tool_calls {
        if tc.id.is_empty() {
            tc.id = format!("call_{}_{}", std::process::id(), tc.index);
        }
    }
    Ok((text_out, tool_calls, usage))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ai_service::types::{FunctionCall, ToolCall};

    fn tc(id: &str, name: &str) -> ToolCall {
        ToolCall {
            id: id.to_string(),
            type_: "function".to_string(),
            function: FunctionCall {
                name: name.to_string(),
                arguments: "{}".to_string(),
            },
        }
    }

    #[test]
    fn sanitize_history_keeps_complete_rounds_untouched() {
        let history = vec![
            LlmMessage::user("写个剧本"),
            {
                let mut m = LlmMessage::assistant("先查技能");
                m.tool_calls = Some(vec![tc("a", "read_skill")]);
                m
            },
            LlmMessage::tool_result("a", "技能内容"),
            LlmMessage::assistant("完成"),
        ];
        let fixed = sanitize_history(history.clone());
        assert_eq!(fixed.len(), history.len());
        assert!(fixed[1].tool_calls.is_some());
        assert_eq!(fixed[2].tool_call_id.as_deref(), Some("a"));
    }

    #[test]
    fn sanitize_history_repairs_orphaned_assistant_tool_calls() {
        // 上一轮中断：assistant 带 tool_calls，但没有 tool 回应，直接跟了一条 user。
        // 这正是触发 OpenAI 400 的场景。
        let history = vec![
            LlmMessage::user("继续"),
            {
                let mut m = LlmMessage::assistant("准备执行命令");
                m.tool_calls = Some(vec![tc("orphan", "execute_command")]);
                m
            },
            LlmMessage::user("新的提问"),
        ];
        let fixed = sanitize_history(history);
        assert_eq!(fixed.len(), 3);
        // 孤立 assistant 降级为纯文本：保留正文、去掉 tool_calls
        assert_eq!(fixed[1].role, "assistant");
        assert!(fixed[1].tool_calls.is_none());
        assert_eq!(fixed[1].content, "准备执行命令");
        // user 消息原样保留
        assert_eq!(fixed[2].role, "user");
        assert_eq!(fixed[2].content, "新的提问");
    }

    #[test]
    fn sanitize_history_drops_unmatched_tool_responses() {
        // 回应 id 与 assistant 的 tool_calls 不匹配 → 整轮视为残缺
        let history = vec![
            LlmMessage::user("a"),
            {
                let mut m = LlmMessage::assistant("");
                m.tool_calls = Some(vec![tc("x", "read_file")]);
                m
            },
            LlmMessage::tool_result("y", "不该出现的回应"),
            LlmMessage::assistant("继续"),
        ];
        let fixed = sanitize_history(history);
        // 残缺 assistant 被降级，错配的 tool 被丢弃
        assert_eq!(fixed.len(), 3);
        assert!(fixed[1].tool_calls.is_none());
        assert!(fixed.iter().all(|m| m.role != "tool"));
    }

    #[test]
    fn sanitize_history_drops_dangling_tool_without_assistant() {
        let history = vec![
            LlmMessage::user("a"),
            LlmMessage::tool_result("ghost", "孤儿工具结果"),
            LlmMessage::assistant("正常回复"),
        ];
        let fixed = sanitize_history(history);
        assert_eq!(fixed.len(), 2);
        assert_eq!(fixed[0].role, "user");
        assert_eq!(fixed[1].role, "assistant");
    }

    #[test]
    fn parse_tool_args_normalizes_nonstandard_shapes() {
        let v = parse_tool_args(r#"{"path":"a.txt","content":"hi"}"#);
        assert_eq!(v["path"], "a.txt");
        assert_eq!(v["content"], "hi");

        let v = parse_tool_args(r#"{"arguments":{"path":"a.txt","content":"hi"}}"#);
        assert_eq!(v["path"], "a.txt");
        assert_eq!(v["content"], "hi");

        let v = parse_tool_args(r#"{"params":{"command":"dir"}}"#);
        assert_eq!(v["command"], "dir");

        let v = parse_tool_args(r#""{\"path\":\"a.txt\"}""#);
        assert_eq!(v["path"], "a.txt");

        let v = parse_tool_args("{not valid json");
        assert!(v.is_object());
        assert_eq!(v, serde_json::json!({}));
    }

}
