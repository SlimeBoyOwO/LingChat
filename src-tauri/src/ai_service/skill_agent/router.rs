//! 流程 Agent（路由）：判断这一轮到底要干什么。
//!
//! 候选枚举进提示词、决策是一次工具调用（不解析自由文本）、结果校验后才采用；路由失败**绝不中止这一轮**，
//! 一律回落到按阶段推的老行为。稳定部分（流程总纲）放 system、易变部分放最后，以保前缀缓存。

use std::path::Path;

use crate::ai_service::llm::LlmClient;
use crate::ai_service::types::{LlmMessage, ToolDefinition, parse_tool_args};

use super::role::{self, TaskKind};
use super::stage::{self, Stage, StageSnapshot};

/// 队列上限。用户一句话很少真有五件事，超了多半是模型在编。
const MAX_QUEUE: usize = 8;

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
    /// 这么判的判据（一句话）。进事件流水，供事后回看路由判断。
    pub reason: Option<String>,
    /// 这一轮落不落盘。**默认 false** —— 判错代价不对称：该落没落只是多问一句，不该落却落了会凭空生成文件。
    pub land: bool,
    pub fallback: bool,
}

impl RoutePlan {
    #[cfg_attr(not(test), allow(dead_code))]
    pub fn current(&self) -> TaskKind {
        self.items.first().map(|i| i.kind).unwrap_or(TaskKind::Chat)
    }

    #[cfg_attr(not(test), allow(dead_code))]
    pub fn target(&self) -> Option<&str> {
        self.items
            .first()
            .map(|i| i.target.trim())
            .filter(|t| !t.is_empty())
    }

    fn single(kind: TaskKind, target: &str, fallback: bool) -> Self {
        Self {
            items: vec![QueueItem {
                kind,
                target: target.to_string(),
            }],
            boundary: None,
            reason: None,
            // 回落路径等于改造前的行为：那时是照写不误的
            land: fallback,
            fallback,
        }
    }

    pub fn chat() -> Self {
        Self::single(TaskKind::Chat, "", true)
    }

    pub fn fallback_for(stage: Stage) -> Self {
        let kind = match stage {
            // 未绑定 / 大纲阶段做的都是"产出设计稿"（含建包、问类型），归编剧
            Stage::Routing | Stage::Setup | Stage::Modify => match stage {
                Stage::Modify => TaskKind::ReviseChapter,
                _ => TaskKind::Outline,
            },
            Stage::Forge => TaskKind::WriteChapter,
            Stage::Polish => TaskKind::Polish,
        };
        Self::single(kind, "", true)
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

/// 判断这一轮要干什么。不会失败：任何一步不成立都回落到 [`RoutePlan::fallback_for`]。
pub async fn route(llm: &LlmClient, input: &RouteInput<'_>) -> RoutePlan {
    if input.user_msg.trim().is_empty() {
        return RoutePlan::chat();
    }
    let Some(hub) = stage::hub_doc(input.skills_dir) else {
        tracing::warn!("[router] 流程总纲缺失，回落到按阶段推导");
        return RoutePlan::fallback_for(input.snapshot.stage);
    };
    if !llm.config().is_usable() {
        return RoutePlan::fallback_for(input.snapshot.stage);
    }

    let messages = vec![
        LlmMessage::system(build_router_system(&hub)),
        LlmMessage::user(build_router_user(input)),
    ];
    let tools = vec![submit_plan_tool()];

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
                Some(plan) => plan,
                None => {
                    tracing::warn!("[router] 没拿到可用的 submit_plan，回落到按阶段推导");
                    RoutePlan::fallback_for(input.snapshot.stage)
                },
            }
        },
        Err(e) => {
            tracing::warn!("[router] 调用失败，回落到按阶段推导: {e}");
            RoutePlan::fallback_for(input.snapshot.stage)
        },
    }
}

// ---------- 提示词 ----------

