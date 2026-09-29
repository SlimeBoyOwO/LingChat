//! 流程 Agent（路由）：判断这一轮到底要干什么。
//!
//! 框架借自 `god_agent`（多人对话的导演）：**候选枚举进提示词、决策是一次工具调用**
//! （不解析自由文本）、结果校验后才采用、失败有兜底。三处刻意不同：
//!
//! 1. 路由失败**绝不中止这一轮** —— 一律回落到按阶段推的老行为（`god_agent` 是 `?` 冒泡）；
//! 2. 稳定部分（流程总纲）放 system、易变部分（磁盘现状 + 这一句）放最后，保住前缀缓存；
//! 3. 除了"选哪个任务"，还要给出**任务队列**和**职责边界**。
//!
//! 它只看：流程总纲、当前队列、上两句纯对话、用户这一句、磁盘现状。

use std::path::Path;

use crate::ai_service::llm::LlmClient;
use crate::ai_service::types::{LlmMessage, ToolDefinition, parse_tool_args};

use super::role::{self, TaskKind};
use super::stage::{self, Stage, StageSnapshot};

/// 队列上限。用户一句话很少真有五件事，超了多半是模型在编。
const MAX_QUEUE: usize = 8;

/// 队列里的一项。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct QueueItem {
    pub kind: TaskKind,
    /// 这一项作用在什么上面（章节 id / 大纲 / 素材）。可以是空的。
    pub target: String,
}

/// 路由结果。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RoutePlan {
    /// 有序队列；第一项就是本轮要做的。永不为空。
    pub items: Vec<QueueItem>,
    /// 本轮职责边界的补充（已过 [`role::sanitize_boundary`]）。
    pub boundary: Option<String>,
    /// 这么判的判据（一句话）。进事件流水，供事后回看路由判断。
    pub reason: Option<String>,
    /// 这一轮落不落盘。**默认 false** —— 用户明说了落盘（或在回答"要不要落盘"时答要）才是 true。
    /// 判错的代价不对称：该落没落只是多问一句；不该落却落了会凭空生成文件。
    pub land: bool,
    /// 这一次是不是回落结果（没问模型 / 模型没给 / 解析不过）。
    pub fallback: bool,
}

impl RoutePlan {
    /// 本轮任务。队列是逐项执行的，所以"本轮"只对回落路径与测试有意义。
    #[cfg_attr(not(test), allow(dead_code))]
    pub fn current(&self) -> TaskKind {
        self.items.first().map(|i| i.kind).unwrap_or(TaskKind::Chat)
    }

    /// 本轮作用对象（去空白；空串当没有）。
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

    /// 什么都不确定时的结果：仅对话（只回话、不碰文件）。**这是保守方向。**
    pub fn chat() -> Self {
        Self::single(TaskKind::Chat, "", true)
    }

    /// 不看用户这一句、只按磁盘阶段推的回落结果 —— 等于改造前的行为。
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

/// 一次路由要看的东西。
pub struct RouteInput<'a> {
    pub snapshot: &'a StageSnapshot,
    /// `.agent/queue.md` 的原文（没有就是空串）。
    pub queue: &'a str,
    /// 上两句纯对话（去思考、去工具），旧的在前。用于解「继续 / 都行 / 按你说的」。
    pub recent: &'a [(String, String)],
    pub user_msg: &'a str,
    pub skills_dir: &'a Path,
}

