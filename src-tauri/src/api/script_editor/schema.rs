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
    /// 所有事件共有的字段（condition / duration）
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
                    .hint("按换行拆成多条依次显示，空行会被丢弃"),
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
                    .hint("玩家输入**包含**该串即结束（子串匹配，不是完全相等）"),
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
                .hint("留空字符串会得到空提示，不会回落默认值")],
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
                FieldSpec::new("next", "下一章（旧字段）", FieldKind::Chapter)
                    .hint("引擎里 next 的优先级高于 next_chapter。新剧本请用上面的「下一章」，这里只为兼容已有数据"),
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
                    .hint("引擎只识别这两个"),
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
                .hint("严格区分大小写；只有这几个值有效，其余一律清空特效")],
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
            fields: vec![FieldSpec::new("musicPath", "音乐", FieldKind::Asset)
                .required()
                .asset("music")],
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
                FieldSpec::new("ambientPath", "环境音", FieldKind::Asset)
                    .required()
                    .asset("ambient"),
                FieldSpec::new("volume", "音量", FieldKind::Number)
                    .placeholder("100")
                    .hint("0–100"),
                FieldSpec::new("loop", "循环", FieldKind::Bool),
                FieldSpec::new("stop", "停止该轨", FieldKind::Bool)
                    .hint("开启时会淡出停止；环境音留空则停止全部轨道"),
                FieldSpec::new("fade", "淡入淡出", FieldKind::Bool),
            ],
        },
    ];

    let common_fields = vec![
        FieldSpec::new("condition", "触发条件", FieldKind::Text)
            .placeholder("留空则总是触发")
            .hint("只支持 var == 值 / var != 值 / 裸变量真值"),
        FieldSpec::new("duration", "duration", FieldKind::Deprecated)
            .disabled("遗留字段，引擎从不读取。保存时原样保留，不会丢数据"),
    ];

    let story_config_fields = vec![
        FieldSpec::new("script_name", "剧本名", FieldKind::Text)
            .required()
            .hint("全局唯一。重名会导致其中一个剧本在列表里被覆盖"),
        FieldSpec::new("description", "简介", FieldKind::Textarea),
        FieldSpec::new("recommand_start", "推荐开始时机", FieldKind::Text)
            .hint("字段名少了一个 m 是历史拼写，照抄即可"),
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
                .hint("填目标剧本的**目录名**，不是显示名")],
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
            note: "比较是字符串比较。未定义的变量：== 恒假、!= 恒真。写 hp >= 5 不会报错，但会被当成一个名叫 \"hp >= 5\" 的变量去查，结果恒假。",
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
        const ENGINE_EVENT_TYPES: [&str; 16] = [
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
        ];

        let schema = build_schema();
        let in_schema: HashSet<&str> = schema.events.iter().map(|e| e.type_key).collect();
        let in_engine: HashSet<&str> = ENGINE_EVENT_TYPES.iter().copied().collect();

        let missing: Vec<_> = in_engine.difference(&in_schema).collect();
        let extra: Vec<_> = in_schema.difference(&in_engine).collect();
        assert!(missing.is_empty(), "schema 缺少事件类型: {:?}", missing);
        assert!(extra.is_empty(), "schema 有引擎不认识的事件类型: {:?}", extra);
        assert_eq!(schema.events.len(), 16);
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

    /// duration 必须以「不可编辑」的形态出现 —— 既让作者知道它存在，
    /// 又不给他们填一个不生效的值。
    #[test]
    fn duration_is_exposed_but_disabled() {
        let schema = build_schema();
        let d = schema
            .common_fields
            .iter()
            .find(|f| f.key == "duration")
            .expect("common_fields 应包含 duration");
        assert!(!d.enabled);
        assert!(matches!(d.kind, FieldKind::Deprecated));
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
