//! 角色与任务类型：流程 Agent 的词汇表。
//! 「任务类型 → 角色 → 手册 / 工具 / 职责边界」只在这里写一遍；模型输出必须是这里的**封闭取值**，
//! 按 key 严格匹配 —— 认不出就认不出（宁可回落到按阶段推的老行为，也不猜）。

/// 任务类型。每一类最多归一个角色。
/// **`Chat` 与其余八类的分界是「谈不谈这个剧本」**（不注入手册、工具只给只读、不动文件）；
/// **意图认不出时一律落到 `Chat`** —— 猜错的代价是乱改文件，只回话的代价只是多问一句。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TaskKind {
    Chat,
    Collect,
    Summarize,
    /// 编写剧本：只写**粗梗概**（每章一两句话）。说"写剧本"只是**写给他看**，落盘与否看 `RoutePlan::land`。
    Outline,
    ReviseOutline,
    WriteChapter,
    ReviseChapter,
    CheckAssets,
    Polish,
}

impl TaskKind {
    pub const ALL: [TaskKind; 9] = [
        TaskKind::Chat,
        TaskKind::Collect,
        TaskKind::Summarize,
        TaskKind::Outline,
        TaskKind::ReviseOutline,
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

    /// **只有「落盘章节」带"落盘"** —— 名字就是"这任务会不会写文件"的答案；其余名字只说做什么内容。
    pub const fn label(self) -> &'static str {
        match self {
            TaskKind::Chat => "仅对话",
            TaskKind::Collect => "收集设想",
            TaskKind::Summarize => "整理设想",
            TaskKind::Outline => "编写剧本",
            TaskKind::ReviseOutline => "改剧本",
            TaskKind::WriteChapter => "落盘章节",
            TaskKind::ReviseChapter => "改章节",
            TaskKind::CheckAssets => "素材盘点",
            TaskKind::Polish => "校验修复",
        }
    }

    /// 这一轮归哪个角色。`Chat` 没有角色 —— 它连手册都不注入。
    pub const fn role(self) -> Option<Role> {
        match self {
            TaskKind::Chat => None,
            TaskKind::Collect
            | TaskKind::Summarize
            | TaskKind::Outline
            | TaskKind::ReviseOutline => Some(Role::Writer),
            TaskKind::WriteChapter | TaskKind::ReviseChapter => Some(Role::Transformer),
            TaskKind::CheckAssets => Some(Role::Demander),
            TaskKind::Polish => Some(Role::Optimizer),
        }
    }

