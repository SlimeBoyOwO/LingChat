//! 角色与任务类型：流程 Agent 的词汇表。
//!
//! 「任务类型 → 角色 → 手册 / 工具 / 职责边界」这三张映射只在这里写一遍。
//! 流程 Agent 的输出必须是这里的**封闭取值**（工具参数的 enum），解析时按 key 严格匹配；
//! 认不出就是认不出 —— 宁可回落到按阶段推的老行为，也不要猜。

/// 任务类型。每一类最多归一个角色。
///
/// **`Chat` 与其余八类的分界线是「谈不谈这个剧本」**：`Chat` 跟剧本无关，
/// 不注入任何手册、工具只给只读、边界写明不动文件。**意图认不出时一律落到 `Chat`** ——
/// 猜错的代价是乱改文件，只回话的代价只是多问一句。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TaskKind {
    /// 仅对话：跟这个剧本无关的闲聊、问软件怎么用。只读、不落盘、不注入手册。
    Chat,
    /// 向用户要信息（这一章讲什么、一共多少章、素材怎么来）。
    Collect,
    /// 复述整理用户的设想。产物是话术，**不落笔**。
    Summarize,
    /// 编写剧本：把剧本写出来（粗梗概那一步 —— 每章一两句话）。
    /// 落盘与否看 `RoutePlan::land`：用户说"写剧本"只是**写给他看**，说"落盘"才写进
    /// `.agent/design.md`。
    Outline,
    /// 改已有的剧本设计。
    ReviseOutline,
    /// 写章节并落盘。
    WriteChapter,
    /// 改已落盘章节的内容。
    ReviseChapter,
    /// 素材盘点：要什么、有什么、差什么。
    CheckAssets,
    /// 校验与修复。
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

    /// 工具参数与队列文件共用的稳定标识。
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

    /// 按 key 严格匹配（忽略大小写与首尾空白）。认不出返回 `None`。
    pub fn parse(raw: &str) -> Option<Self> {
        let key = raw.trim().to_ascii_lowercase();
        Self::ALL.into_iter().find(|k| k.key() == key)
    }

    /// 中文标签：进交接单、队列文件与回执。
    ///
    /// **只有「落盘章节」这个名字带"落盘"** —— 名字就是这个任务会不会写文件的答案。
    /// 其余任务名只说做什么内容；落不落盘由 [`ACTION_VOCAB`] 那套规则与流程 Agent 的
    /// `land` 字段决定（用户说"写大纲"是纯写，说"落盘"才是写文件）。
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

    /// 这一轮该预注入的手册（相对技能目录，按顺序）。
    ///
    /// **按任务给，不按阶段给**：实录显示同一个阶段里「只提问的那一轮」
    /// 根本用不到落盘手册（R1 注入了 6363 字符、0 次写文件）。
    pub const fn materials(self) -> &'static [&'static str] {
        match self {
            TaskKind::Chat => &[],
            TaskKind::Collect | TaskKind::Summarize => &[WRITER_DOC],
            // 写设计稿要写 story_config.yaml，而字段表只在那份参考里
            TaskKind::Outline | TaskKind::ReviseOutline => &[WRITER_DOC, STORY_CONFIG_REF],
            // 落盘 + 事件字段 + 章节模板 + 设计原则（实录里模型为它专门读了一次）
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

    /// 本轮的行为要求（原来挂在"阶段"上，现在挂到任务上 —— 任务才是这一轮的真单位）。
    ///
    /// 一律以**队列**为准：队列是用户这一句里明确要的东西，队列没做完不许收尾。
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

    /// 这一轮能用的工具。`Chat` 只有只读的那两个。
    pub const fn tools(self) -> &'static [&'static str] {
        match self.role() {
            Some(role) => role.tools(),
            None => CHAT_TOOLS,
        }
    }

    /// 本轮的职责边界补充（在底线之外多说一句）。
    ///
    /// 措辞一律按**产出物**说，不写"不要直接书写"这类禁令 —— 禁令是被动约束，
    /// 而"这一轮的产出物是什么"是任务定义：产出物错了就是任务分错了，可测。
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

/// 仅对话能用的工具：**只读**。
///
/// 意图认不出时按仅对话处理，这条限制保证"猜错"不破坏任何文件 ——
/// 最坏的后果是它回一句「这个我改不了，你是想改哪儿吗」。
pub const CHAT_TOOLS: &[&str] = &[TOOL_LIST_FILES, TOOL_READ_FILE];

/// 口述轮能用的工具：**只读**。有正经任务（写剧本、写章节），只是**用户没让落盘**，
/// 所以只让他把内容写进回复。
///
/// `validate_script` 留着：它是**只读**的，"跑一遍校验"本来就该给报告 ——
/// 收掉它只会让"口述版校验"变成什么都干不了。
pub const DICTATE_TOOLS: &[&str] = &[TOOL_LIST_FILES, TOOL_READ_FILE, TOOL_VALIDATE_SCRIPT];

/// 「写」与「落盘」的分界。**流程 Agent 与角色 Agent 都要看到这一段**，
/// 一处定义、两处注入 —— 两边口径不一致时，最先崩的就是用户。
///
/// 由来：用户说「把这段剧情写出来」，模型直接落盘成 `Chapters/*.yaml`。
/// 根因是整套词表里"落盘"这个意思全由"写"承担，谁也分不出他要文字还是要文件。
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

// 手册路径。角色手册由 [`Role::doc`] 拥有，这里只是给切片字面量用的别名。
const WRITER_DOC: &str = Role::Writer.doc();
const DEMANDER_DOC: &str = Role::Demander.doc();
const TRANSFORMER_DOC: &str = Role::Transformer.doc();
const OPTIMIZER_DOC: &str = Role::Optimizer.doc();
/// 剧本配置字段表 —— 写 `story_config.yaml` 只在这份里有。
const STORY_CONFIG_REF: &str = "lingchat-script-editor/references/story-config-reference.md";
/// 17 种事件的字段与默认值。
const EVENT_REF: &str = "lingchat-script-editor/references/event-reference.md";
const CHAPTER_TEMPLATE: &str = "lingchat-script-editor/assets/templates/chapter_template.yaml";
/// 设计原则示范。实录里 4 本手册 4 处指向它，模型为它专门 read_file 了一次。
const DESIGN_PRINCIPLES: &str = "lingchat-script-editor/references/design-principles.md";

/// 四个角色。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Role {
    Writer,
    Demander,
    Transformer,
    Optimizer,
}

