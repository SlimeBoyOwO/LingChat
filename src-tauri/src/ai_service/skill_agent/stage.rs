//! 阶段推导与按阶段上下文配置。
//!
//! 阶段由文件系统事实推导，不落库、不强制流程。
//! 本模块只装配上下文：运行中唯一阻塞等待用户的确认是命令审批，它由工具入口触发，
//! 与阶段无关；这里的「确认门」全部只是提示词。

use std::path::{Path, PathBuf};

use crate::ai_service::types::LlmMessage;
use crate::api::script_editor::validate;
use crate::utils::script_modes::{self, AssetMode, CastMode, ScriptModes};
use crate::utils::script_paths;

/// 流程产物所在目录。点号目录不会被引擎扫描，也不进编辑器枚举；
/// 助手面板的「详情」浮窗直接列这一层，所以加新产物不用改前端。
pub const AGENT_DIR: &str = ".agent";

/// 设计稿在剧本包内的相对路径。
pub const DESIGN_REL_PATH: &str = ".agent/design.md";

/// 章节细节稿。用户在大纲之后补的细节落在这里，**按章分节**。
///
/// 之前没有这个名字：模型自己发明了一个文件来放细节（真机在 `chapter-details.md`
/// 里写了 12KB），系统不知道它存在、交接单也不提，于是"细节改过、设计稿没改"没人发现。
/// 现在它是正式产物：细节写这里，并且**同一处细节改完要顺手把设计稿那一节的梗概对齐**。
pub const CHAPTER_DETAILS_REL_PATH: &str = ".agent/chapter-details.md";

/// 用户约束卡片。剧本包内的相对路径（解析在 `utils::script_modes`，只此一处）。
pub use crate::utils::script_modes::CONSTRAINTS_REL_PATH;

/// 任务队列。这一版只读不写：写它的是流程 Agent（尚未实现）。
pub const QUEUE_REL_PATH: &str = ".agent/queue.md";

/// 素材缺口表。**由代码维护**（见 [`update_assets_gap`]）。
pub const ASSETS_REL_PATH: &str = ".agent/assets.md";

/// 登场角色名单。**由代码从设计稿汇总**（见 [`write_cast`]）。
pub const CAST_REL_PATH: &str = ".agent/cast.md";

/// 自动骨架的标记行。`progress_block` 靠它判断"这份配置还没被补全"。
pub const SKELETON_MARKER: &str = "本文件由系统自动创建";

/// 确保剧本包有个**能加载**的骨架：缺 `story_config.yaml` 就补一份最小的，缺 `Chapters/` 就建。
///
/// 包一诞生就该能打开。新建剧本的正常路径是"先写 `.agent/design.md`、之后再建工程"，
/// 而编辑器是按 `story_config.yaml` 认剧本的 —— 少了它，剧本列表里看不到、
/// 打开报「读取剧本失败」，引擎也把它算作无效目录跳过。真机踩过。
///
/// 只补最小骨架、不当真：`script_name` 取文件夹名（引擎要求两者一致）；
/// 目录布局是 `character/<角色>/<剧本>` 时顺带写上冒险块；其余留给工程创建阶段补全。
/// **已存在就一个字都不动。**
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
    // 布局从 key 就能看出来：三层且首段是 character → 角色卡羁绊冒险
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

// 相对技能目录（`data/game_data/skills`）的材料路径。
// 角色手册的路径由 `role.rs` 拥有，这里只引用，免得同一份路径写两处。
const HUB_DOC: &str = "lingchat-script-editor/SKILL.md";
const WRITER_DOC: &str = super::role::Role::Writer.doc();
const TRANSFORMER_DOC: &str = super::role::Role::Transformer.doc();
const OPTIMIZER_DOC: &str = super::role::Role::Optimizer.doc();
const EVENT_REFERENCE: &str = "lingchat-script-editor/references/event-reference.md";
const CHAPTER_TEMPLATE: &str = "lingchat-script-editor/assets/templates/chapter_template.yaml";

/// 创作阶段。按「上下文配置是否相同」划分，S1–S3 合并为 [`Stage::Setup`]。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Stage {
    /// 未绑定剧本。
    #[default]
    Routing,
    /// 无剧本包，或设计稿尚未列出章节。
    Setup,
    /// 设计稿已声明章节，但尚未写完。
    Forge,
    /// 计划章节齐备。
    Polish,
    /// 有剧本包、无设计稿、且已有章节：修改既有剧本。
    Modify,
}

impl Stage {
    /// 注入系统提示的单行标识。
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

/// 阶段推导结果，以及推导所依据的事实。
#[derive(Debug, Clone, Default)]
pub struct StageSnapshot {
    pub stage: Stage,
    /// 会话绑定的剧本 key（如 `standalone/我的剧本`、`character/角色/剧本`）。
    pub script_key: Option<String>,
    pub script_dir: Option<PathBuf>,
    /// 设计稿声明的章节 id。
    pub plan: Vec<String>,
    /// 磁盘上已有的章节 id。
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

    /// 设计稿中最后一个已落盘的章节 id。
    pub fn last_written(&self) -> Option<&str> {
        self.plan
            .iter()
            .map(String::as_str)
            .rfind(|id| self.written.iter().any(|w| w == id))
    }
}

/// 推导当前阶段。
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

/// 一个阶段的上下文配置。
pub struct StageProfile {
    /// 思考模式覆盖；`None` 表示不干预，沿用 provider 默认。
    pub thinking: Option<bool>,
    /// 稳定材料：相对技能目录的路径，注入系统提示。
    pub system_materials: &'static [&'static str],
    /// 本阶段的行为要求（粒度等），注入在材料之前。
    pub directive: &'static str,
}

// 各阶段的行为要求。
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

/// 取阶段配置。会落到剧本 YAML 的阶段都带 `TRANSFORMER_DOC`（事件字段与 YAML 规范）。
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

/// 读取阶段材料，冠以「角色指令」来源头（区别于看起来像附件的文件转储）。
fn load_system_materials(skills_dir: &Path, stage: Stage) -> String {
    let mut out = String::new();
    for rel in profile(stage).system_materials {
        out.push_str(&load_one_material(skills_dir, rel));
    }
    out
}

