//! IndexTTS2 情绪控制平面。
//!
//! 标签映射、强度归一化、Qwen 描述选择和持久化缓存都由 Rust 管理；
//! Python 只在缓存未命中时执行必要的 Qwen 张量推理。

use std::collections::BTreeMap;
use std::path::PathBuf;

use serde::{Deserialize, Serialize};

pub const EMOTION_DIMENSIONS: usize = 8;
pub type EmotionVector = [f32; EMOTION_DIMENSIONS];

#[derive(Debug, Clone, PartialEq)]
pub enum EmotionPlan {
    /// 已在 Rust 中完成映射、偏置和总强度归一化。
    Vector(EmotionVector),
    /// 需要调用 Python 中的 Qwen 情绪模型；结果由 Rust 缓存。
    Analyze { cache_key: String, prompt: String },
    /// 不提供显式向量，让模型跟随参考音频。
    FollowReference,
}

fn sanitize_label(label: &str) -> String {
    label
        .chars()
        .filter(|ch| !ch.is_whitespace() && !matches!(ch, '【' | '】' | '[' | ']'))
        .collect()
}

fn emotion_scale() -> f32 {
    std::env::var("INDEXTTS_EMO_SCALE")
        .ok()
        .and_then(|value| value.parse::<f32>().ok())
        .filter(|value| value.is_finite() && *value >= 0.0)
        .unwrap_or(1.0)
}

fn normalize(mut vector: EmotionVector) -> EmotionVector {
    // 与 IndexTTS2.normalize_emo_vec(apply_bias=true) 保持一致。
    const BIAS: EmotionVector = [0.9375, 0.875, 1.0, 1.0, 0.9375, 0.9375, 0.6875, 0.5625];
    let scale = emotion_scale();
    for (value, bias) in vector.iter_mut().zip(BIAS) {
        *value = (*value * scale * bias).max(0.0);
    }
    let sum: f32 = vector.iter().sum();
    if sum > 0.8 {
        let factor = 0.8 / sum;
        for value in &mut vector {
            *value *= factor;
        }
    }
    vector
}

