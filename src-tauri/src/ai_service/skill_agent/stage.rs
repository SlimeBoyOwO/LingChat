//! 阶段推导与按阶段上下文配置。
//!
//! 阶段由文件系统事实推导，不落库、不强制流程；真正阻塞的确认只有工具入口的命令审批，
//! 这里的「确认门」全部只是提示词。

use std::path::{Path, PathBuf};

use crate::ai_service::types::LlmMessage;
use crate::api::script_editor::validate;
use crate::utils::script_modes::{self, AssetMode, CastMode, ScriptModes};
use crate::utils::script_paths;

/// 流程产物目录。点号目录不进引擎扫描与编辑器枚举；「详情」浮窗直接列这一层。
pub const AGENT_DIR: &str = ".agent";

pub const DESIGN_REL_PATH: &str = ".agent/design.md";

/// 章节细节稿（按章分节）：模型曾自己发明它写细节而系统不知情，现在是正式产物，改完要对齐设计稿。
pub const CHAPTER_DETAILS_REL_PATH: &str = ".agent/chapter-details.md";

pub use crate::utils::script_modes::CONSTRAINTS_REL_PATH;

pub const QUEUE_REL_PATH: &str = ".agent/queue.md";

pub const ASSETS_REL_PATH: &str = ".agent/assets.md";

pub const CAST_REL_PATH: &str = ".agent/cast.md";

/// 自动骨架的标记行。`progress_block` 靠它判断"这份配置还没被补全"。
pub const SKELETON_MARKER: &str = "本文件由系统自动创建";

/// 确保剧本包有个**能加载**的骨架：缺 `story_config.yaml` 补一份最小的，缺 `Chapters/` 就建。
/// 编辑器按 `story_config.yaml` 认剧本，少了它列表里看不到、引擎也跳过（真机踩过）；**已存在就不动**。
pub fn ensure_package_skeleton(dir: &Path, key: &str) -> std::io::Result<bool> {
    if !dir.join("Chapters").is_dir() {
        std::fs::create_dir_all(dir.join("Chapters"))?;
    }
    let config = dir.join("story_config.yaml");
    if config.exists() {
        return Ok(false);
    }

    let name = dir
        .file_name()
        .map(|s| s.to_string_lossy().to_string())
        .unwrap_or_default();
    let mut text = format!(
        "# LingChat 剧本配置文件\n\
         # {SKELETON_MARKER}（剧本包骨架），请在「大纲与工程创建」阶段按剧本类型补全。\n\
         # 剧本名必须与文件夹名一致：{name}\n\
         script_name: {name}\n\
         intro_chapter: 01\n"
    );
    let segs: Vec<&str> = key.split('/').collect();
    if segs.len() == 3 && segs[0] == "character" {
        text.push_str(&format!(
            "\n# ===== 羁绊冒险专属配置（按目录布局推断）=====\n\
             adventure:\n  is_adventure: true\n  bound_character_folder: \"{}\"\n  \
             trigger:\n    mode: \"manual\"\n",
            segs[1]
        ));
    }
    text.push_str("\nscript_settings:\n  user_name: \"玩家\"\n");
    std::fs::write(&config, text)?;
    Ok(true)
}

// 相对技能目录（`data/game_data/skills`）的材料路径；角色手册路径由 `role.rs` 拥有，这里只引用。
const HUB_DOC: &str = "lingchat-script-editor/SKILL.md";
const WRITER_DOC: &str = super::role::Role::Writer.doc();
const TRANSFORMER_DOC: &str = super::role::Role::Transformer.doc();
const OPTIMIZER_DOC: &str = super::role::Role::Optimizer.doc();
const EVENT_REFERENCE: &str = "lingchat-script-editor/references/event-reference.md";
const CHAPTER_TEMPLATE: &str = "lingchat-script-editor/assets/templates/chapter_template.yaml";

/// 创作阶段。按「上下文配置是否相同」划分，S1–S3 合并为 [`Stage::Setup`]。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Stage {
    /// 无剧本包，或设计稿尚未列出章节。
    #[default]
    Routing,
    Setup,
    /// 设计稿已声明章节，但尚未写完。
    Forge,
    Polish,
    /// 有剧本包、无设计稿、且已有章节：修改既有剧本。
    Modify,
}

impl Stage {
    pub fn label(self) -> &'static str {
        match self {
            Stage::Routing => "未绑定剧本",
            Stage::Setup => "设计与大纲",
            Stage::Forge => "逐章编写",
            Stage::Polish => "校验与修复",
            Stage::Modify => "修改既有剧本",
        }
    }
}

#[derive(Debug, Clone, Default)]
pub struct StageSnapshot {
    pub stage: Stage,
    pub script_key: Option<String>,
    pub script_dir: Option<PathBuf>,
    pub plan: Vec<String>,
    pub written: Vec<String>,
}

impl StageSnapshot {
    /// 下一个待写章节 id（仅 [`Stage::Forge`] 有意义）。
    pub fn next_chapter(&self) -> Option<&str> {
        self.plan
            .iter()
            .map(String::as_str)
            .find(|id| !self.written.iter().any(|w| w == id))
    }

    pub fn last_written(&self) -> Option<&str> {
        self.plan
            .iter()
            .map(String::as_str)
            .rfind(|id| self.written.iter().any(|w| w == id))
    }
}

pub fn derive(script_key: Option<&str>) -> StageSnapshot {
    let Some(key) = script_key else {
        return StageSnapshot::default();
    };
    let Ok(script_dir) = script_paths::resolve_script_dir(key) else {
        // 包还不存在 → 创建流程
        return StageSnapshot {
            stage: Stage::Setup,
            script_key: Some(key.to_string()),
            ..Default::default()
        };
    };

    let written = script_paths::enumerate_chapter_ids(&script_dir);
    let plan = read_plan(&script_dir);

    let stage = if plan.is_empty() {
        // 无设计稿：已有章节说明是既存剧本，否则还在设计与大纲阶段
        if written.is_empty() {
            Stage::Setup
        } else {
            Stage::Modify
        }
    } else if plan.iter().all(|id| written.iter().any(|w| w == id)) {
        Stage::Polish
    } else {
        Stage::Forge
    };

    StageSnapshot {
        stage,
        script_key: Some(key.to_string()),
        script_dir: Some(script_dir),
        plan,
        written,
    }
}

pub struct StageProfile {
    /// 思考模式覆盖；`None` 表示不干预，沿用 provider 默认。
    pub thinking: Option<bool>,
    pub system_materials: &'static [&'static str],
    pub directive: &'static str,
}

const DIRECTIVE_SETUP: &str = "本阶段是设计与大纲：产出设计稿（大纲 + 章节设计 + 素材诉求）。\
    \n- 设计稿落地后**停下等用户确认**，不要接着开始写章节";

const DIRECTIVE_FORGE: &str = "本阶段是逐章编写，**粒度是「一轮一章」**：\
    \n- 一轮只写一章，写完立即停下等用户确认；不要在一条消息里并行写多章\
    \n- 也不要写完一章后不等确认就接着写下一章\
    \n- 通用规则里的「任务必须完成到产出物为止」指的是**当前这一章的产出物**，不是整部剧本\
    \n- **不要调用 `validate_script`**：全剧没写完时它必然报 `chapter_end.dangling`、\
     `graph.unreachable`、`variable.never_read` 等一批「尚未写完」的假错，\
     会把你引向提前补写后续章节；校验留到交付阶段";

