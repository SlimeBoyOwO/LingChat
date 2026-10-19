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
/// 去掉中文方头括号包裹的片段（如情绪标签残留）。
fn strip_corner_re() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| Regex::new(r"【[^】]*】").expect("invalid regex"))
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

        // 前处理：修复标签并清理非法内容
        let text = Self::preprocess_text(text);

        let re = emotion_re();
        let mut i = 0usize;

        // 第一个【情绪】标签之前的文本：正则从 `【` 才开始匹配，过去这一截会被整段丢掉
        // （回复以台词/旁白开头、标签在后面时，第一句根本不进 TTS）。按同一套规则清洗后
        // 并进第一个片段的正文，保证它会被念出来。
        let leading_end = re.find(&text).map(|m| m.start()).unwrap_or(text.len());
        let leading_text = Self::clean_following_text(&text[..leading_end]);
        // 只有需要译文（enable_translate）时才关心标签之前那截里的 <日文>。
        let leading_japanese = if self.options.enable_translate {
            japanese_re()
                .captures(&text[..leading_end])
                .and_then(|c| c.get(1))
                .map(|m| m.as_str().trim().replace('~', "。"))
                .unwrap_or_default()
        } else {
            String::new()
        };

        for cap in re.captures_iter(&text) {
            i += 1;
            let emotion_tag = cap.get(1).map(|m| m.as_str()).unwrap_or("");
            let following_raw = cap.get(2).map(|m| m.as_str()).unwrap_or("");
            let following_text = following_raw.replace('(', "（").replace(')', "）");

            let japanese_text = japanese_re()
                .captures(&following_text)
                .and_then(|c| c.get(1))
                .map(|m| m.as_str().trim().to_string())
                .unwrap_or_default();

            // 修复：如果japanese_text包含波浪号，修复它
            let japanese_text = japanese_text.replace('~', "。");
            let motion_text = motion_re()
                .captures(&following_text)
                .and_then(|c| c.get(1))
                .map(|m| m.as_str().trim().to_string())
                .unwrap_or_default();

            let cleaned_text = Self::clean_following_text(following_raw);
            // 标签之前的正文并进第一个片段，别让第一句被吞掉。
            let cleaned_text = if i == 1 && !leading_text.is_empty() {
                format!("{leading_text}{cleaned_text}")
            } else {
                cleaned_text
            };

            // 翻译关闭：cleaned_text 即 japanese_text
            let japanese_text = if !self.options.enable_translate {
                cleaned_text.clone()
            } else if !japanese_text.is_empty() {
                // 清理：去掉可能出现在日文里的 情绪/动作 片段
                let step1 = motion_re().replace_all(&japanese_text, "");
                let step2 = strip_corner_re().replace_all(&step1, "");
                step2.trim().replace('~', "。")
            } else if i == 1 && !leading_japanese.is_empty() {
                // 日文写在标签前面（`她……<日文>【开心】你好`）时别漏掉
                leading_japanese.clone()
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
                },
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

    /// 与 [`Self::parse_and_classify_emotional_segments`] 相同，但会把被 【情绪】 标签
    /// 打断的同一句并回一段（本地 TTS 按整句合成时用，见 [`merge_unfinished_segments`]）。
    pub fn parse_whole_sentence_segments(&self, text: &str) -> Vec<EmotionSegment> {
        let mut segments = self.parse_and_classify_emotional_segments(text);
        merge_unfinished_segments(&mut segments, self.options.enable_translate);
        segments
    }

    /// 片段正文清洗：统一括号后去掉 `<日文>` 与 `（动作）`。
    fn clean_following_text(raw: &str) -> String {
        let normalized = raw.replace('(', "（").replace(')', "）");
        strip_jp_action_re()
            .replace_all(&normalized, "")
            .trim()
            .to_string()
    }

    /// 文本预处理：修复标签、清理非法内容
    fn preprocess_text(text: &str) -> String {
        let mut processed = text.to_string();

        // 3. 清理违规内容
        // 删除 {} 内容
        let curly_re = Regex::new(r"\{[^{}]*\}").unwrap();
        processed = curly_re.replace_all(&processed, "").to_string();

        // 1. 统一括号风格（不转换书名号，书名号单独处理）
        processed = processed.replace('＜', "<").replace('＞', ">");

        // 移除书名号《》（保留内容，避免被误识别为日文标签）
        processed = processed.replace(['《', '》'], "");

        // 2. 修复未闭合标签（不使用正则前瞻）
        processed = Self::fix_unclosed_tags(&processed);

        processed
    }

    fn fix_unclosed_tags(text: &str) -> String {
        let mut result = String::with_capacity(text.len() + 10);
        let chars: Vec<char> = text.chars().collect();
        let mut i = 0;

        while i < chars.len() {
            if chars[i] == '<' {
                i += 1;

                // 收集标签内容（直到遇到 '>' 或 '\n'）
                let mut tag_content = String::new();
                while i < chars.len() && chars[i] != '>' && chars[i] != '\n' {
                    tag_content.push(chars[i]);
                    i += 1;
                }

                // 检查当前位置是否是 '>'
                let is_closed = i < chars.len() && chars[i] == '>';

                // 重建标签
                result.push('<');
                result.push_str(&tag_content);

                if is_closed {
                    result.push('>');
                    i += 1; // 跳过 '>'
                } else {
                    // 未闭合，补全 '>'
                    result.push('>');
                    // i 在换行符或结束位置，不要额外移动
                }
            } else {
                result.push(chars[i]);
                i += 1;
            }
        }

        result
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

/// 片段正文是否已经收句（句末标点，允许后面跟引号/右括号）。
///
/// 用来判断"这句话说完了没有"：没说完的片段会被并进下一段再送给 TTS。
pub fn ends_with_sentence_end(text: &str) -> bool {
    let trimmed = text.trim_end_matches(|c: char| {
        c.is_whitespace() || matches!(c, '”' | '’' | '"' | '\'' | '」' | '』' | '）' | ')' | '】')
    });
    trimmed
        .chars()
        .last()
        .is_some_and(|c| matches!(c, '。' | '！' | '？' | '.' | '!' | '?'))
}

/// 把相邻片段里"上一段还没说完"的部分并成一段。
///
/// 生产链按 【情绪】 标签切句：`【开心】哇，真的吗【兴奋】那我们走！` 会被切成两段。
/// 对不看情绪的本地 TTS 来说这种切法只有坏处——同一句话做了两次推理，语调和停顿各自
/// 重置。这里把没收到句末标点的片段并到下一段前面，交给 TTS 一次读完。
///
/// `enable_translate` 为 true 时片段自带日文译文，需要跟着正文一起拼上。
pub fn merge_unfinished_segments(segments: &mut Vec<EmotionSegment>, enable_translate: bool) {
    let mut merged: Vec<EmotionSegment> = Vec::with_capacity(segments.len());
    for segment in segments.drain(..) {
        match merged.last_mut() {
            Some(previous) if !ends_with_sentence_end(&previous.following_text) => {
                append_segment(previous, segment, enable_translate);
            },
            _ => merged.push(segment),
        }
    }
    *segments = merged;
}

/// 把 `next` 并进 `target`：正文直接相接（原文里本来就是一句话），补上空的动作/译文。
fn append_segment(target: &mut EmotionSegment, next: EmotionSegment, enable_translate: bool) {
    target.following_text.push_str(&next.following_text);
    if target.motion_text.is_empty() {
        target.motion_text = next.motion_text;
    }
    if !enable_translate {
        // 关闭翻译时 japanese_text 就是 cleaned_text，跟着正文一起变长。
        target.japanese_text = target.following_text.clone();
        return;
    }
    if target.japanese_text.is_empty() {
        target.japanese_text = next.japanese_text;
    } else if !next.japanese_text.is_empty() {
        target.japanese_text.push_str(&next.japanese_text);
    } else {
        // 一边有译文、一边没有：拼出来的译文不完整（日文语音会漏读半句），清掉让
        // 翻译器按拼接后的整句重译一次。
        target.japanese_text.clear();
    }
}

/// `Function.fix_ai_generated_text`：规范化带情绪标签的文本。语义 1:1 对照。
pub fn fix_ai_generated_text(text: &str) -> String {
    let text = text
        .replace('＜', "<")
        .replace('＞', ">")
        .replace(['《', '》'], "");
    let re = emotion_re();
    let mut parts: Vec<String> = Vec::new();
    let mut has_any = false;
    for cap in re.captures_iter(&text) {
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
            motion_re()
                .replace_all(&japanese_text, "")
                .trim()
                .to_string()
        } else {
            japanese_text
        };
        let japanese_text = japanese_text.replace('~', "。");

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

// ============================================================
// 回合结束边界 - 钦灵：用于解决 AI 代替用户说话的问题，可扩展
// ============================================================

/// 越界边界之一：模型把角色标签当成下一次发言的开头写了出来。
///
/// 这是它从预训练里带来的习惯（角色标签 `user` 单独成行），台词里不可能合法出现，
/// 所以拿它当终止信号零误伤。行内可带冒号与空白，但必须整行是这个标签。
fn user_label_re() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| Regex::new(r"(?m)^[ \t]*user[ \t]*[:：]?[ \t]*\r?\n").expect("invalid regex"))
}

/// 越界边界之二：连续两个空行。记忆构建器用它标记「本轮说完了」（台词之间只有一个空行），
/// 模型照抄这个形状；模型自己越界时也会拿它当段落分隔。
fn blank_gap_re() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| Regex::new(r"\n[ \t]*\r?\n[ \t]*\r?\n").expect("invalid regex"))
}

