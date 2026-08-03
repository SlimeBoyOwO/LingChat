//! 事件 schema —— 16 种事件及其全部字段的**单一真相源**。
//!
//! 在这之前，同一份 schema 散落在三处：Rust 的 16 个 handler、前端
//! `src/types/script.ts` 的运行时 payload 类型、原型编辑器的 `constants/events.ts`。
//! 三者互不同步，直接导致原型产出的 `set_variable` / `chapter_end` 跑不通。
//!
//! 现在由 Rust 导出、前端只负责渲染。改引擎时**必须同步改这个文件**，
//! 下方的测试会在字段与 handler 数量不一致时失败。
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
    /// 触发条件：结构化「变量 + 关系 + 值」表单，序列化为 `var == 值` / `var != 值` / 裸变量
    /// （引擎的 evaluate_condition 只认这三种写法，结构化让作者写不出不支持的语法）
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
            label: "角色对话",
            category: "叙事",
            color: "#a78bfa",
            fields: vec![
                character_field(),
                FieldSpec::new("text", "台词", FieldKind::Textarea).required(),
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
                FieldSpec::new("perceive", "能否听到后续台词", FieldKind::Bool).hint(
                    "决定该角色是否出现在后续台词的「感知者」列表里。注意 hide_character 会同时把角色移出感知列表",
                ),
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
            fields: vec![FieldSpec::new("effect", "特效", FieldKind::Select)
                .required()
                .options(effect_options())
                .hint("从下拉里选；选「无特效」会清空当前特效")],
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
                FieldSpec::new("loop", "循环", FieldKind::Bool),
                FieldSpec::new("stop", "停止该轨", FieldKind::Bool)
                    .hint("开启时会淡出停止；环境音留空则停止全部轨道"),
                FieldSpec::new("fade", "淡入淡出", FieldKind::Bool),
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
        FieldSpec::new("duration", "事件间隔（秒）", FieldKind::Number)
            .placeholder("留空或负数 = 等玩家点击")
            .hint("事件展示后自动等待 N 秒再继续，作为事件之间的 CD；留空或填负数表示等玩家点击后才继续"),
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
            supported: vec!["var == 值", "var != 值", "var（真值判断）"],
            unsupported: vec![">", "<", ">=", "<=", "&&", "||", "!", "括号", "算术"],
            note: "比较是按文字逐个比对的。没赋过值的变量不会正常运作，先用「设置变量」给它赋个值再比较。注意「大于/小于」这类比较不支持：写 hp >= 5 不会报错，但会被当成一个名叫 \"hp >= 5\" 的变量去查，结果不会正常运作。",
        },
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashSet;

    /// schema 必须覆盖引擎注册的全部事件类型，一个不多一个不少。
    ///
    /// 引擎那 16 种在 `script_engine::mod::init_events` 里注册，这里硬编码一份
    /// 对照表：任何一侧增删事件都会让这个测试失败，逼着两边同步。
    #[test]
    fn schema_covers_every_registered_event_type() {
        const ENGINE_EVENT_TYPES: [&str; 17] = [
            "narration",
            "player",
            "dialogue",
            "ai_dialogue",
            "free_dialogue",
            "choices",
            "input",
            "set_variable",
            "chapter_end",
            "modify_character",
            "background",
            "background_effect",
            "present_pic",
            "music",
            "sound",
            "ambient",
            "unlock_achievement",
        ];

        let schema = build_schema();
        let in_schema: HashSet<&str> = schema.events.iter().map(|e| e.type_key).collect();
        let in_engine: HashSet<&str> = ENGINE_EVENT_TYPES.iter().copied().collect();

        let missing: Vec<_> = in_engine.difference(&in_schema).collect();
        let extra: Vec<_> = in_schema.difference(&in_engine).collect();
        assert!(missing.is_empty(), "schema 缺少事件类型: {:?}", missing);
        assert!(extra.is_empty(), "schema 有引擎不认识的事件类型: {:?}", extra);
        assert_eq!(schema.events.len(), 17);
    }

    #[test]
    fn every_event_has_at_least_one_field_and_unique_keys() {
        for e in build_schema().events {
            assert!(!e.fields.is_empty(), "{} 没有字段", e.type_key);
            let mut seen = HashSet::new();
            for f in &e.fields {
                assert!(
                    seen.insert(f.key),
                    "{} 的字段 {} 重复了",
                    e.type_key,
                    f.key
                );
            }
        }
    }

    #[test]
    fn asset_fields_declare_their_kind() {
        for e in build_schema().events {
            for f in &e.fields {
                if matches!(f.kind, FieldKind::Asset) {
                    assert!(
                        f.asset_kind.is_some(),
                        "{}.{} 是素材字段但没声明 asset_kind",
                        e.type_key,
                        f.key
                    );
                }
            }
        }
    }

    #[test]
    fn effect_options_come_from_the_engine_constant() {
        let schema = build_schema();
        let effect = schema
            .events
            .iter()
            .find(|e| e.type_key == "background_effect")
            .unwrap();
        let field = &effect.fields[0];
        // None + 5 个合法特效
        assert_eq!(field.options.len(), KNOWN_EFFECTS.len() + 1);
        for k in KNOWN_EFFECTS {
            assert!(field.options.iter().any(|o| o == k), "缺少特效 {}", k);
        }
    }

    /// option_labels 若提供，长度必须与 options 一致，否则前端会错位显示
    #[test]
    fn option_labels_match_options_length() {
        for e in build_schema().events {
            for f in &e.fields {
                assert!(
                    f.option_labels.is_empty() || f.option_labels.len() == f.options.len(),
                    "{}.{} 的 option_labels 长度与 options 不一致",
                    e.type_key,
                    f.key
                );
            }
        }
    }

    /// duration 是所有事件继承基础事件得到的「事件间隔」字段：可编辑（Number），
    /// 引擎会读取并传给前端实现事件间 CD。留空/负数 = 等玩家点击。
    #[test]
    fn duration_is_exposed_and_editable() {
        let schema = build_schema();
        let d = schema
            .common_fields
            .iter()
            .find(|f| f.key == "duration")
            .expect("common_fields 应包含 duration（基础事件字段）");
        assert!(d.enabled, "duration 应可编辑");
        assert!(matches!(d.kind, FieldKind::Number));
    }

    #[test]
    fn set_variable_only_allows_set_var_action() {
        let schema = build_schema();
        let set_var = schema
            .action_types
            .iter()
            .find(|a| a.type_key == "set_var")
            .unwrap();
        assert!(set_var.allowed_in.contains(&"set_variable"));

        let add_line = schema
            .action_types
            .iter()
            .find(|a| a.type_key == "add_line")
            .unwrap();
        // 引擎的 set_variable_event 只处理 set_var
        assert!(!add_line.allowed_in.contains(&"set_variable"));
    }
}