const DIRECTIVE_POLISH: &str = "本阶段是校验与修复：\
    \n- 按诊断逐条修复；需要**新编剧情内容**才能补上的缺口交回用户，不要自行编造\
    \n- 修复完成、校验通过后停下汇报";

const DIRECTIVE_MODIFY: &str = "本阶段是修改既有剧本：\
    \n- 按用户提出的修改需求**逐项**修改，改动完成后停下等用户确认\
    \n- **不要自行扩大改动范围**，也不要顺手重写未被要求改动的章节";

pub fn profile(stage: Stage) -> StageProfile {
    // 创作类阶段需要推理；机械落盘与按诊断码表修复不需要。
    match stage {
        // 未绑定剧本时做的正是 hub 的 0/1/2 阶段（入口判断 + 类型 + 大纲），归属 writer
        Stage::Routing => StageProfile {
            thinking: None,
            system_materials: &[HUB_DOC, WRITER_DOC],
            directive: "",
        },
        Stage::Setup => StageProfile {
            thinking: Some(true),
            system_materials: &[HUB_DOC, WRITER_DOC, TRANSFORMER_DOC],
            directive: DIRECTIVE_SETUP,
        },
        Stage::Forge => StageProfile {
            thinking: Some(false),
            system_materials: &[HUB_DOC, TRANSFORMER_DOC, EVENT_REFERENCE, CHAPTER_TEMPLATE],
            directive: DIRECTIVE_FORGE,
        },
        Stage::Polish => StageProfile {
            thinking: Some(false),
            system_materials: &[HUB_DOC, TRANSFORMER_DOC, OPTIMIZER_DOC],
            directive: DIRECTIVE_POLISH,
        },
        Stage::Modify => StageProfile {
            thinking: Some(true),
            system_materials: &[HUB_DOC, WRITER_DOC, TRANSFORMER_DOC, OPTIMIZER_DOC],
            directive: DIRECTIVE_MODIFY,
        },
    }
}

/// 渲染阶段块（阶段名 + 行为要求 + 角色指令），拼在系统提示末尾以保前缀缓存。
pub fn build_stage_block(skills_dir: &Path, stage: Stage) -> String {
    let profile = profile(stage);
    let mut out = format!("\n\n【当前阶段】{}", stage.label());
    if !profile.directive.is_empty() {
        out.push('\n');
        out.push_str(profile.directive);
    }
    out.push_str(
        "\n\n以下技能文档是本阶段**必须遵守的角色指令**，不是参考资料。\
         \n已注入的文档无需重复调用 read_skill；文档里引用到的其他角色技能，\
         需要时仍用 read_skill 加载。",
    );
    out.push_str(&load_system_materials(skills_dir, stage));
    out
}

fn load_system_materials(skills_dir: &Path, stage: Stage) -> String {
    let mut out = String::new();
    for rel in profile(stage).system_materials {
        out.push_str(&load_one_material(skills_dir, rel));
    }
    out
}

/// 读一份手册，冠以「角色指令」来源头。读不到必须让模型看见：只打日志它会凭记忆补规范。
fn load_one_material(skills_dir: &Path, rel: &str) -> String {
    let mut out = String::new();
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
    out
}

pub struct TaskBlockInput<'a> {
    pub task: super::role::TaskKind,
    pub handoff: super::role::Handoff,
    pub plan: &'a super::router::RoutePlan,
    pub user_msg: &'a str,
    pub skills_dir: &'a Path,
    /// 这一轮落不落盘（用户明说了才 true）。
    pub land: bool,
    pub item_index: usize,
    /// 队列里被跳过的项（做不了），让模型在回执里说明。
    pub skipped: &'a [String],
}

/// 本轮的注入块：任务 + 行为要求 + 职责边界 + 承接说明 + **该任务要的那几本手册**。
/// 按任务注入而非按阶段：同一阶段里「只提问的那一轮」不需要落盘手册。
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
    } = *input;
    let mut out = format!("\n\n【本轮任务】{}", task.label());
    match task.role() {
        Some(role) => out.push_str(&format!("（{}）", role.label())),
        None => out.push_str("（不涉及剧本）"),
    }
    // 队列是逐项执行的：告诉它这是第几项，免得它以为要一次把全部内容塞进一条回复
    if plan.items.len() > 1 {
        out.push_str(&format!(
            "　队列第 {}/{} 项，只做这一项",
            item_index + 1,
            plan.items.len()
        ));
    }
    if !skipped.is_empty() {
        out.push_str(&format!(
            "\n【队列里做不了的项】{}（在回执里逐条说明为什么，别装没看见）",
            skipped.join("；")
        ));
    }

    // 用户这一轮的原话：任务名是从这句话判出来的，一句话里顺手捎带的别的事会漏掉。
    if !user_msg.trim().is_empty() {
        out.push_str(&format!("\n【用户这一轮的原话】{}", user_msg.trim()));
    }

    // 「写」还是「落盘」：这一段两个 Agent 都要看到，口径不一致时最先崩的是用户。
    out.push('\n');
    out.push_str(super::role::ACTION_VOCAB);
    if !land && !task.materials().is_empty() && task.role().is_some() {
        out.push('\n');
        out.push_str(super::role::DICTATE_NOTE);
    }

    // 听用户的：推荐写法只是默认、不是死规矩；"能力范围" = 这一轮能用的工具 + 手上这本手册。
    out.push_str(&format!(
        "\n【听用户的】你这一轮能用的工具是 {}。用户要的事只要这些工具加这本手册做得到，\
         就按他说的做 —— 下面那些只是**推荐写法**，用户有别的写法就以他的为准\
         （他说一次写三章就写三章；他说顺手把某章改掉，而你有 write_file，就改）。\
         做不到的（这一轮没那个手段）不要硬编，直说做不到，并告诉他该在什么时候做。",
        task.tools().join(" / ")
    ));

    // 队列是这一轮的工作清单：只给一行"还剩几项"不够 —— 模型会做完手上这件就收尾。
    let queue = &plan.items;
    if queue.len() > 1 || queue.iter().any(|i| !i.target.trim().is_empty()) {
        out.push_str(&format!(
            "\n\n【本轮队列】共 {} 项，代码会**逐项**交给对应角色执行，这一轮只做其中一项：",
            queue.len()
        ));
        for (i, item) in queue.iter().enumerate() {
            let target = item.target.trim();
            out.push_str(&format!(
                "\n- {} {}：{}",
                if i == item_index {
                    "[本项]"
                } else {
                    "[其余]"
                },
                item.kind.label(),
                if target.is_empty() {
                    "（未指明）"
                } else {
                    target
                }
            ));
        }
        out.push_str(
            "\n**只做标着 [本项] 的那一件**：它的承接与工具就是为它准备的。\
             其余项别顺手替它们做（该轮到谁的活，代码会换个角色再来一轮）。\
             回执里写清这一项做了什么、落到哪个文件。",
        );
    }

    let directive = task.directive();
    if !directive.is_empty() {
        out.push('\n');
        out.push_str(directive);
    }

    let note = match (task.boundary_note(), plan.boundary.as_deref()) {
        (Some(a), Some(b)) => Some(format!("{a}；{b}")),
        (Some(a), None) => Some(a.to_string()),
        (None, Some(b)) => Some(b.to_string()),
        (None, None) => None,
    };
    out.push_str("\n【职责边界】");
    out.push_str(&super::role::render_boundary(note.as_deref()));

    let handoff_text = super::role::render_handoff(handoff);
    if !handoff_text.is_empty() {
        out.push_str("\n【承接说明】");
        out.push_str(&handoff_text);
    }

    let materials = task.materials();
    if materials.is_empty() {
        // 仅对话：连手册都不给，免得它顺手把剧本的事也做了
        out.push_str(
            "\n\n（本轮是仅对话：不注入任何角色指令。这一轮不要动任何文件 —— \
             如果用户其实想改剧本，先问清楚要改哪里。）",
        );
    } else {
        out.push_str(
            "\n\n以下技能文档是本轮**必须遵守的角色指令**，不是参考资料；已注入的无需重复读取。",
        );
        for rel in materials {
            out.push_str(&load_one_material(skills_dir, rel));
        }
    }
    out
}

