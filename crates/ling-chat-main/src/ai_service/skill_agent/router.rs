//! 流程 Agent（路由）：判断这一轮到底要干什么；候选枚举进提示词，决策是一次工具调用（不解析自由文本）。

use std::path::Path;

use crate::ai_service::llm::LlmClient;
use crate::ai_service::types::{LlmMessage, ToolDefinition, parse_tool_args};

use super::role::{self, TaskKind};
use super::stage::{self, StageSnapshot};

/// 判据（`reason`）的长度口径：一处定义、三处共用（schema 告诉模型的 / 解析校验的 / 失败原因截断的），取 60 而非 40。
const REASON_MAX_CHARS: usize = 60;

/// 队列上限：一根保险丝，防"模型一次排几十项"，不防"用户一次说很多件事"；真被截到时打一条 warn。
const MAX_QUEUE: usize = 100;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct QueueItem {
    pub kind: TaskKind,
    pub target: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RoutePlan {
    /// 有序队列；第一项就是本轮要做的。永不为空。
    pub items: Vec<QueueItem>,
    pub boundary: Option<String>,
    /// 这么判的判据（一句话）。进日志，供事后回看路由判断。
    pub reason: Option<String>,
}

impl RoutePlan {
    fn single(kind: TaskKind, target: &str) -> Self {
        Self {
            items: vec![QueueItem {
                kind,
                target: target.to_string(),
            }],
            boundary: None,
            reason: None,
        }
    }

    pub fn chat() -> Self {
        Self::single(TaskKind::Chat, "")
    }

    /// 「只回话」+ 一句为什么，是流程 Agent 失败时唯一的去处（见 [`route`]）：不给任何写工具，就乱改不了文件。
    fn chat_due_to(why: &str) -> Self {
        let mut plan = Self::chat();
        plan.reason = Some(why.chars().take(REASON_MAX_CHARS).collect());
        plan
    }
}

pub struct RouteInput<'a> {
    pub snapshot: &'a StageSnapshot,
    pub queue: &'a str,
    /// 上两句纯对话（去思考、去工具），旧的在前；用于解「继续 / 都行 / 按你说的」。
    pub recent: &'a [(String, String)],
    pub user_msg: &'a str,
    pub skills_dir: &'a Path,
}

/// 判断这一轮要干什么，失败只回话：总纲读不到 / LLM 没配好直接只回话并写明 reason；
pub async fn route(llm: &LlmClient, input: &RouteInput<'_>) -> RoutePlan {
    if input.user_msg.trim().is_empty() {
        return RoutePlan::chat();
    }
    let Some(hub) = stage::prompt::hub_doc(input.skills_dir) else {
        tracing::warn!("[router] 流程总纲缺失（技能库不完整），这一轮只回话");
        return RoutePlan::chat_due_to("技能库不完整：读不到流程总纲");
    };
    if !llm.config().is_usable() {
        tracing::warn!("[router] LLM 配置不可用，这一轮只回话");
        return RoutePlan::chat_due_to("模型没配好：LLM 配置不可用");
    }

    let messages = vec![
        LlmMessage::system(build_router_system(&hub)),
        LlmMessage::user(build_router_user(input)),
    ];
    let tools = vec![submit_plan_tool()];

    let mut last_err = String::from("路由失败");
    for attempt in 1..=2 {
        match llm
            .complete_with_tools(&messages, &tools, Some("auto"))
            .await
        {
            Ok(resp) => {
                let parsed = resp
                    .tool_calls
                    .as_ref()
                    .and_then(|calls| calls.first())
                    .and_then(|tc| parse_plan(&tc.function.arguments));
                match parsed {
                    Some(plan) => return plan,
                    None => {
                        tracing::warn!("[router] 第 {attempt} 次没有可用的 submit_plan");
                        last_err = "没给出可用的计划".to_string();
                    },
                }
            },
            Err(e) => {
                tracing::warn!("[router] 第 {attempt} 次调用失败: {e}");
                last_err = format!("路由调用失败：{e}");
            },
        }
    }

    tracing::warn!("[router] 重试后仍失败，这一轮只回话: {last_err}");
    RoutePlan::chat_due_to(&last_err)
}

/// 稳定部分：角色 + 流程总纲 + 判断规则。放 system 以保前缀缓存。
fn build_router_system(hub: &str) -> String {
    let kinds: Vec<String> = TaskKind::ALL
        .iter()
        .map(|k| match k {
            TaskKind::Outline => format!(
                "`{}` = {}（**写内容**：设计稿里那几行梗概 —— 总大纲、每章一句话、结局走向、章数）",
                k.key(),
                k.label()
            ),
            TaskKind::DraftChapter => format!(
                "`{}` = {}（**写内容**：某一章的小说正文，落到细节稿里那一节；大纲归 `outline`）",
                k.key(),
                k.label()
            ),
            TaskKind::WriteChapter => format!(
                "`{}` = {}（**转文件**：把已经写好的内容转成能跑的 `Chapters/<id>.yaml`，不做创作）",
                k.key(),
                k.label()
            ),
            TaskKind::ReviseChapter => format!(
                "`{}` = {}（改已经转好的章节 YAML；改的时候可以往上对齐细节稿与大纲，\
                 删除时只删这一章的 YAML）",
                k.key(),
                k.label()
            ),
            _ => format!("`{}` = {}", k.key(), k.label()),
        })
        .collect();
    format!(
        include_str!("prompts/router_build_router_system.txt"),
        kinds.join("\n"),
        crate::ai_service::skill_agent::role::ACTION_VOCAB,
        crate::ai_service::skill_agent::role::ROUTING_LAND_RULES,
        hub,
    )
}

