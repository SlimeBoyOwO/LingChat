//! Skill Agent 核心：多轮工具调用循环。

use std::collections::HashMap;
use std::path::Path;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};

use futures_util::StreamExt;
use sea_orm::DatabaseConnection;
use serde_json::Value;

use crate::ai_service::llm::{LlmChunk, LlmClient};
use crate::ai_service::skill_agent::command_executor::ApprovalMap;
use crate::ai_service::skill_agent::config::{SkillAgentConfig, resolve_skill_agent_provider};
use crate::ai_service::skill_agent::events::{SkillAgentEvent, Usage};
use crate::ai_service::skill_agent::{db, role, router, skills, stage, tools};
use crate::ai_service::types::{
    FunctionCall, LlmMessage, ToolCall, ToolDefinition, parse_tool_args,
};

/// 取消标志，跨 chat 运行共享。
pub type CancelFlag = Arc<AtomicBool>;

/// 截断自动续跑时推给模型的纠正提示。只进内存 `messages`，不落库。
#[cfg(desktop)]
const CORRECTIVE_HINT: &str = "（系统提示：你上一条回复因输出长度上限被截断且未调用任何工具。请直接调用 write_file / execute_command 完成当前任务，不要再叙述计划。）";
#[cfg(mobile)]
const CORRECTIVE_HINT: &str = "（系统提示：你上一条回复因输出长度上限被截断且未调用任何工具。请直接调用 write_file 完成当前任务，不要再叙述计划。移动端不提供 execute_command。）";

/// 截断自动续跑预算：最多补一次生成；再次截断仍无工具调用则按现状收尾。
const RECOVERY_BUDGET: usize = 1;

/// 单次对话运行上下文。
pub struct SkillAgentRunContext {
    pub conversation_id: i32,
    /// 流式事件推送通道。
    pub channel: tauri::ipc::Channel<SkillAgentEvent>,
    /// 命令审批映射（桌面端命令审批流使用，移动端不可达）。
    #[cfg_attr(not(desktop), allow(dead_code))]
    pub approvals: ApprovalMap,
    pub db: DatabaseConnection,
    pub llm: Arc<LlmClient>,
    /// 命令层拿得到、核心层拿不到的东西：按任务类型现解析 provider（思考模式按任务给，见 [`role::TaskKind::thinking`]）。
    pub app: tauri::AppHandle,
    pub config: SkillAgentConfig,
    pub sandbox_dir: std::path::PathBuf,
    pub skills_dir: std::path::PathBuf,
    /// `data/` 根目录，在这里解析一次：`stage` 等纯逻辑模块只收参数、不碰全局静态，否则在测试环境里一取就 panic。
    pub data_dir: std::path::PathBuf,
    /// 会话绑定的剧本 key（运行时解析为路径注入系统提示）。
    pub script_key: Option<String>,
    /// 运行开始时就存在的剧本包 key，用来认出"未绑定的会话正往别人的剧本包里写"；这一轮新建的包不在列表里。
    pub existing_script_keys: Vec<String>,
    /// 由剧本包状态推导出的当前阶段。
    pub stage_snapshot: stage::StageSnapshot,
    /// 这一轮新绑定的剧本 key（写 `story_config.yaml` 那一刻绑定，见 `tools.rs::bind_script_key_if_new`）。
    pub bound_script_key: std::sync::Mutex<Option<String>>,
    /// 这一轮算上下文预算用的模型窗口（token），由命令层解析 `/models` 得到；读不到用默认值（见 `stage::budget::DEFAULT_CONTEXT_WINDOW`）。
    pub context_window: usize,
}

impl SkillAgentRunContext {
    /// 记下这一轮新绑定的剧本 key。
    pub fn remember_bound_key(&self, key: &str) {
        if let Ok(mut bound) = self.bound_script_key.lock() {
            *bound = Some(key.to_string());
        }
    }

    fn bound_key(&self) -> Option<String> {
        self.bound_script_key.lock().ok().and_then(|k| k.clone())
    }
}

/// 累积中的工具调用（流式分片拼接）。
#[derive(Debug, Clone)]
struct AccumToolCall {
    index: usize,
    id: String,
    name: String,
    arguments: String,
}