/// 把本轮的队列落盘成 `.agent/queue.md`：**合并而非覆盖** —— 旧文件里没勾的项一律留着
/// （那是"还没做完"的记忆），本轮新项追加在后面；真机上曾有闲聊把队列整份重写，上一轮排着的项当场消失。
/// **勾选只依据代码能核对的事实**：`write_chapter` 的 target 是章节 id 且那一章已落盘；别的类型不打勾。
pub fn write_queue(
    dir: &Path,
    items: &[super::router::QueueItem],
    written: &[String],
) -> std::io::Result<()> {
    let mut entries: Vec<(bool, String)> = read_queue_entries(dir);
    for item in items {
        let text = queue_text(item);
        if item.kind == super::role::TaskKind::Chat && item.target.trim().is_empty() {
            continue;
        }
        if !entries.iter().any(|(_, e)| e == &text) {
            entries.push((false, text));
        }
    }
    // 已落盘章节的「编写章节」打勾；其余保持原状（旧文件里勾过的不会被抹掉）
    for (done, text) in entries.iter_mut() {
        if *done {
            continue;
        }
        if let Some(target) = text.strip_prefix("落盘章节：") {
            if !target.trim().is_empty() && written.iter().any(|w| w == target.trim()) {
                *done = true;
            }
        }
    }
    let mut out = String::from("# 任务队列（流程 Agent 维护；可以直接手改）\n\n");
    for (done, text) in &entries {
        out.push_str(&format!("- [{}] {}\n", if *done { "x" } else { " " }, text));
    }
    std::fs::write(dir.join(QUEUE_REL_PATH), out)
}

fn read_queue_entries(dir: &Path) -> Vec<(bool, String)> {
    let Ok(text) = std::fs::read_to_string(dir.join(QUEUE_REL_PATH)) else {
        return Vec::new();
    };
    let mut out = Vec::new();
    for line in text.lines() {
        let t = line.trim_start();
        let (done, rest) =
            if let Some(r) = t.strip_prefix("- [x]").or_else(|| t.strip_prefix("- [X]")) {
                (true, r)
            } else if let Some(r) = t.strip_prefix("- [ ]") {
                (false, r)
            } else {
                continue;
            };
        let rest = rest.trim();
        if !rest.is_empty() {
            out.push((done, rest.to_string()));
        }
    }
    out
}

fn queue_text(item: &super::router::QueueItem) -> String {
    let target = item.target.trim();
    format!(
        "{}：{}",
        item.kind.label(),
        if target.is_empty() {
            "（未指明）"
        } else {
            target
        }
    )
}

/// 流程总纲全文（流程 Agent 每轮都用它判断流程走到哪）；读不到返回 `None`，调用方应**回落**。
pub fn hub_doc(skills_dir: &Path) -> Option<String> {
    std::fs::read_to_string(skills_dir.join(HUB_DOC))
        .ok()
        .map(|t| t.trim_end().to_string())
}

/// 承接判断要用的事实。与 [`StageSnapshot`] 同源，但保留阶段投影会丢掉的细节。
/// `target_exists` 只认**已落盘**的 —— 唯一看它的是「改章节」，改不到还没写的东西。
pub fn facts_of(snap: &StageSnapshot, target: Option<&str>) -> super::role::ScriptFacts {
    let wanted: Vec<String> = target.map(chapter_ids_in).unwrap_or_default();
    let present: Vec<bool> = wanted
        .iter()
        .map(|t| snap.written.iter().any(|w| w == t))
        .collect();
    let target_exists = if wanted.is_empty() {
        None
    } else {
        Some(present.iter().any(|p| *p))
    };
    // 「文件在」与「读得出章节」是两件事：前者决定能不能改它，后者只是格式提醒，别当前置用。
    let has_design = snap
        .script_dir
        .as_deref()
        .and_then(|d| std::fs::metadata(d.join(DESIGN_REL_PATH)).ok())
        .is_some_and(|m| m.len() > 0);
    let next = snap.next_chapter();
    super::role::ScriptFacts {
        has_design,
        has_plan: !snap.plan.is_empty(),
        has_written: !snap.written.is_empty(),
        has_next: next.is_some(),
        target_exists,
        target_partial: present.iter().any(|p| *p) && present.iter().any(|p| !*p),
        target_is_next: next.is_some_and(|n| wanted.iter().any(|w| w == n)),
    }
}

/// 用户点名的章节 id 列表（`第 3 章` 这类口语写法归一成 `3`）。
/// **区间与列表要拆开**：真机上 `01-04` 按"单个章节 id"查不到 → 判"还没落盘" → 整轮工具被收成只读。
pub fn chapter_ids_in(raw: &str) -> Vec<String> {
    /// 一个区间最多展开这么多章，防止 `01-9999` 这种写法把内存撑爆。
    const MAX_SPAN: u32 = 200;

    let mut out: Vec<String> = Vec::new();
    for part in raw.split([',', '，', '、', ';', '；', '和', '与', ' ']) {
        let p = normalize_chapter_token(part);
        if p.is_empty() {
            continue;
        }
        let Some(dash) = p.find(['-', '~', '–', '—']) else {
            out.push(p);
            continue;
        };
        let (a, b) = (
            p[..dash].trim(),
            p[dash + 1..]
                .trim_start_matches(['-', '~', '–', '—'])
                .trim(),
        );
        // 只有"两边都是同一宽度的数字"才当区间展开；`Intro/01-Intro/04` 这种不猜
        match (a.parse::<u32>(), b.parse::<u32>()) {
            (Ok(start), Ok(end))
                if start <= end
                    && end - start <= MAX_SPAN
                    && a.chars().all(|c| c.is_ascii_digit())
                    && b.chars().all(|c| c.is_ascii_digit()) =>
            {
                let width = a.len().max(b.len());
                for n in start..=end {
                    out.push(format!("{:0width$}", n, width = width));
                }
            },
            _ => out.push(p),
        }
    }
    out.dedup();
    out
}

/// `第 3 章` / `03节` / `"03"` → `03`（去空白与引号，剥掉"第/章/节"）。
fn normalize_chapter_token(raw: &str) -> String {
    let t = raw
        .trim()
        .trim_matches(['"', '\'', '`', '《', '》', '「', '」']);
    let t = t.strip_prefix('第').unwrap_or(t);
    let t = t
        .strip_suffix('章')
        .or_else(|| t.strip_suffix('节'))
        .unwrap_or(t);
    t.trim().to_string()
}