/// 稳定部分：角色 + 流程总纲 + 判断规则。放 system 以保前缀缓存。
fn build_router_system(hub: &str) -> String {
    let kinds: Vec<String> = TaskKind::ALL
        .iter()
        .map(|k| format!("`{}` = {}", k.key(), k.label()))
        .collect();
    format!(
        "你是剧本创作的流程调度。你只做一件事：读懂用户这一句，判断**这一轮该做什么**，\
         并列出这段对话还要做的其余事情。\n\
         \n\
         可用的任务类型：\n{}\n\
         \n\
         {}\n\
         \n\
         判断规则：\n\
         1. `tasks` 是**有序队列**，第一项就是本轮要做的；用户按顺序说了几件事，就按他的顺序排。\
         已经做完的事不要再列。\n\
         2. `target` 写清作用对象：**一章就写一个章节 id**（如 `03`）；\
         多章要拆成多项，**不要写成 `01-04` 这种区间**。没有具体对象就留空。\n\
         3. **用户只是在讲自己的想法、还没让你写**（例如「我想让主角是个剑客」）→ 用 `summarize`；\
         需要先问清情况（这一章讲什么、一共多少章、素材怎么来）→ 用 `collect`。\
         这两类都只产出话术，不落笔。\n\
         4. **`land` 这个字段单独判**：用户这一句说了落盘（落盘 / 生成章节 / 写进 Chapters / 存下来），\
         或者他是在回答你上一轮的「要不要落盘」并且答的是要 —— 才 `land: true`。\
         其余一律 `land: false`。**「写」「写出来」「写大纲」「写第三章」都不等于落盘。**\n\
         5. **用户这一句跟这个剧本无关**（闲聊、问软件怎么用、道谢）→ 用 `chat`。\
         跟剧本有关还是无关，看它谈不谈这个剧本的人物、情节、章节、素材。\n\
         6. **认不出意图就用 `chat`** —— 这一条没有例外，不要猜。\n\
         7. `boundary` 是本轮的边界补充（≤40 字），只能比系统底线**更谨慎**，\
         不许写「顺便」「也把…补上」「直接写完全部」这类放宽范围的措辞。没必要的就留空。\n\
         \n\
         最后调用 `submit_plan` 工具提交。\n\
         \n\
         【流程总纲】\n{}",
        kinds.join("\n"),
        crate::ai_service::skill_agent::role::ACTION_VOCAB,
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
        out.push_str("\n【当前队列（上一轮留下的）】\n");
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

// ---------- 工具 ----------

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
                    "description": "有序任务队列，第一项是本轮要做的。至少一项。",
                    "items": {
                        "type": "object",
                        "properties": {
                            "kind": {"type": "string", "enum": kinds},
                            "target": {
                                "type": "string",
                                "description": "作用对象：章节 id / 大纲 / 素材；没有就留空"
                            }
                        },
                        "required": ["kind"]
                    }
                },
                "boundary": {
                    "type": "string",
                    "description": "本轮边界补充（≤40 字，只能更谨慎）；没必要就留空"
                },
                "land": {
                    "type": "boolean",
                    "description": "这一轮落不落盘：用户明说了落盘（落盘/生成章节/写进 Chapters/存下来），或他在回答你上一轮的「要不要落盘」并答了要 → true。只说「写」「写大纲」「写第三章」→ false（那是纯写，不写文件）。拿不准一律 false。"
                },
                "reason": {
                    "type": "string",
                    "description": "这么判的判据，一句话（≤40 字）。例如「用户在讲想法，没让写」"
                }
            },
            "required": ["tasks"]
        }),
    )
}

// ---------- 解析与校验 ----------

/// 解析并校验 `submit_plan` 的参数。任何一处不合法就整体作废（返回 `None`）—— **不猜**：
/// 认不出的任务类型、空队列、全是空白一律作废，作废的后果是回落，不是乱做。
fn parse_plan(arguments: &str) -> Option<RoutePlan> {
    let args = parse_tool_args(arguments);
    let raw = args.get("tasks")?.as_array()?;

    let mut items: Vec<QueueItem> = Vec::new();
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
        .filter(|s| !s.is_empty() && s.chars().count() <= 60)
        .map(|s| s.to_string());

    let land = args.get("land").and_then(|v| v.as_bool()).unwrap_or(false);

    Some(RoutePlan {
        items,
        boundary,
        reason,
        land,
        fallback: false,
    })
}
