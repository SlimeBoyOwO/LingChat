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
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};

use futures_util::StreamExt;
use sea_orm::DatabaseConnection;
use serde_json::Value;

use crate::ai_service::llm::{LlmChunk, LlmClient};
use crate::ai_service::skill_agent::command_executor::ApprovalMap;
use crate::ai_service::skill_agent::config::SkillAgentConfig;
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
    pub config: SkillAgentConfig,
    pub sandbox_dir: std::path::PathBuf,
    pub skills_dir: std::path::PathBuf,
    /// `data/` 根目录。在这里解析一次：`stage` 等纯逻辑模块只收参数，不碰全局静态，
    /// 否则它们在没有 App 的测试环境里一取就 panic。
    pub data_dir: std::path::PathBuf,
    /// 会话绑定的剧本 key（运行时解析为路径注入系统提示）。
    pub script_key: Option<String>,
    /// 运行开始时就存在的剧本包 key。用来认出"未绑定的会话正往别人的剧本包里写" ——
    /// 这一轮新写出来的包不在这个列表里，所以**新建剧本的流程不受影响**。
    pub existing_script_keys: Vec<String>,
    /// 由剧本包状态推导出的当前阶段。
    pub stage_snapshot: stage::StageSnapshot,
    /// 这一轮算上下文预算用的模型窗口（token）。命令层解析 `/models` 得到；
    /// 读不到就是 DeepSeek 的默认值（见 [`stage::DEFAULT_CONTEXT_WINDOW`]）。
    pub context_window: usize,
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
fn build_script_block(
    sandbox_dir: &Path,
    script_key: Option<&str>,
    known_keys: &[String],
) -> String {
    let Some(key) = script_key else {
        // 未绑定：编辑器里没打开剧本时新建的会话就是这样。
        // 真机踩过：用户在这个状态下说「把第三章写出来」，模型自己扫盘挑了一个
        // 「正好缺第 3 章」的包当成"用户当前的剧本" —— 所以这里把话说死。
        let mut out = String::from(
            "\n\n【当前剧本上下文】\n本会话**还没有绑定剧本**（在编辑器里没打开剧本时新建的会话就是这样）。\
             \n- **不要自己去磁盘上挑一个剧本当作用户的「当前剧本」**，哪怕它看起来正好缺某一章。\
             \n- 用户要弄已有剧本：让他先在编辑器里打开那个剧本、再新建一个会话（会自动绑定）；\
             或者你先把候选列给他，让他挑。\n- 用户要新建剧本：问清剧本名与类型，\
             然后写 `<standalone 或 character/<角色>>/<剧本名>/story_config.yaml`（写完会自动绑定）。\
             \n- 在没绑定之前，除非用户明确让你新建剧本，**不要写任何剧本文件**。",
        );
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

    // 能力说明按**本轮真正给出去的工具**写：广告里列出用不到的工具，
    // 等于告诉模型"还有别的路"，也会把它往不允许的动作上引。
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
        // 「催它动手」只对会落盘的任务说：仅对话 / 只提问的轮次说这个，
        // 会把它逼去写这一轮不该写的文件
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
    // 任务块拼在最后：前三段一次会话内稳定，拼在尾部可保缓存前缀。
    format!("{}{}{}{}", base, script_block, skills_block, task_block)
}

/// 从上两句「纯对话」里取最近 N 轮，用于解「继续 / 都行 / 按你说的」。
///
/// 只取 assistant 的**纯文本**回复（带 tool_calls 的不算）与它前面那条 user 消息。
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

    let script_block = build_script_block(
        &ctx.sandbox_dir,
        ctx.script_key.as_deref(),
        &ctx.existing_script_keys,
    );

    // 已绑定的会话顺手把包骨架补上（修"半成品包"：只有 .agent/、编辑器打不开）。
    // 新建剧本的绑定发生在写文件那一刻，那时也已经补过了；这里是既有包的兜底。
    if let (Some(key), Some(dir)) = (
        ctx.stage_snapshot.script_key.as_deref(),
        ctx.stage_snapshot.script_dir.as_deref(),
    ) {
        if let Err(e) = stage::ensure_package_skeleton(dir, key) {
            tracing::warn!("[skill_agent] 补剧本骨架失败: {e}");
        }
    }

    // ---------- 流程 Agent：这一轮到底要干什么 ----------
    //
    // 必须在消费 history 之前跑：它要「上两句纯对话」来解「继续 / 都行」。
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

    // ---------- 落盘口径：用户说了算，代码兜住 ----------
    //
    // 「写」= 只把内容写进回复；「落盘」= 变成文件。判错的代价不对称：
    // 该落没落只是多问一句，不该落却落了会凭空生成一堆文件。
    let land = wants_landing(&user_msg, &recent, plan.land);
    tracing::info!(
        "[router] 落盘={} 队列={} 项 回落={}",
        land,
        plan.items.len(),
        plan.fallback
    );

    // 队列落盘（只有绑定了剧本包才写）；已完成项按磁盘事实打勾
    if let Some(dir) = ctx.stage_snapshot.script_dir.as_deref() {
        if let Err(e) = stage::write_queue(dir, &plan.items, &ctx.stage_snapshot.written) {
            tracing::warn!("[router] 队列落盘失败: {e}");
        }
    }

    // 首轮判定要在动手之前记：本会话第一条 assistant 回复（含工具轮）结束时用于自动生成标题。
    let is_first_turn = !history.iter().any(|m| m.role == "assistant");
    // 历史规整一次、之后每项复用：DB 里可能残留上一轮中断产生的「无 tool 回应的
    // assistant(tool_calls)」，不处理会触发 OpenAI 400（insufficient tool messages）。
    let base_history = sanitize_history(stage::compact_history(history, &ctx.stage_snapshot));
    // -1 表示无上限（保留全部上下文与工具轮次）；否则为有限轮数，至少 1 轮。
    let max_rounds: usize = if ctx.config.max_tool_rounds < 0 {
        usize::MAX
    } else {
        (ctx.config.max_tool_rounds as usize).max(1)
    };

    // ---------- 逐项执行队列 ----------
    //
    // 一轮 = 把队列**逐项做完**：每项按它自己的角色注入手册与工具，做完再换下一个角色。
    // 早先是一轮只做第一项、其余留给下一轮 —— 用户看到的就是"排完队列就停了，只跟我解释"。
    // 某一项真的做不了（比如改一个还没落盘的章）只跳过它并说明，**不牵连别的项**：
    // 早先"一项做不了就把整轮工具收成只读"，会让队列里所有项一起废掉（真机连着三轮
    // 一个字都没改成，用户说"我让他开始改，操作也被拦下来了"）。
    let mut planned: Vec<(usize, role::TaskKind, role::Handoff)> = Vec::new();
    let mut skipped: Vec<String> = Vec::new();
    for (idx, item) in plan.items.iter().enumerate() {
        if plan.fallback {
            planned.push((idx, item.kind, role::Handoff::Proceed));
            continue;
        }
        let snap = refreshed_snapshot(&ctx);
        let handoff = role::reconcile(
            item.kind,
            stage::facts_of(&snap, Some(item.target.as_str())),
        );
        match handoff {
            role::Handoff::Explain(why) => skipped.push(format!("{}：{why}", item.kind.label())),
            role::Handoff::Prerequisite { first, .. } => planned.push((idx, first, handoff)),
            _ => planned.push((idx, item.kind, handoff)),
        }
    }
    // 一项都做不了时仍要有人把原因说给用户听：拿第一项走"只解释"那一轮
    if planned.is_empty() {
        let item = &plan.items[0];
        let snap = refreshed_snapshot(&ctx);
        let handoff = role::reconcile(
            item.kind,
            stage::facts_of(&snap, Some(item.target.as_str())),
        );
        planned.push((0, item.kind, handoff));
    }

    // 项与项之间的交接材料（只进内存，不落库）：前面各项做了什么，后面各项要看得见
    let mut handoffs: Vec<LlmMessage> = Vec::new();
    let mut reply = String::new();
    let mut usage = TurnUsage::default();

    for (n, (idx, task, handoff)) in planned.iter().enumerate() {
        if cancelled.load(Ordering::SeqCst) {
            let _ = ctx.channel.send(SkillAgentEvent::Status {
                content: "已停止生成".into(),
            });
            break;
        }
        // 每项重新读盘：上一项刚落盘的章节，这一项要看得见
        let snap = refreshed_snapshot(&ctx);
        let allowed: &[&str] = if plan.fallback {
            // 回落到按阶段推的老行为：工具照旧全给，免得连现状都跑不动
            role::ALL_TOOLS
        } else if matches!(handoff, role::Handoff::Explain(_)) {
            // 「说清缺什么，不动文件」—— 靠工具收窄兜住，不只是靠提示词
            role::CHAT_TOOLS
        } else if !land {
            // 用户没让落盘：这一轮只口述，一个字都不许写文件
            role::DICTATE_TOOLS
        } else {
            task.tools()
        };
        tracing::info!(
            "[router] 第{}/{}项 任务={} 目标={:?} 承接={:?} 落盘={} 工具={} 跳过={} 回落={} 判据={}",
            n + 1,
            planned.len(),
            task.label(),
            plan.items.get(*idx).map(|i| i.target.as_str()),
            handoff,
            land,
            allowed.join("/"),
            skipped.len(),
            plan.fallback,
            plan.reason.as_deref().unwrap_or("（无）")
        );
        db::record_event(
            &ctx.db,
            ctx.conversation_id,
            ctx.script_key.as_deref(),
            db::EVENT_ROUTE,
            task.label(),
            &format!(
                "第{}/{}项 任务={} 承接={:?} 队列={} 项 落盘={} 跳过={} 回落={} 工具={} 判据={}",
                n + 1,
                planned.len(),
                task.label(),
                handoff,
                plan.items.len(),
                land,
                skipped.len(),
                plan.fallback,
                allowed.join("/"),
                plan.reason.as_deref().unwrap_or("（无）")
            ),
        )
        .await;

        // 项与项之间在回复里留个界 —— 用户才看得出这是队列的第几项、谁在做
        if planned.len() > 1 {
            let head = format!("\n\n---\n\n**（第 {} 项 · {}）**\n\n", n + 1, task.label());
            let _ = ctx.channel.send(SkillAgentEvent::MessageDelta {
                content: head.clone(),
            });
            reply.push_str(&head);
        }

        // 技能菜单：只在**真的能给 read_skill** 的轮次注入。
        // 实录里它 1848 字符、4 轮贡献 0 次有效 read_skill —— 而正常路径按角色过滤后
        // 没有任何角色能调 read_skill，列出来等于告诉模型"还有别的手册"。
        let skills_block = if allowed.contains(&"read_skill") {
            skills::build_skills_xml(&skills::find_all_skills(&ctx.skills_dir))
        } else {
            String::new()
        };

        let task_block = if plan.fallback {
            // 回落：退回按阶段注入，并把底线边界补上
            let mut block = stage::build_stage_block(&ctx.skills_dir, snap.stage);
            block.push_str("\n\n【职责边界】");
            block.push_str(&role::render_boundary(None));
            block
        } else {
            stage::build_task_block(&stage::TaskBlockInput {
                task: *task,
                handoff: *handoff,
                plan: &plan,
                user_msg: &user_msg,
                skills_dir: &ctx.skills_dir,
                land,
                item_index: *idx,
                skipped: &skipped,
            })
        };
        let system_prompt = build_system_prompt(
            &ctx.config,
            allowed,
            &skills_block,
            &script_block,
            &task_block,
            &ctx.sandbox_dir,
            &ctx.skills_dir,
        );

        // 动态材料不落库，每项重算（待写章节 / 上一章收尾 / 落盘进度）
        let run_materials = stage::build_run_materials(&snap, &ctx.data_dir);
        let mut messages = item_messages(system_prompt, &base_history, &handoffs, run_materials);

        let text = run_item_tools(
            &ctx,
            &mut messages,
            allowed,
            &cancelled,
            max_rounds,
            &mut usage,
            is_first_turn && n == 0,
        )
        .await?;
        handoffs.push(LlmMessage::user(handoff_note(n, *task, &text)));
        reply.push_str(&text);
    }

    // 队列重新落一次：这一轮刚落盘的章节要当场打勾（不然要等下一轮）
    if let (Some(dir), Some(key)) = (
        ctx.stage_snapshot.script_dir.as_deref(),
        ctx.script_key.as_deref(),
    ) {
        let snap = stage::derive(Some(key));
        if let Err(e) = stage::write_queue(dir, &plan.items, &snap.written) {
            tracing::warn!("[router] 队列打勾失败: {e}");
        }
    }

    let _ = ctx.channel.send(SkillAgentEvent::Done {
        final_text: reply,
        usage: usage.into_option(),
    });
    Ok(())
}