    /// 该预注入的手册（相对技能目录）。**按任务给，不按阶段给**：同一阶段里「只提问的那一轮」用不到落盘手册。
    pub const fn materials(self) -> &'static [&'static str] {
        match self {
            TaskKind::Chat => &[],
            TaskKind::Collect | TaskKind::Summarize => &[WRITER_DOC],
            // 写设计稿要写 story_config.yaml，而字段表只在那份参考里
            TaskKind::Outline | TaskKind::ReviseOutline => &[WRITER_DOC, STORY_CONFIG_REF],
            TaskKind::WriteChapter => &[
                TRANSFORMER_DOC,
                EVENT_REF,
                CHAPTER_TEMPLATE,
                DESIGN_PRINCIPLES,
            ],
            TaskKind::ReviseChapter => &[TRANSFORMER_DOC, EVENT_REF],
            TaskKind::CheckAssets => &[DEMANDER_DOC],
            // 修 YAML 要落盘手册，按诊断码表修复要校对手册
            TaskKind::Polish => &[OPTIMIZER_DOC, TRANSFORMER_DOC],
        }
    }

    /// 本轮的行为要求。一律以**队列**为准：队列是用户这一句里明确要的东西，队列没做完不许收尾。
    pub const fn directive(self) -> &'static str {
        match self {
            TaskKind::WriteChapter => {
                "粒度是「一章一个落盘单元」：**先把这一章的小说全文写进 `.agent/chapter-details.md`**\
                 的那一章那一节（起因 → 经过 → 结果，对白、神态、环境都写全；**不写素材名/变量/事件类型\
                 这类施工信息**），再按它写 `Chapters/<id>.yaml`；\
                 写完看 write_file 返回的 [章节自检]，有错误当场改好，这一章才算落盘。\
                 \n队列里还有别的章节，代码会另起一轮，不用你在这一轮里连着写。\
                 \n回执必须让人看得见成果：**哪一章（id + 标题）、落到哪个文件、这一章到底\
                 发生了什么（3~5 条，按事件顺序）、自检结果、设计稿里的下一章是哪一章**。\
                 只报「已落盘」或只报字节数等于没汇报。\
                 \n不要调用 validate_script：全剧没写完时它必然报一批「尚未写完」的假错\
                 （断链、不可达），照着改会把你引向提前补写后续章节。"
            },
            TaskKind::Outline => {
                "**回执里要把剧本贴出来给人看** —— 每章一行：`id` + 标题 + 一句梗概，\
                 末尾带上结局走向与预计章节数。只说「写完了 / 已落盘 / 多少字节」\
                 等于没交付：用户要看的正是这份清单。\
                 \n这一步只写**粗梗概**（每章一两句话）；用户要细的、或者要落盘章节时，\
                 把那一章的**小说全文**写进 `.agent/chapter-details.md`，别把设计稿堆成小说。"
            },
            TaskKind::ReviseOutline => {
                "回执要说清**改前是什么、改后是什么**，并把改过的章节那一行重新贴出来\
                 （`id` + 标题 + 梗概）；只报「已更新」等于没交付。\
                 只改点名的那几处，不要顺手重写没让动的章节。"
            },
            TaskKind::Polish => {
                "按诊断逐条修复；需要新编剧情内容才能补上的缺口交回用户，不要自行编造。\
                 回执要把**还剩哪些诊断**逐条列出来（带 `code` 与位置），别只说「校验通过」。\
                 修完、校验通过后，队列里若还有别的事继续做完再汇报。"
            },
            TaskKind::ReviseChapter => {
                "按用户提出的修改需求逐项修改，不要自行扩大改动范围，\
                 也不要顺手重写未被要求改动的部分；\
                 回执要说清改了哪一章、改前改后各是什么。\
                 队列里若还有别的项，继续做完再收尾。"
            },
            _ => "",
        }
    }

    pub const fn tools(self) -> &'static [&'static str] {
        match self.role() {
            Some(role) => role.tools(),
            None => CHAT_TOOLS,
        }
    }

    /// 措辞一律按**产出物**说，不写"不要直接书写"这类禁令 —— 禁令是被动约束，"产出物是什么"才是任务定义，错了就是任务分错了，可测。
    pub const fn boundary_note(self) -> Option<&'static str> {
        match self {
            TaskKind::Chat => Some(
                "这一轮跟剧本无关：产出物就是这段回复本身，不要动任何文件；\
                 若用户其实想改剧本，先问清楚改哪里",
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

/// 仅对话能用的工具：**只读** —— 认不出意图就按仅对话处理，保证"猜错"不破坏任何文件（最坏只是多问一句）。
pub const CHAT_TOOLS: &[&str] = &[TOOL_LIST_FILES, TOOL_READ_FILE];

/// 口述轮能用的工具：**只读**。`validate_script` 留着：它也是**只读**的，"跑一遍校验"本来就该给报告。
pub const DICTATE_TOOLS: &[&str] = &[TOOL_LIST_FILES, TOOL_READ_FILE, TOOL_VALIDATE_SCRIPT];

/// 「写」与「落盘」的分界。**流程 Agent 与角色 Agent 都要看到这一段**（一处定义、两处注入）。
/// 由来：用户说「把这段剧情写出来」，模型直接落盘成 `Chapters/*.yaml` —— 词表里"落盘"全由"写"承担，分不出要文字还是要文件。
pub const ACTION_VOCAB: &str = "\
【两个动作词，别混用】
- **写（口述）** = 把内容写出来给用户看，**只进回复，一个字都不写文件**。\
用户说「写大纲」「写第三章」「把这段剧情写出来」「写细一点」都是这个意思。
- **落盘** = 变成文件。落盘永远要指得出落到哪：剧本 → `.agent/design.md`；\
章节细节 → `.agent/chapter-details.md`；**可运行章节 → `Chapters/<id>.yaml`**；\
素材缺口 → `.agent/assets.md`；用户约束 → `.agent/constraints.md`。
- **只有下面两种情况才落盘**：①用户这一句明确说了落盘（「落盘」「生成章节」\
「写进 Chapters」「存下来」）；②你**上一轮问过要不要落盘、他答了「要 / 是 / 可以 / 落盘」**。
- **其余一律先写出来给他看**，并在结尾问一句，把区别说明白：\
「这一轮我只写了文字，没动文件；要落盘成 `<文件>` 就说一声『落盘』——落盘之后才会\
变成能跑的 YAML。」分不清要文字还是要文件时，按**写**处理（问一句的代价，比凭空生成文件小得多）。

【`chapter-details.md` 里写什么：小说，不是施工单】
用户会直接打开这个文件读，所以**每一章那一节就是这一章的小说原文**：
- 用写小说的笔法把这一章的**起因 → 经过 → 结果**写完整；
- **人物对白写成对白**（谁说的、说了什么），该有的神态、动作、环境描写都写上；
- 读起来要像一篇小说，用户看到的是**剧情本身**。
- **不要写技术清单**：素材文件名、变量名、事件类型（`input` / `choices` / `ai_dialogue`）、\
「玩家参与点」这类施工信息**一律不要** —— 那些是落盘时你自己处理的事，用户不是来看施工单的。";

/// 口述轮的统一要求：贴在任务块里，覆盖手册里"写完落盘"那部分。
pub const DICTATE_NOTE: &str = "\
【本轮是口述，不落盘】用户没让落盘（也没回答过「要不要落盘」）。\
这一轮**一个字都不许写进文件**：把内容完整写在回复里给他看。\
结尾按「两个动作词」那套说法问一句要不要落盘，并说清落盘会变成哪个文件。";

const WRITER_DOC: &str = Role::Writer.doc();
const DEMANDER_DOC: &str = Role::Demander.doc();
const TRANSFORMER_DOC: &str = Role::Transformer.doc();
const OPTIMIZER_DOC: &str = Role::Optimizer.doc();
/// 剧本配置字段表 —— 写 `story_config.yaml` 只在这份里有。
const STORY_CONFIG_REF: &str = "lingchat-script-editor/references/story-config-reference.md";
const EVENT_REF: &str = "lingchat-script-editor/references/event-reference.md";
const CHAPTER_TEMPLATE: &str = "lingchat-script-editor/assets/templates/chapter_template.yaml";
const DESIGN_PRINCIPLES: &str = "lingchat-script-editor/references/design-principles.md";

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Role {
    Writer,
    Demander,
    Transformer,
    Optimizer,
}

impl Role {
    #[allow(dead_code)]
    pub const ALL: [Role; 4] = [
        Role::Writer,
        Role::Demander,
        Role::Transformer,
        Role::Optimizer,
    ];

    #[allow(dead_code)]
    pub const fn key(self) -> &'static str {
        match self {
            Role::Writer => "writer",
            Role::Demander => "demander",
            Role::Transformer => "transformer",
            Role::Optimizer => "optimizer",
        }
    }

    pub const fn label(self) -> &'static str {
        match self {
            Role::Writer => "编剧",
            Role::Demander => "素材官",
            Role::Transformer => "落盘",
            Role::Optimizer => "校验",
        }
    }

    /// 本角色**唯一**预注入的手册（相对技能目录）。流程总纲另行注入，不算角色手册。
    pub const fn doc(self) -> &'static str {
        match self {
            Role::Writer => "script-writer/SKILL.md",
            Role::Demander => "script-demander/SKILL.md",
            Role::Transformer => "script-transformer/SKILL.md",
            Role::Optimizer => "script-optimizer/SKILL.md",
        }
    }

    /// 本角色能用的工具，**封闭集合** —— 不在里面的硬调也会被拒：光换手册、工具仍全给，模型就知道"还有别的路可以走"；都不给 `read_skill`。
    pub const fn tools(self) -> &'static [&'static str] {
        match self {
            Role::Writer | Role::Demander | Role::Transformer => {
                &[TOOL_LIST_FILES, TOOL_READ_FILE, TOOL_WRITE_FILE]
            },
            Role::Optimizer => &[TOOL_VALIDATE_SCRIPT, TOOL_READ_FILE, TOOL_WRITE_FILE],
        }
    }
}