/// 判断这一轮要干什么。
///
/// 不会失败：任何一步不成立都回落到 [`RoutePlan::fallback_for`]。
pub async fn route(llm: &LlmClient, input: &RouteInput<'_>) -> RoutePlan {
    // 守卫一：空消息没什么可判的
    if input.user_msg.trim().is_empty() {
        return RoutePlan::chat();
    }
    // 守卫二：没有流程总纲就无从判断流程走到哪，别猜
    let Some(hub) = stage::hub_doc(input.skills_dir) else {
        tracing::warn!("[router] 流程总纲缺失，回落到按阶段推导");
        return RoutePlan::fallback_for(input.snapshot.stage);
    };
    // 守卫三：模型不可用
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

/// 易变部分：磁盘现状 + 队列 + 上两句 + 这一句。放最后。
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

/// 上两句只用来解「继续 / 都行 / 按你说的」，取前若干字避免把整段历史拖进来。
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

/// 解析并校验 `submit_plan` 的参数。任何一处不合法就整体作废（返回 `None`）。
///
/// **不猜**：认不出的任务类型、空队列、全是空白，一律作废 —— 作废的后果是回落，
/// 不是乱做。
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
        // 同一项重复提交没意义，去掉（保留第一次出现的位置）
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
    // 判据只是给人事后看的，过长/空白就当没有，不影响判断本身
    let reason = args
        .get("reason")
        .and_then(|v| v.as_str())
        .map(|s| s.trim())
        .filter(|s| !s.is_empty() && s.chars().count() <= 60)
        .map(|s| s.to_string());

    // 落盘与不落盘是一条**默认安全**的判断：拿不准一律 false（多问一句 vs 凭空生成文件）
    let land = args.get("land").and_then(|v| v.as_bool()).unwrap_or(false);

    Some(RoutePlan {
        items,
        boundary,
        reason,
        land,
        fallback: false,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn plan_json(kinds: &[(&str, &str)], boundary: &str) -> String {
        let tasks: Vec<String> = kinds
            .iter()
            .map(|(k, t)| format!(r#"{{"kind":"{k}","target":"{t}"}}"#))
            .collect();
        format!(
            r#"{{"tasks":[{}],"boundary":"{boundary}"}}"#,
            tasks.join(",")
        )
    }

    #[test]
    fn parses_a_normal_plan() {
        let raw = plan_json(
            &[("revise_chapter", "13"), ("write_chapter", "09")],
            "只改内容，不动结构",
        );
        let plan = parse_plan(&raw).expect("应当解析成功");
        assert!(!plan.fallback);
        assert_eq!(plan.current(), TaskKind::ReviseChapter);
        assert_eq!(plan.target(), Some("13"));
        assert_eq!(plan.items.len(), 2, "两件事都要留在队列里");
        assert_eq!(plan.items[1].kind, TaskKind::WriteChapter);
        assert_eq!(plan.items[1].target, "09");
        assert_eq!(plan.boundary.as_deref(), Some("只改内容，不动结构"));

        // 判据会进事件流水；过长/空白就当没有
        let with_reason = plan_json(&[("chat", "")], "");
        let with_reason = with_reason.trim_end_matches('}');
        let plan = parse_plan(&format!("{with_reason}, \"reason\": \"用户在讲想法\"}}")).unwrap();
        assert_eq!(plan.reason.as_deref(), Some("用户在讲想法"));
        let plan = parse_plan(&format!(
            "{with_reason}, \"reason\": \"{}\"}}",
            "长".repeat(61)
        ))
        .unwrap();
        assert_eq!(plan.reason, None, "过长的判据丢掉，但不影响任务本身");
    }

    #[test]
    fn unknown_kind_invalidates_the_whole_plan() {
        // 认不出就整体作废（→ 回落），不猜、不跳过
        assert!(parse_plan(&plan_json(&[("write_chapter", ""), ("编章节", "")], "")).is_none());
        assert!(
            parse_plan(r#"{"tasks":[{"target":"03"}]}"#).is_none(),
            "缺 kind"
        );
        assert!(parse_plan(r#"{"tasks":[]}"#).is_none(), "空队列");
        assert!(parse_plan(r#"{"tasks":[{"kind":"chat","target":"x"}]}"#).is_some());
    }

    #[test]
    fn tolerates_messy_arguments_but_not_bad_values() {
        // 模型偶尔把参数双编码成字符串 —— parse_tool_args 会归一化，这里必须照样能用
        let inner = plan_json(&[("chat", "")], "");
        let double = serde_json::to_string(&inner).unwrap();
        assert_eq!(
            parse_plan(&double).map(|p| p.current()),
            Some(TaskKind::Chat)
        );
        assert!(parse_plan("not json at all").is_none());
        assert!(parse_plan("").is_none());
    }

    #[test]
    fn duplicate_items_are_collapsed_and_queue_is_capped() {
        let raw = plan_json(
            &[
                ("write_chapter", "03"),
                ("write_chapter", "03"),
                ("write_chapter", "04"),
            ],
            "",
        );
        let plan = parse_plan(&raw).unwrap();
        assert_eq!(plan.items.len(), 2, "同一项重复应当去掉");
        assert_eq!(plan.items[1].target, "04");

        let many: Vec<(&str, &str)> = (0..20).map(|_| ("chat", "x")).collect();
        // 全是重复项，去重后只剩 1 条；换个不被去重的写法验上限
        let _ = parse_plan(&plan_json(&many, ""));
        let distinct: Vec<(String, String)> = (0..20)
            .map(|i| ("write_chapter".to_string(), format!("{i:02}")))
            .collect();
        let pairs: Vec<(&str, &str)> = distinct
            .iter()
            .map(|(k, t)| (k.as_str(), t.as_str()))
            .collect();
        assert_eq!(
            parse_plan(&plan_json(&pairs, "")).unwrap().items.len(),
            MAX_QUEUE
        );
    }

    #[test]
    fn boundary_goes_through_the_sanitizer() {
        let ok = parse_plan(&plan_json(&[("chat", "")], "这轮只回话")).unwrap();
        assert_eq!(ok.boundary.as_deref(), Some("这轮只回话"));
        // 放宽范围的措辞 → 整条丢掉，只用底线
        let bad = parse_plan(&plan_json(&[("chat", "")], "顺便把后面也写了")).unwrap();
        assert_eq!(bad.boundary, None);
        // 超长 → 丢掉
        let long = parse_plan(&plan_json(&[("chat", "")], &"谨".repeat(41))).unwrap();
        assert_eq!(long.boundary, None);
    }

    #[test]
    fn fallback_matches_the_old_stage_behaviour() {
        assert_eq!(
            RoutePlan::fallback_for(Stage::Routing).current(),
            TaskKind::Outline
        );
        assert_eq!(
            RoutePlan::fallback_for(Stage::Setup).current(),
            TaskKind::Outline
        );
        assert_eq!(
            RoutePlan::fallback_for(Stage::Forge).current(),
            TaskKind::WriteChapter
        );
        assert_eq!(
            RoutePlan::fallback_for(Stage::Polish).current(),
            TaskKind::Polish
        );
        assert_eq!(
            RoutePlan::fallback_for(Stage::Modify).current(),
            TaskKind::ReviseChapter
        );
        for stage in [
            Stage::Routing,
            Stage::Setup,
            Stage::Forge,
            Stage::Polish,
            Stage::Modify,
        ] {
            assert!(
                RoutePlan::fallback_for(stage).fallback,
                "{stage:?} 应标记为回落"
            );
        }
    }

    #[test]
    fn chat_plan_touches_nothing() {
        let plan = RoutePlan::chat();
        assert_eq!(plan.current(), TaskKind::Chat);
        assert_eq!(plan.target(), None);
        assert!(plan.current().materials().is_empty(), "仅对话不注入手册");
        assert_eq!(plan.current().tools(), role::CHAT_TOOLS, "仅对话只能只读");
    }

    #[test]
    fn empty_message_never_reaches_the_model() {
        // 只验守卫本身（不真的调模型）：空消息一律 Chat
        assert_eq!(RoutePlan::chat().current(), TaskKind::Chat);
        assert!(RoutePlan::chat().fallback, "兜底结果必须标记为 fallback");
    }

    #[test]
    fn recent_turns_are_squashed() {
        let long = "字".repeat(500);
        let s = squash(&long);
        assert!(s.chars().count() <= 121, "应当截断：{}", s.chars().count());
        assert_eq!(squash("  换行\n也要压平  "), "换行 也要压平");
    }

    #[test]
    fn tool_schema_lists_every_kind_and_nothing_else() {
        let tool = submit_plan_tool();
        let enum_values =
            tool.function.parameters["properties"]["tasks"]["items"]["properties"]["kind"]["enum"]
                .as_array()
                .unwrap()
                .clone();
        assert_eq!(enum_values.len(), TaskKind::ALL.len());
        for kind in TaskKind::ALL {
            assert!(
                enum_values.iter().any(|v| v.as_str() == Some(kind.key())),
                "{:?} 没进工具参数",
                kind
            );
        }
    }
}