/// 构建「当前剧本」段：给出 key/路径，并指示 agent 先看已有内容（实时读取，不注入静态快照）。
fn build_script_block(
    sandbox_dir: &Path,
    script_key: Option<&str>,
    known_keys: &[String],
) -> String {
    let Some(key) = script_key else {
        let mut out = String::from(include_str!("prompts/core_build_script_block.txt"));
        if !known_keys.is_empty() {
            out.push_str("\n\n（磁盘上现有的剧本包，仅供你告诉用户「有这些」：");
            out.push_str(&known_keys.join("、"));
            out.push('）');
        }
        return out;
    };
    match crate::utils::script_paths::resolve_script_dir(key) {
        Ok(dir) => {
            let rel = dir.strip_prefix(sandbox_dir).unwrap_or(&dir);
            format!(
                "\n\n【当前剧本上下文】\n剧本 key：{}\n剧本目录：{}（相对于文件沙箱根 {}）\n\n工作之前，请先用 list_files / read_file 查看剧本中已有的内容，再决定如何编写或修改。\n剧本中的素材引用（imagePath / musicPath / soundPath / ambientPath）只写素材文件名本身（如 夜晚.webp），不要带 backgrounds/、musics/ 等类型目录前缀；引擎会按事件类型自动到对应目录查找。",
                key,
                rel.display(),
                sandbox_dir.display()
            )
        },
        Err(_) => String::new(), // 剧本缺失/失效 → 降级，不阻断对话
    }
}

fn build_system_prompt(
    config: &SkillAgentConfig,
    allowed: &[&str],
    skills_block: &str,
    script_block: &str,
    task_block: &str,
    sandbox_dir: &Path,
    skills_dir: &Path,
) -> String {
    let can = |name: &str| allowed.contains(&name);
    let tool_names = tools::tool_names(allowed);
    let platform = if cfg!(mobile) { "移动端" } else { "桌面" };

    let mut abilities = format!(
        "你是运行在本机 LingChat {platform}应用里的 AI 剧本创作助手。你拥有以下能力：\
         \n- 调用工具完成真实操作：{tool_names}"
    );
    if can("read_skill") {
        abilities.push_str("\n- 通过 read_skill 加载技能指令后再执行任务");
    }
    abilities.push_str(&format!(
        "\n- 文件路径默认相对于文件沙箱根目录（{}）\
         \n- 技能目录：{}（技能文件以 SKILL.md 存放，需要时可用 list_files / read_file 直接查看）",
        sandbox_dir.display(),
        skills_dir.display()
    ));
    if can("execute_command") {
        abilities.push_str(if cfg!(mobile) {
            "\n- 当前移动端不提供 execute_command，不能运行 shell 命令"
        } else {
            "\n- execute_command 可能需要用户确认；命令由系统 shell 执行，带空格的参数请用引号包裹（引号会原样传递）"
        });
    }

    let mut rules: Vec<String> = Vec::new();
    if can("read_skill") {
        rules.push(
            "当任务匹配某个技能的描述时，先调用 read_skill 加载该技能，再按指令执行；\
             已读取过的技能不要重复读取"
                .to_string(),
        );
    }
    let mut file_tools = vec!["list_files", "read_file"];
    for extra in ["write_file", "delete_file"] {
        if can(extra) {
            file_tools.push(extra);
        }
    }
    rules.push(format!("需要操作文件时使用 {}", file_tools.join(" / ")));
    if can("write_file") {
        rules.push(
            "任务必须完成到产出物为止：读取技能、查询配色、运行搜索都只是中间步骤，\
             最终必须调用 write_file 实际写出用户要求的文件，才算完成任务"
                .to_string(),
        );
        rules.push(
            "未写出文件之前禁止总结收尾，禁止以「已获取到所需信息」「以上就是设计建议」\
             之类的说法结束回答；继续调用工具，直到文件真正创建成功"
                .to_string(),
        );
        rules.push(
            // 催它动手的规矩旁边必须写出出口："总得写点什么"会让它多写正文，或把已有的那几章重写一遍。
            "**但本项本来就该一个字都不写时，就不写**：用户这一句里没有这件事、\
             或者这一项要的东西盘上已经有了（用户没说要改），就如实说清、**不要动任何文件** —— \
             上面两条不适用；宁可空手收尾，也不许为了「有产出」写出用户没要的东西"
                .to_string(),
        );
        rules.push(
            "写文件时一次性用 write_file 写完整内容，不要提前分段；只有当一次写入因参数过长\
             而失败（报错会附带 [诊断] 提示）时，才改用 write_file（append=true）分段补齐"
                .to_string(),
        );
    }
    rules.push("文件范围受限时如实说明，不要编造文件内容".to_string());

    let numbered = rules
        .iter()
        .enumerate()
        .map(|(i, r)| format!("\n{}. {}", i + 1, r))
        .collect::<String>();
    let default = format!("{abilities}\n使用规则：{numbered}");

    let base = match &config.system_prompt {
        Some(custom) if !custom.trim().is_empty() => custom.clone(),
        _ => default,
    };
    format!("{}{}{}{}", base, script_block, skills_block, task_block)
}

