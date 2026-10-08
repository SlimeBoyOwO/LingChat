//! 角色与任务类型：流程 Agent 的词汇表，「任务类型 → 角色 → 手册 / 工具 / 职责边界」只在这里写一遍。

/// 任务类型，每一类最多归一个角色；`Chat` 与其余八类的分界是「谈不谈这个剧本」（不注入手册、只给只读工具）。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TaskKind {
    Chat,
    Collect,
    Summarize,
    /// 编写大纲：只写粗梗概（每章一两句话），产物是设计稿；要不要产出能跑的章节看计划里有没有章节类任务。
    Outline,
    ReviseOutline,
    /// 写正文：把某一章的小说全文写进细节稿；与 [`TaskKind::Outline`] 是两个项，用户只要大纲时正文一个字都不写。
    DraftChapter,
    WriteChapter,
    ReviseChapter,
    CheckAssets,
    Polish,
}

impl TaskKind {
    pub const ALL: [TaskKind; 10] = [
        TaskKind::Chat,
        TaskKind::Collect,
        TaskKind::Summarize,
        TaskKind::Outline,
        TaskKind::ReviseOutline,
        TaskKind::DraftChapter,
        TaskKind::WriteChapter,
        TaskKind::ReviseChapter,
        TaskKind::CheckAssets,
        TaskKind::Polish,
    ];