pub const TOOL_LIST_FILES: &str = "list_files";
pub const TOOL_READ_FILE: &str = "read_file";
pub const TOOL_WRITE_FILE: &str = "write_file";
pub const TOOL_VALIDATE_SCRIPT: &str = "validate_script";

/// 工具全集。**只有"回落到按阶段推的老行为"时才这么给** —— 正常路径一律按角色收窄。
pub const ALL_TOOLS: &[&str] = &[
    "list_skills",
    "read_skill",
    TOOL_VALIDATE_SCRIPT,
    TOOL_LIST_FILES,
    TOOL_READ_FILE,
    TOOL_WRITE_FILE,
    "delete_file",
    "execute_command",
];

/// 职责边界的**系统底线**：代码写死，流程 Agent 不得覆盖。
/// 「把队列做完」是要害：早先这里写的是「不要自己往下做下一步」，等于命令它做一半就返回。
pub const BOUNDARY_BASELINE: &str = "把本轮队列里的事**全部做完**再收尾；不要做队列以外的事；\
     某一项需要用户给信息才能做，就停下来问，并说明剩下的还在队列里";

/// 边界补充的长度上限（字符数，不是字节数 —— 中文按字算）。
pub const BOUNDARY_MAX_CHARS: usize = 40;

/// 一出现就说明它在给自己放宽范围，整条补充丢弃。
const BOUNDARY_FORBIDDEN: [&str; 6] = ["顺便", "也把", "补上", "一起写", "直接写完", "不用问"];