/// 每项执行前重新读盘 —— 上一项刚落盘的章节，这一项要看得见。
fn refreshed_snapshot(ctx: &SkillAgentRunContext) -> stage::StageSnapshot {
    match ctx.script_key.as_deref() {
        Some(key) => stage::derive(Some(key)),
        None => ctx.stage_snapshot.clone(),
    }
}

/// 拼一项请求的消息表：system + 历史 + 前面各项的交接材料 + 本轮动态材料。
///
/// 单独成函数是为了守住一条真机踩出来的规矩：**请求不能以"没带思考链的
/// assistant 消息"结尾**。DeepSeek 的思考模式在带 tools 时要求那样一条消息必须
/// 把 `reasoning_content` 带回（否则 400 `The reasoning_content in the thinking
/// mode must be passed back to the API`）；队列里第 2 项起前面会多出第 1 项的产出，
/// 而未绑定剧本的会话又没有动态材料垫在后面，正好凑成那个形状。
fn item_messages(
    system_prompt: String,
    base_history: &[LlmMessage],
    handoffs: &[LlmMessage],
    run_materials: String,
) -> Vec<LlmMessage> {
    let mut messages = Vec::with_capacity(base_history.len() + handoffs.len() + 2);
    messages.push(LlmMessage::system(system_prompt));
    messages.extend(base_history.iter().cloned());
    messages.extend(handoffs.iter().cloned());
    if !run_materials.is_empty() {
        messages.push(LlmMessage::user(run_materials));
    }
    messages
}

