//! 消息预处理器：对标 Python `MessageProcessor`。
//!
//! 关键能力：
//! - `append_user_message`：给用户消息追加 `{系统提醒: ...}` 段（时间/桌面/大括号/Temp）。
//! - `parse_and_classify_emotional_segments`：把 AI 回复里的 `【情绪】正文<日语>（动作）`
//!   切成结构化 segment。
//!
//! 与旧版的差异：
//! - 桌面分析（`DesktopAnalyzer`）暂不移植。
//! - 情绪预测：使用 [`EmotionClassifier`]（ONNX）；未注入时 fallback 为 `original_tag`。

use std::sync::{Arc, Mutex, OnceLock};
use std::time::{Duration, Instant};

use chrono::Local;
use regex::Regex;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::ai_service::emotion::EmotionClassifier;

/// 单个情绪片段。字段与旧版 `parse_and_classify_emotional_segments` 返回一致。
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct EmotionSegment {
    pub index: usize,
    pub original_tag: String,
    pub following_text: String,
    pub motion_text: String,
    pub japanese_text: String,
    /// 情绪分类器的预测 label。未启用时等于 `original_tag`。
    pub predicted: String,
    pub confidence: f64,
    pub voice_file: String,

    /// 关联角色信息（由 consumer 填充）
    pub character: Option<String>,
    pub role_id: Option<i32>,
}

/// 处理用户消息的结果。
#[derive(Debug, Clone, Default)]
pub struct UserMessageOutcome {
    pub main: String,
    pub temp: Option<String>,
}

/// 单例 Regex 缓存。
fn emotion_re() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| Regex::new(r"【([^】]*)】([^【】]*)").expect("invalid regex"))
}
fn japanese_re() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| Regex::new(r"<([^>]*)>").expect("invalid regex"))
}
fn motion_re() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| Regex::new(r"（([^）]*)）").expect("invalid regex"))
}
fn bracket_re() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| Regex::new(r"\{([^}]+)\}").expect("invalid regex"))
}
fn temp_re() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| Regex::new(r"(?s)\[!Temp!\](.*?)\[/!Temp!\]").expect("invalid regex"))
}
fn strip_jp_action_re() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| Regex::new(r"<[^>]*>|（[^）]*）").expect("invalid regex"))
}

/// MessageProcessor 配置。
#[derive(Debug, Clone, Copy)]
pub struct ProcessorOptions {
    pub time_sense_enabled: bool,
    /// 对应旧版 `ENABLE_TRANSLATE`。关闭时 `japanese_text` 会回退为 cleaned_text。
    pub enable_translate: bool,
}

impl Default for ProcessorOptions {
    fn default() -> Self {
        Self {
            time_sense_enabled: true,
            enable_translate: false,
        }
    }
}

/// MessageProcessor：无状态逻辑 + 少量时间感知计数。
pub struct MessageProcessor {
    options: ProcessorOptions,
    inner: Mutex<ProcessorInner>,
    classifier: Option<Arc<EmotionClassifier>>,
}

struct ProcessorInner {
    last_time: Instant,
    sys_time_counter: u32,
}

impl MessageProcessor {
    pub fn new(options: ProcessorOptions, classifier: Option<Arc<EmotionClassifier>>) -> Self {
        Self {
            options,
            inner: Mutex::new(ProcessorInner {
                last_time: Instant::now(),
                sys_time_counter: 0,
            }),
            classifier,
        }
    }

    /// 解析并分类情绪片段。语义对应 Python `parse_and_classify_emotional_segments`。
    pub fn parse_and_classify_emotional_segments(&self, text: &str) -> Vec<EmotionSegment> {
        let mut results: Vec<EmotionSegment> = Vec::new();

        let re = emotion_re();
        let mut i = 0usize;
        for cap in re.captures_iter(text) {
            i += 1;
            let emotion_tag = cap.get(1).map(|m| m.as_str()).unwrap_or("");
            let following_raw = cap.get(2).map(|m| m.as_str()).unwrap_or("");
            let following_text = following_raw.replace('(', "（").replace(')', "）");

            let japanese_text = japanese_re()
                .captures(&following_text)
                .and_then(|c| c.get(1))
                .map(|m| m.as_str().trim().to_string())
                .unwrap_or_default();
            let motion_text = motion_re()
                .captures(&following_text)
                .and_then(|c| c.get(1))
                .map(|m| m.as_str().trim().to_string())
                .unwrap_or_default();

            let cleaned_text = strip_jp_action_re()
                .replace_all(&following_text, "")
                .trim()
                .to_string();

            // 翻译关闭：cleaned_text 即 japanese_text
            let japanese_text = if !self.options.enable_translate {
                cleaned_text.clone()
            } else if !japanese_text.is_empty() {
                // 清理：去掉可能出现在日文里的 情绪/动作 片段
                let step1 = motion_re().replace_all(&japanese_text, "");
                let step2 = Regex::new(r"【[^】]*】").unwrap().replace_all(&step1, "");
                step2.trim().replace('~', "。")
            } else {
                japanese_text
            };

            if cleaned_text.is_empty() && japanese_text.is_empty() && motion_text.is_empty() {
                continue;
            }

            // 情绪分类器：有分类器走 ONNX，否则回退为原 tag。
            let (predicted, confidence) = match self.classifier.as_ref() {
                Some(clf) => {
                    let p = clf.predict(emotion_tag, None);
                    (p.label, p.confidence as f64)
                }
                None => (emotion_tag.to_string(), 1.0),
            };

            let voice_file = format!("{}_part_{}.wav", Uuid::new_v4(), i);

            results.push(EmotionSegment {
                index: i,
                original_tag: emotion_tag.to_string(),
                following_text: cleaned_text,
                motion_text,
                japanese_text,
                predicted,
                confidence,
                voice_file,
                character: None,
                role_id: None,
            });
        }

        if results.is_empty() {
            tracing::warn!("未在文本中找到【】格式的情绪标签");
        }

        results
    }