/// 规则只有一条：**它只能把边界说得更谨慎，不能放宽底线** —— 拿不准就丢掉、只用底线，最坏等于退回现状。
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

/// 承接任务时需要的磁盘事实。每轮重算，不落库。
/// 刻意**独立于 `Stage`**：状态是这几个事实的投影，投影会丢信息，而"能不能做这件事"要看的正是事实本身。
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct ScriptFacts {
    /// 设计稿**文件**存在且非空。
    pub has_design: bool,
    /// 设计稿**能解析出章节列表**。与 `has_design` 分开：文件在但格式不对是常有的事 —— 那是"提醒他补格式"，不是"做不了"。
    pub has_plan: bool,
    pub has_written: bool,
    pub has_next: bool,
    /// 用户点名的目标存在吗；`None` = 这一轮没点名。
    pub target_exists: Option<bool>,
    /// 点名的目标里**有的在、有的不在**：照做能做的部分并说明缺哪几章，不能整轮拒绝（早先整轮拒绝，队列里别的项跟着一起废掉）。
    pub target_partial: bool,
    /// 用户点名要写的那一章正好是设计稿里**下一个待写**的章 —— 用来认出"跳着写"：直接写 07 时上一章的 `chapter_end` 还指着不存在的 04，引擎从 03 走就断了。
    pub target_is_next: bool,
}

/// 这一轮的任务能不能直接做；不能的话往哪走。**每个格子都必须有确定答案，没有"其他"兜底分支**。
/// **`Prerequisite` 只在"这件事真的做不了"时才给** —— 用户明确要求做的事，不能因为"读不出设计稿"就被换掉，那种情况给 `ProceedNote`。
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

/// 承接判断。判据全部来自 [`ScriptFacts`]，可被代码重算。前置**只补一级、不递归**：补完磁盘事实会变，下一轮自然落到能做的那一档。
pub fn reconcile(kind: TaskKind, facts: ScriptFacts) -> Handoff {
    use TaskKind as K;

    match kind {
        K::Chat | K::Collect | K::Summarize | K::Outline | K::CheckAssets => Handoff::Proceed,

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

        // 写章节**永远做得成**：用户说了要写就写。设计稿缺失或读不出只是"顺手补上"的提醒 ——
        // 早先把它做成硬前置，结果是用户喊着「把第一二章落盘」，它却一直去写大纲。
        K::WriteChapter => {
            if !facts.has_plan {
                if facts.has_design {
                    Handoff::ProceedNote(
                        "设计稿文件在，但读不出章节列表 —— 章节标题要 `## ` 开头、\
                         紧随其后的第一个非空行写 `id: <章节id>`；这一轮按用户说的写，顺手把格式补对",
                    )
                } else {
                    Handoff::ProceedNote(
                        "还没有设计稿；这一轮按用户说的写，并把设计稿补上，别让后面的章节没依据",
                    )
                }
            } else if !facts.has_next {
                Handoff::ProceedNote(
                    "设计稿里列的章节都写完了；这一章是新加的，记得顺手在设计稿里补一节",
                )
            } else if facts.target_is_next || facts.target_exists != Some(false) {
                Handoff::Proceed
            } else {
                Handoff::ProceedNote(
                    "用户点名的那一章不是设计稿里下一个待写的：前面还有没落盘的章，\
                     直接写它会接不上（上一章的 chapter_end 指向缺的那一章）。\
                     他没明说「知道会接不上，就要跳着写」时，先说清这件事再问一句；\
                     他确认了就照写，并在回执里说明它暂时接不上",
                )
            }
        },

        K::ReviseChapter => {
            if !facts.has_written {
                Handoff::Explain("还没有任何章节可以改")
            } else if facts.target_exists == Some(false) {
                Handoff::Explain("你点名的那一章还没有落盘，改不了它")
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
                // 没写完时整剧校验必然报一批"尚未写完"的假错；这里只提醒、不拒绝 —— 用户可能就是想让已落盘的那几章过一遍。
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
                "本轮实际要先做「{}」：{why}。做完再继续用户原本要的那件事。",
                first.label()
            )
        },
        Handoff::Explain(why) => format!("本轮不做这件事：{why}。把原因告诉用户，不要动任何文件。"),
    }
}