/// 队列项之间的交接材料：本项做完了什么，后面各项要看得见。
///
/// 走 user 角色而不是 assistant：一来这就是交给下一个角色的交接说明，二来见
/// [`item_messages`] —— assistant 角色会撞上思考模式那条要求。
fn handoff_note(index: usize, task: role::TaskKind, text: &str) -> String {
    format!(
        "【本轮第 {} 项 · {} · 产出】\n{}\n\n（以上是你前面各项的产出，供后续项交接；用户的原话只有上面那条 user 消息。）",
        index + 1,
        task.label(),
        text.trim()
    )
}

/// 这一轮落不落盘。
///
/// 流程 Agent 会给一个 `land`，但用户原话里明说了就按他说的 —— 模型漏判的代价是
/// 「用户说落盘，它却在原地问要不要落盘」，比多落一次更烦人。反过来，
/// 只说"写"（写大纲/写章节/把剧情写出来）**不算**落盘。
fn wants_landing(user_msg: &str, recent: &[(String, String)], plan_land: bool) -> bool {
    /// 用户明说落盘的说法。宁可漏一种，也别把"写"塞进来。
    const LAND_WORDS: [&str; 7] = [
        "落盘",
        "存下来",
        "存成",
        "生成章节",
        "写进 chapters",
        "写进chapters",
        "落成",
    ];
    let msg = user_msg.to_lowercase();
    if LAND_WORDS.iter().any(|w| msg.contains(w)) {
        return true;
    }
    if plan_land {
        return true;
    }
    // 上一轮它问过「要不要落盘」，用户回了个短促的"要/好/可以" —— 也算明说
    let asked = recent
        .last()
        .map(|(_, assistant)| assistant.contains("落盘"))
        .unwrap_or(false);
    asked && is_short_yes(user_msg)
}