/// 从上两句「纯对话」里取最近 N 轮，用于解「继续 / 都行 / 按你说的」；只取 assistant 的纯文本回复（带 tool_calls 的不算）。
fn recent_turns(history: &[LlmMessage], n: usize) -> Vec<(String, String)> {
    let mut out: Vec<(String, String)> = Vec::new();
    for (i, msg) in history.iter().enumerate().rev() {
        if msg.role != "assistant" || msg.tool_calls.is_some() || msg.content.trim().is_empty() {
            continue;
        }
        let Some(prev) = history[..i].iter().rev().find(|m| m.role == "user") else {
            continue;
        };
        out.push((prev.content.clone(), msg.content.clone()));
        if out.len() >= n {
            break;
        }
    }
    out.reverse();
    out
}

/// 规整从 DB 加载的历史，修复/丢弃不完整的工具轮次。
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

            let mut j = i + 1;
            let mut actual: Vec<String> = Vec::new();
            while j < history.len() && history[j].role == "tool" {
                if let Some(id) = history[j].tool_call_id.clone() {
                    actual.push(id);
                }
                j += 1;
            }

            let complete = !expected.is_empty() && expected.iter().all(|id| actual.contains(id));

            if complete {
                out.push(msg.clone());
                for t in history.iter().take(j).skip(i + 1) {
                    if let Some(id) = t.tool_call_id.as_ref() {
                        if expected.contains(id) {
                            out.push(t.clone());
                        }
                    }
                }
            } else {
                let mut fixed = msg.clone();
                fixed.tool_calls = None;
                out.push(fixed);
            }
            i = j;
        } else if msg.role == "tool" {
            i += 1;
        } else {
            out.push(msg.clone());
            i += 1;
        }
    }
    out
}

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
    let turn_start = std::time::SystemTime::now(); // 本轮起点：队列用它判断"登记之后有没有人动过那个产物"，必须在任何模型动作之前取。
    let _ = ctx.channel.send(SkillAgentEvent::Status {
        content: format!("思考中…（{}）", approval_mode),
    });

    let script_block = build_script_block(
        &ctx.sandbox_dir,
        ctx.script_key.as_deref(),
        &ctx.existing_script_keys,
    );

    if let (Some(key), Some(dir)) = (
        ctx.stage_snapshot.script_key.as_deref(),
        ctx.stage_snapshot.script_dir.as_deref(),
    ) {
        if let Err(e) = stage::ensure_package_skeleton(dir, key) {
            tracing::warn!("[skill_agent] 补剧本骨架失败: {e}");
        }
    }

    let user_msg = history
        .iter()
        .rev()
        .find(|m| m.role == "user")
        .map(|m| m.content.clone())
        .unwrap_or_default();
    let recent = recent_turns(&history, 2);
    let queue_text = ctx
        .stage_snapshot
        .script_dir
        .as_deref()
        .map(|d| std::fs::read_to_string(d.join(stage::QUEUE_REL_PATH)).unwrap_or_default())
        .unwrap_or_default();
    let plan = router::route(
        &ctx.llm,
        &router::RouteInput {
            snapshot: &ctx.stage_snapshot,
            queue: &queue_text,
            recent: &recent,
            user_msg: &user_msg,
            skills_dir: &ctx.skills_dir,
        },
    )
    .await;

    let creates_chapters = plan
        .items
        .iter()
        .any(|i| i.kind.produces_runnable_chapters());
    tracing::info!(
        "[router] 任务={} 队列={} 项",
        plan.items
            .iter()
            .map(|i| i.kind.key())
            .collect::<Vec<_>>()
            .join("/"),
        plan.items.len()
    );

    save_queue(&ctx, &plan.items, turn_start);

    let is_first_turn = !history.iter().any(|m| m.role == "assistant");
    let base_history =
        sanitize_history(stage::budget::compact_history(history, &ctx.stage_snapshot));
    let max_rounds: usize = if ctx.config.max_tool_rounds < 0 {
        usize::MAX
    } else {
        (ctx.config.max_tool_rounds as usize).max(1)
    };

    let (mut planned, skipped) = preplan(&ctx, &plan);
    let mut deferred: Vec<String> = Vec::new();

    let mut reply = String::new();
    let mut usage = TurnUsage::default();

    let show_headers = plan.items.len() > 1
        || planned
            .iter()
            .any(|(_, _, h)| matches!(h, role::Handoff::Prerequisite { .. }));

    let mut cursor = 0usize;
    while cursor < planned.len() {
        let (idx, task, handoff) = planned[cursor];
        cursor += 1;
        if cancelled.load(Ordering::SeqCst) {
            let _ = ctx.channel.send(SkillAgentEvent::Status {
                content: "已停止生成".into(),
            });
            break;
        }
        let snap = refreshed_snapshot(&ctx);
        let (task, handoff) = recheck_handoff(&plan, idx, &snap, task, handoff);
        let is_last =
            cursor == planned.len() && !matches!(handoff, role::Handoff::Prerequisite { .. });
        let allowed: &[&str] = item_tools(task, handoff);
        let item_llm = resolve_skill_agent_provider(&ctx.app, task.thinking());
        let llm: &LlmClient = item_llm.as_ref().unwrap_or(&ctx.llm);
        tracing::info!(
            "[router] 剧本={} 第{}/{}项 任务={} 目标={:?} 承接={:?} 产出能跑章节={} 工具={} 跳过={} 判据={}",
            ctx.script_key.as_deref().unwrap_or("-"),
            idx + 1,
            plan.items.len(),
            task.label(),
            plan.items.get(idx).map(|i| i.target.as_str()),
            handoff,
            task.produces_runnable_chapters(),
            allowed.join("/"),
            skipped.len(),
            plan.reason.as_deref().unwrap_or("（无）")
        );

        if show_headers {
            let head = item_header(task, handoff, idx, plan.items.len());
            let _ = ctx.channel.send(SkillAgentEvent::ItemStart {
                index: idx + 1,
                total: plan.items.len(),
                title: head.clone(),
            });
            reply.push_str(&head);
        }

        let target = plan.items.get(idx).map(|i| i.target.as_str()).unwrap_or("");

        let skills_block = if allowed.contains(&"read_skill") {
            skills::build_skills_xml(&skills::find_all_skills(&ctx.skills_dir))
        } else {
            String::new()
        };

        let task_block = stage::prompt::build_task_block(&stage::prompt::TaskBlockInput {
            task,
            handoff,
            plan: &plan,
            user_msg: &user_msg,
            skills_dir: &ctx.skills_dir,
            land: creates_chapters,
            item_index: idx,
            skipped: &skipped,
            is_last,
            allowed,
        });
        let system_prompt = build_system_prompt(
            &ctx.config,
            allowed,
            &skills_block,
            &script_block,
            &task_block,
            &ctx.sandbox_dir,
            &ctx.skills_dir,
        );

        let run_materials = stage::prompt::build_run_materials(&snap, &ctx.data_dir);
        let digest = queue_dir(&ctx)
            .map(|dir| stage::prompt::handoff_digest(&dir, &plan.items, idx, &deferred))
            .unwrap_or_default();
        let own = format!(
            // 本项任务书补在最后一条：模型最后读到的必须是"我这一项是什么"。
            include_str!("prompts/core_run_chat.txt"),
            stage::prompt::render_text_of(task, target),
            idx + 1,
            plan.items.len(),
            if target.trim().is_empty() {
                "（未指明）"
            } else {
                target.trim()
            }
        );
        let mut messages =
            item_messages(system_prompt, &base_history, &digest, run_materials, &own);

        let text = run_item_tools(
            &ctx,
            llm,
            &mut messages,
            allowed,
            &cancelled,
            max_rounds,
            &mut usage,
            is_first_turn && cursor == 1,
            tools::ItemScope {
                allow_runnable: task.produces_runnable_chapters(),
                kind: task,
            },
        )
        .await?;
        if let Some(q) = stage::prompt::deferred_question(&text) {
            deferred.push(q);
        }
        reply.push_str(&text);
        save_queue(&ctx, &plan.items, turn_start);

        if !matches!(handoff, role::Handoff::Explain(_)) {
            if let Some(dir) = queue_dir(&ctx) {
                if stage::queue::still_owed(&dir, task, target) {
                    messages.push(LlmMessage::user(format!(
                        "【代码核对】这一项在盘上还没有看到产物（{}）。**增 / 改类**要看到文件在、或被改过；\
                         **删除类**要看到它真的不见了。没落地就不算做完 —— 要么现在就真的做（该调的工具要调，\
                         比如删除要 `delete_file` 返回成功），要么在回执里说清为什么做不到；**不要写「已做完」**。",
                        stage::prompt::render_text_of(task, target)
                    )));
                    let second = run_item_tools(
                        &ctx,
                        llm,
                        &mut messages,
                        allowed,
                        &cancelled,
                        max_rounds,
                        &mut usage,
                        false,
                        tools::ItemScope {
                            allow_runnable: task.produces_runnable_chapters(),
                            kind: task,
                        },
                    )
                    .await?;
                    reply.push_str(&second);
                    save_queue(&ctx, &plan.items, turn_start);
                }
            }
        }

        if matches!(handoff, role::Handoff::Prerequisite { .. }) {
            append_original_if_ready(&ctx, &plan, idx, &mut planned);
        }
    }

    if let Some(dir) = queue_dir(&ctx) {
        let owed: Vec<String> = planned
            .iter()
            .filter(|(_, _, handoff)| !matches!(handoff, role::Handoff::Explain(_)))
            .filter(|(idx, task, _)| {
                let t = plan
                    .items
                    .get(*idx)
                    .map(|i| i.target.as_str())
                    .unwrap_or("");
                stage::queue::still_owed(&dir, *task, t)
            })
            .map(|(idx, task, _)| {
                let t = plan
                    .items
                    .get(*idx)
                    .map(|i| i.target.as_str())
                    .unwrap_or("");
                stage::prompt::render_text_of(*task, t)
            })
            .collect();
        if !owed.is_empty() {
            reply.push_str(&format!(
                "\n\n---\n\n【本轮盘点（代码按盘上事实核的）】盘上没有看到这些项的产物：{}。\
                 删除类可能本来就核不到（属正常）；**上面若有一项的回执写着「做完了」，以这份盘点为准** —— \
                 那一项其实没落地，说一声我可以重跑。\n",
                owed.join("、")
            ));
        }
    }

    let _ = ctx.channel.send(SkillAgentEvent::Done {
        final_text: reply,
        usage: usage.into_option(),
    });
    Ok(())
}

