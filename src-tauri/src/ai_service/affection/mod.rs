//! 六维好感度：存档全局变量持久化、prompt 文案与变更事件载荷。
//!
//! 好感度存放在每个存档的全局变量 JSON 里（`GameStatus::global_variables`，键
//! `affection.{role_id}`），跟随存档保存——读旧档即回到旧档时的感情状态。
//! 运行时由上帝 Agent 定期评估对话后调整（见
//! `god_agent::core::GodAgentCore::evaluate_affection`）。
//!
//! 状态格式为 [`AffectionState`]：总好感度 `total` + 好感六维 + 负面六维（后两者
//! flatten）；`total` 是六维平均的派生值，读/写时都自动与六维同步（手改它不会生效，
//! 下次读取即被六维平均覆盖），仅供查看与外部工具读取。

use std::collections::HashMap;

use serde::{Deserialize, Serialize};

use crate::ai_service::types::{AffectionVector, NegativeVector};

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

/// 好感度变化时写入台词历史的旁白正文：复用既有 add_line 台词工具随记忆构建
/// 自然进入后续上下文，不做每轮注入（避免每次思维链都携带情感状态）。
///
/// 只产出旁白正文，外层 `{旁白: ...}` 包装由调用方统一走
/// `utils::prompt::PromptRole::Narrator::build_prompt`，这里不再自行包裹。
/// 以角色名作主语、玩家名作宾语（共享台词历史由在场多名角色共读，「你」会指代不明）；
/// 数值本身不进文案，只经 [`tier_label`] / [`negative_tier_label`] 翻成程度描述。
pub fn describe_change_for_line(
    name: &str,
    player_name: &str,
    affection: &AffectionVector,
    negative: &NegativeVector,
) -> String {
    let average_tier = tier_label(affection.average());
    let mut text = match average_tier {
        "满溢" => format!("{name}心里满满的都是{player_name}"),
        "炽烈" => format!("{name}深深喜欢上了{player_name}"),
        "深厚" => format!("{name}现在对{player_name}很有好感"),
        "熟络" => format!("{name}和{player_name}已经熟络起来了"),
        "平淡" => format!("{name}和{player_name}处得不咸不淡，关系还在慢慢升温"),
        "初识" => format!("{name}和{player_name}才刚刚认识，对他还很生疏"),
        _ => format!("{name}对{player_name}有些疏离，像隔着一层什么"),
    };

    // 只有正向区间才揉入最强好感维度的细节，避免与生疏/疏离的基调矛盾
    let positive = matches!(average_tier, "满溢" | "炽烈" | "深厚" | "熟络");
    if positive {
        let strongest = AffectionVector::DIMENSIONS
            .iter()
            .max_by_key(|(key, _)| affection.get(key).unwrap_or(0))
            .map(|(key, _)| *key)
            .unwrap_or_default();
        let flavor = match strongest {
            "fondness" => "见到他就不由自主地开心",
            "trust" => "觉得他很值得信赖",
            "intimacy" => "总想和他再靠近一点",
            "rapport" => "和他之间有种说不出的默契",
            "interest" => "对他的一切都充满了好奇",
            _ => "分开一小会儿就开始想他",
        };
        text = format!("{text}，{flavor}");
    }

    let peak = negative.peak();
    if peak > 0 {
        let dim = NegativeVector::DIMENSIONS
            .iter()
            .max_by_key(|(key, _)| negative.get(key).unwrap_or(0))
            .map(|(key, _)| *key)
            .unwrap_or_default();
        let feeling = match dim {
            "anger" => "怒火",
            "hurt" => "委屈",
            "disappointment" => "失望",
            "indifference" => "冷淡",
            "jealousy" => "醋意",
            _ => "疏远感",
        };
        let hint = match negative_tier_label(peak) {
            "失控" => format!("心里的{feeling}几乎失控"),
            "难以平复" => format!("心里的{feeling}久久难以平复"),
            "强烈" => format!("心里的{feeling}越发强烈"),
            "明显" => format!("心里带着明显的{feeling}"),
            _ => format!("心里有一丝若有若无的{feeling}"),
        };
        let joiner = if positive { "，只是" } else { "，" };
        text = format!("{text}{joiner}{hint}");
    }

    format!("{text}。")
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