    pub const fn key(self) -> &'static str {
        match self {
            TaskKind::Chat => "chat",
            TaskKind::Collect => "collect",
            TaskKind::Summarize => "summarize",
            TaskKind::Outline => "outline",
            TaskKind::ReviseOutline => "revise_outline",
            TaskKind::DraftChapter => "draft_chapter",
            TaskKind::WriteChapter => "write_chapter",
            TaskKind::ReviseChapter => "revise_chapter",
            TaskKind::CheckAssets => "check_assets",
            TaskKind::Polish => "polish",
        }
    }

    pub fn parse(raw: &str) -> Option<Self> {
        let key = raw.trim().to_ascii_lowercase();
        Self::ALL.into_iter().find(|k| k.key() == key)
    }

    /// 标签说的都是产物是什么：编剧写稿子（大纲 / 章节正文），落盘角色把内容转成 YAML（能跑的 `Chapters/<id>.yaml`）。
    pub const fn label(self) -> &'static str {
        match self {
            TaskKind::Chat => "仅对话",
            TaskKind::Collect => "收集设想",
            TaskKind::Summarize => "整理设想",
            TaskKind::Outline => "编写大纲",
            TaskKind::ReviseOutline => "改大纲",
            TaskKind::DraftChapter => "写正文",
            TaskKind::WriteChapter => "转成 YAML",
            TaskKind::ReviseChapter => "改 YAML",
            TaskKind::CheckAssets => "素材盘点",
            TaskKind::Polish => "校验体检",
        }
    }

    /// 这一轮归哪个角色。`Chat` 没有角色 —— 它连手册都不注入。
    pub const fn role(self) -> Option<Role> {
        match self {
            TaskKind::Chat => None,
            TaskKind::Collect
            | TaskKind::Summarize
            | TaskKind::Outline
            | TaskKind::ReviseOutline
            | TaskKind::DraftChapter => Some(Role::Writer),
            TaskKind::WriteChapter | TaskKind::ReviseChapter => Some(Role::Transformer),
            TaskKind::CheckAssets => Some(Role::Demander),
            TaskKind::Polish => Some(Role::Optimizer),
        }
    }

    /// 该预注入的手册（相对技能目录），按任务给而非按阶段给：同一阶段里「只提问的那一轮」用不到落盘手册。
    pub const fn materials(self) -> &'static [&'static str] {
        match self {
            TaskKind::Chat => &[],
            TaskKind::Collect | TaskKind::Summarize => &[WRITER_DOC],
            TaskKind::Outline | TaskKind::ReviseOutline => &[WRITER_DOC, STORY_CONFIG_REF],
            TaskKind::DraftChapter => &[WRITER_DOC],
            TaskKind::WriteChapter => &[TRANSFORMER_DOC, EVENT_REF, CHAPTER_TEMPLATE],
            TaskKind::ReviseChapter => &[TRANSFORMER_DOC, EVENT_REF],
            TaskKind::CheckAssets => &[DEMANDER_DOC],
            TaskKind::Polish => &[OPTIMIZER_DOC, TRANSFORMER_DOC],
        }
    }

    /// 本轮的行为要求，一律以队列为准：队列是用户这一句里明确要的东西，没做完不许收尾。
    pub const fn directive(self) -> &'static str {
        match self {
            TaskKind::WriteChapter => {
                include_str!("prompts/role_directive_write_chapter.md")
            },
            TaskKind::Outline => {
                include_str!("prompts/role_directive_outline.md")
            },
            TaskKind::DraftChapter => {
                include_str!("prompts/role_directive_draft_chapter.md")
            },
            TaskKind::ReviseOutline => {
                include_str!("prompts/role_directive_revise_outline.md")
            },
            TaskKind::Polish => {
                include_str!("prompts/role_directive_polish.md")
            },
            TaskKind::ReviseChapter => {
                include_str!("prompts/role_directive_revise_chapter.md")
            },
            _ => "",
        }
    }

    /// 这一轮能用的工具；角色级集合之外只有「改 YAML」多一个 `delete_file`（删章要能整份删掉 `Chapters/08.yaml`）。
    pub const fn tools(self) -> &'static [&'static str] {
        match self {
            TaskKind::ReviseChapter => TRANSFORMER_REVISE_TOOLS,
            _ => match self.role() {
                Some(role) => role.tools(),
                None => CHAT_TOOLS,
            },
        }
    }

    /// 这一轮要不要开思考模式，按任务类型给而非按阶段给：要开的是这一轮要做的那件事。
    pub const fn thinking(self) -> Option<bool> {
        match self {
            TaskKind::Collect
            | TaskKind::Summarize
            | TaskKind::Outline
            | TaskKind::ReviseOutline
            | TaskKind::DraftChapter => Some(true),
            TaskKind::Chat
            | TaskKind::WriteChapter
            | TaskKind::ReviseChapter
            | TaskKind::CheckAssets
            | TaskKind::Polish => Some(false),
        }
    }

    /// 这一类的产出物是不是能跑的章节文件（`Chapters/<id>.yaml` / `story_config.yaml`）：是的话这一轮就必然要产出能跑章节。
    pub const fn produces_runnable_chapters(self) -> bool {
        matches!(self, TaskKind::WriteChapter | TaskKind::ReviseChapter)
    }

    /// 这一类的产出物是不是 `.agent/` 下的稿子（设计稿 / 章节细节稿 / 素材缺口表）；是的话这一轮就给 `write_file`。
    pub const fn drafts_into_agent_dir(self) -> bool {
        matches!(
            self,
            TaskKind::Outline
                | TaskKind::ReviseOutline
                | TaskKind::DraftChapter
                | TaskKind::CheckAssets
        )
    }

    /// 这一项能动什么：贴在任务块里的硬约束，与工具层那两道闸门同一口径（见 `tools::draft_scope_refusal`）。
    pub fn scope_note(self, target: &str) -> String {
        let t = target.trim();
        let chap = if t.is_empty() {
            "点名的这一章".to_string()
        } else {
            format!("第 {t} 章")
        };
        let id = if t.is_empty() { "<id>" } else { t };
        match self {
            TaskKind::Outline => "只写 `.agent/design.md`（新建剧本那一轮，连带建包写的 \
                 `constraints.md` / `assets.md` 骨架也算这一项）。**`.agent/chapter-details.md` \
                 一个字都不要写**，`Chapters/` 与 `story_config.yaml` 也不许碰。"
                .to_string(),
            TaskKind::ReviseOutline => "只改 `.agent/design.md` 里点名的那几处（以及 \
                 `constraints.md` 里的章数口径）。删大纲那一章时，连带删 `.agent/chapter-details.md` \
                 里那一节（往下连带可以）；此外不许碰正文，也不许碰 `Chapters/`。"
                .to_string(),
            TaskKind::DraftChapter => format!(
                include_str!("prompts/role_scope_note_draft_chapter.md"),
                chap = chap
            ),
            TaskKind::WriteChapter => format!(
                include_str!("prompts/role_scope_note_write_chapter.md"),
                id = id
            ),
            TaskKind::ReviseChapter => format!(
                "只改或删 `Chapters/{id}.yaml`。**改的时候可以往上对齐**：涉及剧情内容时同步 \
                 `.agent/chapter-details.md` 里那一节，梗概也因此过时时顺带对齐 `.agent/design.md` 里那一行。\
                 **删的时候只删这一章的 YAML** —— 正文与大纲一个字都不许动（往上删不可以）。"
            ),
            TaskKind::CheckAssets => "只写 `.agent/assets.md`；别的文件都不许动。".to_string(),
            TaskKind::Polish => "只出体检报告，**不动任何文件**。".to_string(),
            TaskKind::Chat | TaskKind::Collect | TaskKind::Summarize => {
                "这一轮的产物就是这段回复，**不动任何文件**。".to_string()
            },
        }
    }

    /// 措辞以产物为主；只读轮可补一句「不要动文件」的禁令（见下）。
    pub const fn boundary_note(self) -> Option<&'static str> {
        match self {
            TaskKind::Chat => Some(
                "这一轮跟剧本无关：产出物就是这段回复本身，不要动任何文件；\
                 若用户其实想改大纲或改剧情，先问清楚改哪里",
            ),
            TaskKind::Collect => {
                Some("产出物是向用户提的问题（缺哪条问哪条）；不要写文件、不要编剧情")
            },
            TaskKind::Summarize => {
                Some("产出物是复述用户的想法 + 还缺哪条信息；不要写文件、不要往下编剧情")
            },
            _ => None,
        }
    }
}