/// 开跑前的预扫：逐项用"预计产物"判一次承接，排出本轮真正要做的顺序与跳过的说明。
fn preplan(
    ctx: &SkillAgentRunContext,
    plan: &router::RoutePlan,
) -> (Vec<(usize, role::TaskKind, role::Handoff)>, Vec<String>) {
    let mut planned = Vec::new();
    let mut skipped = Vec::new();
    let mut pending = stage::evidence::Pending::default();
    for (idx, item) in plan.items.iter().enumerate() {
        let snap = refreshed_snapshot(ctx);
        let handoff = role::reconcile(
            item.kind,
            stage::evidence::facts_of(&snap, Some(item.target.as_str()), &pending),
        );
        match handoff {
            role::Handoff::Explain(why) => skipped.push(format!("{}：{why}", item.kind.label())),
            role::Handoff::Prerequisite { first, .. } => {
                pending.add(stage::evidence::Pending::of(first, item.target.as_str()));
                pending.add(stage::evidence::Pending::of(
                    item.kind,
                    item.target.as_str(),
                ));
                planned.push((idx, first, handoff));
            },
            _ => {
                pending.add(stage::evidence::Pending::of(
                    item.kind,
                    item.target.as_str(),
                ));
                planned.push((idx, item.kind, handoff));
            },
        }
    }
    if planned.is_empty() {
        let item = &plan.items[0];
        let snap = refreshed_snapshot(ctx);
        let handoff = role::reconcile(
            item.kind,
            stage::evidence::facts_of(
                &snap,
                Some(item.target.as_str()),
                &stage::evidence::Pending::default(),
            ),
        );
        planned.push((0, item.kind, handoff));
    }
    (planned, skipped)
}

