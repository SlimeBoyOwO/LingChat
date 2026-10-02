//! 注入文本：任务块、交接单、材料与进度行，只读入参、只产文本。

use super::evidence::{extract_chapter_block, mark_phrase, read_chapter, tail_state};
use super::queue::{QUEUE_JSON_REL_PATH, read_queue_progress, read_state};
use super::{CHAPTER_DETAILS_REL_PATH, DESIGN_REL_PATH, SKELETON_MARKER, Stage, StageSnapshot};
use crate::ai_service::skill_agent::role::TaskKind;
use crate::ai_service::skill_agent::router::QueueItem;
use std::path::Path;

const HUB_DOC: &str = "lingchat-script-editor/SKILL.md";

/// 追加一段行为要求（前导空行 + 全文）；空的不注入。
fn push_directive(out: &mut String, directive: &str) {
    if !directive.is_empty() {
        out.push('\n');
        out.push_str(directive);
    }
}

/// 逐份读入手册并冠以「角色指令」来源头；读不到要把错误告诉模型，否则它会凭记忆补规范。
fn push_materials(out: &mut String, skills_dir: &Path, materials: &[&str]) {
    for rel in materials {
        let path = skills_dir.join(rel);
        match std::fs::read_to_string(&path) {
            Ok(text) => {
                out.push_str(&format!("\n\n【角色指令 · {}】\n", rel));
                out.push_str(text.trim_end());
            },
            Err(_) => {
                tracing::warn!("[skill_agent] 阶段材料缺失: {}", path.display());
                out.push_str(&format!(
                    "\n\n【角色指令缺失 · {rel}】\n本文件读取失败。其中的规范不得凭记忆代替，\
                     也不要静默继续；先告知用户技能文件缺失（可能需要在数据同步里重新勾选）。\n"
                ));
            },
        }
    }
}

pub struct TaskBlockInput<'a> {
    pub task: crate::ai_service::skill_agent::role::TaskKind,
    pub handoff: crate::ai_service::skill_agent::role::Handoff,
    pub plan: &'a crate::ai_service::skill_agent::router::RoutePlan,
    pub user_msg: &'a str,
    pub skills_dir: &'a Path,
    /// 这一轮要不要产出能跑章节：由计划里有没有 `WriteChapter` / `ReviseChapter` 派生（见 `core`），不看用户怎么说。
    pub land: bool,
    pub item_index: usize,
    /// 队列里被跳过的项（做不了），让模型在回执里说明。
    pub skipped: &'a [String],
    /// 这一项是不是本轮最后执行的：补过前置的项会把原任务追加到本轮末尾，只能由 `core` 按执行游标认定。
    pub is_last: bool,
    /// 这一轮真正下发的工具清单（`core.rs` 按项算出的 `allowed`）。
    pub allowed: &'a [&'a str],
}