fn build_router_user(input: &RouteInput<'_>) -> String {
    let snap = input.snapshot;
    let mut out = String::from("【剧本现状】\n");
    match snap.script_key.as_deref() {
        Some(key) => out.push_str(&format!("剧本：{key}\n")),
        None => out.push_str("剧本：还没绑定（用户可能想新建一个）\n"),
    }
    if snap.plan.is_empty() {
        out.push_str("设计稿：无（或没列出章节）\n");
    } else {
        out.push_str(&format!("设计稿已列出章节：{}\n", snap.plan.join(" ")));
    }
    out.push_str(&format!(
        "已落盘章节：{}\n",
        if snap.written.is_empty() {
            "（无）".to_string()
        } else {
            snap.written.join(" ")
        }
    ));

    if !input.queue.trim().is_empty() {
        out.push_str(
            "\n【上一轮排过、还没做完的（**只是状态，不是本轮的活**）】\n\
             除非用户这一句是在让你接着做（「继续」「那把它弄完」「剩下的做完」），\
             否则**不要**把它们排进 `tasks` —— 他这一句说的是什么，就排什么。\n",
        );
        out.push_str(input.queue.trim());
        out.push('\n');
    }
    if !input.recent.is_empty() {
        out.push_str("\n【上两句对话】\n");
        for (user, assistant) in input.recent {
            out.push_str(&format!(
                "用户：{}\n你：{}\n",
                squash(user),
                squash(assistant)
            ));
        }
    }
    out.push_str(&format!("\n【用户这一句】\n{}", input.user_msg.trim()));
    out
}

fn squash(text: &str) -> String {
    const MAX: usize = 120;
    let t = text.trim().replace('\n', " ");
    if t.chars().count() <= MAX {
        return t;
    }
    format!("{}…", t.chars().take(MAX).collect::<String>())
}

/// 路由的提交工具。参数用 enum 约束任务类型 —— 模型只能在我们列出的取值里选。
fn submit_plan_tool() -> ToolDefinition {
    let kinds: Vec<&str> = TaskKind::ALL.iter().map(|k| k.key()).collect();
    ToolDefinition::new(
        "submit_plan",
        "提交这一轮的判断：本轮要做的任务，以及这段对话后续还要做的事（有序）。",
        serde_json::json!({
            "type": "object",
            "properties": {
                "tasks": {
                    "type": "array",
                    "description": format!(
                        "有序任务队列，第一项是本轮要做的。至少一项，最多 {MAX_QUEUE} 项。"
                    ),
                    "items": {
                        "type": "object",
                        "properties": {
                            "kind": {"type": "string", "enum": kinds},
                            "target": {
                                "type": "string",
                                "description": "作用对象：**章节 id 或章号列表**（如 03 / 02,03）。\
        写正文 / 转成 YAML / 改 YAML **一章一项**，多章要排成多项；只有编写大纲 / 改大纲可以一项带列表。\
        没有具体对象（素材盘点、校验、仅对话）就留空 —— 不要写「大纲」「素材」这类词进去"
                            }
                        },
                        "required": ["kind"]
                    }
                },
                "boundary": {
                    "type": "string",
                    "description": "本轮边界补充（≤40 字，只能更谨慎）；没必要就留空"
                },
                "reason": {
                    "type": "string",
                    "description": format!(
                        "这么判的判据，一句话（≤{REASON_MAX_CHARS} 字）。例如「用户在讲想法，没让写」"
                    )
                }
            },
            "required": ["tasks"]
        }),
    )
}

/// 解析并校验 `submit_plan`：任一处不合法就整体作废（返回 `None`），后果是重试一次、再失败只回话，不猜。
fn parse_plan(arguments: &str) -> Option<RoutePlan> {
    let args = parse_tool_args(arguments);
    let raw = args.get("tasks")?.as_array()?;

    let mut items: Vec<QueueItem> = Vec::new();
    if raw.len() > MAX_QUEUE {
        // 被保险丝截到了就记一条：上限是防"模型抽风"，不是常规路径，出现即异常，必须留痕。
        tracing::warn!(
            "[router] submit_plan 给了 {} 项，超过上限 {}，丢掉后 {} 项",
            raw.len(),
            MAX_QUEUE,
            raw.len() - MAX_QUEUE
        );
    }
    for item in raw.iter().take(MAX_QUEUE) {
        let kind = item.get("kind").and_then(|v| v.as_str())?;
        let kind = TaskKind::parse(kind)?;
        let target = item
            .get("target")
            .and_then(|v| v.as_str())
            .unwrap_or("")
            .trim()
            .to_string();
        if items.iter().any(|i| i.kind == kind && i.target == target) {
            continue;
        }
        items.push(QueueItem { kind, target });
    }
    if items.is_empty() {
        return None;
    }

    let boundary = args
        .get("boundary")
        .and_then(|v| v.as_str())
        .and_then(role::sanitize_boundary);
    let reason = args
        .get("reason")
        .and_then(|v| v.as_str())
        .map(|s| s.trim())
        .filter(|s| !s.is_empty() && s.chars().count() <= REASON_MAX_CHARS)
        .map(|s| s.to_string());

    Some(RoutePlan {
        items,
        boundary,
        reason,
    })
}