impl Role {
    /// 四个角色的全集。只有测试在遍历它（用来验证"每个角色都有任务类型"）。
    #[allow(dead_code)]
    pub const ALL: [Role; 4] = [
        Role::Writer,
        Role::Demander,
        Role::Transformer,
        Role::Optimizer,
    ];

    /// 稳定标识（队列文件与日志里用；目前只有测试在读）。
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

    /// 本角色能用的工具，**封闭集合**。
    ///
    /// 不在这里的工具，模型就算硬调也会被拒 —— 光换手册、工具仍全给是藏不住的：
    /// 模型看得到全部工具的说明，也就等于知道"还有别的路可以走"。
    /// 所有角色都不给 `read_skill` / `list_skills`（连"去拿别的手册"的手段都没有）。
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
///
/// 「把队列做完」是这一版的要害：用户一轮里提了三件事，就得三件都做完再收尾。
/// 早先这里写的是「不要自己往下做下一步」，等于命令它做一半就返回。
pub const BOUNDARY_BASELINE: &str = "把本轮队列里的事**全部做完**再收尾；不要做队列以外的事；\
     某一项需要用户给信息才能做，就停下来问，并说明剩下的还在队列里";

/// 边界补充的长度上限（字符数，不是字节数 —— 中文按字算）。
pub const BOUNDARY_MAX_CHARS: usize = 40;

/// 一出现就说明它在给自己放宽范围，整条补充丢弃。
const BOUNDARY_FORBIDDEN: [&str; 6] = ["顺便", "也把", "补上", "一起写", "直接写完", "不用问"];

/// 校验流程 Agent 给出的边界补充。
///
/// 规则只有一条：**它只能把边界说得更谨慎，不能放宽底线。** 所以拿不准就丢掉，
/// 只用底线 —— 最坏情况等于退回现状，不会更差。
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

/// 本轮交给模型的职责边界：底线一行 + 补充一行。
pub fn render_boundary(supplement: Option<&str>) -> String {
    match supplement {
        Some(s) => format!("{BOUNDARY_BASELINE}\n本轮补充：{s}"),
        None => BOUNDARY_BASELINE.to_string(),
    }
}

/// 承接任务时需要的磁盘事实。每轮重算，不落库。
///
/// 刻意**独立于 `Stage`**：状态是这三个事实的投影，投影会丢信息，
/// 而"能不能做这件事"要看的正是这几个事实本身。
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct ScriptFacts {
    /// 设计稿**文件**存在且非空。
    pub has_design: bool,
    /// 设计稿**能解析出章节列表**。与 `has_design` 分开：文件在但格式不对是常有的事，
    /// 那是"提醒他补格式"，不是"这件事做不了"。
    pub has_plan: bool,
    /// 至少有一章落盘。
    pub has_written: bool,
    /// 设计稿里还有没落盘的章节。
    pub has_next: bool,
    /// 用户点名的目标存在吗；`None` = 这一轮没点名。
    pub target_exists: Option<bool>,
    /// 点名的目标里**有的在、有的不在**（`01-04` 而 04 还没落盘）。
    /// 这类要照做能做的部分并说明缺哪几章，不能整轮拒绝。
    pub target_partial: bool,
    /// 用户点名要写的那一章，正好是设计稿里**下一个待写**的章。
    ///
    /// 用来认出"跳着写"：设计稿有 7 章、只落盘了 3 章时直接写 07，
    /// 上一章的 `chapter_end` 还指着不存在的 04、变量也接不上 —— 引擎从 03 走就断了。
    /// 用户确实认可这种拒绝（试玩反馈）。
    pub target_is_next: bool,
}

/// 这一轮的任务能不能直接做；不能的话往哪走。
///
/// **每个格子都必须有确定答案，没有"其他"兜底分支** —— 用户在任何状态下
/// 跳到任何操作，都要么做它、要么给出前置、要么说清缺什么。
///
/// **`Prerequisite` 只在"这件事真的做不了"时才给**（比如没有章节可改）。
/// 用户已经明确要求做的事，不能因为"我们读不出设计稿"就被换掉 ——
/// 那种情况给 `ProceedNote`：照做，同时把问题告诉他。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Handoff {
    /// 就做它。
    Proceed,
    /// 照做，但交接单里带一句提醒（不换任务、不动工具）。
    ProceedNote(&'static str),
    /// 先做前置（同一轮可以紧接着做）。
    Prerequisite { first: TaskKind, why: &'static str },
    /// 当前状态做不了，也不该硬做：说清楚，不动文件。
    Explain(&'static str),
}

/// 承接判断。判据全部来自 [`ScriptFacts`]，可被代码重算。
///
/// 前置**只补一级、不递归**：补完之后磁盘事实会变，下一轮自然落到能做的那一档。
/// 一轮连补两级，模型很容易一口气做过头。
pub fn reconcile(kind: TaskKind, facts: ScriptFacts) -> Handoff {
    use TaskKind as K;

    match kind {
        // 只回话 / 只提问 / 只复述 / 只写大纲 / 只盘点：任何状态都能做
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

        // 写章节**永远做得成**：用户说了要写就写。设计稿缺失或格式读不出，
        // 只是"顺手补上/补对"的提醒 —— 早先把它做成硬前置，结果是
        // 用户喊着「把第一二章落盘」，它却一直去写大纲。
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
                // 下一个待写章，或者点名的章已经在磁盘上（重写）→ 直接做
                Handoff::Proceed
            } else {
                // 跳着写：前面还有没落盘的章。不是不许，是**会接不上** ——
                // 上一章的 chapter_end 指向的正是缺的那一章，变量也还没产生。
                // 用户没明说"知道会接不上、就要跳着写"时，先把这件事说清楚。
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
                // `01-04` 而 04 还没落盘：改能改的，并说清哪几章还没有 ——
                // 早先这里整轮拒绝，队列里别的项跟着一起废掉。
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
                // 没写完时整剧校验必然报一批"尚未写完"的假错。这里只提醒、不拒绝：
                // 用户可能就是想让已落盘的那几章过一遍。
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

/// 把承接结果变成交给模型的一行说明（`Proceed` 时为空）。
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

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashSet;
    use std::path::Path;

    #[test]
    fn every_role_has_at_least_one_task_kind() {
        let used: HashSet<&str> = TaskKind::ALL
            .iter()
            .filter_map(|k| k.role())
            .map(|r| r.key())
            .collect();
        for role in Role::ALL {
            assert!(used.contains(role.key()), "{role:?} 没有任何任务类型");
        }
    }

    #[test]
    fn chat_is_the_only_kind_without_a_role() {
        for kind in TaskKind::ALL {
            assert_eq!(kind.role().is_none(), kind == TaskKind::Chat, "{kind:?}");
        }
        assert!(
            TaskKind::Chat.materials().is_empty(),
            "仅对话不该注入任何手册"
        );
        // 写章节要落盘手册 + 事件字段 + 模板 + 设计原则（实录里这几样都真的被用到）
        let write = TaskKind::WriteChapter.materials();
        assert!(write.contains(&Role::Transformer.doc()), "{write:?}");
        assert!(write.len() >= 3, "写章节只注入一本手册不够：{write:?}");
        // 只提问的那一轮不需要落盘手册
        assert_eq!(TaskKind::Collect.materials(), &[Role::Writer.doc()]);
    }

    #[test]
    fn keys_are_unique_and_parse_round_trips() {
        let keys: HashSet<&str> = TaskKind::ALL.iter().map(|k| k.key()).collect();
        assert_eq!(keys.len(), TaskKind::ALL.len(), "任务 key 有重复");
        for kind in TaskKind::ALL {
            assert_eq!(TaskKind::parse(kind.key()), Some(kind));
            assert_eq!(TaskKind::parse(kind.label()), None, "中文标签不算 key");
        }
        // 大小写与空白容忍；认不出的（含近似写法）一律 None，不猜
        assert_eq!(
            TaskKind::parse("  WRITE_CHAPTER "),
            Some(TaskKind::WriteChapter)
        );
        assert_eq!(TaskKind::parse("write-chapter"), None);
        assert_eq!(TaskKind::parse("写章节"), None);
        assert_eq!(TaskKind::parse(""), None);
    }

    #[test]
    fn only_talk_kinds_do_not_touch_files() {
        // 只出话术的三类，边界说明里必须点明"不要写文件" —— 用产出物措辞，不写禁令
        for kind in [TaskKind::Chat, TaskKind::Collect, TaskKind::Summarize] {
            let note = kind.boundary_note().expect("只出话术的任务要有边界说明");
            assert!(
                note.contains("不要写文件") || note.contains("不要动任何文件"),
                "{kind:?} 的边界没点明不落笔：{note}"
            );
        }
        for kind in [TaskKind::Outline, TaskKind::WriteChapter, TaskKind::Polish] {
            assert_eq!(
                kind.boundary_note(),
                None,
                "{kind:?} 是要落盘的，不该说不写文件"
            );
        }

        // 仅对话是"认不出意图"的兜底，所以"不动文件"要做成硬约束：连写工具都不给
        assert_eq!(TaskKind::Chat.tools(), CHAT_TOOLS);

        // 收集 / 整理设想虽然也只出话术，但得能写 `.agent/constraints.md`
        // 把用户的答复记下来（例如素材模式），所以写工具照给 ——
        // 它们的"不落笔"靠边界说明；硬收窄会顺手堵掉合法动作。
        for kind in [TaskKind::Collect, TaskKind::Summarize] {
            assert!(
                kind.tools().contains(&TOOL_WRITE_FILE),
                "{kind:?} 要能把用户答复记进约束卡片"
            );
        }
    }

    // ---------- 承接表：状态 × 任务 ----------
    //
    // 这张表就是本期要保证的东西：用户在任何状态跳到任何操作，都有确定答案。
    // 五列对应计划里的 R/S · F(刚列完) · F(写一半) · P · M。

    /// `plan` 默认为与 `design` 一致（文件在且读得出）；格式读不出的情况单独造。
    ///
    /// 默认"点名的那一章就是下一个待写章" —— 也就是最常见的那条路；
    /// 跳章单独特判（见 `write_chapter_only_skipping_ahead_gets_a_note`）。
    fn facts(design: bool, written: bool, next: bool) -> ScriptFacts {
        ScriptFacts {
            has_design: design,
            has_plan: design,
            has_written: written,
            has_next: next,
            target_exists: None,
            target_partial: false,
            target_is_next: next,
        }
    }

    fn empty() -> ScriptFacts {
        facts(false, false, false) // R / S：连设计稿都没有
    }
    fn outlined() -> ScriptFacts {
        facts(true, false, true) // F：刚列完章节，一章还没写
    }
    fn midway() -> ScriptFacts {
        facts(true, true, true) // F：写了一半
    }
    fn finished() -> ScriptFacts {
        facts(true, true, false) // P：计划里的全写完了
    }
    fn legacy() -> ScriptFacts {
        facts(false, true, false) // M：改既有剧本，没有设计稿
    }

    fn all_states() -> [ScriptFacts; 5] {
        [empty(), outlined(), midway(), finished(), legacy()]
    }

    #[test]
    fn handoff_matrix_covers_every_state_and_task() {
        use Handoff::*;
        let first_of = |h: Handoff| match h {
            Prerequisite { first, .. } => Some(first),
            _ => None,
        };

        // 只回话 / 只提问 / 只复述 / 只写大纲 / 只盘点：五种状态都能做
        for kind in [
            TaskKind::Chat,
            TaskKind::Collect,
            TaskKind::Summarize,
            TaskKind::Outline,
            TaskKind::CheckAssets,
        ] {
            for f in all_states() {
                assert_eq!(reconcile(kind, f), Proceed, "{kind:?} 在 {f:?}");
            }
        }

        // 改大纲：没有设计稿就先写设计稿
        for f in [empty(), legacy()] {
            assert_eq!(
                first_of(reconcile(TaskKind::ReviseOutline, f)),
                Some(TaskKind::Outline),
                "{f:?}"
            );
        }
        for f in [outlined(), midway(), finished()] {
            assert_eq!(reconcile(TaskKind::ReviseOutline, f), Proceed, "{f:?}");
        }

        // 写章节：**永远做得成** —— 用户说了要写就写，设计稿的问题只是提醒。
        // （早先这里给的是 Prerequisite(Outline)，结果用户喊「把第一二章落盘」
        //   而它一直去写大纲，章节永远落不了盘。真机踩过。）
        for f in [empty(), legacy(), finished()] {
            assert!(
                matches!(reconcile(TaskKind::WriteChapter, f), ProceedNote(_)),
                "{f:?} 应当照做 + 提醒，而不是换任务"
            );
        }
        assert_eq!(reconcile(TaskKind::WriteChapter, outlined()), Proceed);
        assert_eq!(reconcile(TaskKind::WriteChapter, midway()), Proceed);

        // 设计稿文件在但读不出章节列表 → 也是照做 + 提醒补格式
        let unreadable = ScriptFacts {
            has_design: true,
            has_plan: false,
            ..empty()
        };
        assert!(matches!(
            reconcile(TaskKind::WriteChapter, unreadable),
            ProceedNote(note) if note.contains("id:")
        ));

        // 改章节：没有章节就是真做不了；有章节就能改
        for f in [empty(), outlined()] {
            assert!(
                matches!(reconcile(TaskKind::ReviseChapter, f), Explain(_)),
                "{f:?}"
            );
        }
        for f in [midway(), finished(), legacy()] {
            assert_eq!(reconcile(TaskKind::ReviseChapter, f), Proceed, "{f:?}");
        }
        // 点名的那一章不在磁盘上 → 说清，不做
        let missing = ScriptFacts {
            target_exists: Some(false),
            ..midway()
        };
        assert!(matches!(
            reconcile(TaskKind::ReviseChapter, missing),
            Explain(_)
        ));

        // 校验：没有章节才是真做不了；没写完只是提醒（别照着假错去提前补写）
        for f in [empty(), outlined()] {
            assert!(
                matches!(reconcile(TaskKind::Polish, f), Explain(_)),
                "{f:?}"
            );
        }
        assert!(matches!(
            reconcile(TaskKind::Polish, midway()),
            ProceedNote(_)
        ));
        for f in [finished(), legacy()] {
            assert_eq!(reconcile(TaskKind::Polish, f), Proceed, "{f:?}");
        }
    }

    #[test]
    fn every_state_task_combination_yields_an_answer() {
        // 兜底检查：没有空档，也不会 panic；且非 Proceed 的都带说明文字
        for kind in TaskKind::ALL {
            for f in all_states() {
                let text = render_handoff(reconcile(kind, f));
                match reconcile(kind, f) {
                    Handoff::Proceed => assert!(text.is_empty(), "{kind:?} 在 {f:?}"),
                    _ => assert!(
                        text.contains("本轮"),
                        "{kind:?} 在 {f:?} 的说明不像话：{text}"
                    ),
                }
            }
        }
    }

    #[test]
    fn one_prerequisite_is_always_enough() {
        // 补一级前置之后磁盘事实就变，原任务下一轮自然能做 —— 不在一轮里连补两级
        let Handoff::Prerequisite { first, .. } = reconcile(TaskKind::ReviseOutline, empty())
        else {
            panic!("没有设计稿时，改大纲应当先写大纲")
        };
        assert_eq!(first, TaskKind::Outline);
        // 设计稿写出来之后就能改了
        assert_eq!(
            reconcile(TaskKind::ReviseOutline, facts(true, false, true)),
            Handoff::Proceed
        );
    }

    #[test]
    fn write_chapter_only_skipping_ahead_gets_a_note() {
        // 设计稿 7 章、只落盘了 3 章，用户点名写 07：不是不许，是**会接不上**
        // （03 的 chapter_end 指着不存在的 04）。真机上模型自己拒绝了，用户认可。
        let skipping = ScriptFacts {
            has_design: true,
            has_plan: true,
            has_written: true,
            has_next: true,
            target_exists: Some(false),
            target_partial: false,
            target_is_next: false,
        };
        let Handoff::ProceedNote(note) = reconcile(TaskKind::WriteChapter, skipping) else {
            panic!("跳章要带一句说明，而不是默默照做")
        };
        assert!(note.contains("接不上"), "{note}");
        assert!(note.contains("跳着写"), "{note}");

        // 正常顺序（点名的就是下一个待写章）→ 无话可说，直接做
        let in_order = ScriptFacts {
            target_is_next: true,
            ..skipping
        };
        assert_eq!(
            reconcile(TaskKind::WriteChapter, in_order),
            Handoff::Proceed
        );

        // 点名的是已经落盘的章（重写）→ 也别拦
        let rewrite = ScriptFacts {
            target_exists: Some(true),
            ..skipping
        };
        assert_eq!(reconcile(TaskKind::WriteChapter, rewrite), Handoff::Proceed);

        // 计划里的都写完了（新加一章）→ 老规矩：提醒顺手补设计稿
        let extra = ScriptFacts {
            has_next: false,
            target_is_next: false,
            ..skipping
        };
        let Handoff::ProceedNote(note) = reconcile(TaskKind::WriteChapter, extra) else {
            panic!("新加的章应当照做 + 提醒补设计稿")
        };
        assert!(note.contains("新加的"), "{note}");
    }

    #[test]
    fn no_role_can_reach_other_manuals_or_shell() {
        for role in Role::ALL {
            let tools = role.tools();
            for banned in [
                "read_skill",
                "list_skills",
                "execute_command",
                "delete_file",
            ] {
                assert!(!tools.contains(&banned), "{:?} 不该拿到 {banned}", role);
            }
            assert!(tools.contains(&TOOL_WRITE_FILE), "{:?} 得能写", role);
        }
    }

    #[test]
    fn only_optimizer_can_run_validation() {
        assert!(Role::Optimizer.tools().contains(&TOOL_VALIDATE_SCRIPT));
        for role in [Role::Writer, Role::Demander, Role::Transformer] {
            assert!(!role.tools().contains(&TOOL_VALIDATE_SCRIPT), "{role:?}");
        }
    }

    #[test]
    fn role_docs_exist_on_disk() {
        // 路径写歪会在运行时变成「角色指令缺失」，在真机上才发现；这里直接核对磁盘。
        // 没带技能数据的环境（CI）跳过。
        let skills = Path::new(env!("CARGO_MANIFEST_DIR"))
            .parent()
            .map(|p| p.join("data/game_data/skills"));
        let Some(skills) = skills.filter(|p| p.is_dir()) else {
            return;
        };
        for role in Role::ALL {
            let path = skills.join(role.doc());
            assert!(
                path.is_file(),
                "{:?} 的手册不存在：{}",
                role,
                path.display()
            );
        }
    }

    #[test]
    fn boundary_supplement_must_be_short_and_cautious() {
        assert_eq!(
            sanitize_boundary("这轮只改设计稿，不要动已落盘章节").as_deref(),
            Some("这轮只改设计稿，不要动已落盘章节")
        );
        // 空白 / 超长（按字符算）→ 丢弃
        assert_eq!(sanitize_boundary("   "), None);
        assert_eq!(sanitize_boundary(&"谨慎".repeat(21)), None);
        // 放宽范围的措辞 → 整条丢弃
        for bad in ["顺便把第 3 章也写了", "也把后面的补上", "直接写完全部"] {
            assert_eq!(sanitize_boundary(bad), None, "{bad} 不该通过");
        }
    }

    #[test]
    fn boundary_render_keeps_baseline_always() {
        let with = render_boundary(Some("只改设计稿"));
        assert!(with.starts_with(BOUNDARY_BASELINE), "{with}");
        assert!(with.contains("只改设计稿"), "{with}");
        assert_eq!(render_boundary(None), BOUNDARY_BASELINE);
    }
}