pub fn build_run_materials(snap: &StageSnapshot, data_dir: &Path) -> String {
    let Some(dir) = snap.script_dir.as_deref() else {
        return String::new();
    };
    let design = std::fs::read_to_string(dir.join(DESIGN_REL_PATH)).unwrap_or_default();

    let mut out = match snap.stage {
        Stage::Setup | Stage::Modify if !design.is_empty() => {
            format!("\n\n【现有设计稿】\n{}", design.trim_end())
        },
        Stage::Forge => {
            let mut forge = String::new();
            if let Some(id) = snap.next_chapter() {
                if let Some(block) = extract_chapter_block(&design, id) {
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
    out.push_str(&progress_block(snap, dir, data_dir));
    out
}

/// 交接单里的进度事实；「下一章是否已有内容」补的是删章那一脚 —— 代码只会说"还没写"。
fn progress_block(snap: &StageSnapshot, dir: &Path, data_dir: &Path) -> String {
    let modes = ScriptModes::read(dir);
    let mut lines: Vec<String> = vec![format!(
        "素材模式：{}（`.agent/constraints.md` 是唯一来源；要改就改那一行，并检查别处有没有旧说法）",
        modes.asset.describe()
    )];
    if uses_character_cards(snap, dir, modes.cast) {
        lines.push(format!(
            "角色卡模式：{}（同上，改一处就全都对齐）",
            modes.cast.describe()
        ));
    }
    // 卡片里自己前后矛盾时当场说破 —— 不然模型会像真机那样连着几轮问用户按哪个走
    if let Some(note) = constraint_conflict(dir) {
        lines.push(format!("⚠️ {note}"));
    }
    lines.push(config_line(dir));
    if dir.join(DESIGN_REL_PATH).is_file() || !snap.written.is_empty() {
        lines.push(design_line(dir));
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

    // 换模式的影响面：已落盘的章节不会跟着重写，所以先把要改的地方摊开
    if !snap.written.is_empty() {
        let missing = missing_assets_of_written(snap, dir, data_dir);
        if !missing.is_empty() {
            lines.push(asset_impact_line(modes.asset, &missing));
        }
    }

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

/// 已落盘的素材缺口在当前模式下意味着什么：换了模式已写好的章节不会自动重写，得把影响面摊开。
fn asset_impact_line(mode: AssetMode, missing: &[String]) -> String {
    const SHOW: usize = 4;

    let mut list = missing
        .iter()
        .take(SHOW)
        .cloned()
        .collect::<Vec<_>>()
        .join("、");
    if missing.len() > SHOW {
        list.push_str(&format!("…（共 {} 处）", missing.len()));
    }
    let n = missing.len();
    match mode {
        AssetMode::OnlyExisting => format!(
            "按「只用已有」有 {} 处要改：{}。若这些其实打算之后补素材，\
             说一句「素材我之后补，先留位置」就能改成「先预留」",
            n, list
        ),
        AssetMode::Reserve => format!(
            "已留空 {} 处：{}。逐条补 / 改 / 接受看 .agent/assets.md；\
             改成「只用已有」它们会变成必须修的错",
            n, list
        ),
        AssetMode::None => format!(
            "「零素材」模式下这些引用本来就不该有（{}）：{}。改成纯文字剧情，或说一句换模式",
            n, list
        ),
        AssetMode::Unspecified => format!(
            "已落盘章节里有 {} 处引用了磁盘上没有的素材：{}。先问用户要「只用已有」「先预留」还是「零素材」",
            n, list
        ),
    }
}

fn constraint_conflict(dir: &Path) -> Option<String> {
    let text = std::fs::read_to_string(dir.join(CONSTRAINTS_REL_PATH)).ok()?;
    script_modes::conflict_note(&text)
}

/// 设计稿那一行：告诉模型代码**从设计稿里读出了什么**；「设计稿在、但读不出章节 id」最坑。
fn design_line(dir: &Path) -> String {
    let Ok(text) = std::fs::read_to_string(dir.join(DESIGN_REL_PATH)) else {
        return "设计稿：缺（还没有 .agent/design.md）".to_string();
    };
    let plan = parse_plan(&text);
    if plan.is_empty() {
        "设计稿：在，但读不出章节 —— 每个 `##` 标题后的**第一个非空行**必须写 `id: 01`\
         （写在别处的 id 会被忽略），补上才能按章推进"
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

fn read_queue_progress(dir: &Path) -> Option<String> {
    let text = std::fs::read_to_string(dir.join(QUEUE_REL_PATH)).ok()?;
    let (mut done, mut pending) = (0usize, Vec::new());
    for line in text.lines() {
        let t = line.trim_start();
        if let Some(rest) = t.strip_prefix("- [x]").or_else(|| t.strip_prefix("- [X]")) {
            if !rest.trim().is_empty() {
                done += 1;
            }
        } else if let Some(rest) = t.strip_prefix("- [ ]") {
            if !rest.trim().is_empty() {
                pending.push(rest.trim().to_string());
            }
        }
    }
    let total = done + pending.len();
    if total == 0 {
        return None;
    }
    let mut out = format!("共 {} 项，已完成 {}，还剩 {}", total, done, pending.len());
    if let Some(next) = pending.first() {
        out.push_str(&format!("（下一项：{}）", next));
    }
    if !pending.is_empty() {
        // 队列是"还欠着的事"不是"这一轮的事"：用户不要了就得去掉，否则它每轮都在催。
        out.push_str("。没勾的是还欠着的：用户说了不用做、或者已经改口做别的，就把它从队列文件里删掉并说一声");
    }
    Some(out)
}

fn read_plan(script_dir: &Path) -> Vec<String> {
    std::fs::read_to_string(script_dir.join(DESIGN_REL_PATH))
        .map(|t| parse_plan(&t))
        .unwrap_or_default()
}

fn parse_plan(markdown: &str) -> Vec<String> {
    let lines: Vec<&str> = markdown.lines().collect();
    let mut plan: Vec<String> = Vec::new();
    for (start, end) in block_ranges(&lines) {
        if let Some(id) = block_id(&lines[start + 1..end]) {
            if !plan.iter().any(|p| p == id) {
                plan.push(id.to_string());
            }
        }
    }
    plan
}

fn extract_chapter_block(markdown: &str, id: &str) -> Option<String> {
    let lines: Vec<&str> = markdown.lines().collect();
    for (start, end) in block_ranges(&lines) {
        if block_id(&lines[start + 1..end]) == Some(id) {
            return Some(lines[start..end].join("\n").trim_end().to_string());
        }
    }
    None
}

fn block_ranges(lines: &[&str]) -> Vec<(usize, usize)> {
    let starts: Vec<usize> = lines
        .iter()
        .enumerate()
        .filter(|(_, l)| l.trim_start().starts_with("##"))
        .map(|(i, _)| i)
        .collect();
    starts
        .iter()
        .enumerate()
        .map(|(n, &s)| (s, starts.get(n + 1).copied().unwrap_or(lines.len())))
        .collect()
}

fn block_id<'a>(block: &[&'a str]) -> Option<&'a str> {
    let first = block.iter().map(|l| l.trim()).find(|l| !l.is_empty())?;
    let rest = first.strip_prefix("id:")?;
    Some(rest.trim().trim_matches(&['"', '\''][..]).trim())
}

/// 取章节末尾的注释块，即 `SKILL.md`「状态注释原则」要求记录的收尾状态。
fn tail_state(chapter_text: &str) -> String {
    let mut tail: Vec<&str> = Vec::new();
    for line in chapter_text.lines().rev() {
        let t = line.trim();
        if t.starts_with('#') {
            tail.push(line);
        } else if !t.is_empty() {
            break;
        }
    }
    tail.reverse();
    tail.join("\n")
}

fn read_chapter(script_dir: &Path, id: &str) -> String {
    chapter_file(script_dir, id)
        .and_then(|p| std::fs::read_to_string(p).ok())
        .unwrap_or_default()
}

fn chapter_file(script_dir: &Path, id: &str) -> Option<PathBuf> {
    script_paths::resolve_chapter_file(script_dir, id, false).ok()
}

/// 丢弃已被超越的章节写入轮次，避免历史里堆积整章 YAML；整轮丢弃以保持历史结构合法。
pub fn compact_history(history: Vec<LlmMessage>, snap: &StageSnapshot) -> Vec<LlmMessage> {
    if snap.stage != Stage::Forge {
        return history;
    }
    let current = snap.next_chapter();
    let mut out = Vec::with_capacity(history.len());
    let mut i = 0;
    while i < history.len() {
        if history[i].role == "assistant" && history[i].tool_calls.is_some() {
            let mut j = i + 1;
            while j < history.len() && history[j].role == "tool" {
                j += 1;
            }
            if !is_superseded_chapter_write(&history[i], snap, current) {
                out.extend_from_slice(&history[i..j]);
            }
            i = j;
        } else {
            out.push(history[i].clone());
            i += 1;
        }
    }
    out
}

fn is_superseded_chapter_write(
    msg: &LlmMessage,
    snap: &StageSnapshot,
    current: Option<&str>,
) -> bool {
    let Some(calls) = msg.tool_calls.as_ref().filter(|c| !c.is_empty()) else {
        return false;
    };
    calls.iter().all(|tc| {
        written_chapter_id(&tc.function.name, &tc.function.arguments)
            .is_some_and(|id| Some(id.as_str()) != current && snap.written.iter().any(|w| w == &id))
    })
}

// ---------- 上下文预算 ----------
// 目标只有一个：**绝不把超窗口的请求发出去换 400**：每次请求前估一次，到水位就只丢"丢了
// 不损失能力"的东西（判据 = 能不能重取），丢完还超就由调用方**主动报错**。
// 不设"目标水位"数字：按"无用程度"逐级丢、够用就停（多数停在第 ① 级）。

/// 单次请求给模型**输出**留的余量上限。DeepSeek 的 `max_output_tokens` 是 393,216，但一章 YAML 远用不到。
/// **它是上限，不是定值**（见 [`input_cap`]）：小窗口按定值留会把硬线压到窗口的 1/3，
/// 正常会话也会被折、被拒；实际预留取 `min(128K, 窗口/8)`。
pub const OUTPUT_RESERVE_TOKENS: usize = 128 * 1024;

/// 窗口读不到时的兜底：DeepSeek 官方文档与 `/models` 都写 1,048,576（1M）。取最大值而非
/// 保守小值：宁可少收束也不丢记忆，真超限时由第 ③ 级的主动报错兜住（[`BudgetOutcome::TooLong`]）。
pub const DEFAULT_CONTEXT_WINDOW: usize = 1_048_576;

const TRIGGER_PERCENT: usize = 80;

const MIN_FOLD_CHARS: usize = 1200;

/// 最近几轮原文保留（正在进行的事，不能压）。
const KEEP_RECENT_TURNS: usize = 2;

/// 估算 token：CJK 按 0.85 token/字，其余按 0.3；每条消息再加 4 的结构开销。
/// **故意估高**：估高只是早一点收束（无害），估低就会撞窗口（致命）。真机 10 个会话按
/// CJK=1.0 校准过（估算高出实际 6%~27%，真实约 0.7 token/字），取 0.85 留约 10% 余量。
pub fn estimate_tokens(messages: &[LlmMessage]) -> usize {
    messages.iter().map(estimate_message_tokens).sum()
}

fn estimate_message_tokens(msg: &LlmMessage) -> usize {
    let mut wide = 0usize;
    let mut narrow = 0usize;
    let mut count = |text: &str| {
        for ch in text.chars() {
            if is_wide_char(ch) {
                wide += 1;
            } else {
                narrow += 1;
            }
        }
    };
    count(&msg.content);
    if let Some(calls) = msg.tool_calls.as_deref() {
        for call in calls {
            // 工具参数也要算：整章 YAML 就在 arguments 里
            count(&call.function.name);
            count(&call.function.arguments);
        }
    }
    wide * 85 / 100 + narrow * 3 / 10 + 4
}

fn is_wide_char(ch: char) -> bool {
    matches!(ch as u32, 0x2E80..=0x9FFF | 0xAC00..=0xD7AF | 0xF900..=0xFAFF | 0xFE30..=0xFE4F | 0xFF00..=0xFFEF)
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BudgetOutcome {
    Untouched {
        tokens: usize,
    },
    Trimmed {
        before: usize,
        after: usize,
        folded_reads: usize,
        digested_turns: usize,
    },
    /// 丢到不能再丢仍然超硬线 —— **调用方必须主动报错，不许发请求**。
    TooLong {
        tokens: usize,
        cap: usize,
    },
}

impl BudgetOutcome {
    pub fn note(&self) -> Option<String> {
        match self {
            BudgetOutcome::Untouched { .. } => None,
            BudgetOutcome::Trimmed {
                before,
                after,
                folded_reads,
                digested_turns,
            } => {
                let mut did: Vec<String> = Vec::new();
                if *folded_reads > 0 {
                    did.push(format!("折叠 {folded_reads} 条历史只读结果"));
                }
                if *digested_turns > 0 {
                    did.push(format!("把 {digested_turns} 轮更早的对话压成摘要"));
                }
                Some(format!(
                    "上下文偏长（约 {before} → {after} token），已{}（你的原话与最近几轮都完整保留）",
                    did.join("、")
                ))
            },
            BudgetOutcome::TooLong { tokens, cap } => Some(format!(
                "这个对话的上下文满了（约 {tokens} token，上限 {cap}）：已无法再自动压缩。\
                 请**新开一个对话**继续 —— 剧本文件都在磁盘上，新会话里照样能接着改。"
            )),
        }
    }
}

/// 按窗口收束一次，到水位才动手；丢不下去返回 [`BudgetOutcome::TooLong`]。顺序（先丢最不可惜的）：
/// ① 旧的只读工具结果折成一行 —— 随时可重取；② 更早的轮次压成一行摘要 —— user 原话保留原文。
pub fn apply_context_budget(messages: &mut Vec<LlmMessage>, window: usize) -> BudgetOutcome {
    let cap = input_cap(window);
    let trigger = cap * TRIGGER_PERCENT / 100;

    let before = estimate_tokens(messages);
    if before < trigger {
        return BudgetOutcome::Untouched { tokens: before };
    }

    // 最近两轮是"正在进行的事"，整段保护起来
    let protected_from = recent_turns_start(messages, KEEP_RECENT_TURNS);

    let folded_reads = fold_old_readonly_results(messages, protected_from);
    let after_fold = estimate_tokens(messages);
    if after_fold <= cap {
        // 没找到可丢的、也没超硬线 → 原样发；水位只是"开始找可丢的东西"的线，不是命令。
        return if folded_reads == 0 {
            BudgetOutcome::Untouched { tokens: before }
        } else {
            BudgetOutcome::Trimmed {
                before,
                after: after_fold,
                folded_reads,
                digested_turns: 0,
            }
        };
    }

    let digested_turns = digest_older_turns(messages, protected_from);
    let after_digest = estimate_tokens(messages);
    if after_digest <= cap {
        return BudgetOutcome::Trimmed {
            before,
            after: after_digest,
            folded_reads,
            digested_turns,
        };
    }

    BudgetOutcome::TooLong {
        tokens: after_digest,
        cap,
    }
}

pub fn budget_allows_send(outcome: &BudgetOutcome) -> bool {
    !matches!(outcome, BudgetOutcome::TooLong { .. })
}

/// 输入硬线：窗口减去输出余量，预留取 `min(128K, 窗口/8)`，**必须随窗口缩小**（定值会把 200k 压到 72k）。
fn input_cap(window: usize) -> usize {
    let reserve = OUTPUT_RESERVE_TOKENS.min(window / 8);
    window.saturating_sub(reserve).max(1)
}

/// 从末尾往前数 `keep` 个"轮"的起点下标；第 0 条（system）与第一个 user 之前的内容永远在保护区。
pub(crate) fn recent_turns_start(messages: &[LlmMessage], keep: usize) -> usize {
    let mut seen = 0usize;
    for (i, m) in messages.iter().enumerate().rev() {
        if m.role == "user" {
            seen += 1;
            if seen == keep {
                return i;
            }
        }
    }
    0
}

pub(crate) fn turn_ranges(messages: &[LlmMessage]) -> Vec<(usize, usize)> {
    let starts: Vec<usize> = messages
        .iter()
        .enumerate()
        .filter(|(i, m)| *i > 0 && m.role == "user")
        .map(|(i, _)| i)
        .collect();
    starts
        .iter()
        .enumerate()
        .map(|(n, &s)| (s, starts.get(n + 1).copied().unwrap_or(messages.len())))
        .collect()
}

fn call_meta(messages: &[LlmMessage]) -> std::collections::HashMap<String, (String, String)> {
    let mut out = std::collections::HashMap::new();
    for m in messages {
        let Some(calls) = m.tool_calls.as_deref() else {
            continue;
        };
        for call in calls {
            out.insert(
                call.id.clone(),
                (
                    call.function.name.clone(),
                    arg_path(&call.function.arguments),
                ),
            );
        }
    }
    out
}

fn arg_path(arguments: &str) -> String {
    serde_json::from_str::<serde_json::Value>(arguments)
        .ok()
        .and_then(|v| v.get("path").and_then(|p| p.as_str()).map(str::to_string))
        .unwrap_or_default()
}

/// 只读、且**随时可以再读一次**的工具 —— 结果丢了不损失能力。
fn is_rereadable_tool(name: &str) -> bool {
    matches!(name, "read_file" | "list_files")
}

fn fold_line(name: &str, path: &str, chars: usize) -> String {
    let what = if path.is_empty() {
        String::new()
    } else {
        format!(" {path}")
    };
    format!("[已折叠] {name}{what}（原 {chars} 字符）。要看细节就再 {name} 一次。")
}

/// ① 把保护区之外的旧只读结果折成一行。只换 `content`，**不删消息** ——
/// `assistant(tool_calls)` 与它的 `tool` 回应必须成对，删了就 400。
fn fold_old_readonly_results(messages: &mut [LlmMessage], protected_from: usize) -> usize {
    let meta = call_meta(messages);
    let mut folded = 0usize;
    for (i, msg) in messages.iter_mut().enumerate() {
        if i >= protected_from || msg.role != "tool" {
            continue;
        }
        let Some(id) = msg.tool_call_id.as_deref() else {
            continue;
        };
        let Some((name, path)) = meta.get(id) else {
            continue;
        };
        if !is_rereadable_tool(name) {
            continue;
        }
        let chars = msg.content.chars().count();
        if chars < MIN_FOLD_CHARS {
            continue;
        }
        msg.content = fold_line(name, path, chars);
        folded += 1;
    }
    folded
}

/// ② 把保护区之外的每一轮压成一行摘要：**user 原话保留原文**，其余合并成一条 assistant 摘要。
fn digest_older_turns(messages: &mut Vec<LlmMessage>, protected_from: usize) -> usize {
    let ranges = turn_ranges(messages);
    let mut drop = vec![false; messages.len()];
    let mut digests: Vec<(usize, String)> = Vec::new();
    for (start, end) in ranges {
        if start >= protected_from || messages[start].role != "user" {
            continue;
        }
        // 只有一句 user 话、背后没有任何动作的"回合"没有可压的东西，压缩纯属噪音。
        let Some(digest) = turn_digest(&messages[start..end]) else {
            continue;
        };
        for flag in drop.iter_mut().take(end).skip(start + 1) {
            *flag = true;
        }
        digests.push((start + 1, digest));
    }
    if digests.is_empty() {
        return 0;
    }

    let count = digests.len();
    let mut out = Vec::with_capacity(messages.len());
    for (i, msg) in messages.drain(..).enumerate() {
        if !drop[i] {
            out.push(msg);
        }
        if let Some((_, text)) = digests.iter().find(|(at, _)| *at == i) {
            out.push(LlmMessage::assistant(text.clone()));
        }
    }
    *messages = out;
    count
}

/// 一轮的摘要：用过哪些工具（带路径）+ 最后的结论开头；无动作无结论的回合返回 `None`。
fn turn_digest(turn: &[LlmMessage]) -> Option<String> {
    let mut used: Vec<String> = Vec::new();
    for msg in turn {
        let Some(calls) = msg.tool_calls.as_deref() else {
            continue;
        };
        for call in calls {
            let path = arg_path(&call.function.arguments);
            used.push(if path.is_empty() {
                call.function.name.clone()
            } else {
                format!("{}({path})", call.function.name)
            });
        }
    }
    let tail: String = turn
        .iter()
        .rev()
        .find(|m| m.role == "assistant" && m.tool_calls.is_none())
        .map(|m| m.content.chars().take(60).collect())
        .unwrap_or_default();
    if used.is_empty() && tail.trim().is_empty() {
        return None;
    }
    let mut out = String::from("[摘要] ");
    if used.is_empty() {
        out.push_str("这一轮没有调用工具。");
    } else {
        out.push_str(&format!("这一轮用过：{}。", used.join("、")));
    }
    if !tail.trim().is_empty() {
        out.push_str(&format!("结论：{}…", tail.trim()));
    }
    Some(out)
}

pub(crate) fn chapter_id_of_path(path: &str) -> Option<String> {
    let normalized = path.replace('\\', "/");
    let tail = normalized.split("/Chapters/").nth(1)?;
    let id = tail
        .strip_suffix(".yaml")
        .or_else(|| tail.strip_suffix(".yml"))?;
    (!id.is_empty()).then(|| id.to_string())
}

pub fn script_key_of_story_config(path: &str) -> Option<String> {
    let normalized = path.replace('\\', "/");
    let (_, tail) = normalized.rsplit_once("/scripts/")?;
    let key = tail.strip_suffix("/story_config.yaml")?.trim_matches('/');
    (!key.is_empty()).then(|| key.to_string())
}

/// 从剧本包内任意写入路径反推 key；key 层级不固定，按已知剧本包列表最长匹配来认。
pub(crate) fn script_key_of_script_path(path: &str, keys: &[String]) -> Option<String> {
    let normalized = path.replace('\\', "/");
    let mut keys: Vec<&String> = keys.iter().collect();
    keys.sort_by_key(|k| std::cmp::Reverse(k.len()));
    keys.into_iter()
        .find(|k| normalized.contains(&format!("/scripts/{}/", k.trim_matches('/'))))
        .cloned()
}

/// 从剧本包内任意写入路径反推 key —— **靠三类锚点**，不要求那个包已被枚举到。
/// 包根 = `.agent/`、`Chapters/`、`story_config.yaml` 三者中任一个之前的那段路径。
/// 不能只按"已知剧本包"匹配：新建剧本的中间态"只有 `.agent/design.md`、还没建工程"认不出来
/// → 会话不绑 script_key → `derive` 退化成 Routing、整套自检失效（真机踩过）。
pub fn script_key_of_package_path(path: &str) -> Option<String> {
    let normalized = path.replace('\\', "/");
    let (_, tail) = normalized.split_once("/scripts/")?;
    for anchor in ["/.agent/", "/Chapters/"] {
        if let Some((root, _)) = tail.split_once(anchor) {
            if !root.is_empty() {
                return Some(root.to_string());
            }
        }
    }
    tail.strip_suffix("/story_config.yaml")
        .filter(|root| !root.is_empty())
        .map(str::to_string)
}

fn written_chapter_id(tool: &str, arguments: &str) -> Option<String> {
    if tool != "write_file" {
        return None;
    }
    let args: serde_json::Value = serde_json::from_str(arguments).ok()?;
    chapter_id_of_path(args.get("path")?.as_str()?)
}

/// 本会话这个剧本生效的素材模式。解析只此一处（`utils::script_modes`），**整剧校验与章节自检共用**。
fn read_asset_mode(script_dir: &Path) -> AssetMode {
    ScriptModes::read(script_dir).asset
}

/// 这个剧本用不用人物卡 —— 决定交接单里要不要带角色卡模式那一行。
/// 三种情况都算用：已声明模式、角色卡羁绊冒险、仓库里已建 `characters/`。
fn uses_character_cards(snap: &StageSnapshot, script_dir: &Path, cast: CastMode) -> bool {
    if cast != CastMode::Unspecified {
        return true;
    }
    if snap
        .script_key
        .as_deref()
        .is_some_and(|k| k.replace('\\', "/").starts_with("character/"))
    {
        return true;
    }
    std::fs::read_dir(script_dir.join("characters")).is_ok_and(|mut d| d.next().is_some())
}

#[derive(Debug, Default, PartialEq, Eq)]
pub struct ChapterCheck {
    pub errors: Vec<String>,
    pub warnings: Vec<String>,
    /// 本章引用了但磁盘上没有的素材名（先预留模式下只登记，不阻断）。
    pub missing_assets: Vec<String>,
    pub mode: AssetMode,
}

impl ChapterCheck {
    pub fn is_empty(&self) -> bool {
        self.errors.is_empty() && self.warnings.is_empty()
    }

    pub fn render(&self) -> String {
        let mut out = String::new();
        if !self.errors.is_empty() {
            out.push_str("\n\n[章节自检] 这一章还不算写完，先改这些：\n");
            for e in &self.errors {
                out.push_str(&format!("- {}\n", e));
            }
            out.push_str("改好后重新 write_file 覆盖它（不要 append）。\n");
        }
        if !self.warnings.is_empty() {
            out.push_str("\n[章节自检] 记下来即可，不阻断：\n");
            for w in &self.warnings {
                out.push_str(&format!("- {}\n", w));
            }
        }
        if !self.missing_assets.is_empty() {
            out.push_str(&format!(
                "缺的素材（{}）已经记进 .agent/assets.md，别等交付才发现。\n",
                self.missing_assets.join("、")
            ));
            out.push_str(self.mode.switch_hint());
        }
        out
    }
}

/// 章节写入后的轻量自检；不适用或无问题时返回 `None`。不做整剧本校验（未写完时的断链诊断
/// 会误导模型去补后续章节），但素材当场就能定论。`data_dir` 由调用方给，判定逻辑才测得动。
pub fn check_written_chapter(
    snap: &StageSnapshot,
    path: &str,
    data_dir: &Path,
) -> Option<ChapterCheck> {
    let (dir, id) = chapter_of_write(snap, path)?;
    check_chapter(dir, &id, data_dir)
}

/// 认出「刚写的是本会话这个剧本的哪一章」；认不出就不产生自检（数据目录也留到确认之后再取）。
fn chapter_of_write<'a>(snap: &'a StageSnapshot, path: &str) -> Option<(&'a Path, String)> {
    let dir = snap.script_dir.as_deref()?;
    let id = chapter_id_of_path(path)?;
    // 只认本会话绑定剧本自己的章节：路径可能是相对或绝对，所以用包含判断；大小写无关（Windows）。
    let normalized = path.replace('\\', "/").to_lowercase();
    let key = snap.script_key.as_deref()?;
    if !normalized.contains(&format!("{}/chapters/", key.to_lowercase())) {
        return None;
    }
    Some((dir, id))
}

fn check_chapter(dir: &Path, id: &str, data_dir: &Path) -> Option<ChapterCheck> {
    let file = chapter_file(dir, id)?;
    let value = match crate::utils::yaml_file::read_yaml_as_json(&file) {
        Ok(v) => v,
        Err(e) => {
            return Some(ChapterCheck {
                errors: vec![format!("`{}` 不是可解析的 YAML：{}", id, e)],
                ..Default::default()
            });
        },
    };

    let mut check = ChapterCheck {
        errors: chapter_shape_problems(&value),
        mode: read_asset_mode(dir),
        ..Default::default()
    };

    let findings = validate::check_chapter_assets(data_dir, dir, id, &value);
    if check.mode == AssetMode::None {
        // 零素材：不该引用任何素材，所以不必逐条报"找不到"——有引用本身就是错
        let refs = validate::chapter_media_paths(&value);
        if !refs.is_empty() {
            check.errors.push(format!(
                "素材模式是「零素材」，但这一章引用了 {} 个素材：{}。\
                 改成纯文字剧情（旁白 + 对白把画面写出来），或说一句换模式",
                refs.len(),
                refs.join("、")
            ));
        }
    } else {
        for d in findings {
            if let Some(name) = validate::missing_asset_name(&d) {
                if !check.missing_assets.iter().any(|m| m == name) {
                    check.missing_assets.push(name.to_string());
                }
            }
            let at = d.event_index.map(|i| i + 1).unwrap_or_default();
            let line = format!("第 {} 个事件 · {}", at, d.message);
            if script_modes::asset_missing_is_error(check.mode) {
                check.errors.push(line);
            } else {
                check.warnings.push(line);
            }
        }
    }
    // 设计稿说这一章谁登场而 events 里一次都没出现 → 警告：想加的角色写在设计稿里、落盘时漏了。
    let design = std::fs::read_to_string(dir.join(DESIGN_REL_PATH)).unwrap_or_default();
    if let Some(block) = extract_chapter_block(&design, id) {
        let appearing = appearing_cast(&value);
        for who in declared_cast(&block) {
            if !appearing.iter().any(|a| a.eq_ignore_ascii_case(&who)) {
                check.warnings.push(format!(
                    "设计稿说本章「{who}」登场，但这一章里没有它出场\
                     （没有任何事件的 `character` 是 {who}）"
                ));
            }
        }
    }
    if !check.missing_assets.is_empty() && check.mode == AssetMode::Unspecified {
        check.warnings.push(format!(
            "`素材模式`还没声明：这一轮先问用户「只用已有」「先预留」还是「零素材」，\
             写进 {}（格式 `- 素材模式：只用已有`）。\
             未声明时缺失只按警告算，但这正是用户最在意的那类错。",
            CONSTRAINTS_REL_PATH
        ));
    }

    (!check.is_empty()).then_some(check)
}

/// 设计稿里这一章声明登场的角色（`登场:` 那一行）；与 YAML 的 `character` 同口径，可直接比名字。
fn declared_cast(chapter_block: &str) -> Vec<String> {
    for line in chapter_block.lines() {
        let head = line.trim_start_matches(['-', '*', ' ', '\t']);
        let Some(rest) = head.strip_prefix("登场") else {
            continue;
        };
        let value = rest.trim_start_matches([':', '：']);
        return value
            .split([',', '，', '、', '/', ' '])
            .map(|s| s.trim())
            .filter(|s| !s.is_empty())
            .map(|s| s.to_string())
            .collect();
    }
    Vec::new()
}

/// 章节 YAML 里实际出现的角色：事件顶层的 `character` 字段（不按事件类型硬编码，扫键更不易漏）。
fn appearing_cast(value: &serde_json::Value) -> Vec<String> {
    let Some(events) = value.get("events").and_then(|v| v.as_array()) else {
        return Vec::new();
    };
    let mut out: Vec<String> = Vec::new();
    for ev in events {
        let Some(name) = ev.get("character").and_then(|v| v.as_str()) else {
            continue;
        };
        let name = name.trim();
        if !name.is_empty() && !out.iter().any(|c| c == name) {
            out.push(name.to_string());
        }
    }
    out
}

/// 把设计稿里的登场角色汇总成 `.agent/cast.md`；由**代码**维护，让模型抄一遍只会多一次出错机会。
pub fn write_cast(dir: &Path) -> std::io::Result<()> {
    let design = std::fs::read_to_string(dir.join(DESIGN_REL_PATH)).unwrap_or_default();
    let lines: Vec<&str> = design.lines().collect();

    let mut order: Vec<String> = Vec::new();
    let mut where_: Vec<String> = Vec::new();
    for (start, end) in block_ranges(&lines) {
        let block = &lines[start..end];
        let Some(id) = block_id(&block[1..]) else {
            continue;
        };
        for who in declared_cast(&block.join("\n")) {
            match order.iter().position(|w| w.eq_ignore_ascii_case(&who)) {
                Some(i) => {
                    if !where_[i].split(' ').any(|c| c == id) {
                        where_[i].push_str(&format!(" {id}"));
                    }
                },
                None => {
                    order.push(who);
                    where_.push(id.to_string());
                },
            }
        }
    }

    let mut out = String::from(
        "# 登场角色名单\n\n\
         由系统从设计稿各章的「登场:」汇总。`MAIN` 是主角，`玩家` 是玩家自己。\n\n",
    );
    if order.is_empty() {
        out.push_str("（设计稿还没列出登场角色）\n");
    } else {
        out.push_str("| 角色 | 登场章节 |\n| --- | --- |\n");
        for (who, chapters) in order.iter().zip(where_.iter()) {
            out.push_str(&format!("| {who} | {chapters} |\n"));
        }
    }
    std::fs::write(dir.join(CAST_REL_PATH), out)
}

/// 把本章的素材缺口并进 `.agent/assets.md`；由**代码**维护，同名素材只登记一次。
pub fn update_assets_gap(dir: &Path, chapter: &str, missing: &[String]) -> std::io::Result<()> {
    if missing.is_empty() {
        return Ok(());
    }
    let path = dir.join(ASSETS_REL_PATH);
    let old = std::fs::read_to_string(&path).unwrap_or_default();
    let mut rows: Vec<String> = old
        .lines()
        .filter(|l| l.trim_start().starts_with("-「"))
        .map(|l| l.trim().to_string())
        .collect();

    for name in missing {
        let marker = format!("-「{name}」");
        if rows.iter().any(|r| r.starts_with(&marker)) {
            continue;
        }
        rows.push(format!("-「{name}」（第 {chapter} 章引用，磁盘上没有）"));
    }

    let mut out = String::from(
        "# 素材缺口表\n\n\
         由系统按章节自检维护：写完一章发现引用了磁盘上没有的素材就登记在这里。\n\
         处理方式：补素材 / 改剧情 / 明确接受这个素材没有（三选一，问用户）。\n\n",
    );
    out.push_str(&rows.join("\n"));
    out.push('\n');
    std::fs::write(path, out)
}

/// 已落盘章节引用的磁盘上没有的素材；与当前模式无关 —— 回答"换个模式会多出/少掉哪些要改的地方"。
fn missing_assets_of_written(snap: &StageSnapshot, dir: &Path, data_dir: &Path) -> Vec<String> {
    let mut out = Vec::new();
    for id in &snap.written {
        let Some(file) = chapter_file(dir, id) else {
            continue;
        };
        let Ok(value) = crate::utils::yaml_file::read_yaml_as_json(&file) else {
            continue;
        };
        for d in validate::check_chapter_assets(data_dir, dir, id, &value) {
            if let Some(name) = validate::missing_asset_name(&d) {
                out.push(format!("{} {}", id, name));
            }
        }
    }
    out
}

fn chapter_shape_problems(value: &serde_json::Value) -> Vec<String> {
    let mut problems = Vec::new();

    if value
        .get("name")
        .and_then(|v| v.as_str())
        .unwrap_or("")
        .trim()
        .is_empty()
    {
        problems.push("缺少顶层 `name`".to_string());
    }

    let Some(events) = value.get("events").and_then(|v| v.as_array()) else {
        problems.push("缺少 `events` 列表".to_string());
        return problems;
    };
    let Some(last) = events.last() else {
        problems.push("`events` 是空的".to_string());
        return problems;
    };

    if last.get("type").and_then(|v| v.as_str()) != Some("chapter_end") {
        problems.push(format!(
            "最后一个事件是 `{}`，必须以 `chapter_end` 收尾",
            last.get("type")
                .and_then(|v| v.as_str())
                .unwrap_or("(缺失)")
        ));
        return problems;
    }

    // 引擎优先 next，其次 next_chapter；end_type 缺省即 linear
    let end_type = last
        .get("end_type")
        .and_then(|v| v.as_str())
        .unwrap_or("linear");
    if end_type == "linear"
        && last.get("next").and_then(|v| v.as_str()).is_none()
        && last.get("next_chapter").and_then(|v| v.as_str()).is_none()
    {
        problems.push(
            "`chapter_end` 是 linear 却没给 `next_chapter` / `next`（结尾写 \"end\"）".to_string(),
        );
    }

    problems
}