/// 短促的肯定回答（"要" / "好，落盘" / "可以"）。
fn is_short_yes(user_msg: &str) -> bool {
    let t = user_msg
        .trim()
        .trim_end_matches(['。', '！', '!', '~', '～']);
    if t.is_empty() || t.chars().count() > 8 {
        return false;
    }
    [
        "要", "好", "可以", "行", "是", "对", "嗯", "ok", "OK", "Ok", "落盘", "来吧", "开始",
    ]
    .iter()
    .any(|w| t.contains(w))
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

/// 跑**一项**任务的工具轮：流式生成 → 执行工具 → 直到模型不再调工具。
///
/// 返回这一项的最终回复文本；每一轮的 assistant / tool 都照旧落库。
async fn run_item_tools(
    ctx: &SkillAgentRunContext,
    messages: &mut Vec<LlmMessage>,
    allowed: &[&str],
    cancelled: &CancelFlag,
    max_rounds: usize,
    usage: &mut TurnUsage,
    first_turn: bool,
) -> Result<String, String> {
    // 截断自动续跑预算（最多补一次生成）
    let mut recovery_budget: usize = RECOVERY_BUDGET;
    let mut last_text = String::new();

    for round in 0..max_rounds {
        if cancelled.load(Ordering::SeqCst) {
            break;
        }
        // 每次请求前收一次预算：本轮工具结果刚 append 进来，涨得最快的正是这里。
        // 丢到不能再丢仍然超硬线时**不发请求** —— 主动报一句人话，比换 provider 的
        // 英文 400 好得多（用户至少知道该新开一个对话）。
        match stage::apply_context_budget(messages, ctx.context_window) {
            outcome if !stage::budget_allows_send(&outcome) => {
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
            match stream_completion(ctx, messages, &defs, cancelled).await {
                Ok(r) => r,
                Err(e) => {
                    // provider 自己判超限时，也翻译成同一句人话（兜底：预算估算与
                    // provider 的真实计数总有偏差，这条保证用户永远看得懂）
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

        // 无工具调用 → 这一项完成
        if tool_calls.is_empty() {
            // 被输出长度上限截断（finish_reason=max_tokens）且未取消 → 推一条纠正提示
            // 自动续跑一次。纠正提示只进内存 messages，不落库，用户界面无感知。
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

            // 首轮（本会话第一条 assistant 回复，含工具调用轮）→ 后台自动生成会话标题。
            // 不阻塞 Done：生成/写库/通知都在独立任务里完成；用户已在 UI 手动
            // 改名后（title 非空）自动生成会跳过（见 auto_title_conversation）。
            if first_turn {
                let title_db = ctx.db.clone();
                let title_llm = Arc::clone(&ctx.llm);
                let title_channel = ctx.channel.clone();
                let conv_id = ctx.conversation_id;
                // 标题源：最后一条 user 消息（即本轮提问）+ 本次回复开头摘要
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

            let (ok, mut output) = tools::execute_tool(ctx, allowed, &tc.name, &args).await;

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
            let _ = db::insert_message(&ctx.db, ctx.conversation_id, &tool_msg, None, None).await;
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

// ---------- 会话自动命名 ----------

/// 首轮回复结束后后台生成会话标题（由 `run_chat` 收尾处 spawn，不阻塞回复流）。
///
/// 生成前二次检查标题仍为空：用户可能已手动改名（或在 UI 上新建了标题），
/// 非空则跳过，保证「用户已设置会话名时不再自动生成」。
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
/// 回退截取首条用户消息前 15 字并去掉尾部标点。
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

// ---------- LLM 调用（双路径） ----------

async fn stream_completion(
    ctx: &SkillAgentRunContext,
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
    let llm = &ctx.llm;
    let mut text_out = String::new();
    // 思考链单独累积：只展示不落 LLM 上下文（Reasoning chunk 不进 text_out）。
    let mut reasoning_out = String::new();
    // 本轮 token 用量：由 provider 的 StreamEnd.usage / 非流式响应的 usage 填充；
    // provider 未上报时保持 None，调用方按「无数据」处理。
    let mut usage: Option<Usage> = None;
    // 最后一次 StreamEnd 携带的归一化停止原因（"stop" / "max_tokens" / …）。
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
                    // LlmUsage（LLM 层）→ Usage（skill_agent 事件层）字段同名直转
                    usage = end_usage.map(|u| Usage {
                        prompt_tokens: u.prompt_tokens,
                        completion_tokens: u.completion_tokens,
                        total_tokens: u.total_tokens,
                        cached_tokens: u.cached_tokens,
                    });
                },
                LlmChunk::ToolCallProgress { .. } => {
                    // 剧本编辑器的 agent 会话不需要参数生成进度提示
                },
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

/// provider 的报错是不是"超出上下文"。
///
/// 各家措辞不同，只做宽松匹配：**宁可漏判**（原样透出报错）也不要误判
/// （把别的错误说成"上下文满了"，用户会去新开会话却发现问题还在）。
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