/// 本轮的注入块：任务 + 行为要求 + 职责边界 + 承接说明 + 该任务要的那几本手册（按任务注入而非按阶段）。
pub fn build_task_block(input: &TaskBlockInput<'_>) -> String {
    let TaskBlockInput {
        task,
        handoff,
        plan,
        user_msg,
        skills_dir,
        land,
        item_index,
        skipped,
        is_last,
        allowed,
    } = *input;
    let mut out = format!("\n\n【本轮任务】{}", task.label());
    match task.role() {
        Some(role) => out.push_str(&format!("（{}）", role.label())),
        None => out.push_str("（不涉及剧本）"),
    }
    if let Some(target) = plan
        .items
        .get(item_index)
        .map(|i| i.target.trim())
        .filter(|t| !t.is_empty())
    {
        out.push_str(&format!("｜本项目标：{target}"));
    }
    let prerequisite = matches!(
        handoff,
        crate::ai_service::skill_agent::role::Handoff::Prerequisite { .. }
    );
    if plan.items.len() > 1 {
        out.push_str(&format!(
            "　队列第 {}/{} 项，{}",
            item_index + 1,
            plan.items.len(),
            if prerequisite {
                "先补前置（见下）"
            } else {
                "只做这一项"
            }
        ));
    }
    if let crate::ai_service::skill_agent::role::Handoff::Prerequisite { why, .. } = handoff {
        if let Some(original) = plan.items.get(item_index) {
            let target = original.target.trim();
            let original_desc = if target.is_empty() {
                original.kind.label().to_string()
            } else {
                format!("{}：{}", original.kind.label(), target)
            };
            out.push_str(&format!(
                "\n【这一项为什么换了名字】原定这一项是「{original_desc}」；{why} —— \
                 所以本轮先做前置的「{}」。同轮会接着做回原任务（见【承接说明】）。",
                task.label()
            ));
        }
    }
    if !skipped.is_empty() {
        out.push_str(&format!(
            "\n【队列里做不了的项】{}（在回执里逐条说明为什么，别装没看见）",
            skipped.join("；")
        ));
    }
    if plan.items.len() > 1 {
        out.push_str(include_str!("../prompts/stage_build_task_block.txt"));
    }
    if is_last {
        out.push_str(
            "\n【本轮最后一项】你是本轮最后执行的一项：先把这一项交待完\
             （做了什么 / 发现了什么 / 要用户定什么），**然后**用一句话把本轮每项各自做了什么列全 ——\
             用户只看最后这一段也知道全貌。",
        );
    }

    if !user_msg.trim().is_empty() {
        out.push_str(&format!("\n【用户这一轮的原话】{}", user_msg.trim()));
    }

    out.push_str(include_str!("../prompts/stage_build_task_block_2.txt"));

    out.push('\n');
    out.push_str(crate::ai_service::skill_agent::role::ACTION_VOCAB);
    if !land && !task.materials().is_empty() && task.role().is_some() {
        out.push('\n');
        out.push_str(crate::ai_service::skill_agent::role::DICTATE_NOTE);
    }

    out.push_str(&format!(
        include_str!("../prompts/stage_build_task_block_3.txt"),
        allowed.join(" / ")
    ));

    out.push_str(&format!(
        // 本项能动什么：三层方向规则（改可以往上对齐、删只能往下连带），工具层另有对应闸门。
        include_str!("../prompts/stage_build_task_block_4.txt"),
        task.scope_note(
            plan.items
                .get(item_index)
                .map(|i| i.target.trim())
                .unwrap_or("")
        )
    ));

    if plan.items.len() > 1 {
        out.push_str(&format!(
            "\n\n【本轮共 {} 项，这是第 {} 项】代码会**逐项**交给对应角色执行：\
             **你只管这一项**，其余项由代码另起一段交给对应角色，**别代劳、也别替它们汇报**。",
            plan.items.len(),
            item_index + 1
        ));
    }

    out.push_str(
        "\n\n【回执怎么写】只讲**这一轮盘上发生的改变**：动了哪个文件、改前是什么、改后是什么\
         （新增/删掉了哪一章哪一节、把哪一行改成了什么）；盘上**本来就有的**东西不要重新介绍一遍 ——\
         比如前面 1~10 章早就写好了、这一轮写的是第 11 章，那就只说第 11 章，别把 1~10 章再列一遍。",
    );
    if is_last {
        out.push_str(
            "你是本轮最后一项：用户要定的事在这里一次说清（其余各项已由代码各自交付）。\
             **交接材料里标着「还没核对到产物」的项，也要如实交代**（说我这一轮没核对到它的产物、\
             缺什么、要他定什么），不许当成做完了。",
        );
    } else {
        out.push_str(&format!(
            include_str!("../prompts/stage_build_task_block_5.txt"),
            DEFERRED_MARK = DEFERRED_MARK
        ));
    }

    push_directive(&mut out, task.directive());

    let note = match (task.boundary_note(), plan.boundary.as_deref()) {
        (Some(a), Some(b)) => Some(format!("{a}；{b}")),
        (Some(a), None) => Some(a.to_string()),
        (None, Some(b)) => Some(b.to_string()),
        (None, None) => None,
    };
    out.push_str("\n【职责边界】");
    out.push_str(&crate::ai_service::skill_agent::role::render_boundary(
        note.as_deref(),
    ));

    let handoff_text = crate::ai_service::skill_agent::role::render_handoff(handoff);
    if !handoff_text.is_empty() {
        out.push_str("\n【承接说明】");
        out.push_str(&handoff_text);
    }

    let materials = task.materials();
    if materials.is_empty() {
        out.push_str("\n\n（本轮不注入任何角色指令。）");
    } else {
        out.push_str(
            "\n\n以下技能文档是本轮**必须遵守的角色指令**，不是参考资料；已注入的无需重复读取。",
        );
        push_materials(&mut out, skills_dir, materials);
    }
    out
}

