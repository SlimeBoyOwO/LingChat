//! 事件 schema —— 16 种事件及其全部字段的**单一真相源**。
//!
//! 在这之前，同一份 schema 散落在三处：Rust 的 16 个 handler、前端
//! `src/types/script.ts` 的运行时 payload 类型、原型编辑器的 `constants/events.ts`。
//! 三者互不同步，直接导致原型产出的 `set_variable` / `chapter_end` 跑不通。
//!
//! 现在由 Rust 导出、前端只负责渲染。改引擎时**必须同步改这个文件**。
//!
//! # 词表的归属
//!
//! 不是所有取值都由 Rust 拥有：
//!
//! - **情绪**由前端拥有（`src/controllers/emotion/config.ts` 决定情绪→立绘
//!   文件名的映射），所以这里只标 `kind: "emotion"`，选项由前端填。
//! - **章节名**是每个剧本自己的，前端从已加载的章节列表填。
//! - **素材文件名**同理，前端从素材索引填。
//! - **角色**是 `MAIN` 加上该剧本 `characters/` 下的目录名。
//! - **背景特效**由 Rust 拥有（`background_effect_event::KNOWN_EFFECTS`），
//!   因为它对应前端组件是否存在，本文件直接引用那个常量。

use serde::Serialize;

use crate::ai_service::game_system::script_engine::events::background_effect_event::KNOWN_EFFECTS;

/// 字段该用什么控件渲染。
#[derive(Debug, Clone, Copy, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum FieldKind {
    /// 单行文本
    Text,
    /// 多行文本
    Textarea,
    /// 数字
    Number,
    /// 开关
    Bool,
    /// 固定候选项，选项在 `options` 里
    Select,
    /// 角色引用：MAIN + 剧本内 NPC，选项由前端填
    Character,
    /// 情绪：选项由前端的情绪表填
    Emotion,
    /// 章节引用：选项由前端从章节列表填，额外带一个「剧本结束」
    Chapter,
    /// 素材文件名：选项由前端从素材索引填，`asset_kind` 指明是哪一类
    Asset,
    /// `choices` 的选项列表（专用编辑器）
    ChoiceOptions,
    /// `chapter_end` 的分支列表（专用编辑器）
    BranchOptions,
    /// `set_variable` 的赋值组（专用编辑器）
    VarOptions,
    /// 触发条件：编辑器可生成简单关系式；运行时另支持数字比较及 `&&` / `||` 组合。
    Condition,
    /// 遗留字段：只展示、不可编辑、保存时原样保留
    Deprecated,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct FieldSpec {
    /// YAML 里的键名，**大小写与风格照抄引擎**（camelCase 与 snake_case 混用是现状）
    pub key: &'static str,
    pub label: &'static str,
    pub kind: FieldKind,
    pub required: bool,
    /// 素材类别，仅 `kind == Asset` 时有意义
    #[serde(skip_serializing_if = "Option::is_none")]
    pub asset_kind: Option<&'static str>,
    /// `kind == Select` 的候选项
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub options: Vec<String>,
    /// 与 `options` 对齐的显示名（可空）。比如 action 的选项值必须写引擎认的
    /// `show_character`，但下拉里想给作者看「show_character（显示角色）」。
    /// 空列表时前端直接显示 `options` 原文。
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub option_labels: Vec<String>,
    /// 缺省值的人类可读描述（不是真正的默认值，仅作占位提示）
    #[serde(skip_serializing_if = "Option::is_none")]
    pub placeholder: Option<&'static str>,
    /// 引擎真实默认值的人类可读描述（与引擎代码逐项核对）。
    /// 可选字段「不设置」时按此展示，避免作者猜不到默认是什么。
    #[serde(skip_serializing_if = "Option::is_none")]
    pub default_desc: Option<&'static str>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub hint: Option<&'static str>,
    /// 该字段当前是否可用。false 时编辑器禁用并展示 `hint`
    pub enabled: bool,
}

