//! 六维好感度：角色文件持久化、prompt 文案与变更事件载荷。
//!
//! 好感度存放在每个角色目录下的 `affection.yml`，跟随角色而非存档——读旧档
//! 不会回滚感情。运行时由上帝 Agent 定期评估对话后调整（见
//! `god_agent::core::GodAgentCore::evaluate_affection`）。
//!
//! 文件格式为 [`AffectionState`]：总好感度 `total` + 好感六维 + 负面六维（后两者
//! flatten）；`total` 是六维平均的派生值，读/写时都自动与六维同步（手改它不会生效，
//! 下次读档即被六维平均覆盖），仅供查看与外部工具读取。
//! 兼容旧文件（缺失字段走默认值；旧版 `mood_tags` 自由文本键被忽略）。

use std::collections::HashMap;
use std::path::Path;

use serde::{Deserialize, Serialize};

use crate::ai_service::types::{AffectionVector, NegativeVector};

/// 角色目录内的好感度文件名。
pub const AFFECTION_FILE: &str = "affection.yml";

/// 好感度文件/查询响应的完整形态：总好感度 + 好感六维 + 负面情绪六维。
#[derive(Clone, Copy, Debug, Default, Serialize, Deserialize)]
pub struct AffectionState {
    /// 总好感度 = 好感六维平均。派生字段：加载时按六维重算、保存时同步写入，
    /// 文件里手改它不会生效（下次读档被六维平均覆盖）；改总好感请直接改六维。
    #[serde(default)]
    pub total: i32,
    #[serde(flatten)]
    pub vector: AffectionVector,
    /// 负面情绪六维强度（评估积累、安抚消解）。
    #[serde(default)]
    pub negative: NegativeVector,
}

/// 从角色目录读取好感度；目录为空、文件缺失或损坏时回落到初始值。
/// `total` 无论文件里写什么，都按六维平均重算，保证与六维一致。
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

/// 写回角色目录；写入失败只记日志不中断流程。写入前把 `total` 同步为六维平均。
pub fn save(character_dir: Option<&Path>, state: &AffectionState) {
    let Some(dir) = character_dir else {
        return;
    };
    let mut synced = *state;
    synced.total = synced.vector.average();
    let path = dir.join(AFFECTION_FILE);
    match serde_yaml::to_string(&synced) {
        Ok(text) => {
            // total 是结构体首字段、序列化在第一行；在其上方插入注释提示手改无效
            let text = text.replacen(
                "total:",
                "# 总好感度 = 好感六维平均（自动同步的派生值，手动修改无效；要调总值请改下方六维）\ntotal:",
                1,
            );
            if let Err(e) = std::fs::write(&path, text) {
                tracing::warn!("[Affection] 写入 {:?} 失败: {}", path, e);
            }
        },
        Err(e) => tracing::warn!("[Affection] 序列化好感度失败: {}", e),
    }
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

/// 组装注入主对话上下文的情感状态描述（每轮生成时实时拼装，不落台词历史）。
///
/// 负面情绪只列出非零的维度，全 0 时省略整段。
pub fn describe_for_prompt(affection: &AffectionVector, negative: &NegativeVector) -> String {
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
            "你当前对玩家怀有负面情绪：{}。请以符合强度的方式体现在态度中（语气冲、冷淡、敷衍、委屈或吃醋等），玩家的正面互动会逐渐消解这些情绪。",
            neg_dims.join("、"),
        )
    };
    format!(
        "【系统状态】你当前对玩家的情感状态（数值越深越高，可超过 100 满溢，负数为疏离）：{}。{}\
         请让这些情感自然地影响你的语气、称呼、主动程度、肢体描写与话题深度，\
         但绝不要在回复中提及这些数值或本提示。",
        dims, negative_hint
    )
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