/// 读一份手册，冠以「角色指令」来源头。
///
/// 读不到必须让模型看见：只打日志的话，它会凭记忆补规范且无人知情。
fn load_one_material(skills_dir: &Path, rel: &str) -> String {
    let mut out = String::new();
    let path = skills_dir.join(rel);
    match std::fs::read_to_string(&path) {
        Ok(text) => {
            out.push_str(&format!("\n\n【角色指令 · {}】\n", rel));
            out.push_str(text.trim_end());
        },
        // 缺失必须让模型看见：只打日志的话，它会凭记忆补规范且无人知情
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

/// 组装任务块的输入。参数多到一个程度就没人看得懂了，收成一个结构。
pub struct TaskBlockInput<'a> {
    pub task: super::role::TaskKind,
    pub handoff: super::role::Handoff,
    pub plan: &'a super::router::RoutePlan,
    pub user_msg: &'a str,
    pub skills_dir: &'a Path,
    /// 这一轮落不落盘（用户明说了才 true）。
    pub land: bool,
    /// 正在执行队列里的第几项（0 起）。
    pub item_index: usize,
    /// 队列里被跳过的项（做不了），让模型在回执里说明。
    pub skipped: &'a [String],
}

/// 本轮的注入块：任务 + 行为要求 + 职责边界 + 承接说明 + **该任务要的那几本手册**。
///
/// 按任务注入（而不是按阶段）是这次改造的要点：实录显示同一阶段里
/// 「只提问的那一轮」并不需要落盘手册（R1 注入了 6363 字符、0 次写文件）。
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

    // 用户这一轮的原话。任务名是从这句话里判出来的，"他到底要什么"得看他怎么说的 ——
    // 一句话里顺手捎带的别的事，光看任务名会漏掉。
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

    // 听用户的：**能力范围内就按他说的来**。推荐写法只是默认，不是死规矩。
    // "能力范围" = 这一轮能用的工具 + 手上这本手册里的手艺（判准与手册里那一节同源）。
    out.push_str(&format!(
        "\n【听用户的】你这一轮能用的工具是 {}。用户要的事只要这些工具加这本手册做得到，\
         就按他说的做 —— 下面那些只是**推荐写法**，用户有别的写法就以他的为准\
         （他说一次写三章就写三章；他说顺手把某章改掉，而你有 write_file，就改）。\
         做不到的（这一轮没那个手段）不要硬编，直说做不到，并告诉他该在什么时候做。",
        task.tools().join(" / ")
    ));

    // 队列是这一轮的**工作清单**：用户提了三件事就得三件都做完再收尾。
    // 只给一行"还剩几项"不够 —— 模型会做完手上这件就收尾。
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

    // 任务自带的产出物说明 + 流程 Agent 给的补充，两条都留下
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

/// 把本轮的队列落盘成 `.agent/queue.md`。
///
/// **勾选只依据能被代码核对的事实**：`write_chapter` 的 target 是章节 id、
/// 而那一章已经落盘 → 算做完。别的类型（改章节、盘点、校验）**不打勾** ——
/// "那一章早就在磁盘上"不等于"这一轮改完了它"，按磁盘事实打勾会当场误判成做完。
/// 队列是给人看的事实，用户可以随时手改；它不落库。
/// 队列落盘。
///
/// **是合并，不是覆盖**：真机上有一轮用户只是闲聊，路由判成「仅对话」，
/// 队列文件被整份重写成"仅对话"那一项 —— 上一轮排着的「编写章节：07」当场消失，
/// 模型下一轮自己发现"队列自相矛盾"。
/// 所以：旧文件里**没勾的项一律留着**（这正是"还没做完"的记忆），
/// 本轮新项追加在后面；同名同对象的项不重复添加。
pub fn write_queue(
    dir: &Path,
    items: &[super::router::QueueItem],
    written: &[String],
) -> std::io::Result<()> {
    let mut entries: Vec<(bool, String)> = read_queue_entries(dir);
    for item in items {
        let text = queue_text(item);
        // 「仅对话」没有待办可言，别往队列里塞
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

/// 队列文件里一行 → `(是否已勾, 条目文本)`。
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

/// 队列里一项怎么写。与 [`read_queue_progress`] 的解析对齐。
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

/// 流程总纲全文。稳定部分，流程 Agent 每次都要用它来判断流程走到哪了。
///
/// 读不到就返回 `None` —— 调用方应当**回落**而不是凭记忆补规则。
pub fn hub_doc(skills_dir: &Path) -> Option<String> {
    std::fs::read_to_string(skills_dir.join(HUB_DOC))
        .ok()
        .map(|t| t.trim_end().to_string())
}

/// 承接判断要用的事实。与 [`StageSnapshot`] 同源，但保留阶段投影会丢掉的细节。
///
/// `target` 是用户点名的章节 id；`target_exists` 只认**已落盘**的
/// （唯一看它的是「改章节」，改不到还没写的东西）。
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
    // 「文件在」与「读得出章节」是两件事：前者决定能不能改它，
    // 后者只是"格式对不对"的提醒 —— 别把后者当前置用。
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

/// 用户点名的章节 id 列表。
///
/// **区间与列表要拆开**：真机上模型把 target 写成 `01-04`，代码按"单个章节 id"去查，
/// 查不到 → 判"你点名的那一章还没有落盘" → 整轮工具被收成只读 → 队列里其余项全废。
/// 这里顺手把 `第 3 章` 这种口语写法也归一成 `3`。
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

/// 组装本轮动态材料（待写章节 + 上一章收尾状态 + 落盘进度）；不落库，每轮重算。
///
/// `data_dir` 由调用方给，理由同 [`check_written_chapter`]。
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

/// 交接单里的进度事实：素材模式 + 已落盘清单 + 素材缺口影响面 + 下一章是否已有内容 + 队列剩余。
///
/// 「下一章是否已有内容」补的是删章那一脚：阶段按文件事实推，代码只能说"还没写"，
/// 而磁盘上那一章可能真有内容，模型得知道写下去会覆盖什么。
/// `data_dir` 由调用方给（判素材要用它），这样这条逻辑离开全局静态也能测。
fn progress_block(snap: &StageSnapshot, dir: &Path, data_dir: &Path) -> String {
    let modes = ScriptModes::read(dir);
    let mut lines: Vec<String> = vec![format!(
        "素材模式：{}（`.agent/constraints.md` 是唯一来源；要改就改那一行，并检查别处有没有旧说法）",
        modes.asset.describe()
    )];
    // 不用人物卡的剧本不必被这一行占地方
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
    // 有设计稿就照实报"读出了什么"；没有但已经有章节（既存剧本）也提一句，
    // 全新的空剧本不提 —— 那时候还没到设计这一步。
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

/// 已落盘的素材缺口在**当前模式下**意味着什么 —— 用户改主意时的影响面。
///
/// 这一行是给"改主意"准备的：换了模式，已经写好的章节不会自动重写，
/// 所以得先把"会多出／少掉哪些要改的地方"摊开。
fn asset_impact_line(mode: AssetMode, missing: &[String]) -> String {
    /// 列全没意义，模型和用户都只需要知道大概规模与前几个。
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

/// 用户声明的模式在卡片里自相矛盾时的那一句提示（读不到卡片就没有）。
fn constraint_conflict(dir: &Path) -> Option<String> {
    let text = std::fs::read_to_string(dir.join(CONSTRAINTS_REL_PATH)).ok()?;
    script_modes::conflict_note(&text)
}

/// 设计稿那一行：告诉模型代码**从设计稿里读出了什么**。
///
/// 「设计稿在、但一个章节 id 都读不出来」是最坑的一种：代码这边算不出"下一章"，
/// 模型却以为清单已经列好了，于是要么接着往下写、要么反复问用户"章节哪去了"。
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

/// 剧本配置那一行：让模型知道现在是"还没建"还是"只有系统骨架、等你补全"。
///
/// 不这么做的话，自动骨架会让模型以为工程已经建好了，就不去补 description /
/// 触发方式 / 成就这些真正要按剧本类型决定的东西。
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

/// 设计稿里排在「本章」之后的那一章。
fn chapter_after(snap: &StageSnapshot) -> Option<String> {
    let current = snap.next_chapter()?;
    let idx = snap.plan.iter().position(|p| p == current)?;
    snap.plan.get(idx + 1).cloned()
}

/// 队列进度（`.agent/queue.md` 的 `- [ ]` / `- [x]`）。文件不存在返回 `None`。
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
        // 队列是"还欠着的事"，不是"这一轮的事"。用户不要了就得有人把它去掉，
        // 否则它永远挂着、每轮都在催。
        out.push_str("。没勾的是还欠着的：用户说了不用做、或者已经改口做别的，就把它从队列文件里删掉并说一声");
    }
    Some(out)
}

fn read_plan(script_dir: &Path) -> Vec<String> {
    std::fs::read_to_string(script_dir.join(DESIGN_REL_PATH))
        .map(|t| parse_plan(&t))
        .unwrap_or_default()
}

/// 解析设计稿的章节列表：`##` 块内第一个非空行为 `id:` 才算章节。
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

/// 取出设计稿中 `id:` 等于 `id` 的块（含标题行，到下一个 `##` 为止）。
fn extract_chapter_block(markdown: &str, id: &str) -> Option<String> {
    let lines: Vec<&str> = markdown.lines().collect();
    for (start, end) in block_ranges(&lines) {
        if block_id(&lines[start + 1..end]) == Some(id) {
            return Some(lines[start..end].join("\n").trim_end().to_string());
        }
    }
    None
}

/// `##` 标题切出的块的行区间（区间从标题行开始，到下一个标题行之前）。
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

/// 块内第一个非空行若为 `id:` 行，返回其值。
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

/// 章节 id → 磁盘文件。布局与合法性交给 `script_paths`，避免两处各写一遍规则。
fn chapter_file(script_dir: &Path, id: &str) -> Option<PathBuf> {
    script_paths::resolve_chapter_file(script_dir, id, false).ok()
}

/// 丢弃已被超越的章节写入轮次，避免历史里堆积整章 YAML。
///
/// 整轮丢弃（含 tool 回应）以保持历史结构合法；只改内存，DB 不动。
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
//
// 目标只有一个：**绝不把超窗口的请求发出去换 400**。做法是每次请求前估一次，
// 到水位就只丢"丢了不损失能力"的东西（能不能重取 = 判据），丢完还超才继续降级，
// 一路降不下去就由调用方**主动报错**，而不是让 provider 拒。
//
// 不设"目标水位"：不写"压到 40k"这类数字。触发后按"无用程度"逐级丢，
// 够用就停 —— 绝大多数情况停在第 ① 级（旧只读结果），因为它通常占体量的一半以上。

/// 单次请求给模型**输出**留的余量（上限）。
///
/// DeepSeek 的 `max_output_tokens` 是 393,216，但一章 YAML 加思考链远用不到；
/// 128K 已经很宽裕（窗口的 12.5%）。
///
/// **但它是上限，不是定值**：见 [`input_cap`] —— 小窗口上按定值留会把输入上限
/// 压得极小（200k 窗口留 128K → 硬线只剩 72k，正常会话也会被折、被拒），
/// 所以实际预留取 `min(128K, 窗口/8)`。
pub const OUTPUT_RESERVE_TOKENS: usize = 128 * 1024;

/// 窗口读不到时的兜底：DeepSeek 官方文档与 `/models` 都写 1,048,576（1M）。
///
/// 之所以用"最大值"而不是保守的小值：本项目的默认 provider 就是 DeepSeek，
/// 而宁可少收束也不要在正常会话里丢记忆 —— 真有超限风险时，第 ③ 级之后的
/// 主动报错会兜住（见 [`BudgetOutcome::TooLong`]）。
pub const DEFAULT_CONTEXT_WINDOW: usize = 1_048_576;

/// 收束水位：可用输入的 80%。低于它一律原样发。
const TRIGGER_PERCENT: usize = 80;

/// 值得折的只读结果下限（字符）。小结果折了省不下什么，还多一行噪音。
const MIN_FOLD_CHARS: usize = 1200;

/// 最近几轮原文保留（正在进行的事，不能压）。
const KEEP_RECENT_TURNS: usize = 2;

/// 估算 token：CJK 按 0.85 token/字，其余按 0.3；每条消息再加 4 的结构开销。
///
/// **故意估高**：估高只是早一点收束（无害），估低就会撞窗口（致命）。
///
/// 校准数据（真机 10 个会话，估算 vs 实际 `prompt_tokens`，两者都含系统提示）：
/// 按 CJK=1.0 估时，估算高出实际 6%~27%（会话 #7 232,542 vs 183,744）。
/// 也就是真实 CJK 约 0.7 token/字 —— 这里取 0.85，留约 10% 的保守余量，
/// 而不是按 1.0 让它在真实占用约 60% 时就动手（那会过早丢记忆）。
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

/// 宽字符（中日韩、全角标点）—— 这些基本一个字符一个 token。
fn is_wide_char(ch: char) -> bool {
    matches!(ch as u32, 0x2E80..=0x9FFF | 0xAC00..=0xD7AF | 0xF900..=0xFAFF | 0xFE30..=0xFE4F | 0xFF00..=0xFFEF)
}

/// 一次收束做了什么。调用方据此决定要不要告诉用户。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BudgetOutcome {
    /// 没到水位，原样发。
    Untouched { tokens: usize },
    /// 丢掉了一些"可重取"的内容，仍在硬线内。
    Trimmed {
        before: usize,
        after: usize,
        folded_reads: usize,
        digested_turns: usize,
    },
    /// 丢到不能再丢仍然超硬线 —— **调用方必须主动报错，不许发请求**。
    TooLong { tokens: usize, cap: usize },
}

impl BudgetOutcome {
    /// 给用户看的一句话（`None` 表示不必打扰）。
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

/// 按窗口收束一次。到水位才动手；丢不下去就返回 [`BudgetOutcome::TooLong`]。
///
/// 顺序（先丢最不可惜的）：
/// ① 旧的只读工具结果（`read_file` / `list_files`）折成一行 —— 随时可重取；
/// ② 更早的轮次整体压成一行摘要 —— 用户的原话仍保留原文。
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
        // 没找到可丢的东西、也没超硬线 → 原样发，**别拿"折了 0 条"去打扰用户**。
        // 水位只是"开始找可丢的东西"的线，不是"必须丢点什么"的命令。
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

/// 请求前要不要拦（`TooLong` 一律不许发；其余照发）。
pub fn budget_allows_send(outcome: &BudgetOutcome) -> bool {
    !matches!(outcome, BudgetOutcome::TooLong { .. })
}

/// 输入硬线：窗口减去给输出留的余量。
///
/// 预留取 `min(128K, 窗口/8)` —— **必须随窗口缩小**：定值 128K 在小窗口上会
/// 把硬线压到窗口的三分之一（200k → 只剩 72k），于是"以前完全正常的会话"
/// 也会被折叠甚至被拒，那是自己造出来的故障。
fn input_cap(window: usize) -> usize {
    let reserve = OUTPUT_RESERVE_TOKENS.min(window / 8);
    window.saturating_sub(reserve).max(1)
}

/// 从末尾往前数 `keep` 个"轮"（user 消息开头）的起点下标。
///
/// 第 0 条（system）与第一个 user 之前的内容永远在保护区内。
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

/// 轮的范围（每轮从一条 user 开始，到下一个 user 之前）。第 0 条 system 不属于任何轮。
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

/// 工具调用 id → (工具名, 参数里的 path)。用来认出"这条 tool 结果是哪个工具产生的"。
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

/// 从工具参数的 JSON 里取 `path`（取不到就空串）。
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

/// ② 把保护区之外的每一轮压成一行摘要：**该轮的 user 原话保留原文**，其余消息
/// 合并成一条 assistant 摘要（写了哪个文件、用过哪些工具、最后说了什么）。
fn digest_older_turns(messages: &mut Vec<LlmMessage>, protected_from: usize) -> usize {
    let ranges = turn_ranges(messages);
    let mut drop = vec![false; messages.len()];
    let mut digests: Vec<(usize, String)> = Vec::new();
    for (start, end) in ranges {
        if start >= protected_from || messages[start].role != "user" {
            continue;
        }
        // 只有一句 user 话、背后没有任何动作的"回合"（队列各项的交接材料就是这样）
        // 没有可压的东西 —— 给它插一条「这一轮没有调用工具」纯属噪音。
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

/// 一轮的摘要：用过哪些工具（带路径）+ 最后的结论开头。
///
/// 没有任何动作、也没有结论的回合（只有一句 user 话）返回 `None` —— 那种回合
/// 没有可压的内容，压缩它只会平白插一条噪音。
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

/// 从写入路径里取出章节 id（`…/Chapters/<id>.yaml`，允许 `\` 分隔与子目录）。
pub(crate) fn chapter_id_of_path(path: &str) -> Option<String> {
    let normalized = path.replace('\\', "/");
    let tail = normalized.split("/Chapters/").nth(1)?;
    let id = tail
        .strip_suffix(".yaml")
        .or_else(|| tail.strip_suffix(".yml"))?;
    (!id.is_empty()).then(|| id.to_string())
}

/// 从写入路径反推剧本 key：`…/scripts/<key…>/story_config.yaml`。
pub fn script_key_of_story_config(path: &str) -> Option<String> {
    let normalized = path.replace('\\', "/");
    let (_, tail) = normalized.rsplit_once("/scripts/")?;
    let key = tail.strip_suffix("/story_config.yaml")?.trim_matches('/');
    (!key.is_empty()).then(|| key.to_string())
}

/// 从剧本包内任意写入路径反推 key（`…/scripts/<key…>/…`）。
///
/// key 的层级不固定（`standalone/x`、`character/角色/x`、扁平 `x`），所以拿已知的剧本包列表
/// 去匹配，而不是猜切几段；嵌套包时优先最长匹配。`keys` 由调用方传入，便于一次扫描多处复用。
pub(crate) fn script_key_of_script_path(path: &str, keys: &[String]) -> Option<String> {
    let normalized = path.replace('\\', "/");
    let mut keys: Vec<&String> = keys.iter().collect();
    keys.sort_by_key(|k| std::cmp::Reverse(k.len()));
    keys.into_iter()
        .find(|k| normalized.contains(&format!("/scripts/{}/", k.trim_matches('/'))))
        .cloned()
}

/// 从剧本包内任意写入路径反推 key —— **靠三类锚点**，不要求那个包已经被枚举到。
///
/// 包根 = `.agent/`、`Chapters/`、`story_config.yaml` 三者中任一个之前的那段路径。
///
/// 为什么不能只按"已知剧本包"匹配：`enumerate_script_keys()` 要求包里有
/// `story_config.yaml`，而**新建剧本的正常中间态是「只有 `.agent/design.md`、
/// 还没建工程」**。真机踩过这个坑：包认不出来 → 会话一直没绑定 script_key →
/// `derive` 退化成 Routing、`script_dir` 为 None → 章节自检/登场名单/缺口表/队列
/// 整套都不生效，用户喊着「落盘第一二章」而它一直在写大纲。
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

/// 若这是一次写章节文件的调用，返回章节 id。
fn written_chapter_id(tool: &str, arguments: &str) -> Option<String> {
    if tool != "write_file" {
        return None;
    }
    let args: serde_json::Value = serde_json::from_str(arguments).ok()?;
    chapter_id_of_path(args.get("path")?.as_str()?)
}

/// 本会话这个剧本当前生效的素材模式。
///
/// 解析只有一处（`utils::script_modes`），**整剧校验与章节自检共用同一份判定** ——
/// 免得两处各读一遍 `.agent/constraints.md`，一处认得出、一处认不出（真机踩过：
/// 用户允许缺口，章节自检只是警告，整剧校验却一直报错误）。
fn read_asset_mode(script_dir: &Path) -> AssetMode {
    ScriptModes::read(script_dir).asset
}

/// 这个剧本用不用人物卡 —— 决定交接单里要不要带角色卡模式那一行。
///
/// 三种情况都算用：用户已经声明过模式、是角色卡羁绊冒险、或者仓库里已经建了 `characters/`。
/// 纯独立剧本没这些，就不必被这一行占地方。
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

/// 一章的自检结果：错误必须当场改完，警告只记录。
#[derive(Debug, Default, PartialEq, Eq)]
pub struct ChapterCheck {
    pub errors: Vec<String>,
    pub warnings: Vec<String>,
    /// 本章引用了但磁盘上没有的素材名（先预留模式下只登记，不阻断）。
    pub missing_assets: Vec<String>,
    /// 本轮生效的素材模式；决定回执里怎么告诉用户"还能改"。
    pub mode: AssetMode,
}

impl ChapterCheck {
    pub fn is_empty(&self) -> bool {
        self.errors.is_empty() && self.warnings.is_empty()
    }

    /// 自检回执，按「能不能就此收工」组织。
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
            // 卡在这里的其实是用户当初选的那个模式，所以把换法的句子一并给出
            out.push_str(self.mode.switch_hint());
        }
        out
    }
}

/// 章节写入后的轻量自检；不适用或无问题时返回 `None`。
///
/// 不做整剧本校验：未写完时的断链诊断会误导模型去补后续章节。
/// 但素材是「写完这一章就能定论」的事实，所以这里当场查。
///
/// `data_dir` 由调用方给 —— 本模块不碰全局静态，判定逻辑才测得动。
pub fn check_written_chapter(
    snap: &StageSnapshot,
    path: &str,
    data_dir: &Path,
) -> Option<ChapterCheck> {
    let (dir, id) = chapter_of_write(snap, path)?;
    check_chapter(dir, &id, data_dir)
}

/// 认出「刚写的是本会话这个剧本的哪一章」；认不出就不产生自检。
///
/// 数据目录留到确认是自己剧本的章节之后才取 —— 它跟这一步无关，
/// 提前取会在没有数据目录的场合（测试）无谓地炸掉。
fn chapter_of_write<'a>(snap: &'a StageSnapshot, path: &str) -> Option<(&'a Path, String)> {
    let dir = snap.script_dir.as_deref()?;
    let id = chapter_id_of_path(path)?;
    // 只认本会话绑定剧本自己的章节。写的是相对还是绝对路径不一定，所以用包含判断；
    // 大小写无关是因为 Windows 上实际目录名可能与绑定值不同。
    let normalized = path.replace('\\', "/").to_lowercase();
    let key = snap.script_key.as_deref()?;
    if !normalized.contains(&format!("{}/chapters/", key.to_lowercase())) {
        return None;
    }
    Some((dir, id))
}

/// 判定本体：结构 + 素材；素材的松紧由 `.agent/constraints.md` 里的素材模式决定。
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
    // 设计稿说这一章谁登场，而这一章的 events 里一次都没出现 → 警告。
    // 这就是「想加的角色没加」：写在设计稿里，落盘时漏了。
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

/// 设计稿里这一章声明登场的角色（`登场:` 那一行）。
///
/// 与 YAML 里的 `character` 字段同口径（实录的剧本两边都写 `MAIN` / 角色 key），
/// 所以可以直接比名字。
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

/// 章节 YAML 里实际出现的角色：事件顶层的 `character` 字段。
///
/// 不按事件类型硬编码 —— `dialogue` / `ai_dialogue` / `free_dialogue` /
/// `modify_character` 都带这个键，直接扫键更不容易漏。
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

/// 把设计稿里的登场角色汇总成 `.agent/cast.md`。
///
/// 同缺口表一样由**代码**维护：名单是从设计稿各章的 `登场:` 直接汇总出来的事实，
/// 让模型抄一遍只会多一次出错机会。它是给人看的（浮窗里能看到），也用来回答
/// 「这个角色到底出现过没有」。
pub fn write_cast(dir: &Path) -> std::io::Result<()> {
    let design = std::fs::read_to_string(dir.join(DESIGN_REL_PATH)).unwrap_or_default();
    let lines: Vec<&str> = design.lines().collect();

    // 章节 id → 登场名单；顺便记住谁在哪几章登场
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

/// 把本章的素材缺口并进 `.agent/assets.md`。
///
/// 缺口表由**代码**维护而不是让模型抄一遍：它就是把自检已经算出来的事实写下来，
/// 抄一遍只会多一次出错机会。同名素材只登记一次（保留最早引用的那一章）。
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

/// 已落盘章节里引用了磁盘上不存在的素材，返回 `章节 id + 素材名`。///
/// 与当前模式无关 —— 它回答的是「换个模式会多出/少掉哪些要改的地方」，
/// 也就是用户改主意时的影响面。改了模式，已写好的章节不会自动重写，得摊开给他看。
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

/// 章节的硬性结构要求。
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

#[cfg(test)]
mod tests {
    use super::*;

    const DESIGN: &str = "\
# 设计稿

## 大纲
一些说明。
id: 不是章节首行

## 第1章 · 雨夜
id: 01
梗概: 相遇

## 第2章 · 分别
id: Intro/02
梗概: 告别
";

    #[test]
    fn plan_reads_only_blocks_whose_first_line_is_id() {
        assert_eq!(parse_plan(DESIGN), vec!["01", "Intro/02"]);
    }

    #[test]
    fn plan_ignores_duplicates() {
        assert_eq!(parse_plan("## A\nid: 01\n## B\nid: 01\n"), vec!["01"]);
    }

    #[test]
    fn plan_skips_heading_without_id() {
        assert!(parse_plan("## 只有标题\n正文\n").is_empty());
    }

    #[test]
    fn extract_block_includes_heading_and_stops_at_next() {
        let block = extract_chapter_block(DESIGN, "01").unwrap();
        assert!(block.starts_with("## 第1章 · 雨夜"));
        assert!(block.contains("id: 01"));
        assert!(block.contains("梗概: 相遇"));
        assert!(!block.contains("告别"));
    }

    #[test]
    fn extract_block_handles_missing() {
        assert!(extract_chapter_block(DESIGN, "99").is_none());
    }

    #[test]
    fn tail_state_keeps_trailing_comments() {
        let text =
            "name: 第一章\nevents:\n  - type: chapter_end\n\n# 背景：教室\n# 音乐：quiet.mp3\n";
        assert_eq!(tail_state(text), "# 背景：教室\n# 音乐：quiet.mp3");
    }

    #[test]
    fn tail_state_empty_without_comments() {
        assert_eq!(tail_state("name: x\nevents: []\n"), "");
    }

    #[test]
    fn chapter_file_supports_subdirs() {
        let dir = Path::new("/pkg");
        assert_eq!(
            chapter_file(dir, "01").as_deref(),
            Some(Path::new("/pkg/Chapters/01.yaml"))
        );
        assert_eq!(
            chapter_file(dir, "Intro/intro").as_deref(),
            Some(Path::new("/pkg/Chapters/Intro/intro.yaml"))
        );
    }

    #[test]
    fn chapter_file_rejects_unsafe_ids() {
        let dir = Path::new("/pkg");
        assert_eq!(chapter_file(dir, "../story_config"), None);
        assert_eq!(chapter_file(dir, "end"), None);
    }

    #[test]
    fn script_key_from_any_package_path() {
        let keys = vec![
            "standalone/我的剧本".to_string(),
            "character/风雪/高塔逆位".to_string(),
            "character/风雪/高塔逆位/嵌套".to_string(),
        ];
        let hit = |p: &str| script_key_of_script_path(p, &keys);
        // 章节 / 设计稿 / 配置，以及 Windows 反斜杠路径都能反推
        assert_eq!(
            hit("D:/d/data/game_data/scripts/character/风雪/高塔逆位/Chapters/01.yaml").as_deref(),
            Some("character/风雪/高塔逆位")
        );
        assert_eq!(
            hit(r"data\game_data\scripts\standalone\我的剧本\.agent\design.md").as_deref(),
            Some("standalone/我的剧本")
        );
        // 嵌套包优先最长匹配
        assert_eq!(
            hit("game_data/scripts/character/风雪/高塔逆位/嵌套/Chapters/01.yaml").as_deref(),
            Some("character/风雪/高塔逆位/嵌套")
        );
        assert_eq!(hit("game_data/scenes.json"), None);
    }

    #[test]
    fn thinking_off_for_mechanical_stages() {
        assert_eq!(profile(Stage::Forge).thinking, Some(false));
        assert_eq!(profile(Stage::Polish).thinking, Some(false));
        assert_eq!(profile(Stage::Setup).thinking, Some(true));
    }

    #[test]
    fn stages_that_touch_yaml_include_transformer_doc() {
        for stage in [Stage::Setup, Stage::Forge, Stage::Polish, Stage::Modify] {
            assert!(
                profile(stage)
                    .system_materials
                    .iter()
                    .any(|m| m.starts_with("script-transformer/")),
                "{stage:?} 会写/改剧本 YAML，材料里必须含 script-transformer"
            );
        }
    }

    #[test]
    fn forge_directive_states_one_chapter_per_turn() {
        let d = profile(Stage::Forge).directive;
        assert!(d.contains("一轮只写一章"), "Forge 必须写明粒度：{d}");
        assert!(d.contains("多章"), "Forge 必须禁止多章：{d}");
    }

    #[test]
    fn forge_directive_discourages_whole_script_validation() {
        let d = profile(Stage::Forge).directive;
        assert!(
            d.contains("validate_script"),
            "Forge 必须提醒别调 validate_script：{d}"
        );
        assert!(
            d.contains("never_read") || d.contains("假错"),
            "应说清会报什么错：{d}"
        );
    }

    #[test]
    fn non_routing_stages_all_have_directives() {
        for stage in [Stage::Setup, Stage::Forge, Stage::Polish, Stage::Modify] {
            assert!(
                !profile(stage).directive.is_empty(),
                "{stage:?} 缺少行为要求"
            );
        }
        assert!(profile(Stage::Routing).directive.is_empty());
    }

    #[test]
    fn stage_block_marks_materials_as_directives() {
        let block = build_stage_block(Path::new("/nonexistent-skills"), Stage::Forge);
        assert!(block.contains("【当前阶段】"));
        assert!(block.contains("必须遵守的角色指令"));
        assert!(block.contains("一轮只写一章"));
        // 材料读不到时必须写进提示词，否则模型会凭记忆补规范
        assert!(block.contains("角色指令缺失"), "{block}");
    }

    #[test]
    fn routing_stage_includes_writer_doc() {
        assert!(
            profile(Stage::Routing)
                .system_materials
                .iter()
                .any(|m| m.starts_with("script-writer/")),
            "未绑定剧本时做的是 0/1/2 阶段，必须给 writer"
        );
    }

    #[test]
    fn stage_materials_are_skill_relative() {
        for stage in [
            Stage::Routing,
            Stage::Setup,
            Stage::Forge,
            Stage::Polish,
            Stage::Modify,
        ] {
            for m in profile(stage).system_materials {
                assert!(
                    !m.starts_with("references/") && !m.starts_with("assets/"),
                    "{stage:?} 的材料 `{m}` 是裸相对路径，预注入后无法解析"
                );
            }
        }
    }

    #[test]
    fn derive_without_key_is_routing() {
        assert_eq!(derive(None).stage, Stage::Routing);
    }

    /// 造一轮 write_file 的工具轮次。
    fn write_round(call_id: &str, path: &str, tool: &str) -> Vec<LlmMessage> {
        let args = format!(r#"{{"path":"{}"}}"#, path.replace('\\', "\\\\"));
        let mut assistant = LlmMessage::assistant("我来写。");
        assistant.tool_calls = Some(vec![crate::ai_service::types::ToolCall {
            id: call_id.into(),
            type_: "function".into(),
            function: crate::ai_service::types::FunctionCall {
                name: tool.into(),
                arguments: args,
            },
        }]);
        vec![assistant, LlmMessage::tool_result(call_id, "已写入")]
    }

    fn forge(plan: &[&str], written: &[&str]) -> StageSnapshot {
        StageSnapshot {
            stage: Stage::Forge,
            script_key: None,
            script_dir: None,
            plan: plan.iter().map(|s| s.to_string()).collect(),
            written: written.iter().map(|s| s.to_string()).collect(),
        }
    }

    #[test]
    fn written_chapter_id_parses_path() {
        assert_eq!(
            written_chapter_id("write_file", r#"{"path":"/p/Chapters/01.yaml"}"#).as_deref(),
            Some("01")
        );
        assert_eq!(
            written_chapter_id("write_file", r#"{"path":"p\\Chapters\\Intro\\a.yaml"}"#).as_deref(),
            Some("Intro/a")
        );
        // 非章节文件 / 非写入工具 → 不认
        assert_eq!(
            written_chapter_id("write_file", r#"{"path":"/p/story_config.yaml"}"#),
            None
        );
        assert_eq!(
            written_chapter_id("read_file", r#"{"path":"/p/Chapters/01.yaml"}"#),
            None
        );
    }

    #[test]
    fn compact_drops_superseded_chapter_rounds() {
        let mut history = write_round("c1", "/p/Chapters/01.yaml", "write_file");
        history.push(LlmMessage::user("继续下一章"));
        // 01 已落盘、当前待写 02 → 01 那轮被压掉
        let out = compact_history(history.clone(), &forge(&["01", "02"], &["01"]));
        assert_eq!(out.len(), 1);
        assert_eq!(out[0].role, "user");
        // 当前待写就是 01 时，01 那轮保留
        let out = compact_history(history, &forge(&["01", "02"], &[]));
        assert_eq!(out.len(), 3);
    }

    #[test]
    fn compact_keeps_non_chapter_writes() {
        let mut history = write_round("c1", "/p/story_config.yaml", "write_file");
        history.push(LlmMessage::user("改配置"));
        let out = compact_history(history, &forge(&["01"], &["01"]));
        assert_eq!(out.len(), 3);
    }

    #[test]
    fn compact_is_noop_outside_forge() {
        let history = write_round("c1", "/p/Chapters/01.yaml", "write_file");
        let mut snap = forge(&["01", "02"], &["01"]);
        snap.stage = Stage::Polish;
        assert_eq!(compact_history(history, &snap).len(), 2);
    }

    #[test]
    fn chapter_id_of_path_handles_variants() {
        assert_eq!(
            chapter_id_of_path("/p/Chapters/01.yaml").as_deref(),
            Some("01")
        );
        assert_eq!(
            chapter_id_of_path("p\\Chapters\\Intro\\a.yml").as_deref(),
            Some("Intro/a")
        );
        assert_eq!(chapter_id_of_path("/p/story_config.yaml"), None);
        assert_eq!(chapter_id_of_path("/p/Chapters/01.txt"), None);
    }

    #[test]
    fn shape_accepts_minimal_valid_chapter() {
        let ok = serde_json::json!({
            "name": "雨夜",
            "events": [
                {"type": "narration", "text": "…"},
                {"type": "chapter_end", "next": "end"},
            ],
        });
        assert!(chapter_shape_problems(&ok).is_empty());
    }

    #[test]
    fn shape_flags_missing_name_or_events() {
        let no_name = serde_json::json!({"events": [{"type": "chapter_end", "next": "end"}]});
        assert!(
            chapter_shape_problems(&no_name)
                .iter()
                .any(|m| m.contains("name"))
        );

        let no_events = serde_json::json!({"name": "x"});
        assert!(
            chapter_shape_problems(&no_events)
                .iter()
                .any(|m| m.contains("events"))
        );

        let empty = serde_json::json!({"name": "x", "events": []});
        assert!(
            chapter_shape_problems(&empty)
                .iter()
                .any(|m| m.contains("空"))
        );
    }

    #[test]
    fn shape_requires_chapter_end_last() {
        let p = chapter_shape_problems(&serde_json::json!({
            "name": "x",
            "events": [
                {"type": "chapter_end", "next": "end"},
                {"type": "narration", "text": "还在说"},
            ],
        }));
        assert!(p.iter().any(|m| m.contains("chapter_end")));
    }

    #[test]
    fn shape_requires_next_only_for_linear() {
        let linear = serde_json::json!({"name": "x", "events": [{"type": "chapter_end"}]});
        assert!(
            chapter_shape_problems(&linear)
                .iter()
                .any(|m| m.contains("linear"))
        );

        // end_type 缺省即 linear；给了 next 就通过
        let with_next =
            serde_json::json!({"name": "x", "events": [{"type": "chapter_end", "next": "end"}]});
        assert!(chapter_shape_problems(&with_next).is_empty());

        // branching 不要求 next
        let branching = serde_json::json!({
            "name": "x",
            "events": [{"type": "chapter_end", "end_type": "branching"}],
        });
        assert!(chapter_shape_problems(&branching).is_empty());
    }

    #[test]
    fn check_written_chapter_ignores_other_scripts() {
        let snap = StageSnapshot {
            script_dir: Some(PathBuf::from("/p")),
            script_key: Some("standalone/A".into()),
            ..Default::default()
        };
        // 数据目录随便给一个：这些路径压根走不到要用它那一步
        let data_dir = Path::new("/nonexistent-data");
        assert!(
            check_written_chapter(&snap, "/p/standalone/B/Chapters/01.yaml", data_dir).is_none()
        );
        // 未绑定剧本时不检查
        assert!(
            check_written_chapter(&StageSnapshot::default(), "/p/Chapters/01.yaml", data_dir)
                .is_none()
        );
    }

    #[test]
    fn check_written_chapter_is_testable_end_to_end() {
        // 以前这个入口内部自己取全局 data 目录，一传合法路径就会 panic，
        // 整条链路没法测。现在目录从参数进来，这里能直接跑完整判定。
        let (pkg, data_dir) = tmp_script_package("entry", BAD_CHAPTER, "- 素材模式：只用已有\n");
        let chapter = pkg.join("Chapters").join("01.yaml");

        let check = check_written_chapter(&snap_of(&pkg), chapter.to_str().unwrap(), &data_dir)
            .expect("缺 name + 缺素材，必须给出自检");
        assert_eq!(check.errors.len(), 2, "{:?}", check.errors);

        let _ = std::fs::remove_dir_all(pkg.parent().unwrap().parent().unwrap());
    }

    #[test]
    fn script_key_from_story_config_path() {
        assert_eq!(
            script_key_of_story_config(
                "game_data/scripts/character/DeepSeek/雨夜爆种/story_config.yaml"
            )
            .as_deref(),
            Some("character/DeepSeek/雨夜爆种")
        );
        // Windows 分隔符
        assert_eq!(
            script_key_of_story_config(
                r"D:\d\data\game_data\scripts\standalone\我的剧本\story_config.yaml"
            )
            .as_deref(),
            Some("standalone/我的剧本")
        );
        // flat 布局
        assert_eq!(
            script_key_of_story_config("game_data/scripts/我的剧本/story_config.yaml").as_deref(),
            Some("我的剧本")
        );
        // 章节文件 / .bak / 不在 scripts 下 → 一律不认
        assert_eq!(
            script_key_of_story_config("game_data/scripts/x/Chapters/01.yaml"),
            None
        );
        assert_eq!(
            script_key_of_story_config("game_data/scripts/x/story_config.yaml.bak"),
            None
        );
        assert_eq!(
            script_key_of_story_config("data/skills/foo/story_config.yaml"),
            None
        );
    }

    #[test]
    fn real_chapters_pass_shape_check() {
        // 自检每次写章节都会跑，误报会当场拦住作者。拿仓库里真实章节当样本回归。
        // 没带剧本数据的环境（CI）直接跳过。
        let scripts = Path::new(env!("CARGO_MANIFEST_DIR"))
            .parent()
            .map(|p| p.join("data/game_data/scripts"));
        let Some(scripts) = scripts.filter(|p| p.is_dir()) else {
            return;
        };

        let mut checked = 0usize;
        let mut failures: Vec<String> = Vec::new();
        collect_chapter_files(&scripts, &mut |file| {
            let Ok(value) = crate::utils::yaml_file::read_yaml_as_json(file) else {
                return;
            };
            checked += 1;
            let problems = chapter_shape_problems(&value);
            if !problems.is_empty() {
                failures.push(format!("{}：{}", file.display(), problems.join("；")));
            }
        });

        assert!(
            checked > 0,
            "{} 下没扫到章节，样本目录可能不对",
            scripts.display()
        );
        assert!(
            failures.is_empty(),
            "真实章节被自检误报：\n{}",
            failures.join("\n")
        );
    }

    /// 递归收集 `Chapters/` 下的章节文件（`.agent/` 等点号目录不看）。
    fn collect_chapter_files(dir: &Path, f: &mut impl FnMut(&Path)) {
        let Ok(entries) = std::fs::read_dir(dir) else {
            return;
        };
        for entry in entries.flatten() {
            let path = entry.path();
            let name = entry.file_name().to_string_lossy().to_string();
            if name.starts_with('.') {
                continue;
            }
            if path.is_dir() {
                collect_chapter_files(&path, f);
            } else if name.ends_with(".yaml") && path.to_string_lossy().contains("Chapters") {
                f(&path);
            }
        }
    }

    #[test]
    fn declared_cast_reads_the_line_and_tolerates_separators() {
        let block = "## 第3章 · 雨夜\nid: 03\n梗概: …\n登场: MAIN, 老管家、小雨\n素材: 夜晚.webp\n";
        assert_eq!(declared_cast(block), vec!["MAIN", "老管家", "小雨"]);
        // 没有这一行 → 空（不误报）
        assert!(declared_cast("## 第3章\nid: 03\n梗概: …\n").is_empty());
    }

    #[test]
    fn appearing_cast_collects_character_fields() {
        let chapter = serde_json::json!({
            "name": "雨夜",
            "events": [
                {"type": "narration", "text": "…"},
                {"type": "dialogue", "character": "MAIN", "text": "…"},
                {"type": "modify_character", "character": "老管家", "action": "show_character"},
                {"type": "ai_dialogue", "character": "MAIN"},
            ],
        });
        assert_eq!(appearing_cast(&chapter), vec!["MAIN", "老管家"]);
    }

    #[test]
    fn zero_asset_mode_rejects_even_existing_assets() {
        // 章里引用的「夜晚.webp」磁盘上真的有（tmp_script_package 会预置），
        // 但模式是零素材 → 连它也不该引用
        let chapter = "name: 雨夜\nevents:\n  - type: background\n    imagePath: 夜晚.webp\n  - type: chapter_end\n    next: end\n";
        let (pkg, data_dir) = tmp_script_package("zero", chapter, "- 素材模式：零素材\n");
        let check = check_chapter(&pkg, "01", &data_dir).expect("应当给出自检");
        assert_eq!(check.errors.len(), 1, "{:?}", check.errors);
        assert!(check.errors[0].contains("零素材"), "{:?}", check.errors);
        assert!(check.errors[0].contains("夜晚.webp"), "{:?}", check.errors);
        // 不该再去逐条报"找不到"（有引用本身就是错）
        assert!(
            check.missing_assets.is_empty(),
            "{:?}",
            check.missing_assets
        );
        let _ = std::fs::remove_dir_all(pkg.parent().unwrap().parent().unwrap());
    }

    #[test]
    fn self_check_flags_declared_but_absent_character() {
        let clean = "name: 雨夜\nevents:\n  - type: dialogue\n    character: MAIN\n  - type: chapter_end\n    next: end\n";
        let (pkg, data_dir) = tmp_script_package("cast", clean, "- 素材模式：只用已有\n");
        // 设计稿说这一章老管家也登场，但 YAML 里没有它
        std::fs::write(
            pkg.join(DESIGN_REL_PATH),
            "## 第1章\nid: 01\n登场: MAIN, 老管家\n",
        )
        .unwrap();

        let check = check_chapter(&pkg, "01", &data_dir).expect("应当给出自检");
        assert!(
            check
                .warnings
                .iter()
                .any(|w| w.contains("老管家") && w.contains("没有它出场")),
            "{:?}",
            check.warnings
        );
        // 真登场了的那个不该被报
        assert!(
            !check.warnings.iter().any(|w| w.contains("「MAIN」")),
            "{check:?}"
        );
        let _ = std::fs::remove_dir_all(pkg.parent().unwrap().parent().unwrap());
    }

    #[test]
    fn assets_gap_file_is_written_once_per_asset() {
        let dir = tmp_script_dir("gap");
        update_assets_gap(&dir, "03", &["夜晚.png".into(), "走廊.mp3".into()]).unwrap();
        let text = std::fs::read_to_string(dir.join(ASSETS_REL_PATH)).unwrap();
        assert!(
            text.contains("-「夜晚.png」（第 03 章引用，磁盘上没有）"),
            "{text}"
        );
        assert!(text.contains("走廊.mp3"), "{text}");

        // 同一份素材在另一章再缺一次：不重复登记，保留最早那一章
        update_assets_gap(&dir, "05", &["夜晚.png".into()]).unwrap();
        let text = std::fs::read_to_string(dir.join(ASSETS_REL_PATH)).unwrap();
        assert_eq!(text.matches("夜晚.png").count(), 1, "{text}");
        assert!(text.contains("第 03 章"), "{text}");

        // 没有缺口时不动文件
        let before = text.clone();
        update_assets_gap(&dir, "06", &[]).unwrap();
        assert_eq!(
            std::fs::read_to_string(dir.join(ASSETS_REL_PATH)).unwrap(),
            before
        );
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn cast_file_aggregates_declared_roles() {
        let dir = tmp_script_dir("castfile");
        std::fs::write(
            dir.join(DESIGN_REL_PATH),
            "## 第1章\nid: 01\n登场: MAIN, 老管家\n\n## 第2章\nid: 02\n登场: MAIN、小雨\n",
        )
        .unwrap();
        write_cast(&dir).unwrap();
        let text = std::fs::read_to_string(dir.join(CAST_REL_PATH)).unwrap();
        assert!(text.contains("| MAIN | 01 02 |"), "{text}");
        assert!(text.contains("| 老管家 | 01 |"), "{text}");
        assert!(text.contains("| 小雨 | 02 |"), "{text}");

        // 设计稿还没列登场时也要给出一份（而不是报错）
        std::fs::write(dir.join(DESIGN_REL_PATH), "## 只是标题\n正文\n").unwrap();
        write_cast(&dir).unwrap();
        assert!(
            std::fs::read_to_string(dir.join(CAST_REL_PATH))
                .unwrap()
                .contains("还没列出登场角色")
        );
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn package_key_derives_from_anchors_without_story_config() {
        // 新建剧本的正常中间态：只有 `.agent/`，还没有 story_config.yaml
        let hit = |p: &str| script_key_of_package_path(p);
        assert_eq!(
            hit("game_data/scripts/character/风雪/无限占卜/.agent/design.md").as_deref(),
            Some("character/风雪/无限占卜")
        );
        assert_eq!(
            hit(r"D:\d\data\game_data\scripts\standalone\我的剧本\Chapters\01.yaml").as_deref(),
            Some("standalone/我的剧本")
        );
        assert_eq!(
            hit("game_data/scripts/character/风雪/无限占卜/story_config.yaml").as_deref(),
            Some("character/风雪/无限占卜")
        );
        // 嵌套剧本包照样认
        assert_eq!(
            hit("game_data/scripts/character/风雪/高塔逆位/嵌套/Chapters/01.yaml").as_deref(),
            Some("character/风雪/高塔逆位/嵌套")
        );
        // 不在 scripts 下 / 没有锚点 → 一律不认
        assert_eq!(hit("game_data/skills/script-writer/.agent/a.md"), None);
        assert_eq!(
            hit("game_data/scripts/character/风雪/无限占卜/Assets/夜晚.webp"),
            None
        );
    }

    #[test]
    fn task_block_names_the_item_and_carries_the_user_words() {
        use crate::ai_service::skill_agent::{
            role,
            router::{QueueItem, RoutePlan},
        };
        let q = |kind, target: &str| QueueItem {
            kind,
            target: target.to_string(),
        };
        let plan = |items: Vec<QueueItem>| RoutePlan {
            items,
            boundary: None,
            reason: None,
            land: true,
            fallback: false,
        };
        let queue = vec![
            q(role::TaskKind::WriteChapter, "03"),
            q(role::TaskKind::ReviseChapter, "12"),
        ];
        let block = build_task_block(&TaskBlockInput {
            task: role::TaskKind::WriteChapter,
            handoff: role::Handoff::Proceed,
            plan: &plan(queue),
            user_msg: "把第三第四张都写了，顺手把第十二章那句改掉",
            skills_dir: Path::new("/nonexistent-skills"),
            land: true,
            item_index: 0,
            skipped: &[],
        });
        assert!(block.contains("【本轮队列】共 2 项"), "{block}");
        // 队列是逐项执行的：这一轮只做标明的那一项
        assert!(block.contains("[本项] 落盘章节：03"), "{block}");
        assert!(block.contains("[其余] 改章节：12"), "{block}");
        assert!(block.contains("只做标着 [本项] 的那一件"), "{block}");
        // 用户原话要带给角色；能力范围内要按用户的来
        assert!(block.contains("【用户这一轮的原话】"), "{block}");
        assert!(block.contains("顺手把第十二章那句改掉"), "{block}");
        assert!(block.contains("【听用户的】"), "{block}");
        assert!(block.contains("只是**推荐写法**"), "{block}");
        assert!(block.contains("这些工具加这本手册做得到"), "{block}");
        // 落盘轮不该出现"口述"那一段
        assert!(!block.contains("【本轮是口述，不落盘】"), "{block}");
        assert!(block.contains("两个动作词"), "{block}");

        // 只有一项、又没指明对象（用户就说了一句"继续"）→ 不刷队列段
        let single = plan(vec![q(role::TaskKind::Chat, "")]);
        let block = build_task_block(&TaskBlockInput {
            task: role::TaskKind::Chat,
            handoff: role::Handoff::Proceed,
            plan: &single,
            user_msg: "继续",
            skills_dir: Path::new("/x"),
            land: false,
            item_index: 0,
            skipped: &[],
        });
        assert!(!block.contains("【本轮队列】"), "{block}");
    }

    #[test]
    fn dictate_turn_forbids_writing_files() {
        use crate::ai_service::skill_agent::{
            role,
            router::{QueueItem, RoutePlan},
        };
        let plan = RoutePlan {
            items: vec![QueueItem {
                kind: role::TaskKind::WriteChapter,
                target: "03".into(),
            }],
            boundary: None,
            reason: None,
            land: false,
            fallback: false,
        };
        let block = build_task_block(&TaskBlockInput {
            task: role::TaskKind::WriteChapter,
            handoff: role::Handoff::Proceed,
            plan: &plan,
            user_msg: "把第三章写出来我看看",
            skills_dir: Path::new("/nonexistent-skills"),
            land: false,
            item_index: 0,
            skipped: &[],
        });
        // 用户只说"写"→ 这一轮只口述；提示词必须把它说死，并教它怎么问落盘
        assert!(block.contains("【本轮是口述，不落盘】"), "{block}");
        assert!(block.contains("一个字都不许写进文件"), "{block}");
        assert!(block.contains("要不要落盘"), "{block}");
    }

    #[test]
    fn skipped_items_are_reported_to_the_model() {
        use crate::ai_service::skill_agent::{
            role,
            router::{QueueItem, RoutePlan},
        };
        let plan = RoutePlan {
            items: vec![QueueItem {
                kind: role::TaskKind::ReviseChapter,
                target: "07".into(),
            }],
            boundary: None,
            reason: None,
            land: true,
            fallback: false,
        };
        let skipped = vec!["改章节：你点名的那一章还没有落盘，改不了它".to_string()];
        let block = build_task_block(&TaskBlockInput {
            task: role::TaskKind::ReviseChapter,
            handoff: role::Handoff::Explain("你点名的那一章还没有落盘，改不了它"),
            plan: &plan,
            user_msg: "改第七章",
            skills_dir: Path::new("/nonexistent-skills"),
            land: true,
            item_index: 0,
            skipped: &skipped,
        });
        assert!(block.contains("【队列里做不了的项】"), "{block}");
        assert!(block.contains("改不了它"), "{block}");
    }

    #[test]
    fn queue_only_ticks_chapters_that_actually_landed() {
        use crate::ai_service::skill_agent::{role, router::QueueItem};
        let dir = tmp_script_dir("queue-tick");
        let items = vec![
            QueueItem {
                kind: role::TaskKind::WriteChapter,
                target: "01".into(),
            },
            QueueItem {
                kind: role::TaskKind::ReviseChapter,
                target: "02".into(),
            },
        ];
        // 01 是这一轮写的、02 早就在磁盘上
        write_queue(&dir, &items, &["01".into(), "02".into()]).unwrap();
        let text = std::fs::read_to_string(dir.join(QUEUE_REL_PATH)).unwrap();
        assert!(text.contains("- [x] 落盘章节：01"), "{text}");
        assert!(
            text.contains("- [ ] 改章节：02"),
            "「它早就在磁盘上」不等于「这一轮改完了它」，不该打勾：{text}"
        );
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn skeleton_makes_a_new_package_loadable_and_never_overwrites() {
        // 每个测试**只碰自己那一个目录**。这个用例需要把包放在某个根下，
        // 所以自己开一个 root —— 绝不能用 `tmp_script_dir(..).parent()`：
        // 那等于整个 %TEMP%，一旦 remove_dir_all 就会扫掉别人的临时文件。
        let root = std::env::temp_dir().join(format!("lingchat-skeleton-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&root);

        let pkg = root.join("无限占卜");
        std::fs::create_dir_all(&pkg).unwrap();
        let key = "character/风雪/无限占卜";
        assert!(
            ensure_package_skeleton(&pkg, key).unwrap(),
            "第一次应当创建"
        );
        assert!(pkg.join("Chapters").is_dir(), "章节目录要一起建");
        let text = std::fs::read_to_string(pkg.join("story_config.yaml")).unwrap();
        assert!(text.contains("script_name: 无限占卜"), "{text}");
        assert!(text.contains(SKELETON_MARKER), "{text}");
        assert!(text.contains("bound_character_folder: \"风雪\""), "{text}");

        // 第二次不动它（也不覆盖模型后来写的正式配置）
        assert!(!ensure_package_skeleton(&pkg, key).unwrap());
        std::fs::write(pkg.join("story_config.yaml"), "script_name: 无限占卜\n").unwrap();
        assert!(!ensure_package_skeleton(&pkg, key).unwrap());
        assert_eq!(
            std::fs::read_to_string(pkg.join("story_config.yaml")).unwrap(),
            "script_name: 无限占卜\n",
            "已存在就一个字都不能动"
        );

        // 独立剧本不该带冒险块
        let flat = root.join("我的剧本");
        std::fs::create_dir_all(&flat).unwrap();
        ensure_package_skeleton(&flat, "standalone/我的剧本").unwrap();
        assert!(
            !std::fs::read_to_string(flat.join("story_config.yaml"))
                .unwrap()
                .contains("adventure")
        );

        let _ = std::fs::remove_dir_all(&root);
    }

    #[test]
    fn config_line_tells_the_model_the_skeleton_is_not_finished() {
        let dir = tmp_script_dir("cfgline");
        assert!(config_line(&dir).contains("缺"), "还没建时要说明");
        ensure_package_skeleton(&dir, "standalone/A").unwrap();
        assert!(
            config_line(&dir).contains("最小骨架"),
            "只有骨架时要催它按类型补全：{}",
            config_line(&dir)
        );
        std::fs::write(dir.join("story_config.yaml"), "script_name: A\n").unwrap();
        assert!(config_line(&dir).contains("已就绪"));
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn design_line_says_what_the_code_actually_read_out_of_the_design() {
        let dir = tmp_script_dir("designline");
        assert!(design_line(&dir).contains("缺"), "{}", design_line(&dir));
        // 设计稿在、但没有一个 `##` 块的第一行是 `id:` → 代码算不出下一章
        std::fs::write(
            dir.join(DESIGN_REL_PATH),
            "# 设计\n\n## 第一章 雨夜\n梗概: …\nid: 01\n",
        )
        .unwrap();
        let line = design_line(&dir);
        assert!(line.contains("读不出章节"), "{line}");
        assert!(line.contains("id: 01"), "要说清怎么写才认：{line}");
        // 写对了就报出代码认到的清单
        std::fs::write(
            dir.join(DESIGN_REL_PATH),
            "# 设计\n\n## 第一章 雨夜\nid: 01\n\n## 第二章 清晨\nid: 02\n",
        )
        .unwrap();
        let line = design_line(&dir);
        assert!(line.contains("2 章") && line.contains("01 02"), "{line}");
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn artifact_paths_all_live_under_agent_dir() {
        // 「详情」浮窗按目录列产物、按目录取文件，写歪一处就会静默漏掉一份
        for rel in [DESIGN_REL_PATH, CONSTRAINTS_REL_PATH, QUEUE_REL_PATH] {
            assert!(
                rel.starts_with(&format!("{AGENT_DIR}/")),
                "`{rel}` 不在 {AGENT_DIR}/ 下"
            );
        }
    }

    #[test]
    fn chapter_check_render_separates_blocking_from_recorded() {
        let check = ChapterCheck {
            errors: vec!["缺少顶层 `name`".into()],
            warnings: vec!["第 2 个事件 · 找不到素材「夜晚.png」".into()],
            missing_assets: vec!["夜晚.png".into()],
            mode: AssetMode::OnlyExisting,
        };
        let text = check.render();
        assert!(text.contains("还不算写完"), "{text}");
        assert!(text.contains("重新 write_file 覆盖"), "{text}");
        assert!(text.contains("记下来即可"), "{text}");
        assert!(text.contains(".agent/assets.md"), "{text}");
        // 卡人的其实是用户当初选的那个模式，所以回执里必须带上"怎么改"
        assert!(text.contains("改成「先预留」"), "{text}");
    }

    #[test]
    fn every_asset_mode_says_how_to_switch() {
        for mode in [
            AssetMode::Unspecified,
            AssetMode::OnlyExisting,
            AssetMode::Reserve,
        ] {
            let hint = mode.switch_hint();
            assert!(
                hint.contains("素材模式") && hint.contains("说一句"),
                "{mode:?} 没给出改法：{hint}"
            );
        }
    }

    #[test]
    fn impact_line_frames_the_switch_both_ways() {
        let missing = vec!["03 夜晚.png".to_string(), "05 走廊.webp".to_string()];
        // 收紧：告诉他这些会变成必须修的错
        let strict = asset_impact_line(AssetMode::Reserve, &missing);
        assert!(strict.contains("必须修的错"), "{strict}");
        assert!(strict.contains("03 夜晚.png、05 走廊.webp"), "{strict}");
        // 放宽：告诉他一句话就能不再拦
        let loose = asset_impact_line(AssetMode::OnlyExisting, &missing);
        assert!(loose.contains("改成「先预留」"), "{loose}");
        // 未定：先问，不替他决定
        let undecided = asset_impact_line(AssetMode::Unspecified, &missing);
        assert!(undecided.contains("先问用户"), "{undecided}");
    }

    #[test]
    fn impact_line_caps_the_listing() {
        let many: Vec<String> = (0..9).map(|i| format!("0{i} 素材{i}.webp")).collect();
        let line = asset_impact_line(AssetMode::Reserve, &many);
        assert!(line.contains("共 9 处"), "{line}");
        assert!(!line.contains("素材8"), "不该把 9 条全列出来：{line}");
    }

    #[test]
    fn empty_chapter_check_renders_nothing_to_say() {
        assert!(ChapterCheck::default().is_empty());
        assert_eq!(ChapterCheck::default().render(), "");
    }

    #[test]
    fn chapter_after_follows_plan_order() {
        // 本章 = 03 → 下一章 = 04A
        let snap = forge(&["01", "02", "03", "04A"], &["01", "02"]);
        assert_eq!(chapter_after(&snap).as_deref(), Some("04A"));
        // 最后一章之后没有了
        let snap = forge(&["01", "02"], &["01"]);
        assert_eq!(chapter_after(&snap), None);
        // 没有计划就没有"下一章"
        assert_eq!(chapter_after(&forge(&[], &[])), None);
    }

    /// 造一个只属于这条测试的临时剧本包目录。
    fn tmp_script_dir(tag: &str) -> PathBuf {
        let dir =
            std::env::temp_dir().join(format!("lingchat-stage-{}-{}", tag, std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(dir.join(".agent")).unwrap();
        dir
    }

    #[test]
    fn asset_mode_comes_from_constraints_file() {
        let dir = tmp_script_dir("mode");
        assert_eq!(read_asset_mode(&dir), AssetMode::Unspecified);
        std::fs::write(
            dir.join(CONSTRAINTS_REL_PATH),
            "# 用户约束\n\n- 素材模式：只用已有\n- 玩家扮演：风雪\n",
        )
        .unwrap();
        assert_eq!(read_asset_mode(&dir), AssetMode::OnlyExisting);
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn queue_is_merged_not_overwritten_so_a_chat_turn_cannot_wipe_pending_work() {
        use crate::ai_service::skill_agent::role::TaskKind;
        use crate::ai_service::skill_agent::router::QueueItem;
        let dir = tmp_script_dir("queue-merge");
        let q = |kind, target: &str| QueueItem {
            kind,
            target: target.to_string(),
        };

        // 用户一轮里排了两件事
        write_queue(
            &dir,
            &[
                q(TaskKind::WriteChapter, "04"),
                q(TaskKind::CheckAssets, ""),
            ],
            &["01".into()],
        )
        .unwrap();
        let text = std::fs::read_to_string(dir.join(QUEUE_REL_PATH)).unwrap();
        assert!(text.contains("- [ ] 落盘章节：04"), "{text}");
        assert!(text.contains("- [ ] 素材盘点：（未指明）"), "{text}");

        // 下一轮只是闲聊（仅对话）—— 队列**不该**被这条闲聊清空（真机踩过）
        write_queue(&dir, &[q(TaskKind::Chat, "")], &["01".into()]).unwrap();
        let text = std::fs::read_to_string(dir.join(QUEUE_REL_PATH)).unwrap();
        assert!(
            text.contains("- [ ] 落盘章节：04"),
            "闲聊轮把待办抹掉了：{text}"
        );
        assert!(!text.contains("仅对话"), "仅对话不该进队列：{text}");

        // 第四章真落盘之后才打勾；同一项不重复添加
        write_queue(
            &dir,
            &[q(TaskKind::WriteChapter, "04")],
            &["01".into(), "04".into()],
        )
        .unwrap();
        let text = std::fs::read_to_string(dir.join(QUEUE_REL_PATH)).unwrap();
        assert!(text.contains("- [x] 落盘章节：04"), "{text}");
        assert_eq!(
            text.matches("落盘章节：04").count(),
            1,
            "重复添加了：{text}"
        );
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn conflicting_mode_statements_are_called_out_in_the_handoff() {
        // 真机：卡片里「零素材」和别处旧说法并存，交接单却说"未声明"，
        // 模型连着几轮追问按哪个走。现在两件事都修了：读得出来 + 矛盾要说破。
        let dir = tmp_script_dir("mode-conflict");
        std::fs::write(
            dir.join(CONSTRAINTS_REL_PATH),
            "# 约束\n\n- 素材：零素材（依据：用户改口）\n- 素材模式：只用已有\n",
        )
        .unwrap();
        let mut snap = forge(&[], &[]);
        snap.script_dir = Some(dir.clone());
        let block = progress_block(&snap, &dir, &dir);
        // 以最后一条为准
        assert!(block.contains("只用已有"), "{block}");
        assert!(block.contains("互相矛盾"), "{block}");
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn chapter_details_file_is_reported_when_it_exists() {
        let dir = tmp_script_dir("details");
        let mut snap = forge(&[], &[]);
        snap.script_dir = Some(dir.clone());
        assert!(!progress_block(&snap, &dir, &dir).contains("章节细节稿"));
        std::fs::write(dir.join(CHAPTER_DETAILS_REL_PATH), "## 第4章\n…\n").unwrap();
        let block = progress_block(&snap, &dir, &dir);
        assert!(block.contains("章节细节稿"), "{block}");
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn cast_mode_line_shows_up_only_when_the_script_uses_character_cards() {
        // 用户已经声明过 → 一定要带出来（他要改主意就得有这一行可改）
        let dir = tmp_script_dir("cast-declared");
        std::fs::write(dir.join(CONSTRAINTS_REL_PATH), "- 角色卡：允许缺失\n").unwrap();
        let mut snap = forge(&[], &[]);
        snap.script_dir = Some(dir.clone());
        let block = progress_block(&snap, &dir, &dir);
        assert!(block.contains("角色卡模式：允许缺失"), "{block}");
        let _ = std::fs::remove_dir_all(&dir);

        // 角色卡羁绊冒险 → 没声明也要问
        let dir = tmp_script_dir("cast-adventure");
        let mut snap = forge(&[], &[]);
        snap.script_dir = Some(dir.clone());
        snap.script_key = Some("character/诺一/无限占卜".to_string());
        let block = progress_block(&snap, &dir, &dir);
        assert!(block.contains("角色卡模式：未声明"), "{block}");
        let _ = std::fs::remove_dir_all(&dir);

        // 已建了 characters/ 也一样
        let dir = tmp_script_dir("cast-dir");
        std::fs::create_dir_all(dir.join("characters").join("风雪")).unwrap();
        let mut snap = forge(&[], &[]);
        snap.script_dir = Some(dir.clone());
        assert!(progress_block(&snap, &dir, &dir).contains("角色卡模式"));
        let _ = std::fs::remove_dir_all(&dir);

        // 纯独立剧本、没人声明过 → 不占地方
        let dir = tmp_script_dir("cast-none");
        let mut snap = forge(&[], &[]);
        snap.script_dir = Some(dir.clone());
        snap.script_key = Some("standalone/我的剧本".to_string());
        let block = progress_block(&snap, &dir, &dir);
        assert!(!block.contains("角色卡模式"), "{block}");
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn target_ranges_and_lists_are_split_into_chapters() {
        assert_eq!(chapter_ids_in("03"), vec!["03"]);
        assert_eq!(chapter_ids_in("01-04"), vec!["01", "02", "03", "04"]);
        assert_eq!(chapter_ids_in("01~03"), vec!["01", "02", "03"]);
        assert_eq!(chapter_ids_in("1-3"), vec!["1", "2", "3"]);
        assert_eq!(chapter_ids_in("01、02"), vec!["01", "02"]);
        assert_eq!(chapter_ids_in("01 02"), vec!["01", "02"]);
        assert_eq!(chapter_ids_in("第 3 章"), vec!["3"]);
        // 子目录章节 id 不能被当成区间拆坏
        assert_eq!(chapter_ids_in("Intro/01"), vec!["Intro/01"]);
        assert_eq!(chapter_ids_in(""), Vec::<String>::new());
        // 区间太夸张就当普通名字，绝不展开
        assert_eq!(chapter_ids_in("01-9999"), vec!["01-9999"]);
    }

    #[test]
    fn a_range_target_is_not_reported_as_missing() {
        use crate::ai_service::skill_agent::role::{Handoff, TaskKind, reconcile};
        // 真机：队列里写着 `改章节：01-04`，代码按"单个章节 id"查不到 → 判"那一章还没落盘"
        // → 整轮工具收成只读，队列里其余项全废。01~03 在盘上、04 没有 → 应当照改能改的。
        let snap = forge(&["01", "02", "03", "04"], &["01", "02", "03"]);
        let facts = facts_of(&snap, Some("01-04"));
        assert_eq!(facts.target_exists, Some(true), "有在盘上的就不算'找不到'");
        assert!(facts.target_partial, "04 还没落盘，要能说出来");
        let Handoff::ProceedNote(note) = reconcile(TaskKind::ReviseChapter, facts) else {
            panic!("部分存在要照做 + 说明，不是整轮拒绝")
        };
        assert!(note.contains("还没落盘"), "{note}");

        // 一个都不在盘上 → 仍然是"改不了"
        let facts = facts_of(&snap, Some("07-09"));
        assert_eq!(facts.target_exists, Some(false));
        assert!(matches!(
            reconcile(TaskKind::ReviseChapter, facts),
            Handoff::Explain(_)
        ));
    }

    #[test]
    fn queue_progress_counts_and_names_next() {
        let dir = tmp_script_dir("queue");
        assert_eq!(read_queue_progress(&dir), None);
        std::fs::write(
            dir.join(QUEUE_REL_PATH),
            "# 队列\n- [x] 改章节细节：13–19 章\n- [ ] 写章节：第 09 章\n- [ ] 查素材\n",
        )
        .unwrap();
        let progress = read_queue_progress(&dir).unwrap();
        assert!(progress.contains("共 3 项"), "{progress}");
        assert!(progress.contains("还剩 2"), "{progress}");
        assert!(progress.contains("写章节：第 09 章"), "{progress}");
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn progress_block_reports_mode_and_next_chapter() {
        let dir = tmp_script_dir("progress");
        std::fs::write(dir.join(CONSTRAINTS_REL_PATH), "- 素材模式：先预留\n").unwrap();
        std::fs::write(dir.join(QUEUE_REL_PATH), "- [ ] 写章节：第 09 章\n").unwrap();
        let mut snap = forge(&["01", "02", "03", "04"], &["01", "02"]);
        snap.script_dir = Some(dir.clone());

        let block = progress_block(&snap, &dir, &dir);
        assert!(block.contains("【落盘进度】"), "{block}");
        assert!(block.contains("先预留"), "{block}");
        assert!(block.contains("已落盘：01 02"), "{block}");
        // 本章 = 03，下一章 04 在磁盘上还没有
        assert!(block.contains("下一章 04：尚未落盘"), "{block}");
        assert!(block.contains("队列：共 1 项"), "{block}");
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn progress_block_flags_existing_next_chapter() {
        let dir = tmp_script_dir("progress2");
        // 01 被删了、02 03 还在：本章 = 01，而 02 已有内容
        let mut snap = forge(&["01", "02", "03"], &["02", "03"]);
        snap.script_dir = Some(dir.clone());
        let block = progress_block(&snap, &dir, &dir);
        assert!(block.contains("下一章 02：磁盘上已有内容"), "{block}");
        assert!(block.contains("素材模式：未声明"), "{block}");
        let _ = std::fs::remove_dir_all(&dir);
    }

    /// 造一个完整的剧本包：`<tmp>/scripts/standalone/A/`。
    ///
    /// 里面预置 `Assets/Backgrounds/夜晚.webp` —— 于是「章节里写 夜晚.png」正好是
    /// 「换个扩展名就能找到」那一种，笔误提示与素材模式两条路都能验到。
    fn tmp_script_package(tag: &str, chapter: &str, constraints: &str) -> (PathBuf, PathBuf) {
        let pkg = std::env::temp_dir().join(format!(
            "lingchat-stage-{}-{}/scripts/standalone/A",
            tag,
            std::process::id()
        ));
        let _ = std::fs::remove_dir_all(pkg.parent().unwrap().parent().unwrap());
        std::fs::create_dir_all(pkg.join(".agent")).unwrap();
        std::fs::write(pkg.join(CONSTRAINTS_REL_PATH), constraints).unwrap();
        std::fs::create_dir_all(pkg.join("Chapters")).unwrap();
        std::fs::write(pkg.join("Chapters/01.yaml"), chapter).unwrap();
        let backgrounds = pkg.join("Assets").join("Backgrounds");
        std::fs::create_dir_all(&backgrounds).unwrap();
        std::fs::write(backgrounds.join("夜晚.webp"), b"x").unwrap();
        let data_dir = pkg.parent().unwrap().parent().unwrap().to_path_buf();
        (pkg, data_dir)
    }

    fn snap_of(pkg: &Path) -> StageSnapshot {
        StageSnapshot {
            stage: Stage::Forge,
            script_key: Some("standalone/A".into()),
            script_dir: Some(pkg.to_path_buf()),
            plan: vec!["01".into()],
            written: vec![],
        }
    }

    /// 一个结构错（缺 name）+ 一个素材缺口（磁盘上没有 夜晚.png）的章节。
    const BAD_CHAPTER: &str = "name: ''\nevents:\n  - type: background\n    imagePath: 夜晚.png\n  - type: chapter_end\n    next: end\n";

    #[test]
    fn self_check_flags_shape_and_missing_asset_as_errors_in_only_existing_mode() {
        let (pkg, data_dir) = tmp_script_package("e2e-only", BAD_CHAPTER, "- 素材模式：只用已有\n");
        let check = check_chapter(&pkg, "01", &data_dir).expect("有问题就必须给出自检");

        // 结构错 + 素材缺失 = 两条都必须当场改
        assert_eq!(check.errors.len(), 2, "{:?}", check.errors);
        assert!(check.errors.iter().any(|e| e.contains("name")), "{check:?}");
        let asset = check
            .errors
            .iter()
            .find(|e| e.contains("夜晚.png"))
            .unwrap();
        assert!(asset.contains("疑似写错了扩展名"), "{asset}");
        assert!(check.warnings.is_empty(), "{:?}", check.warnings);
        assert_eq!(check.missing_assets, vec!["夜晚.png"]);
        assert!(check.render().contains("还不算写完"));
        let _ = std::fs::remove_dir_all(pkg.parent().unwrap().parent().unwrap());
    }

    #[test]
    fn self_check_downgrades_missing_asset_in_reserve_mode() {
        let (pkg, data_dir) =
            tmp_script_package("e2e-reserve", BAD_CHAPTER, "- 素材模式：先预留\n");
        let check = check_chapter(&pkg, "01", &data_dir).unwrap();

        // 同一份章节：素材缺口从错误降成警告，只剩结构错是非改不可的
        assert_eq!(check.errors.len(), 1, "{:?}", check.errors);
        assert!(
            check.warnings.iter().any(|w| w.contains("夜晚.png")),
            "{check:?}"
        );
        // 已经声明过模式，就不该再催问
        assert!(
            !check.warnings.iter().any(|w| w.contains("还没声明")),
            "{check:?}"
        );
        let _ = std::fs::remove_dir_all(pkg.parent().unwrap().parent().unwrap());
    }

    #[test]
    fn self_check_asks_for_asset_mode_when_undeclared() {
        let (pkg, data_dir) = tmp_script_package("e2e-undeclared", BAD_CHAPTER, "");
        let check = check_chapter(&pkg, "01", &data_dir).unwrap();
        assert!(
            check
                .warnings
                .iter()
                .any(|w| w.contains("素材模式") && w.contains("还没声明")),
            "{:?}",
            check.warnings
        );
        let _ = std::fs::remove_dir_all(pkg.parent().unwrap().parent().unwrap());
    }

    #[test]
    fn self_check_skips_clean_chapter() {
        let clean = "name: 雨夜\nevents:\n  - type: chapter_end\n    next: end\n";
        let (pkg, data_dir) = tmp_script_package("e2e-clean", clean, "- 素材模式：只用已有\n");
        assert_eq!(check_chapter(&pkg, "01", &data_dir), None);
        let _ = std::fs::remove_dir_all(pkg.parent().unwrap().parent().unwrap());
    }

    #[test]
    fn switch_impact_covers_every_written_chapter() {
        let (pkg, data_dir) = tmp_script_package("impact", BAD_CHAPTER, "- 素材模式：先用已有的\n");
        // 02 也留了一个空
        std::fs::write(
            pkg.join("Chapters/02.yaml"),
            "name: 第二章\nevents:\n  - type: music\n    musicPath: 走廊.mp3\n  - type: chapter_end\n    next: end\n",
        )
        .unwrap();

        let mut snap = snap_of(&pkg);
        snap.plan = vec!["01".into(), "02".into()];
        snap.written = vec!["01".into(), "02".into()];

        let missing = missing_assets_of_written(&snap, &pkg, &data_dir);
        assert_eq!(missing, vec!["01 夜晚.png", "02 走廊.mp3"], "{missing:?}");

        // 「先用已有的」不是规范写法 → 当未声明，所以影响面那行是"先问用户"
        let block = progress_block(&snap, &pkg, &data_dir);
        assert!(block.contains("素材模式：未声明"), "{block}");
        assert!(block.contains("唯一来源"), "{block}");
        assert!(block.contains("01 夜晚.png"), "{block}");
        let _ = std::fs::remove_dir_all(pkg.parent().unwrap().parent().unwrap());
    }

    #[test]
    fn self_check_ignores_chapters_of_other_scripts() {
        let (pkg, _) = tmp_script_package("e2e-other", BAD_CHAPTER, "- 素材模式：只用已有\n");
        let other = pkg
            .parent()
            .unwrap()
            .join("B")
            .join("Chapters")
            .join("01.yaml");
        assert!(
            chapter_of_write(&snap_of(&pkg), other.to_str().unwrap()).is_none(),
            "别的剧本的章节不该拿来自检"
        );
        // 自己的章节认得出来
        let mine = pkg.join("Chapters").join("01.yaml");
        assert_eq!(
            chapter_of_write(&snap_of(&pkg), mine.to_str().unwrap()).map(|(_, id)| id),
            Some("01".to_string())
        );
        let _ = std::fs::remove_dir_all(pkg.parent().unwrap().parent().unwrap());
    }

    // ---------- 上下文预算 ----------

    /// 造一轮：user + assistant(tool_calls) + tool 结果。
    fn tool_turn(
        user: &str,
        call_id: &str,
        tool: &str,
        path: &str,
        result: &str,
    ) -> Vec<LlmMessage> {
        let args = format!(r#"{{"path":"{path}"}}"#);
        let mut assistant = LlmMessage::assistant("我来看一眼。");
        assistant.tool_calls = Some(vec![crate::ai_service::types::ToolCall {
            id: call_id.into(),
            type_: "function".into(),
            function: crate::ai_service::types::FunctionCall {
                name: tool.into(),
                arguments: args,
            },
        }]);
        vec![
            LlmMessage::user(user),
            assistant,
            LlmMessage::tool_result(call_id, result),
            LlmMessage::assistant("看完了。"),
        ]
    }

    /// 造一段"很长"的只读结果。
    fn long_read(call_id: &str, path: &str) -> Vec<LlmMessage> {
        let long = "章节正文。".repeat(400); // 2000 字 > MIN_FOLD_CHARS
        tool_turn("读一下", call_id, "read_file", path, &long)
    }

    #[test]
    fn small_windows_keep_a_sane_input_cap() {
        // 预留必须随窗口缩小：定值 128K 会让 200k 窗口的硬线只剩 72k
        assert_eq!(input_cap(1_048_576), 1_048_576 - OUTPUT_RESERVE_TOKENS);
        assert_eq!(input_cap(200_000), 200_000 - 25_000);
        assert_eq!(input_cap(64_000), 64_000 - 8_000);
        assert!(
            input_cap(200_000) > 150_000,
            "200k 窗口的硬线不该被压到 150k 以下"
        );
    }

    #[test]
    fn estimate_stays_conservative_but_not_absurd() {
        let zh = LlmMessage::user("字".repeat(100));
        let en = LlmMessage::user("a".repeat(100));
        let zh_tokens = estimate_tokens(&[zh]);
        let en_tokens = estimate_tokens(&[en]);
        // 真机校准：中文实际约 0.7 token/字，这里取 0.85（估高 = 安全方向）
        assert!(
            (85..=100).contains(&zh_tokens),
            "中文估计应当略高于实测的 0.7/字：{zh_tokens}"
        );
        assert!(
            en_tokens < zh_tokens,
            "英文应当明显更便宜：{en_tokens} vs {zh_tokens}"
        );
    }

    #[test]
    fn small_history_is_untouched() {
        let mut messages = vec![LlmMessage::system("系统"), LlmMessage::user("你好")];
        let before = estimate_tokens(&messages);
        let outcome = apply_context_budget(&mut messages, DEFAULT_CONTEXT_WINDOW);
        assert_eq!(outcome, BudgetOutcome::Untouched { tokens: before });
        assert_eq!(messages.len(), 2, "没到水位就不许动");
    }

    #[test]
    fn old_read_results_are_folded_and_recent_ones_kept() {
        let mut messages = vec![LlmMessage::system("系统")];
        // 三轮旧历史 + 两轮"最近"（保护区）—— 最近两轮里的长结果不能折
        messages.extend(long_read("c1", ".agent/design.md"));
        messages.extend(long_read("c2", "Chapters/01.yaml"));
        messages.extend(long_read("c3", "Chapters/02.yaml"));
        messages.extend(long_read("c4", "Chapters/03.yaml"));
        messages.extend(long_read("c5", "Chapters/04.yaml"));

        let window = 7_000; // cap=5,250 / trigger=4,200 → 折完刚好落回去
        let outcome = apply_context_budget(&mut messages, window);
        let BudgetOutcome::Trimmed { folded_reads, .. } = outcome else {
            panic!("应当折叠：{outcome:?}");
        };
        assert!(folded_reads >= 2, "旧的只读结果应当被折：{folded_reads}");

        let folded: Vec<&str> = messages
            .iter()
            .filter(|m| m.content.starts_with("[已折叠]"))
            .map(|m| m.content.as_str())
            .collect();
        assert!(folded.iter().any(|c| c.contains("design.md")), "{folded:?}");

        // 最近两轮的长结果保持原文（模型正在用）
        let last = messages
            .iter()
            .filter(|m| m.role == "tool")
            .next_back()
            .expect("还有 tool 消息");
        assert!(
            !last.content.starts_with("[已折叠]"),
            "最近一轮的结果不该被折"
        );
    }

    #[test]
    fn user_words_and_pairing_survive() {
        let mut messages = vec![LlmMessage::system("系统")];
        messages.extend(long_read("c1", "a.md"));
        messages.extend(long_read("c2", "b.md"));
        messages.extend(long_read("c3", "c.md"));
        messages.extend(long_read("c4", "d.md"));

        apply_context_budget(&mut messages, 4000);

        assert!(
            messages.iter().any(|m| m.content == "读一下"),
            "user 原话要留"
        );
        // 结构不许坏：每个 tool 都要有其 assistant(tool_calls) 主
        let mut pending: Vec<&str> = Vec::new();
        for m in &messages {
            if let Some(calls) = m.tool_calls.as_deref() {
                pending.extend(calls.iter().map(|c| c.id.as_str()));
            }
            if m.role == "tool" {
                let id = m.tool_call_id.as_deref().unwrap_or("");
                assert!(
                    pending.iter().any(|p| *p == id),
                    "孤儿 tool 消息会导致 400：{id}"
                );
            }
        }
    }

    #[test]
    fn nothing_useless_to_drop_stays_untouched() {
        // 水位到了，但历史里全是"不能丢"的东西（用户原话 + 最近两轮，没有旧工具结果）
        // → 在硬线内就原样发，**不许拿"折了 0 条"去打扰用户**。
        let mut messages = vec![
            LlmMessage::system("系统"),
            LlmMessage::user(&"字".repeat(1300)),
            LlmMessage::assistant("好。"),
            LlmMessage::user(&"字".repeat(1300)),
        ];
        let before = estimate_tokens(&messages);
        let outcome = apply_context_budget(&mut messages, 4_000);
        assert_eq!(outcome, BudgetOutcome::Untouched { tokens: before });
        assert!(outcome.note().is_none(), "没什么可丢时不该给用户发状态");
    }

    #[test]
    fn pure_note_turns_are_not_digested() {
        // 队列各项的交接材料就是"只有一句 user 话"的回合：压它只会插噪音，
        // 所以它既不算被压、也不该多出一条「这一轮没有调用工具」。
        let mut messages = vec![
            LlmMessage::system("系统"),
            LlmMessage::user("【本轮第 1 项 · 收集设想 · 产出】我问了他几个问题"),
            LlmMessage::user("【本轮第 2 项 · 落盘章节 · 产出】落了第一章"),
            LlmMessage::user("【待写章节 · 02】…"),
        ];
        let before = estimate_tokens(&messages);
        let outcome = apply_context_budget(&mut messages, 4_000);
        assert_eq!(outcome, BudgetOutcome::Untouched { tokens: before });
        assert!(
            !messages.iter().any(|m| m.content.contains("[摘要]")),
            "纯交接材料回合不该被压成摘要：{:?}",
            messages
                .iter()
                .map(|m| m.content.as_str())
                .collect::<Vec<_>>()
        );
    }

    #[test]
    fn impossible_budget_refuses_to_send() {
        // 窗口小到连"最近一轮原文"都塞不下 → 只能主动报错
        let mut messages = vec![LlmMessage::system("系统")];
        messages.extend(long_read("c1", "a.md"));
        messages.extend(long_read("c2", "b.md"));
        let outcome = apply_context_budget(&mut messages, 100);
        assert!(
            matches!(outcome, BudgetOutcome::TooLong { .. }),
            "{outcome:?}"
        );
        assert!(!budget_allows_send(&outcome), "TooLong 一律不许发请求");
        let note = outcome.note().expect("要给用户一句话");
        assert!(note.contains("新开一个对话"), "{note}");
    }

    #[test]
    fn older_turns_become_one_line_digests() {
        // 只在"折不动"的东西撑大时才走第 ② 级 —— 所以用 write_file 的大结果
        // （写类结果永不折：它带着 [章节自检]，折了会重复写、漏改）。
        let big = "已写入 Chapters/01.yaml（3,600 字节）".to_string() + &"e".repeat(3_600);
        let mut messages = vec![LlmMessage::system("系统")];
        for (i, id) in ["c1", "c2", "c3"].iter().enumerate() {
            messages.push(LlmMessage::user(format!("第{i}件事")));
            messages.push(LlmMessage::assistant("好。"));
            messages.extend(tool_turn(
                "写一下",
                id,
                "write_file",
                "Chapters/01.yaml",
                &big,
            ));
        }
        // 最近两轮：正常的只读结果（要被保护）
        messages.extend(long_read("c4", "Chapters/02.yaml"));
        messages.extend(long_read("c5", "Chapters/03.yaml"));

        let outcome = apply_context_budget(&mut messages, 6_000);
        let BudgetOutcome::Trimmed { digested_turns, .. } = outcome else {
            panic!("应当走到摘要级：{outcome:?}");
        };
        assert!(
            digested_turns >= 3,
            "更早的轮次要被压成摘要：{digested_turns}"
        );
        assert!(
            messages.iter().any(|m| m.content.contains("[摘要]")),
            "应当有摘要行"
        );
        assert!(
            messages
                .iter()
                .any(|m| m.role == "user" && m.content.starts_with("第0件事")),
            "更早轮次的 user 原话仍要留原文"
        );
    }
}