/// 真正执行这一项之前，用真实事实再判一次（预扫那趟用的是"预计产物"）：原任务已经能做了就直接做，不再跑这个前置。
fn recheck_handoff(
    plan: &router::RoutePlan,
    idx: usize,
    snap: &stage::StageSnapshot,
    task: role::TaskKind,
    handoff: role::Handoff,
) -> (role::TaskKind, role::Handoff) {
    let role::Handoff::Prerequisite { .. } = handoff else {
        return (task, handoff);
    };
    let Some(original) = plan.items.get(idx) else {
        return (task, handoff);
    };
    let again = role::reconcile(
        original.kind,
        stage::evidence::facts_of(
            snap,
            Some(original.target.as_str()),
            &stage::evidence::Pending::default(),
        ),
    );
    match again {
        role::Handoff::Proceed | role::Handoff::ProceedNote(_) => (original.kind, again),
        _ => (task, handoff),
    }
}

/// 权限按任务给：任务的产物是什么，就只给写什么的那套工具。
fn item_tools(task: role::TaskKind, handoff: role::Handoff) -> &'static [&'static str] {
    if matches!(handoff, role::Handoff::Explain(_)) {
        role::CHAT_TOOLS
    } else if task.produces_runnable_chapters() {
        task.tools()
    } else if task.drafts_into_agent_dir() {
        role::DRAFT_TOOLS
    } else {
        role::DICTATE_TOOLS
    }
}

/// 项与项之间的分界标题：序号一律用队列里的位置（与注入的「队列第 i/N 项」同口径）。
fn item_header(task: role::TaskKind, handoff: role::Handoff, idx: usize, total: usize) -> String {
    format!(
        "\n\n---\n\n**（第 {}/{} 项 · {}）**\n\n",
        idx + 1,
        total,
        if matches!(handoff, role::Handoff::Prerequisite { .. }) {
            format!("{} · 前置", task.label()) // 这一项是"为了做原任务先补的前置"，标出来用户才看得懂为什么任务名变了
        } else {
            task.label().to_string()
        }
    )
}