/// 中间项把"要用户拍板的事"写成这一行，代码原样摘给后面各项（尤其收尾那一项）。
pub const DEFERRED_MARK: &str = "【留给最后一项问】";

/// 交接材料：本轮在前面排过的项 + 它们在账本上的状态 + 中间项留下的待问事项。
pub fn handoff_digest(dir: &Path, items: &[QueueItem], upto: usize, deferred: &[String]) -> String {
    if upto == 0 && deferred.is_empty() {
        return String::new();
    }
    let state = read_state(dir, QUEUE_JSON_REL_PATH);
    let mut out = String::new();
    if upto > 0 {
        out.push_str(&format!(
            "\n\n【本轮在你之前排过的项】（这是进度，不是你的任务；你这一项是第 {}/{} 项）",
            upto + 1,
            items.len()
        ));
        for (i, item) in items.iter().take(upto).enumerate() {
            let target = item.target.trim();
            let phrase = state
                .iter()
                .find(|e| e.kind == item.kind && e.target == target)
                .map(|e| mark_phrase(e.mark))
                .unwrap_or("没有可核对的产物（话术 / 只读体检这类）");
            out.push_str(&format!(
                "\n- 第 {} 项 {} —— {phrase}",
                i + 1,
                render_text_of(item.kind, target)
            ));
        }
        out.push_str(
            "\n（这一份只讲进度：哪一项具体做了什么，去看盘上那几个文件；**不要照它去做事**。）",
        );
    }
    if !deferred.is_empty() {
        out.push_str("\n【前面各项留给最后一项问用户的事】");
        for q in deferred {
            out.push_str(&format!("\n- {q}"));
        }
    }
    out
}

/// 从一项的回执里摘出 [`DEFERRED_MARK`] 那一行；没写就没有，不猜也不从正文里反解。
pub fn deferred_question(reply: &str) -> Option<String> {
    reply
        .lines()
        .map(str::trim)
        .find(|l| l.starts_with(DEFERRED_MARK))
        .map(|l| l.trim_start_matches(DEFERRED_MARK).trim().to_string())
        .filter(|l| !l.is_empty())
}

/// 一项的正文（`类型标签：目标`），视图与注入的进度行共用；目标为空时只写类型标签。
pub fn render_text_of(kind: TaskKind, target: &str) -> String {
    if target.trim().is_empty() {
        return kind.label().to_string();
    }
    format!("{}：{}", kind.label(), target.trim())
}

/// 流程总纲全文（流程 Agent 每轮用它判断流程走到哪）；读不到返回 `None`，调用方应回落。
pub fn hub_doc(skills_dir: &Path) -> Option<String> {
    std::fs::read_to_string(skills_dir.join(HUB_DOC))
        .ok()
        .map(|t| t.trim_end().to_string())
}