    /// 处理用户消息，提取 `{...}` 旁白、`[!Temp!]...[/!Temp!]` 临时指令，拼接系统提醒。
    pub async fn append_user_message(&self, user_message: &str) -> UserMessageOutcome {
        let mut processed_message = user_message.to_string();
        let mut user_instruction_part = String::new();
        let mut temp_instruction_part = String::new();

        // 大括号
        let bracket_matches: Vec<String> = bracket_re()
            .captures_iter(user_message)
            .filter_map(|c| c.get(1).map(|m| m.as_str().to_string()))
            .collect();
        if !bracket_matches.is_empty() {
            processed_message = bracket_re()
                .replace_all(&processed_message, "")
                .trim()
                .to_string();
            user_instruction_part = format!("旁白: {}", bracket_matches.join("; "));
        }

        // [!Temp!]..
        let temp_matches: Vec<String> = temp_re()
            .captures_iter(user_message)
            .filter_map(|c| c.get(1).map(|m| m.as_str().to_string()))
            .collect();
        if !temp_matches.is_empty() {
            processed_message = temp_re()
                .replace_all(&processed_message, "")
                .trim()
                .to_string();
            let pieces: Vec<String> = temp_matches.iter().map(|m| format!("${m}$")).collect();
            temp_instruction_part = pieces.join("%");
        }

        // 时间感知
        let now = Instant::now();
        let mut sys_time_part = String::new();
        if self.options.time_sense_enabled {
            let (should_emit, _reset) = {
                let inner = self.inner.lock().unwrap();
                let long_enough = now.duration_since(inner.last_time) > Duration::from_secs(3600);
                (long_enough || inner.sys_time_counter < 1, long_enough)
            };
            if should_emit {
                let formatted = Local::now().format("%Y/%m/%d %H:%M").to_string();
                sys_time_part = format!("{formatted} ");
            }
        }

        // 构建系统提醒
        let mut system_parts: Vec<String> = Vec::new();
        let mut sys_flag = false;
        if !sys_time_part.is_empty() {
            system_parts.push(sys_time_part);
            sys_flag = true;
        }
        if !user_instruction_part.is_empty() {
            system_parts.push(user_instruction_part.clone());
        }
        if !temp_instruction_part.is_empty() {
            system_parts.push(temp_instruction_part.clone());
        }

        if !system_parts.is_empty() {
            let prefix = if sys_flag { "系统提醒: " } else { "" };
            processed_message.push_str(&format!("\n{{{}{}}}", prefix, system_parts.join(" ")));
        }

        // 更新计数
        {
            let mut inner = self.inner.lock().unwrap();
            inner.last_time = now;
            inner.sys_time_counter = inner.sys_time_counter.saturating_add(1);
            if inner.sys_time_counter >= 2 {
                inner.sys_time_counter = 0;
            }
        }

        tracing::info!("处理后的用户信息是: {processed_message}");
        UserMessageOutcome {
            main: processed_message,
            temp: if temp_instruction_part.is_empty() {
                None
            } else {
                Some(temp_instruction_part)
            },
        }
    }
}

/// `Function.fix_ai_generated_text`：规范化带情绪标签的文本。语义 1:1 对照。
pub fn fix_ai_generated_text(text: &str) -> String {
    let re = emotion_re();
    let mut parts: Vec<String> = Vec::new();
    let mut has_any = false;
    for cap in re.captures_iter(text) {
        has_any = true;
        let emotion_tag = cap.get(1).map(|m| m.as_str()).unwrap_or("");
        let full_tag = format!("【{emotion_tag}】");
        let following_raw = cap.get(2).map(|m| m.as_str()).unwrap_or("");
        let following_text = following_raw.replace('(', "（").replace(')', "）");

        let japanese_text = japanese_re()
            .captures(&following_text)
            .and_then(|c| c.get(1))
            .map(|m| m.as_str().trim().to_string())
            .unwrap_or_default();
        let motion_text = motion_re()
            .captures(&following_text)
            .and_then(|c| c.get(1))
            .map(|m| m.as_str().trim().to_string())
            .unwrap_or_default();
        let cleaned_text = strip_jp_action_re()
            .replace_all(&following_text, "")
            .trim()
            .to_string();

        let japanese_text = if !japanese_text.is_empty() {
            motion_re().replace_all(&japanese_text, "").trim().to_string()
        } else {
            japanese_text
        };

        let mut normalized = full_tag;
        if !cleaned_text.is_empty() {
            normalized.push_str(&cleaned_text);
        }
        if !japanese_text.is_empty() {
            normalized.push('<');
            normalized.push_str(&japanese_text);
            normalized.push('>');
        }
        if !motion_text.is_empty() {
            normalized.push('（');
            normalized.push_str(&motion_text);
            normalized.push('）');
        }

        if !cleaned_text.is_empty() || !japanese_text.is_empty() {
            parts.push(normalized);
        }
    }

    if !has_any {
        return text.to_string();
    }
    parts.concat()
}