/// 前置刚补完，原任务可能立刻就能做：同一轮里接着做掉（只追加一次，追加的这项不再触发本分支）。
fn append_original_if_ready(
    ctx: &SkillAgentRunContext,
    plan: &router::RoutePlan,
    idx: usize,
    planned: &mut Vec<(usize, role::TaskKind, role::Handoff)>,
) {
    let Some(original) = plan.items.get(idx) else {
        return;
    };
    let snap = refreshed_snapshot(ctx);
    let again = role::reconcile(
        original.kind,
        stage::evidence::facts_of(
            // 兜底要用真实磁盘事实：前面那一项可能没真写出来（预计 ≠ 事实）。
            &snap,
            Some(original.target.as_str()),
            &stage::evidence::Pending::default(),
        ),
    );
    if !matches!(
        again,
        role::Handoff::Prerequisite { .. } | role::Handoff::Explain(_)
    ) {
        planned.push((idx, original.kind, again));
    }
}

/// 每项执行前重新读盘（上一项刚落盘的章节这一项才看得见），key 现取而不是用回合起点那份快照。
fn refreshed_snapshot(ctx: &SkillAgentRunContext) -> stage::StageSnapshot {
    match ctx.script_key.clone().or_else(|| ctx.bound_key()) {
        Some(key) => stage::derive(Some(&key)),
        None => ctx.stage_snapshot.clone(),
    }
}

/// 队列写进哪个剧本包：每次写的时候现算，不取回合开始那份快照。
fn queue_dir(ctx: &SkillAgentRunContext) -> Option<std::path::PathBuf> {
    if let Some(dir) = ctx.stage_snapshot.script_dir.as_deref() {
        return Some(dir.to_path_buf());
    }
    let key = ctx.bound_key()?;
    stage::derive(Some(&key)).script_dir
}

/// 队列落盘。失败只记一行日志：账本是辅助记忆，不该把这一轮弄失败。
fn save_queue(
    ctx: &SkillAgentRunContext,
    items: &[router::QueueItem],
    registered_at: std::time::SystemTime,
) {
    let Some(dir) = queue_dir(ctx) else {
        return;
    };
    if let Err(e) = stage::queue::write_queue(&dir, items, registered_at) {
        tracing::warn!("[router] 队列落盘失败: {e}");
    }
}

/// 拼一项请求的消息表：system + 历史 + 交接材料（前面各项 + 状态）+ 本轮动态材料 + 本项任务书（放最后，走 user 角色）。
fn item_messages(
    system_prompt: String,
    base_history: &[LlmMessage],
    handoff_digest: &str,
    run_materials: String,
    own_task: &str,
) -> Vec<LlmMessage> {
    let mut messages = Vec::with_capacity(base_history.len() + 3);
    messages.push(LlmMessage::system(system_prompt));
    messages.extend(base_history.iter().cloned());
    if !handoff_digest.trim().is_empty() {
        messages.push(LlmMessage::user(handoff_digest.to_string()));
    }
    if !run_materials.is_empty() {
        messages.push(LlmMessage::user(run_materials));
    }
    messages.push(LlmMessage::user(own_task.to_string()));
    messages
}

/// 本轮累计用量。逐项加起来，收尾时一次报给前端。
#[derive(Default)]
struct TurnUsage {
    prompt: u64,
    completion: u64,
    cached: u64,
}

impl TurnUsage {
    fn add(&mut self, usage: Option<&Usage>) {
        if let Some(u) = usage {
            self.prompt += u.prompt_tokens;
            self.completion += u.completion_tokens;
            self.cached += u.cached_tokens;
        }
    }

    fn into_option(self) -> Option<Usage> {
        (self.prompt + self.completion > 0).then(|| Usage {
            prompt_tokens: self.prompt,
            completion_tokens: self.completion,
            total_tokens: self.prompt + self.completion,
            cached_tokens: self.cached,
        })
    }
}