/// 仅对话能用的工具（只读）：判不出这一轮要干什么就只回话，最坏只是多问一句，不会乱改文件。
pub const CHAT_TOOLS: &[&str] = &[TOOL_LIST_FILES, TOOL_READ_FILE];

/// 只读轮的通用工具集（`list_files` / `read_file` / `validate_script`，三个都是只读的）。
pub const DICTATE_TOOLS: &[&str] = &[TOOL_LIST_FILES, TOOL_READ_FILE, TOOL_VALIDATE_SCRIPT];

/// 只写 `.agent/` 稿子的轮次：有 `write_file` / `edit_file`，没有 `validate_script` / `delete_file`，往能跑的章节写会被挡回。
pub const DRAFT_TOOLS: &[&str] = &[
    TOOL_LIST_FILES,
    TOOL_READ_FILE,
    TOOL_WRITE_FILE,
    TOOL_EDIT_FILE,
];

/// 「写」与「转成 YAML」两个动作词的定义 + 细节稿里写什么；流程 Agent 与角色 Agent 都要看到（一处定义、两处注入）。
pub const ACTION_VOCAB: &str = include_str!("prompts/role_action_vocab.md");

/// 判任务的规则：这一句到底该产出文字还是产出能跑的章节；只注入流程 Agent（`router.rs`），角色 Agent 看不到。
pub const ROUTING_LAND_RULES: &str = include_str!("prompts/role_routing_land_rules.md");

/// 不产出能跑章节的那一轮的统一要求：贴在任务块里，覆盖手册里"写完就转成 YAML"那部分。
pub const DICTATE_NOTE: &str = include_str!("prompts/role_dictate_note.md");

const WRITER_DOC: &str = Role::Writer.doc();
const DEMANDER_DOC: &str = Role::Demander.doc();
const TRANSFORMER_DOC: &str = Role::Transformer.doc();
const OPTIMIZER_DOC: &str = Role::Optimizer.doc();
/// 剧本配置字段表 —— 写 `story_config.yaml` 只在这份里有。
const STORY_CONFIG_REF: &str = "lingchat-script-editor/references/story-config-reference.md";
const EVENT_REF: &str = "lingchat-script-editor/references/event-reference.md";
const CHAPTER_TEMPLATE: &str = "lingchat-script-editor/assets/templates/chapter_template.yaml";

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Role {
    Writer,
    Demander,
    Transformer,
    Optimizer,
}