impl FieldSpec {
    fn new(key: &'static str, label: &'static str, kind: FieldKind) -> Self {
        FieldSpec {
            key,
            label,
            kind,
            required: false,
            asset_kind: None,
            options: Vec::new(),
            option_labels: Vec::new(),
            placeholder: None,
            default_desc: None,
            hint: None,
            enabled: true,
        }
    }
    fn required(mut self) -> Self {
        self.required = true;
        self
    }
    fn hint(mut self, h: &'static str) -> Self {
        self.hint = Some(h);
        self
    }
    fn placeholder(mut self, p: &'static str) -> Self {
        self.placeholder = Some(p);
        self
    }
    /// 标注引擎真实默认值（人类可读），供「不设置」选项展示
    fn default_desc(mut self, d: &'static str) -> Self {
        self.default_desc = Some(d);
        self
    }
    fn options<I: IntoIterator<Item = S>, S: Into<String>>(mut self, opts: I) -> Self {
        self.options = opts.into_iter().map(Into::into).collect();
        self
    }
    fn option_labels<I: IntoIterator<Item = S>, S: Into<String>>(mut self, labels: I) -> Self {
        self.option_labels = labels.into_iter().map(Into::into).collect();
        self
    }
    fn asset(mut self, kind: &'static str) -> Self {
        self.asset_kind = Some(kind);
        self
    }
    fn disabled(mut self, why: &'static str) -> Self {
        self.enabled = false;
        self.hint = Some(why);
        self
    }
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct EventSpec {
    /// YAML 的 `type:` 值
    pub type_key: &'static str,
    pub label: &'static str,
    /// 分组，用于事件面板的归类
    pub category: &'static str,
    /// 时间线上的语义色（十六进制）
    pub color: &'static str,
    pub fields: Vec<FieldSpec>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ScriptSchema {
    /// 16 种事件
    pub events: Vec<EventSpec>,
    /// 所有事件共有的字段（触发条件 / 事件间隔）
    pub common_fields: Vec<FieldSpec>,
    /// `story_config.yaml` 的字段
    pub story_config_fields: Vec<FieldSpec>,
    /// `choices` / `set_variable` 的 action 类型
    pub action_types: Vec<ActionSpec>,
    /// 羁绊冒险解锁条件类型
    pub unlock_condition_types: Vec<UnlockConditionSpec>,
    /// `%player%` 会被替换的字段名（仅顶层）
    pub placeholder_fields: Vec<&'static str>,
    /// condition 语法说明，直接展示给作者
    pub condition_syntax: ConditionSyntax,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ActionSpec {
    pub type_key: &'static str,
    pub label: &'static str,
    pub hint: &'static str,
    /// 哪些事件的 actions 支持它
    pub allowed_in: Vec<&'static str>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct UnlockConditionSpec {
    pub type_key: &'static str,
    pub label: &'static str,
    pub fields: Vec<FieldSpec>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ConditionSyntax {
    pub supported: Vec<&'static str>,
    pub unsupported: Vec<&'static str>,
    pub note: &'static str,
}

// ============================================================
// 构造
// ============================================================

fn character_field() -> FieldSpec {
    FieldSpec::new("character", "角色", FieldKind::Character)
        .required()
        .hint("MAIN = 当前选中的主角；其余为本剧本 characters/ 下的目录名")
}

fn emotion_field() -> FieldSpec {
    FieldSpec::new("emotion", "情绪", FieldKind::Emotion)
        .hint("表外的值会回落成「正常」")
}

fn effect_options() -> Vec<String> {
    let mut v = vec!["None".to_string()];
    v.extend(KNOWN_EFFECTS.iter().map(|s| s.to_string()));
    v
}

pub fn build_schema() -> ScriptSchema {
    let events = vec![
        // ---------- 叙事 ----------
        EventSpec {
            type_key: "narration",
            label: "旁白",
            category: "叙事",
            color: "#94a3b8",
            fields: vec![
                FieldSpec::new("text", "旁白文本", FieldKind::Textarea)
                    .required()
                    .hint("多行会逐行依次显示，空行被跳过"),
                FieldSpec::new("displayName", "说话人标签", FieldKind::Text)
                    .placeholder("旁白"),
            ],
        },
        EventSpec {
            type_key: "player",
            label: "玩家台词",
            category: "叙事",
            color: "#38bdf8",
            fields: vec![
                FieldSpec::new("text", "台词", FieldKind::Textarea).required(),
                FieldSpec::new("displayName", "显示名", FieldKind::Text)
                    .placeholder("（跟随玩家名）"),
            ],
        },
        EventSpec {
            type_key: "dialogue",
            label: "AI台词",
            category: "叙事",
            color: "#a78bfa",
            fields: vec![
                character_field(),
                FieldSpec::new("text", "台词", FieldKind::Textarea)
                    .required()
                    .hint("想让这句话真正「说」出来，必须同时满足两个条件：1) 台词最开头用【开心】【难过】等情绪标注；2) 所选角色已在「角色设置」里开启语音。缺一不可，否则只会显示文字、不会发声。例：【开心】今天能见到你，我真的很高兴！"),
                emotion_field(),
                FieldSpec::new("displayName", "显示名", FieldKind::Text),
                FieldSpec::new("displaySubtitle", "副标题", FieldKind::Text),
            ],
        },
        // ---------- AI ----------
        EventSpec {
            type_key: "ai_dialogue",
            label: "AI 对话",
            category: "AI",
            color: "#e879f9",
            fields: vec![
                character_field(),
                FieldSpec::new("prompt", "剧情提示", FieldKind::Textarea).hint(
                    "以旁白身份注入上下文引导 AI；留空则纯靠已有台词生成。注意提示会留在上下文里累积",
                ),
            ],
        },
        EventSpec {
            type_key: "free_dialogue",
            label: "自由对话",
            category: "AI",
            color: "#f472b6",
            fields: vec![
                character_field(),
                FieldSpec::new("hint", "输入框提示", FieldKind::Text)
                    .placeholder("自由对话..."),
                FieldSpec::new("max_rounds", "最大轮数", FieldKind::Number)
                    .placeholder("-1")
                    .hint("留空或 ≤0 表示不限轮数，此时唯一出口是玩家输入包含结束语"),
                FieldSpec::new("end_line", "结束语", FieldKind::Text)
                    .placeholder("结束")
                    .hint("玩家输入里出现这个文字就会结束对话（比如「结束」）"),
                FieldSpec::new("prompt", "每轮剧情提示", FieldKind::Textarea),
                FieldSpec::new("end_prompt", "末轮剧情提示", FieldKind::Textarea),
            ],
        },
        // ---------- 交互 ----------
        EventSpec {
            type_key: "choices",
            label: "选项",
            category: "交互",
            color: "#818cf8",
            fields: vec![
                FieldSpec::new("options", "选项列表", FieldKind::ChoiceOptions)
                    .required()
                    .hint("顺序即优先级；不带文案的选项匹配任意输入，必须放最后"),
                FieldSpec::new("allow_free", "允许自由输入", FieldKind::Bool)
                    .default_desc("false")
                    .hint("开启后玩家可以在输入框里直接打字作答"),
            ],
        },
        EventSpec {
            type_key: "input",
            label: "等待输入",
            category: "交互",
            color: "#60a5fa",
            fields: vec![FieldSpec::new("hint", "输入框提示", FieldKind::Text)
                .placeholder("请输入...")
                .hint("不填时输入框显示默认提示「请输入...」")],
        },
        // ---------- 流程 ----------
        EventSpec {
            type_key: "set_variable",
            label: "设置变量",
            category: "流程",
            color: "#f87171",
            fields: vec![FieldSpec::new("options", "赋值组", FieldKind::VarOptions)
                .required()
                .hint("每组可带条件；与 choices 不同，这里所有满足条件的组都会执行")],
        },
        EventSpec {
            type_key: "chapter_end",
            label: "章节结束",
            category: "流程",
            color: "#e2e8f0",
            fields: vec![
                FieldSpec::new("end_type", "结束方式", FieldKind::Select)
                    .required()
                    .options(["linear", "branching", "ai_judged"])
                    .hint("linear 直接跳转；branching 按条件分支；ai_judged 交给 LLM 判断"),
                FieldSpec::new("next_chapter", "下一章", FieldKind::Chapter)
                    .hint("仅 linear 使用；选「剧本结束」即整个剧本结束"),
                FieldSpec::new("options", "分支", FieldKind::BranchOptions)
                    .hint("branching / ai_judged 使用；顺序即优先级，可设一个 default 兜底"),
                FieldSpec::new("prompt", "AI 判定提示", FieldKind::Textarea)
                    .hint("仅 ai_judged 使用"),
                // 只展示不给编辑。`next` 在引擎里优先级**高于** `next_chapter`，
                // 两个都能填的话，作者改了上面那个「下一章」却不生效，而界面上
                // 两处都写着下一章 —— 这是最难自己看出来的一类问题。老数据原样
                // 保留，校验器会提示把它并到 next_chapter 去。
                FieldSpec::new("next", "下一章（旧字段）", FieldKind::Deprecated)
                    .disabled("引擎里它的优先级高于「下一章」。老剧本才有，新剧本请只用上面那个"),
            ],
        },
        // ---------- 演出 ----------
        EventSpec {
            type_key: "modify_character",
            label: "角色调整",
            category: "演出",
            color: "#fbbf24",
            fields: vec![
                character_field(),
                FieldSpec::new("action", "动作", FieldKind::Select)
                    .options(["show_character", "hide_character"])
                    .option_labels(["show_character（显示角色）", "hide_character（隐藏角色）"]),
                emotion_field(),
                FieldSpec::new("clothes", "服装", FieldKind::Text)
                    .hint("对应 avatar/<服装>/ 子目录；留空或 default 表示不进子目录"),
                FieldSpec::new("perceive", "能否听到后续台词", FieldKind::Bool)
                    .default_desc("保持当前状态")
                    .hint(
                    "决定该角色是否出现在后续台词的「感知者」列表里。注意 hide_character 会同时把角色移出感知列表",
                ),
                FieldSpec::new("flash", "立绘故障闪现", FieldKind::Bool)
                    .default_desc("false")
                    .hint("硬切闪现指定情绪立绘，供短促惊吓演出"),
                FieldSpec::new("noise", "立绘噪点区域", FieldKind::Select)
                    .options(["none", "eyes", "mouth", "eyes_mouth"])
                    .default_desc("none")
                    .hint("黑噪点侵蚀眼睛/嘴部；none 清除当前侵蚀"),
                FieldSpec::new("noise_fade_in", "噪点淡入（秒）", FieldKind::Number)
                    .placeholder("0")
                    .hint("仅 noise 非 none 时生效；0 表示立即出现"),
            ],
        },
        EventSpec {
            type_key: "background",
            label: "背景",
            category: "演出",
            color: "#34d399",
            fields: vec![
                FieldSpec::new("imagePath", "背景图", FieldKind::Asset)
                    .required()
                    .asset("background"),
                FieldSpec::new("transition", "过渡时长（秒）", FieldKind::Number)
                    .placeholder("1.0"),
            ],
        },
        EventSpec {
            type_key: "background_effect",
            label: "背景特效",
            category: "演出",
            color: "#2dd4bf",
            fields: vec![
                FieldSpec::new("effect", "特效", FieldKind::Select)
                    .required()
                    .options(effect_options())
                    .hint("从下拉里选；选「无特效」会清空。运行时支持在 YAML 中用 + 叠加多个特效"),
                FieldSpec::new("text", "特效文本", FieldKind::Textarea)
                    .hint("UiCorrupt 等文字侵蚀特效的显示内容"),
                FieldSpec::new("echo", "回声文本", FieldKind::Text)
                    .hint("可选的故障回声/残影文本"),
            ],
        },
        EventSpec {
            type_key: "present_pic",
            label: "插图",
            category: "演出",
            color: "#a3e635",
            fields: vec![
                FieldSpec::new("imagePath", "图片", FieldKind::Asset)
                    .required()
                    .asset("pic"),
                FieldSpec::new("scale", "缩放", FieldKind::Number).placeholder("1.0"),
            ],
        },
        // ---------- 声音 ----------
        EventSpec {
            type_key: "music",
            label: "背景音乐",
            category: "声音",
            color: "#fb923c",
            fields: vec![
                FieldSpec::new("musicPath", "音乐", FieldKind::Asset)
                    .required()
                    .asset("music"),
                FieldSpec::new("playbackSpeed", "播放速度", FieldKind::Number)
                    .hint("1.0 = 原速；留空同 1.0。范围建议 0.5–2.0，超出可能失真"),
            ],
        },
        EventSpec {
            type_key: "sound",
            label: "音效",
            category: "声音",
            color: "#facc15",
            fields: vec![FieldSpec::new("soundPath", "音效", FieldKind::Asset)
                .required()
                .asset("sound")],
        },
        EventSpec {
            type_key: "ambient",
            label: "环境音",
            category: "声音",
            color: "#22d3ee",
            fields: vec![
                // 刻意不 required：开了「停止该轨」时留空正是「停掉全部轨道」的写法，
                // 标成必填会让这种正常用法被校验器判成缺字段。
                FieldSpec::new("ambientPath", "环境音", FieldKind::Asset)
                    .asset("ambient")
                    .hint("播放时必填；配合下面的「停止该轨」留空表示停掉全部环境音"),
                FieldSpec::new("volume", "音量", FieldKind::Number)
                    .placeholder("100")
                    .hint("0–100"),
                FieldSpec::new("loop", "循环", FieldKind::Bool).default_desc("true"),
                FieldSpec::new("stop", "停止该轨", FieldKind::Bool)
                    .default_desc("false")
                    .hint("开启时会淡出停止；环境音留空则停止全部轨道"),
                FieldSpec::new("fade", "淡入淡出", FieldKind::Bool).default_desc("true"),
            ],
        },
        // ---------- DLC / 恐怖剧本扩展 ----------
        EventSpec {
            type_key: "force_choice",
            label: "强制选项",
            category: "交互",
            color: "#ef4444",
            fields: vec![
                FieldSpec::new("options", "选项列表", FieldKind::ChoiceOptions)
                    .required()
                    .hint("与普通 choices 相同，可给选项配置 condition / lock_hint"),
                FieldSpec::new("forced", "强制选择文本", FieldKind::Text)
                    .required()
                    .hint("必须与一个未锁定选项的 text 完全一致"),
            ],
        },
        EventSpec {
            type_key: "poem_game",
            label: "写诗小游戏",
            category: "交互",
            color: "#f472b6",
            fields: vec![
                FieldSpec::new("backgroundPath", "写诗背景", FieldKind::Asset).asset("background"),
                FieldSpec::new("musicPath", "正常音乐", FieldKind::Asset).asset("music"),
                FieldSpec::new("glitchMusicPath", "崩坏音乐", FieldKind::Asset).asset("music"),
                FieldSpec::new("warmStickerPath", "温暖角色贴纸", FieldKind::Asset).asset("pic"),
                FieldSpec::new("scriptStickerPath", "剧本角色贴纸", FieldKind::Asset).asset("pic"),
                FieldSpec::new("voidStickerPath", "空白角色贴纸", FieldKind::Asset).asset("pic"),
                FieldSpec::new("wordListPath", "词库文件", FieldKind::Text)
                    .placeholder("poem_words.yaml")
                    .hint("只能填写剧本根目录下的文件名"),
                FieldSpec::new("resultVar", "结果变量", FieldKind::Text)
                    .placeholder("poem_tone"),
                FieldSpec::new("rounds", "轮数", FieldKind::Number)
                    .placeholder("20")
                    .hint("引擎限制 1–20"),
                FieldSpec::new("glitch", "强制崩坏词", FieldKind::Bool),
                FieldSpec::new("mode", "写诗模式", FieldKind::Select)
                    .options(["normal", "act2", "act2_final"])
                    .default_desc("normal"),
            ],
        },
        EventSpec {
            type_key: "wait",
            label: "等待",
            category: "流程",
            color: "#64748b",
            fields: vec![FieldSpec::new("seconds", "等待秒数", FieldKind::Number)
                .placeholder("1.0")
                .hint("用于无需玩家点击的演出停顿")],
        },
        EventSpec {
            type_key: "random_var",
            label: "随机变量",
            category: "流程",
            color: "#a855f7",
            fields: vec![
                FieldSpec::new("variable", "变量名", FieldKind::Text).required(),
                FieldSpec::new("chance", "为 true 的概率", FieldKind::Number)
                    .placeholder("0.5")
                    .hint("范围 0–1，超出时会截断"),
            ],
        },
        EventSpec {
            type_key: "character_file",
            label: "角色文件",
            category: "流程",
            color: "#dc2626",
            fields: vec![
                FieldSpec::new("action", "操作", FieldKind::Select)
                    .required()
                    .options(["ensure", "exists", "delete", "open_folder"]),
                FieldSpec::new("file", "标记文件", FieldKind::Text)
                    .hint("必须在 story_config.script_settings.character_files 白名单中"),
                FieldSpec::new("resultVar", "检测结果变量", FieldKind::Text)
                    .hint("exists 操作把布尔结果写入该变量"),
                FieldSpec::new("result_var", "旧结果变量字段", FieldKind::Deprecated)
                    .disabled("兼容旧剧本；新剧本请使用 resultVar"),
            ],
        },
        EventSpec {
            type_key: "watch_file",
            label: "监视角色文件",
            category: "流程",
            color: "#b91c1c",
            fields: vec![
                FieldSpec::new("action", "操作", FieldKind::Select)
                    .options(["start", "stop"])
                    .default_desc("start"),
                FieldSpec::new("file", "监视文件", FieldKind::Text)
                    .hint("start 时必填，且必须在角色文件白名单中"),
                FieldSpec::new("on_missing", "文件消失后跳转", FieldKind::Chapter)
                    .hint("start 时必填"),
            ],
        },
        EventSpec {
            type_key: "voice_shift",
            label: "语音变调",
            category: "声音",
            color: "#7c3aed",
            fields: vec![
                FieldSpec::new("rate", "播放倍率", FieldKind::Number)
                    .placeholder("1.0")
                    .hint("引擎限制 0.5–1.5；会同时改变语速与音调"),
                FieldSpec::new("pitch", "纯音高偏移（半音）", FieldKind::Number)
                    .placeholder("0")
                    .hint("引擎限制 -12–12；通过 Web Audio detune 实现"),
            ],
        },
        EventSpec {
            type_key: "horror_log",
            label: "恐怖日志刷屏",
            category: "演出",
            color: "#991b1b",
            fields: vec![
                FieldSpec::new("text", "日志文本", FieldKind::Textarea).required(),
                FieldSpec::new("lines", "重复行数", FieldKind::Number)
                    .placeholder("1")
                    .hint("引擎会限制最大行数"),
            ],
        },
        EventSpec {
            type_key: "console_window",
            label: "原生系统窗口",
            category: "演出",
            color: "#450a0a",
            fields: vec![
                FieldSpec::new("title", "窗口标题", FieldKind::Text),
                FieldSpec::new("text", "窗口正文", FieldKind::Textarea).required(),
                FieldSpec::new("style", "窗口样式", FieldKind::Select)
                    .options(["console", "blood_cmd", "error", "warning", "notepad"])
                    .default_desc("console"),
                FieldSpec::new("count", "窗口数量", FieldKind::Number)
                    .placeholder("1")
                    .hint("引擎限制 1–4"),
                FieldSpec::new("interval", "窗口间隔（秒）", FieldKind::Number)
                    .placeholder("0.25"),
                FieldSpec::new("lifetime", "最长存活（秒）", FieldKind::Number)
                    .placeholder("4")
                    .hint("引擎限制 1–12 秒；玩家可提前关闭"),
            ],
        },
        EventSpec {
            type_key: "jumpscare",
            label: "突脸惊吓",
            category: "演出",
            color: "#7f1d1d",
            fields: vec![
                FieldSpec::new("imagePath", "突脸图片", FieldKind::Asset)
                    .required()
                    .asset("pic"),
                FieldSpec::new("soundPath", "惊吓音效", FieldKind::Asset).asset("sound"),
            ],
        },
        EventSpec {
            type_key: "glitch_window",
            label: "应用内故障窗口",
            category: "演出",
            color: "#6b21a8",
            fields: vec![
                FieldSpec::new("title", "窗口标题", FieldKind::Text),
                FieldSpec::new("text", "窗口正文", FieldKind::Textarea).required(),
                FieldSpec::new("style", "样式", FieldKind::Select)
                    .required()
                    .options(["terminal", "error"]),
                FieldSpec::new("count", "窗口数量", FieldKind::Number).placeholder("1"),
                FieldSpec::new("interval", "窗口间隔（秒）", FieldKind::Number)
                    .placeholder("0.25"),
                FieldSpec::new("lifetime", "存活时间（秒）", FieldKind::Number)
                    .placeholder("4"),
            ],
        },
        EventSpec {
            type_key: "window_title",
            label: "窗口标题故障",
            category: "演出",
            color: "#4c1d95",
            fields: vec![FieldSpec::new("title", "窗口标题", FieldKind::Text)
                .hint("留空可把标题切回应用默认值")],
        },
        EventSpec {
            type_key: "main_menu_effect",
            label: "主菜单主题",
            category: "演出",
            color: "#581c87",
            fields: vec![
                FieldSpec::new("theme", "主题", FieldKind::Select)
                    .required()
                    .options(["normal", "blood", "ghost"]),
                FieldSpec::new("message", "菜单短句", FieldKind::Textarea)
                    .hint("最多 160 字；主题状态按剧本归属持久化"),
            ],
        },
        // ---------- 成就 ----------
        EventSpec {
            type_key: "unlock_achievement",
            label: "解锁成就",
            category: "成就",
            color: "#fbbf24",
            fields: vec![
                FieldSpec::new("achievement_id", "成就键名", FieldKind::Text)
                    .required()
                    .placeholder("如：summer_star")
                    .hint("给这个成就起的英文标识，不能与内置成就或本剧本其他成就重名（校验器会提示）"),
                FieldSpec::new("title", "成就标题", FieldKind::Text)
                    .required()
                    .placeholder("如：夏日之星")
                    .hint("玩家在成就列表里看到的成就名字"),
                FieldSpec::new("description", "成就描述", FieldKind::Textarea)
                    .required()
                    .hint("达成条件说明，展示给玩家看"),
            ],
        },
    ];

    let common_fields = vec![
        FieldSpec::new("condition", "触发条件", FieldKind::Condition)
            .hint("设置条件后，只有满足条件时本事件才会执行；留空则必定触发"),
        FieldSpec::new("duration", "事件时长/间隔（秒）", FieldKind::Number)
            .placeholder("由具体事件解释")
            .hint("仅部分事件读取：台词/选项可作推进时长，jumpscare 用作覆盖层寿命；wait 请使用 seconds"),
    ];

    let story_config_fields = vec![
        FieldSpec::new("script_name", "剧本名", FieldKind::Text)
            .required()
            .hint("全局唯一，重名会导致其中一个剧本在列表里被覆盖"),
        FieldSpec::new("description", "简介", FieldKind::Textarea),
        FieldSpec::new("recommand_start", "推荐开始时机", FieldKind::Text)
            .placeholder("例如：好感度达到 30 之后")
            .hint("展示给玩家看的推荐时机说明，仅作展示，不影响剧情判断"),
        FieldSpec::new("intro_chapter", "开场章节", FieldKind::Chapter).required(),
        FieldSpec::new("main_character", "剧本主角目录", FieldKind::Text)
            .hint("正式进入时切换并锁定到全局 characters/ 下的目录名；结束后恢复原主角"),
        FieldSpec::new("content_warning", "内容警告类型", FieldKind::Select)
            .options(["horror"])
            .hint("horror 会在进入前要求玩家确认，并允许受限系统演出授权"),
        FieldSpec::new("editor_locked", "禁止编辑", FieldKind::Bool)
            .default_desc("false")
            .hint("发行版 DLC 可锁定编辑器修改，但仍允许校验和正式游玩"),
    ];

    let action_types = vec![
        ActionSpec {
            type_key: "add_line",
            label: "追加一句玩家台词",
            hint: "以玩家名义写入对话历史，AI 能看到",
            allowed_in: vec!["choices"],
        },
        ActionSpec {
            type_key: "set_var",
            label: "设置变量",
            hint: "表达式形如 flag = true / count += 1 / hp -= 5",
            allowed_in: vec!["choices", "set_variable"],
        },
    ];

    let unlock_condition_types = vec![
        UnlockConditionSpec {
            type_key: "chat_count",
            label: "累计聊天条数达到",
            fields: vec![FieldSpec::new("threshold", "条数", FieldKind::Number).required()],
        },
        UnlockConditionSpec {
            type_key: "time_range",
            label: "处于时间段内",
            fields: vec![
                FieldSpec::new("start_hour", "起始小时", FieldKind::Number).required(),
                FieldSpec::new("end_hour", "结束小时", FieldKind::Number)
                    .required()
                    .hint("起始大于结束表示跨零点"),
            ],
        },
        UnlockConditionSpec {
            type_key: "adventure_completed",
            label: "已完成某个羁绊冒险",
            fields: vec![FieldSpec::new("adventure_folder", "剧本目录名", FieldKind::Text)
                .required()
                .hint("填目标剧本的目录名（不是显示名）")],
        },
        UnlockConditionSpec {
            type_key: "achievement_unlocked",
            label: "已解锁某个成就",
            fields: vec![FieldSpec::new("achievement_id", "成就 id", FieldKind::Text).required()],
        },
    ];

    ScriptSchema {
        events,
        common_fields,
        story_config_fields,
        action_types,
        unlock_condition_types,
        // 与 events_handler.rs 的 replace_placeholder 覆盖范围一致
        placeholder_fields: vec![
            "text",
            "prompt",
            "hint",
            "end_line",
            "dialog_prompt",
            "end_prompt",
            "content",
            "description",
        ],
        condition_syntax: ConditionSyntax {
            supported: vec![
                "var == 值",
                "var != 值",
                "var >= 数字 / <= / > / <",
                "条件 && 条件",
                "条件 || 条件",
                "var（真值判断）",
            ],
            unsupported: vec!["!", "括号", "算术"],
            note: "== / != 按文字比较，大小比较要求右侧是数字；&& 优先于 || 递归组合。暂不支持括号、取反和算术表达式。",
        },
    }
}