/// 跑一项任务的工具轮：流式生成 → 执行工具 → 直到模型不再调工具；返回这一项的最终回复文本。
async fn run_item_tools(
    ctx: &SkillAgentRunContext,
    llm: &LlmClient,
    messages: &mut Vec<LlmMessage>,
    allowed: &[&str],
    cancelled: &CancelFlag,
    max_rounds: usize,
    usage: &mut TurnUsage,
    first_turn: bool,
    item: tools::ItemScope,
) -> Result<String, String> {
    let mut recovery_budget: usize = RECOVERY_BUDGET;
    let mut last_text = String::new();

    for round in 0..max_rounds {
        if cancelled.load(Ordering::SeqCst) {
            break;
        }
        match stage::budget::apply_context_budget(messages, ctx.context_window) {
            outcome if !stage::budget::budget_allows_send(&outcome) => {
                let note = outcome.note().unwrap_or_else(|| "上下文已满".into());
                tracing::warn!("[ctx] {}", note.replace('\n', " "));
                let _ = ctx.channel.send(SkillAgentEvent::Error {
                    message: note.clone(),
                });
                return Err(note);
            },
            outcome => {
                if let Some(note) = outcome.note() {
                    tracing::info!("[ctx] {}", note.replace('\n', " "));
                    let _ = ctx.channel.send(SkillAgentEvent::Status { content: note });
                }
            },
        }
        let defs = tools::tool_definitions(allowed);
        let (assistant_text, reasoning_text, tool_calls, finish_reason, round_usage) =
            match stream_completion(ctx, llm, messages, &defs, cancelled).await {
                Ok(r) => r,
                Err(e) => {
                    let message = if looks_like_context_overflow(&e) {
                        format!(
                            "这个对话的上下文满了（模型拒绝了这一次请求）。请**新开一个对话**继续 —— \
                             剧本文件都在磁盘上，新会话里照样能接着改。\n\n原始报错：{}",
                            e.lines().next().unwrap_or("")
                        )
                    } else {
                        e.clone()
                    };
                    let _ = ctx.channel.send(SkillAgentEvent::Error {
                        message: message.clone(),
                    });
                    return Err(message);
                },
            };
        usage.add(round_usage.as_ref());
        last_text = assistant_text.clone();

        if tool_calls.is_empty() {
            let truncated = finish_reason.as_deref() == Some("max_tokens");
            let was_cancelled = cancelled.load(Ordering::SeqCst);
            if truncated && !was_cancelled && recovery_budget > 0 {
                recovery_budget -= 1;
                let _ = ctx.channel.send(SkillAgentEvent::Status {
                    content: "检测到回复被截断，正在让模型继续…".into(),
                });
                messages.push(LlmMessage::user(CORRECTIVE_HINT));
                continue;
            }

            let final_msg = LlmMessage::assistant(&assistant_text);
            let _ = db::insert_message(
                &ctx.db,
                ctx.conversation_id,
                &final_msg,
                Some(&reasoning_text),
                round_usage.as_ref(),
            )
            .await;

            if first_turn {
                let title_db = ctx.db.clone();
                let title_llm = Arc::clone(&ctx.llm);
                let title_channel = ctx.channel.clone();
                let conv_id = ctx.conversation_id;
                let first_user = messages
                    .iter()
                    .rev()
                    .find(|m| m.role == "user")
                    .map(|m| m.content.clone())
                    .unwrap_or_default();
                let reply_summary: String = assistant_text.chars().take(200).collect();
                tauri::async_runtime::spawn(async move {
                    auto_title_conversation(
                        title_db,
                        title_llm,
                        title_channel,
                        conv_id,
                        first_user,
                        reply_summary,
                    )
                    .await;
                });
            }
            return Ok(assistant_text);
        }

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
            image_data_url: None,
        };
        messages.push(assistant_msg.clone());
        let _ = db::insert_message(
            &ctx.db,
            ctx.conversation_id,
            &assistant_msg,
            Some(&reasoning_text),
            round_usage.as_ref(),
        )
        .await;

        for tc in &tool_calls {
            run_one_tool_call(ctx, tc, allowed, item, messages).await;
        }

        if round == max_rounds - 1 {
            let _ = ctx.channel.send(SkillAgentEvent::Error {
                message: format!("已达到最大工具调用轮数（{}），已停止", max_rounds),
            });
            break;
        }
    }

    Ok(last_text)
}

/// 执行一次工具调用：发事件 → 跑工具 → 回填结果消息并持久化。
async fn run_one_tool_call(
    ctx: &SkillAgentRunContext,
    tc: &AccumToolCall,
    allowed: &[&str],
    item: tools::ItemScope,
    messages: &mut Vec<LlmMessage>,
) {
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

    let args_invalid =
        !tc.arguments.trim().is_empty() && serde_json::from_str::<Value>(&tc.arguments).is_err();
    let (ok, output) = if args_invalid {
        let snippet: String = tc.arguments.chars().take(400).collect();
        (
            false,
            format!(
                "这次调用**没有执行**：参数不是合法 JSON，多半是内容太长被截断。收到的参数开头：\n{snippet}\n\n\
                 改小一点再来 —— 只改文件里的一段就用 `edit_file`（给要替换掉的原文，参数很小）；\
                 要写一份长内容，按节拆成几次写。**不要**改用 append 硬补：那只会把内容写重复。"
            ),
        )
    } else {
        tools::execute_tool(ctx, allowed, &tc.name, &args, item).await
    };

    let _ = ctx.channel.send(SkillAgentEvent::ToolResult {
        call_id: call_id.clone(),
        tool: tc.name.clone(),
        ok,
        output: output.clone(),
        error: None,
    });

    let tool_msg = LlmMessage::tool_result(tc.id.clone(), &output);
    messages.push(tool_msg.clone());
    let _ = db::insert_message(&ctx.db, ctx.conversation_id, &tool_msg, None, None).await;
}