impl Role {
    pub const fn label(self) -> &'static str {
        match self {
            Role::Writer => "编剧",
            Role::Demander => "素材官",
            Role::Transformer => "落盘",
            Role::Optimizer => "校验",
        }
    }

    /// 本角色唯一预注入的手册（相对技能目录）；流程总纲另行注入，不算角色手册。
    pub const fn doc(self) -> &'static str {
        match self {
            Role::Writer => "script-writer/SKILL.md",
            Role::Demander => "script-demander/SKILL.md",
            Role::Transformer => "script-transformer/SKILL.md",
            Role::Optimizer => "script-optimizer/SKILL.md",
        }
    }

    /// 本角色能用的工具，封闭集合：不在里面的硬调也会被拒，否则模型知道"还有别的路可以走"。都不给 `read_skill`。
    pub const fn tools(self) -> &'static [&'static str] {
        match self {
            Role::Writer | Role::Demander | Role::Transformer => &[
                TOOL_LIST_FILES,
                TOOL_READ_FILE,
                TOOL_WRITE_FILE,
                TOOL_EDIT_FILE,
            ],
            Role::Optimizer => DICTATE_TOOLS,
        }
    }
}

pub const TOOL_LIST_FILES: &str = "list_files";
pub const TOOL_READ_FILE: &str = "read_file";
pub const TOOL_WRITE_FILE: &str = "write_file";
pub const TOOL_EDIT_FILE: &str = "edit_file";
pub const TOOL_DELETE_FILE: &str = "delete_file";
pub const TOOL_VALIDATE_SCRIPT: &str = "validate_script";

/// 「改 YAML」能用的工具：比「转成 YAML」多一个 `delete_file`（删章要能整份删掉 `Chapters/08.yaml`）和一个 `validate_script`。
const TRANSFORMER_REVISE_TOOLS: &[&str] = &[
    TOOL_LIST_FILES,
    TOOL_READ_FILE,
    TOOL_WRITE_FILE,
    TOOL_EDIT_FILE,
    TOOL_DELETE_FILE,
    TOOL_VALIDATE_SCRIPT,
];

/// 职责边界的系统底线：代码写死，流程 Agent 不得覆盖；措辞必须是「本轮排给你的任务」，否则它会把上一轮剩下的行也做掉。
pub const BOUNDARY_BASELINE: &str = include_str!("prompts/role_boundary_baseline.md");

/// 边界补充的长度上限（字符数，不是字节数 —— 中文按字算）。
pub const BOUNDARY_MAX_CHARS: usize = 40;

/// 一出现就说明它在给自己放宽范围，整条补充丢弃。
const BOUNDARY_FORBIDDEN: [&str; 6] = ["顺便", "也把", "补上", "一起写", "直接写完", "不用问"];

/// 规则只有一条：它只能把边界说得更谨慎、不能放宽底线；拿不准就丢掉，只用底线。
pub fn sanitize_boundary(raw: &str) -> Option<String> {
    let text = raw.trim();
    if text.is_empty() || text.chars().count() > BOUNDARY_MAX_CHARS {
        return None;
    }
    if BOUNDARY_FORBIDDEN.iter().any(|w| text.contains(w)) {
        return None;
    }
    Some(text.to_string())
}

pub fn render_boundary(supplement: Option<&str>) -> String {
    match supplement {
        Some(s) => format!("{BOUNDARY_BASELINE}\n本轮补充：{s}"),
        None => BOUNDARY_BASELINE.to_string(),
    }
}

/// 承接任务时需要的磁盘事实，每轮重算、不落库；刻意独立于 `Stage`，因为投影会丢信息，而能不能做看的正是事实本身。
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct ScriptFacts {
    /// 设计稿文件存在且非空。
    pub has_design: bool,
    /// 设计稿能解析出章节列表；与 `has_design` 分开，文件在但格式不对只是要提醒补格式，不是做不了。
    pub has_plan: bool,
    pub has_written: bool,
    pub has_next: bool,
    /// 用户点名的目标存在吗；`None` = 这一轮没点名。
    pub target_exists: Option<bool>,
    /// 点名的目标里有的在、有的不在：照做能做的部分并说明缺哪几章，不能整轮拒绝。
    pub target_partial: bool,
    /// 点名的这一章正好是设计稿里下一个待写的章：用来认出"跳着写"，直接写 07 会让上一章的 `chapter_end` 指着不存在的 04。
    pub target_is_next: bool,
    /// 点名的这一章在 `.agent/chapter-details.md` 里有没有剧情小节；落盘只做转换，所以这是它唯一的前置。
    pub has_draft: bool,
}

