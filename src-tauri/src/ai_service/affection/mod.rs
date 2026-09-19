//! 六维好感度：存档全局变量持久化、prompt 文案与变更事件载荷。
//!
//! 好感度存放在每个存档的全局变量 JSON 里（`GameStatus::global_variables`，键
//! `affection.{role_id}`），跟随存档保存——读旧档即回到旧档时的感情状态。
//! 角色目录下的旧版 `affection.yml` 仅在全局变量缺失时作为初始值读取（遗留
//! 兼容），不再写入。运行时由上帝 Agent 定期评估对话后调整（见
//! `god_agent::core::GodAgentCore::evaluate_affection`）。
//!
//! 状态格式为 [`AffectionState`]：总好感度 `total` + 好感六维 + 负面六维（后两者
//! flatten）；`total` 是六维平均的派生值，读/写时都自动与六维同步（手改它不会生效，
//! 下次读取即被六维平均覆盖），仅供查看与外部工具读取。
//! 兼容旧数据（缺失字段走默认值；旧版 `mood_tags` 自由文本键被忽略）。

use std::collections::HashMap;
use std::path::Path;

use serde::{Deserialize, Serialize};

use crate::ai_service::types::{AffectionVector, NegativeVector};

/// 角色目录内的旧版好感度文件名（仅遗留兼容读取，不再写入）。
pub const AFFECTION_FILE: &str = "affection.yml";

/// 存档全局变量键前缀：`affection.{role_id}`。
pub const VAR_KEY_PREFIX: &str = "affection.";

/// 好感度状态的完整形态：总好感度 + 好感六维 + 负面情绪六维。
#[derive(Clone, Copy, Debug, Default, Serialize, Deserialize)]
pub struct AffectionState {
    /// 总好感度 = 好感六维平均。派生字段：读取时按六维重算、写入时同步，
    /// 手改它不会生效（下次读取被六维平均覆盖）；改总好感请直接改六维。
    #[serde(default)]
    pub total: i32,
    #[serde(flatten)]
    pub vector: AffectionVector,
    /// 负面情绪六维强度（评估积累、安抚消解）。
    #[serde(default)]
    pub negative: NegativeVector,
}

/// 角色在存档全局变量里的键。
pub fn var_key(role_id: i32) -> String {
    format!("{VAR_KEY_PREFIX}{role_id}")
}

/// 状态 → 可存入存档全局变量的 JSON；写入前把 `total` 同步为六维平均。
pub fn state_to_value(state: &AffectionState) -> serde_json::Value {
    let mut synced = *state;
    synced.total = synced.vector.average();
    serde_json::to_value(synced).unwrap_or(serde_json::Value::Null)
}

/// 从存档全局变量 JSON 还原状态；缺失或损坏时返回 None（调用方回落初始值）。
/// `total` 无论存的是什么，都按六维平均重算。
pub fn state_from_value(value: &serde_json::Value) -> Option<AffectionState> {
    let mut state: AffectionState = serde_json::from_value(value.clone()).ok()?;
    state.total = state.vector.average();
    Some(state)
}

/// 遗留兼容：从角色目录读取旧版 `affection.yml` 作为初始值；目录为空、文件
/// 缺失或损坏时回落到初始值。好感度已迁移到存档全局变量，此文件不再写入。
pub fn load(character_dir: Option<&Path>) -> AffectionState {
    let mut state = match character_dir {
        Some(dir) => std::fs::read_to_string(dir.join(AFFECTION_FILE))
            .ok()
            .and_then(|text| serde_yaml::from_str(&text).ok())
            .unwrap_or_default(),
        None => AffectionState::default(),
    };
    state.total = state.vector.average();
    state
}

/// 好感度数值 → 程度词（供 prompt 注入；数值允许溢出：>100 满溢、负数疏离）。
pub fn tier_label(value: i32) -> &'static str {
    match value {
        ..=-1 => "疏离",
        0..=20 => "初识",
        21..=40 => "平淡",
        41..=60 => "熟络",
        61..=80 => "深厚",
        81..=100 => "炽烈",
        _ => "满溢",
    }
}

/// 负面情绪强度 → 程度词（供 prompt 注入；>100 视为失控）。
pub fn negative_tier_label(value: i32) -> &'static str {
    match value {
        ..=0 => "无",
        1..=20 => "轻微",
        21..=40 => "明显",
        41..=70 => "强烈",
        71..=100 => "难以平复",
        _ => "失控",
    }
}

/// 组装情感状态描述的核心文本。负面情绪只列出非零的维度，全 0 时省略整段。
///
/// `subject` 是称呼角色的主语：写入共享台词历史时用角色名（在场多名角色
/// 共读同一份历史，「你」会指代不明）。
fn describe_with_subject(
    subject: &str,
    affection: &AffectionVector,
    negative: &NegativeVector,
) -> String {
    let dims = AffectionVector::DIMENSIONS
        .iter()
        .map(|(key, label)| {
            let v = affection.get(key).unwrap_or(0);
            format!("{} {}（{}）", label, v, tier_label(v))
        })
        .collect::<Vec<_>>()
        .join("、");
    let neg_dims = NegativeVector::DIMENSIONS
        .iter()
        .filter_map(|(key, label)| {
            let v = negative.get(key).unwrap_or(0);
            (v > 0).then(|| format!("{} {}（{}）", label, v, negative_tier_label(v)))
        })
        .collect::<Vec<_>>();
    let negative_hint = if neg_dims.is_empty() {
        String::new()
    } else {
        format!(
            "{subject}当前对玩家怀有负面情绪：{}。请以符合强度的方式体现在{subject}的态度中（语气冲、冷淡、敷衍、委屈或吃醋等），玩家的正面互动会逐渐消解这些情绪。",
            neg_dims.join("、"),
        )
    };
    format!(
        "【系统状态】{subject}当前对玩家的情感状态（数值越深越高，可超过 100 满溢，负数为疏离）：{}。{}\
         请让这些情感自然地影响{subject}的语气、称呼、主动程度、肢体描写与话题深度，\
         但绝不要在回复中提及这些数值或本提示。",
        dims, negative_hint
    )
}

/// 好感度变化时写入台词历史的旁白文本：复用既有 add_line 台词工具随记忆构建
/// 自然进入后续上下文，不做每轮注入（避免每次思维链都携带情感状态）。
/// 以角色名作主语，避免多角色在场时「你」指代不明。
pub fn describe_change_for_line(
    name: &str,
    affection: &AffectionVector,
    negative: &NegativeVector,
) -> String {
    describe_with_subject(name, affection, negative)
}

/// 「好感度变化」事件的载荷（`affection:changed`，供前端刷新状态卡片与徽章）。
#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "snake_case")]
pub struct AffectionChangedPayload {
    pub role_id: i32,
    /// 本次实际发生变化的好感维度增量（键名为维度序列化键）。
    pub deltas: HashMap<String, i32>,
    /// 本次实际发生变化的负面情绪维度增量。
    pub negative_deltas: HashMap<String, i32>,
    /// 调整后的完整好感六维数值。
    pub values: AffectionVector,
    /// 调整后的完整负面六维数值。
    pub negative: NegativeVector,
    /// 好感六维平均（前端显示的总好感）。
    pub average: i32,
    /// 上帝 Agent 给出的调整理由。
    pub reason: String,
}