fn blend_vector(label: &str) -> Option<EmotionVector> {
    // 维度：[高兴, 愤怒, 悲伤, 恐惧, 反感, 低落, 惊讶, 平静]
    let vector = match label {
        "高兴" | "开心" | "喜悦" | "快乐" => [0.65, 0.0, 0.0, 0.0, 0.0, 0.0, 0.10, 0.0],
        "愉快" => [0.60, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.10],
        "幸福" => [0.60, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.15],
        "兴奋" => [0.70, 0.0, 0.0, 0.0, 0.0, 0.0, 0.15, 0.0],
        "期待" => [0.55, 0.0, 0.0, 0.0, 0.0, 0.0, 0.10, 0.0],
        "调皮" => [0.55, 0.0, 0.0, 0.0, 0.0, 0.0, 0.15, 0.05],
        "情动" => [0.50, 0.0, 0.0, 0.0, 0.0, 0.05, 0.0, 0.15],

        "生气" => [0.0, 0.65, 0.0, 0.0, 0.15, 0.0, 0.0, 0.0],
        "愤怒" => [0.0, 0.70, 0.0, 0.0, 0.10, 0.0, 0.0, 0.0],
        "恼火" => [0.0, 0.60, 0.0, 0.0, 0.15, 0.0, 0.0, 0.0],
        "气愤" => [0.0, 0.65, 0.0, 0.0, 0.10, 0.0, 0.0, 0.0],
        "暴怒" => [0.0, 0.75, 0.0, 0.0, 0.0, 0.0, 0.05, 0.0],

        "难过" => [0.0, 0.0, 0.60, 0.0, 0.0, 0.20, 0.0, 0.0],
        "悲伤" | "伤心" => [0.0, 0.0, 0.65, 0.0, 0.0, 0.15, 0.0, 0.0],
        "哭泣" => [0.0, 0.0, 0.70, 0.0, 0.0, 0.15, 0.0, 0.0],
        "委屈" => [0.0, 0.0, 0.55, 0.05, 0.0, 0.20, 0.0, 0.0],
        "伤感" => [0.0, 0.0, 0.55, 0.0, 0.0, 0.25, 0.0, 0.0],
        "低落" | "沮丧" | "失落" => [0.0, 0.0, 0.15, 0.0, 0.0, 0.60, 0.0, 0.0],
        "忧郁" | "消沉" => [0.0, 0.0, 0.10, 0.0, 0.0, 0.65, 0.0, 0.0],

        "害怕" => [0.0, 0.0, 0.0, 0.65, 0.0, 0.0, 0.15, 0.0],
        "恐惧" => [0.0, 0.0, 0.0, 0.70, 0.0, 0.0, 0.10, 0.0],
        "紧张" => [0.0, 0.0, 0.0, 0.50, 0.0, 0.0, 0.15, 0.10],
        "不安" => [0.0, 0.0, 0.0, 0.50, 0.0, 0.15, 0.0, 0.0],
        "慌张" => [0.0, 0.0, 0.0, 0.55, 0.0, 0.0, 0.25, 0.0],
        "惊慌" => [0.0, 0.0, 0.0, 0.60, 0.0, 0.0, 0.25, 0.0],
        "担心" => [0.0, 0.0, 0.0, 0.45, 0.0, 0.20, 0.0, 0.0],

        "厌恶" | "反感" => [0.0, 0.10, 0.0, 0.0, 0.65, 0.0, 0.0, 0.0],
        "恶心" => [0.0, 0.05, 0.0, 0.0, 0.70, 0.0, 0.0, 0.0],
        "嫌弃" => [0.0, 0.10, 0.0, 0.0, 0.60, 0.0, 0.0, 0.0],
        "讨厌" => [0.0, 0.15, 0.0, 0.0, 0.55, 0.0, 0.0, 0.0],

        "惊讶" | "吃惊" => [0.05, 0.0, 0.0, 0.0, 0.0, 0.0, 0.65, 0.0],
        "震惊" => [0.0, 0.0, 0.0, 0.10, 0.0, 0.0, 0.70, 0.0],
        "诧异" => [0.0, 0.0, 0.0, 0.05, 0.0, 0.0, 0.60, 0.0],
        "意外" => [0.05, 0.0, 0.0, 0.0, 0.0, 0.0, 0.60, 0.0],

        "平静" | "自然" | "正常" => [0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.60],
        "冷静" => [0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.65],
        "淡定" => [0.05, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.60],
        "无语" => [0.0, 0.0, 0.0, 0.0, 0.0, 0.10, 0.0, 0.50],
        "无奈" => [0.0, 0.0, 0.0, 0.0, 0.0, 0.15, 0.0, 0.50],
        "尴尬" => [0.15, 0.0, 0.0, 0.10, 0.0, 0.0, 0.0, 0.45],
        "自信" => [0.20, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.55],
        "害羞" => [0.20, 0.0, 0.0, 0.10, 0.0, 0.0, 0.0, 0.45],
        "难为情" => [0.15, 0.0, 0.0, 0.10, 0.0, 0.0, 0.0, 0.45],
        "认真" => [0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.60],
        "疑惑" => [0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.20, 0.45],
        _ => return None,
    };
    Some(normalize(vector))
}

fn label_prompt(label: &str) -> String {
    match label {
        "高兴" => "开心愉快，语气轻快",
        "调皮" => "俏皮调侃，带着笑意",
        "情动" => "温柔动情，略带羞怯",
        "生气" => "生气恼火，语气激动",
        "哭泣" => "委屈地哭着，带着哭腔",
        "害怕" => "害怕不安，声音发紧",
        "紧张" => "紧张局促，声音发紧",
        "慌张" => "慌张失措，语速偏快",
        "担心" => "担心忧虑，语气放轻",
        "尴尬" => "尴尬窘迫，支支吾吾",
        "自信" => "自信从容，语气坚定",
        "害羞" => "害羞腼腆，声音变软",
        "认真" => "认真专注，语气平稳",
        "无语" => "无奈无语，语气平淡",
        "厌恶" => "厌恶嫌弃，语气冷淡",
        "疑惑" => "疑惑不解，带着疑问",
        "难为情" => "难为情，不好意思，声音发虚",
        "惊讶" => "惊讶诧异，音调抬高",
        "平静" => "平静自然，语气舒缓",
        other => other,
    }
    .to_string()
}