/// 这一轮的任务能不能直接做，不能的话往哪走；每个格子都必须有确定答案，没有"其他"兜底分支。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Handoff {
    Proceed,
    /// 照做，但交接单里带一句提醒（不换任务、不动工具）。
    ProceedNote(&'static str),
    Prerequisite {
        first: TaskKind,
        why: &'static str,
    },
    /// 当前状态做不了，也不该硬做：说清楚，不动文件。
    Explain(&'static str),
}

/// 承接判断：判据全部来自 [`ScriptFacts`]、可被代码重算；前置只补一级不递归，补完磁盘事实变了，下一轮自然落到能做的那一档。
pub fn reconcile(kind: TaskKind, facts: ScriptFacts) -> Handoff {
    use TaskKind as K;

    match kind {
        K::Chat | K::Collect | K::Summarize | K::Outline | K::DraftChapter | K::CheckAssets => {
            Handoff::Proceed
        },

        K::ReviseOutline => {
            if facts.has_design {
                Handoff::Proceed
            } else {
                Handoff::Prerequisite {
                    first: K::Outline,
                    why: "还没有设计稿，先把设计稿写出来才谈得上改它",
                }
            }
        },

        K::WriteChapter => {
            if facts.target_exists.is_some() && !facts.has_draft {
                Handoff::Prerequisite {
                    first: K::DraftChapter,
                    why: "细节稿里还没有这一章的正文，先让编剧把它写出来",
                }
            } else if !facts.has_plan {
                if facts.has_design {
                    Handoff::ProceedNote(
                        "设计稿文件在，但读不出章节列表 —— 章节标题要 `## ` 开头、\
                         紧随其后的第一个非空行写 `id: <章节id>`；这一轮按用户说的写，\
                         读不出的格式问题记进回执（改设计稿归「改大纲」那一项）",
                    )
                } else {
                    Handoff::ProceedNote(
                        "还没有设计稿：这一轮按用户说的转；**大纲不归你写**（设计稿由「编写大纲」\
                         那一项产出），回执里说一句这一章没有大纲依据、要另排一项",
                    )
                }
            } else if !facts.has_next {
                Handoff::ProceedNote(
                    "设计稿里列的章节都写完了；这一章是新加的，**别顺手往设计稿里补一节** ——\
                     回执里说一句得再排一项「编写大纲 / 改大纲」把这一章补进设计稿",
                )
            } else if facts.target_is_next || facts.target_exists != Some(false) {
                Handoff::Proceed
            } else {
                Handoff::ProceedNote(include_str!("prompts/role_reconcile_write_chapter.md"))
            }
        },

        K::ReviseChapter => {
            if !facts.has_written {
                Handoff::Explain("还没有任何章节转成 YAML，没得改")
            } else if facts.target_exists == Some(false) {
                Handoff::Explain("你点名的那一章还没有转成 YAML，改不了它")
            } else if facts.target_partial {
                Handoff::ProceedNote(
                    "用户点名的章节里有还没落盘的：这一轮先改已经落盘的那些，\
                     并在回执里点明哪几章还没有、所以这轮没动",
                )
            } else {
                Handoff::Proceed
            }
        },

        K::Polish => {
            if !facts.has_written {
                Handoff::Explain("还没有章节可以校验")
            } else if facts.has_next {
                Handoff::ProceedNote(
                    "剧本还没写完：整剧校验会报一批「尚未写完」的假错（断链、不可达），\
                     不要照着那些去提前补写后续章节；只处理与已落盘章节有关的问题",
                )
            } else {
                Handoff::Proceed
            }
        },
    }
}

pub fn render_handoff(handoff: Handoff) -> String {
    match handoff {
        Handoff::Proceed => String::new(),
        Handoff::ProceedNote(note) => format!("本轮照做，但注意：{note}。"),
        Handoff::Prerequisite { first, why } => {
            format!(
                // 措辞必须说成"同一轮"：说「再继续」会被读成"下一轮还有机会"，它做完前置就收尾。
                "本轮实际要先做「{}」：{why}。**做完这一项，同一轮会接着做用户原本要的那件**——\
                 不用等下一轮、也不用问用户，两件都做完这一轮才算完。",
                first.label()
            )
        },
        Handoff::Explain(why) => format!("本轮不做这件事：{why}。把原因告诉用户，不要动任何文件。"),
    }
}