/// 首轮回复结束后后台生成会话标题（由 `run_chat` 收尾处 spawn，不阻塞回复流）。
async fn auto_title_conversation(
    db: DatabaseConnection,
    llm: Arc<LlmClient>,
    channel: tauri::ipc::Channel<SkillAgentEvent>,
    conversation_id: i32,
    first_user_msg: String,
    reply_summary: String,
) {
    let Ok(Some(conv)) = db::get_conversation(&db, conversation_id).await else {
        return;
    };
    let titled = conv
        .title
        .as_deref()
        .map(str::trim)
        .is_some_and(|t| !t.is_empty());
    if titled {
        return;
    }
    let title = generate_title(&llm, &first_user_msg, &reply_summary).await;
    if title.is_empty() {
        return;
    }
    if db::update_conversation_title(&db, conversation_id, title.clone())
        .await
        .is_err()
    {
        return;
    }
    let _ = channel.send(SkillAgentEvent::ConversationTitle { title });
}

/// 生成 4-10 字会话标题。优先 LLM（非流式单次调用），失败/未配置时
async fn generate_title(llm: &LlmClient, first_user_msg: &str, reply_summary: &str) -> String {
    let mut candidate = String::new();
    if llm.config().is_usable() {
        let msgs = vec![
            LlmMessage::system(
                "你是会话命名助手。根据用户的提问与助手的回复，用中文生成 4-10 个字的短标题，\
                 概括这次对话的主题。只输出标题本身，不要引号、标点或任何解释。",
            ),
            LlmMessage::user(format!(
                "用户提问：{}\n助手回复：{}",
                first_user_msg, reply_summary
            )),
        ];
        match llm.complete(&msgs).await {
            Ok(text) => {
                let t = text
                    .trim()
                    .trim_matches(|c| matches!(c, '"' | '「' | '」' | '《' | '》'));
                if !t.is_empty() {
                    candidate = t.chars().take(20).collect();
                }
            },
            Err(e) => tracing::warn!("[SkillAgent] 自动生成会话标题失败，回退截取: {e}"),
        }
    }
    if candidate.is_empty() {
        candidate = first_user_msg
            .trim()
            .chars()
            .take(15)
            .collect::<String>()
            .trim_end_matches(['，', '。', '！', '？', '；', '、', ',', '.', '!', '?', ':'])
            .to_string();
    }
    candidate
}

async fn stream_completion(
    ctx: &SkillAgentRunContext,
    llm: &LlmClient,
    messages: &[LlmMessage],
    defs: &[ToolDefinition],
    cancelled: &CancelFlag,
) -> Result<
    (
        String,
        String,
        Vec<AccumToolCall>,
        Option<String>,
        Option<Usage>,
    ),
    String,
> {
    let mut text_out = String::new();
    let mut reasoning_out = String::new();
    let mut usage: Option<Usage> = None;
    let mut finish_reason: Option<String> = None;
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
                    let _ = ctx
                        .channel
                        .send(SkillAgentEvent::MessageDelta { content: c });
                },
                LlmChunk::Reasoning(r) => {
                    reasoning_out.push_str(&r);
                    let _ = ctx.channel.send(SkillAgentEvent::Reasoning { content: r });
                },
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
                },
                LlmChunk::StreamEnd {
                    reason,
                    usage: end_usage,
                } => {
                    finish_reason = reason;
                    usage = end_usage.map(|u| Usage {
                        prompt_tokens: u.prompt_tokens,
                        completion_tokens: u.completion_tokens,
                        total_tokens: u.total_tokens,
                        cached_tokens: u.cached_tokens,
                    });
                },
                LlmChunk::ToolCallProgress { .. } => {},
            }
        }
    } else {
        let resp = llm
            .complete_with_tools(messages, defs, Some("auto"))
            .await
            .map_err(|e| e.to_string())?;
        usage = resp.usage.as_ref().map(|u| Usage {
            prompt_tokens: u.prompt_tokens,
            completion_tokens: u.completion_tokens,
            total_tokens: u.total_tokens,
            cached_tokens: u.cached_tokens,
        });
        if let Some(c) = resp.content {
            if !c.is_empty() {
                text_out.push_str(&c);
                let _ = ctx
                    .channel
                    .send(SkillAgentEvent::MessageDelta { content: c });
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
    Ok((text_out, reasoning_out, tool_calls, finish_reason, usage))
}

/// provider 的报错是不是"超出上下文"：各家措辞不同，只做宽松匹配。
fn looks_like_context_overflow(err: &str) -> bool {
    const PATTERNS: [&str; 5] = [
        "maximum context length",
        "context_length_exceeded",
        "context window",
        "too many tokens",
        "reduce the length",
    ];
    let lower = err.to_lowercase();
    PATTERNS.iter().any(|p| lower.contains(p))
}