pub fn build_plan(mode: &str, label: &str, text: &str) -> EmotionPlan {
    let mode = mode.trim().to_ascii_lowercase();
    let label = sanitize_label(label);
    match mode.as_str() {
        "blend" => {
            if label.is_empty() {
                EmotionPlan::FollowReference
            } else if let Some(vector) = blend_vector(&label) {
                EmotionPlan::Vector(vector)
            } else {
                tracing::warn!("IndexTTS2 未识别情绪标签“{label}”，跟随参考音频");
                EmotionPlan::FollowReference
            }
        }
        "qwen" if !label.is_empty() => {
            let prompt = label_prompt(&label);
            EmotionPlan::Analyze {
                cache_key: format!("label:{prompt}"),
                prompt,
            }
        }
        "auto" if !text.trim().is_empty() => EmotionPlan::Analyze {
            cache_key: format!("auto:{}", text.trim()),
            prompt: text.trim().to_string(),
        },
        _ => EmotionPlan::FollowReference,
    }
}

#[derive(Debug, Default, Serialize, Deserialize)]
struct EmotionCacheFile {
    #[serde(default)]
    vectors: BTreeMap<String, EmotionVector>,
}

#[derive(Debug)]
pub struct EmotionCache {
    path: PathBuf,
    entries: BTreeMap<String, EmotionVector>,
}

impl EmotionCache {
    pub fn load(path: impl Into<PathBuf>) -> Self {
        let path = path.into();
        let entries = std::fs::read(&path)
            .ok()
            .and_then(|bytes| serde_json::from_slice::<EmotionCacheFile>(&bytes).ok())
            .map(|file| file.vectors)
            .unwrap_or_default();
        Self { path, entries }
    }

    pub fn get(&self, key: &str) -> Option<EmotionVector> {
        self.entries.get(key).copied()
    }

    pub fn insert(&mut self, key: String, vector: EmotionVector) -> Result<(), String> {
        if vector.iter().any(|value| !value.is_finite()) {
            return Err("Qwen 情绪向量包含非有限数值".into());
        }
        self.entries.insert(key, vector);
        self.save()
    }

    fn save(&self) -> Result<(), String> {
        if let Some(parent) = self.path.parent() {
            std::fs::create_dir_all(parent)
                .map_err(|error| format!("创建情绪缓存目录失败: {error}"))?;
        }
        let payload = serde_json::to_vec_pretty(&EmotionCacheFile {
            vectors: self.entries.clone(),
        })
        .map_err(|error| format!("序列化情绪缓存失败: {error}"))?;
        let temp = self.path.with_extension(format!(
            "{}.tmp-{}",
            self.path
                .extension()
                .and_then(|ext| ext.to_str())
                .unwrap_or("json"),
            uuid::Uuid::new_v4()
        ));
        std::fs::write(&temp, payload).map_err(|error| format!("写入情绪缓存失败: {error}"))?;
        if self.path.exists() {
            std::fs::remove_file(&self.path)
                .map_err(|error| format!("替换旧情绪缓存失败: {error}"))?;
        }
        std::fs::rename(&temp, &self.path).map_err(|error| format!("提交情绪缓存失败: {error}"))
    }
}

pub fn vector_from_slice(values: &[f32]) -> Result<EmotionVector, String> {
    values.try_into().map_err(|_| {
        format!(
            "情绪向量维度错误：期望 {EMOTION_DIMENSIONS}，实际 {}",
            values.len()
        )
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn blend_vectors_are_normalized_in_rust() {
        let EmotionPlan::Vector(vector) = build_plan("blend", "【生气】", "测试") else {
            panic!("应生成显式向量");
        };
        assert!(vector[1] > vector[4]);
        assert!(vector.iter().sum::<f32>() <= 0.800_001);
    }

    #[test]
    fn qwen_and_auto_cache_keys_are_stable() {
        assert_eq!(
            build_plan("qwen", "哭泣", "文本"),
            EmotionPlan::Analyze {
                cache_key: "label:委屈地哭着，带着哭腔".into(),
                prompt: "委屈地哭着，带着哭腔".into(),
            }
        );
        assert_eq!(
            build_plan("auto", "", "  hello  "),
            EmotionPlan::Analyze {
                cache_key: "auto:hello".into(),
                prompt: "hello".into(),
            }
        );
    }
}