pub fn build_run_materials(snap: &StageSnapshot) -> String {
    let Some(dir) = snap.script_dir.as_deref() else {
        return String::new();
    };
    let design = snap.design.as_deref().unwrap_or_default();

    let mut out = match snap.stage {
        Stage::Setup | Stage::Modify if !design.is_empty() => {
            format!("\n\n【现有设计稿】\n{}", design.trim_end())
        },
        Stage::Forge => {
            let mut forge = String::new();
            if let Some(id) = snap.next_chapter() {
                if let Some(block) = extract_chapter_block(design, id) {
                    forge.push_str(&format!("\n\n【待写章节 · {}】\n{}", id, block));
                }
            }
            if let Some(prev) = snap.last_written() {
                let tail = tail_state(&read_chapter(dir, prev));
                if !tail.is_empty() {
                    forge.push_str(&format!("\n\n【上一章（{}）收尾状态】\n{}", prev, tail));
                }
            }
            forge
        },
        _ => String::new(),
    };
    out.push_str(&progress_block(snap, dir));
    out
}

/// 交接单里的进度事实；设计稿取快照里那份，同一份事实只算一次。
pub(super) fn progress_block(snap: &StageSnapshot, dir: &Path) -> String {
    let mut lines: Vec<String> = vec![config_line(dir)];
    if dir.join(DESIGN_REL_PATH).is_file() || !snap.written.is_empty() {
        lines.push(design_line(snap.design.as_deref(), &snap.plan));
    }
    if dir.join(CHAPTER_DETAILS_REL_PATH).is_file() {
        lines.push(format!(
            "章节细节稿：在（{}）—— 落盘时的细节以它为准，跟设计稿的梗概有出入就当场对齐",
            CHAPTER_DETAILS_REL_PATH
        ));
    }

    lines.push(format!(
        "已落盘：{}",
        if snap.written.is_empty() {
            "（无）".to_string()
        } else {
            snap.written.join(" ")
        }
    ));

    if let Some(after) = chapter_after(snap) {
        let exists = snap.written.iter().any(|w| w == &after);
        lines.push(format!(
            "下一章 {}：{}",
            after,
            if exists {
                "磁盘上已有内容（本轮改的是它前面，别顺手覆盖它）"
            } else {
                "尚未落盘"
            }
        ));
    }
    if let Some(queue) = read_queue_progress(dir) {
        lines.push(format!("队列：{}", queue));
    }

    let mut out = String::from("\n\n【落盘进度】\n");
    for line in lines {
        out.push_str(&format!("- {}\n", line));
    }
    out
}

/// 设计稿那一行：告诉模型代码从设计稿里读出了什么；`design` 只回答读没读到（空文件也算读到），章节列表用事实层解析好的 `plan`。
fn design_line(design: Option<&str>, plan: &[String]) -> String {
    if design.is_none() {
        return "设计稿：缺（还没有 .agent/design.md）".to_string();
    }
    if plan.is_empty() {
        "设计稿：在，但读不出章节 —— 每个 `##` 标题后的**第一个非空行**必须写 `id: 01`\
         （写在别处的 id 会被忽略），补上才能按章推进（改设计稿是「改大纲」那一项的活，\
         本项不是它就把这件事写进回执）"
            .to_string()
    } else {
        format!("设计稿：{} 章（{}）", plan.len(), plan.join(" "))
    }
}

/// 剧本配置那一行（"还没建" / "只有系统骨架"）：不说清的话，模型会以为工程已建好、不去补 description。
fn config_line(dir: &Path) -> String {
    match std::fs::read_to_string(dir.join("story_config.yaml")) {
        Err(_) => "剧本配置：缺（系统会补一份最小骨架，能打开但不是成品）".to_string(),
        Ok(text) if text.contains(SKELETON_MARKER) => {
            "剧本配置：只有系统建的最小骨架 —— 请在工程创建阶段按剧本类型补全\
             （简介 / 触发方式 / 解锁与成就 / 玩家称呼）"
                .to_string()
        },
        Ok(_) => "剧本配置：已就绪".to_string(),
    }
}

fn chapter_after(snap: &StageSnapshot) -> Option<String> {
    let current = snap.next_chapter()?;
    let idx = snap.plan.iter().position(|p| p == current)?;
    snap.plan.get(idx + 1).cloned()
}