/// 命中的回合结束边界：起始字节位置 + 判据名（判据名只用于日志，便于分辨是哪条规则生效）。
pub type TurnEnd = (usize, &'static str);

/// 找出回复里第一个回合结束边界；模型正常收尾时返回 `None`。
///
/// 只认「前面已经有正文」的边界：回复开头的空行/`user` 行不算，否则整段会被截成空。
pub fn find_turn_end(text: &str) -> Option<TurnEnd> {
    let non_empty_prefix = |start: usize| !text[..start].trim().is_empty();

    let user_label = user_label_re()
        .find_iter(text)
        .find(|m| non_empty_prefix(m.start()))
        .map(|m| (m.start(), "user标签"));
    let blank_gap = blank_gap_re()
        .find_iter(text)
        .find(|m| non_empty_prefix(m.start()))
        .map(|m| (m.start(), "两空行"));

    match (user_label, blank_gap) {
        (Some(a), Some(b)) => Some(if a.0 <= b.0 { a } else { b }),
        (found, None) | (None, found) => found,
    }
}

/// 截到回合结束边界之前（含边界写法本身一起丢掉）。没有边界时原样返回。
/// 用于把已经存进台词表的越界内容从喂给模型的上下文里摘掉。
pub fn truncate_at_turn_end(text: &str) -> &str {
    match find_turn_end(text) {
        Some((pos, _)) => &text[..pos],
        None => text,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn processor() -> MessageProcessor {
        MessageProcessor::new(ProcessorOptions::default(), None)
    }

    /// 第一个【情绪】标签之前的文本不再被丢掉（并进第一段正文）。
    #[test]
    fn keeps_text_before_first_emotion_tag() {
        let p = processor();

        // （动作）剥掉后，标签之前的旁白并进第一段，和标签正文连成一句。
        let segs =
            p.parse_and_classify_emotional_segments("（她转过头）窗外的雨还在下。【开心】你好呀。");
        assert_eq!(segs.len(), 1);
        assert_eq!(segs[0].following_text, "窗外的雨还在下。你好呀。");
        assert_eq!(segs[0].original_tag, "开心");
        assert_eq!(segs[0].japanese_text, "窗外的雨还在下。你好呀。");

        // 常规写法（标签就在句首）行为不变。
        let segs = p.parse_and_classify_emotional_segments("【开心】你好呀。");
        assert_eq!(segs.len(), 1);
        assert_eq!(segs[0].following_text, "你好呀。");

        // 标签之前只有动作时不会凭空多出正文。
        let segs =
            p.parse_and_classify_emotional_segments("（她轻轻叹气）【无奈】算了，随便你吧。");
        assert_eq!(segs.len(), 1);
        assert_eq!(segs[0].following_text, "算了，随便你吧。");
    }

    /// 被 【情绪】 标签打断的同一句会被并回一段；已经收句的不动。
    #[test]
    fn merges_segments_split_inside_one_sentence() {
        let p = processor();

        let segs = p.parse_whole_sentence_segments("【开心】哇，真的吗【兴奋】那我们赶紧出发吧！");
        assert_eq!(segs.len(), 1);
        assert_eq!(segs[0].following_text, "哇，真的吗那我们赶紧出发吧！");
        assert_eq!(segs[0].original_tag, "开心");

        // 两句各自收句 → 保留两段（句与句之间的停顿不动）。
        let segs =
            p.parse_whole_sentence_segments("【开心】哇，真的吗？【兴奋】那我们赶紧出发吧！");
        assert_eq!(segs.len(), 2);

        // 原解析器（不看情绪的云端 TTS 走这条）行为不变。
        let segs =
            p.parse_and_classify_emotional_segments("【开心】哇，真的吗【兴奋】那我们赶紧出发吧！");
        assert_eq!(segs.len(), 2);
    }

    /// 打开翻译时，一边有译文一边没有：清掉译文让翻译器按整句重译，别漏读半句。
    #[test]
    fn drops_partial_translation_when_merging() {
        let p = MessageProcessor::new(
            ProcessorOptions {
                enable_translate: true,
                ..Default::default()
            },
            None,
        );
        let segs =
            p.parse_whole_sentence_segments("【开心】哇，真的吗<哇真的吗>【兴奋】那我们走！");
        assert_eq!(segs.len(), 1);
        assert_eq!(segs[0].following_text, "哇，真的吗那我们走！");
        assert_eq!(segs[0].japanese_text, "");
    }

    #[test]
    fn detects_finished_sentences() {
        assert!(ends_with_sentence_end("你好。"));
        assert!(ends_with_sentence_end("真的吗？”"));
        assert!(ends_with_sentence_end("let's go!"));
        assert!(!ends_with_sentence_end("真的吗"));
        assert!(!ends_with_sentence_end("她说："));
        assert!(!ends_with_sentence_end("我……"));
        assert!(!ends_with_sentence_end(""));
    }
}
