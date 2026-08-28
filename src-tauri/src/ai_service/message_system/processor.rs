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

/// 演出标签（<shake level="high"> 等，issue #658）保护/还原。
///
/// 后端解析（日语翻译提取、strip_jp_action_re 剥除、情绪标签处理）会把 `<...>` 当日语通道，
/// 必须先替换为占位符再解析、解析后按需还原：
/// - 消息内容（following_text）：还原标签，前端解析渲染动画
/// - TTS/译文（japanese_text）、动作文本（motion_text）：去掉标签，避免读出/显示标签文本
const EFFECT_TAG_RE: &str = r"</?(?:emphasis|shake|blur|float)\b[^>]*>";

fn protect_effect_tags(text: &str) -> (String, Vec<String>) {
    let re = Regex::new(EFFECT_TAG_RE).expect("invalid effect tag regex");
    let mut tags: Vec<String> = Vec::new();
    let mut out = String::with_capacity(text.len());
    let mut last = 0usize;
    for cap in re.captures_iter(text) {
        let m = cap.get(0).expect("match 0");
        out.push_str(&text[last..m.start()]);
        let ph = format!("\u{0}FX_TAG_{}\u{0}", tags.len());
        tags.push(m.as_str().to_string());
        out.push_str(&ph);
        last = m.end();
    }
    out.push_str(&text[last..]);
    (out, tags)
}

fn restore_effect_tags(protected: &str, tags: &[String]) -> String {
    let mut out = protected.to_string();
    for (i, tag) in tags.iter().enumerate() {
        out = out.replace(&format!("\u{0}FX_TAG_{}\u{0}", i), tag);
    }
    out
}

fn remove_effect_tags(protected: &str, tags: &[String]) -> String {
    let mut out = protected.to_string();
    for i in 0..tags.len() {
        out = out.replace(&format!("\u{0}FX_TAG_{}\u{0}", i), "");
    }
    out
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
        // 演出标签保护：避免被误判为日语翻译 <...> 或被 strip_jp_action_re 剥除
        let (protected, tags) = protect_effect_tags(&text);

        let re = emotion_re();
        let mut i = 0usize;

        for cap in re.captures_iter(&protected) {
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

            // 还原演出标签：消息内容保留（前端解析渲染动画）；TTS/译文与动作文本去掉标签
            let cleaned_text = restore_effect_tags(&cleaned_text, &tags);
            let japanese_text = remove_effect_tags(&japanese_text, &tags);
            let motion_text = remove_effect_tags(&motion_text, &tags);

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

    /// 文本预处理：修复标签、清理非法内容
    fn preprocess_text(text: &str) -> String {
        let mut processed = text.to_string();

        // 3. 清理违规内容
        // 删除 {} 内容
        let curly_re = Regex::new(r"\{[^{}]*\}").unwrap();
        processed = curly_re.replace_all(&processed, "").to_string();

        // 1. 统一括号风格（不转换书名号，书名号单独处理）
        processed = processed
            .replace('＜', "<")
            .replace('＞', ">");

        // 移除书名号《》（保留内容，避免被误识别为日文标签）
        processed = processed
            .replace('《', "")
            .replace('》', "");

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

/// `Function.fix_ai_generated_text`：规范化带情绪标签的文本。语义 1:1 对照。
pub fn fix_ai_generated_text(text: &str) -> String {
    let text = text
        .replace('＜', "<")
        .replace('＞', ">")
        .replace('《', "")
        .replace('》', "");
    // 演出标签保护：避免被误判为日语翻译 <...> 或被剥除
    let (protected, tags) = protect_effect_tags(&text);
    let re = emotion_re();
    let mut parts: Vec<String> = Vec::new();
    let mut has_any = false;
    for cap in re.captures_iter(&protected) {
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
        return restore_effect_tags(&protected, &tags);
    }
    restore_effect_tags(&parts.concat(), &tags)
}

#[cfg(test)]
mod effect_tag_protection_tests {
    use super::{
        fix_ai_generated_text, protect_effect_tags, remove_effect_tags, restore_effect_tags,
        MessageProcessor, ProcessorOptions,
    };

    /// 占位符形态：标签被替换为 NUL 包裹的标记，与任何解析正则（<...>、【】、（））都不冲突
    #[test]
    fn protect_restore_roundtrip_keeps_tags() {
        let raw = "今天的你！真的<shake level=\"high\">很好看！</shake>";
        let (protected, tags) = protect_effect_tags(raw);
        assert_eq!(
            protected,
            "今天的你！真的\u{0}FX_TAG_0\u{0}很好看！\u{0}FX_TAG_1\u{0}"
        );
        assert_eq!(tags.len(), 2);
        assert_eq!(restore_effect_tags(&protected, &tags), raw);
    }

    /// TTS/译文路径：标签占位符被剔除，避免读出标签文本
    #[test]
    fn remove_tags_for_tts_path() {
        let raw = "今天的你！真的<shake level=\"high\">很好看！</shake>";
        let (protected, tags) = protect_effect_tags(raw);
        assert_eq!(remove_effect_tags(&protected, &tags), "今天的你！真的很好看！");
    }

    /// 消息内容路径：fix_ai_generated_text 后标签保留，前端才能解析渲染动画
    #[test]
    fn fix_ai_generated_text_preserves_effect_tags() {
        let input = "【情动】今天的你！真的<shake level=\"high\">很好看！</shake>";
        let out = fix_ai_generated_text(input);
        assert!(out.contains("<shake level=\"high\">"), "开标签应保留: {out}");
        assert!(out.contains("</shake>"), "闭标签应保留: {out}");
    }

    /// 中日双语模式：演出标签不应被误抓为日语翻译
    #[test]
    fn japanese_extraction_not_confused_by_effect_tags() {
        let mp = MessageProcessor::new(
            ProcessorOptions {
                enable_translate: true,
                ..Default::default()
            },
            None,
        );
        let input =
            "【情动】今天的你！真的<shake level=\"high\">很好看！</shake><本当に綺麗だよ>";
        let segs = mp.parse_and_classify_emotional_segments(input);
        assert_eq!(segs.len(), 1);
        let s = &segs[0];
        assert_eq!(s.japanese_text, "本当に綺麗だよ", "译文不应被标签干扰");
        assert_eq!(
            s.following_text,
            "今天的你！真的<shake level=\"high\">很好看！</shake>",
            "消息内容应保留标签"
        );
    }

    /// 中文模式：japanese_text 回退 cleaned_text，但 TTS 文本必须剔除标签
    #[test]
    fn cn_mode_tts_text_has_no_tags() {
        let mp = MessageProcessor::new(ProcessorOptions::default(), None);
        let input = "【情动】今天的你！真的<shake level=\"high\">很好看！</shake>";
        let segs = mp.parse_and_classify_emotional_segments(input);
        let s = &segs[0];
        assert_eq!(
            s.following_text,
            "今天的你！真的<shake level=\"high\">很好看！</shake>"
        );
        assert_eq!(s.japanese_text, "今天的你！真的很好看！", "TTS 文本不含标签");
    }

    /// 未知标签不属于保护范围（保护只针对已知演出标签），不生成占位符
    #[test]
    fn unknown_tags_are_not_protected() {
        let raw = "今天<weather>天气</weather>不错";
        let (protected, tags) = protect_effect_tags(raw);
        assert!(tags.is_empty(), "未知标签不应进入保护: {protected}");
        assert_eq!(protected, raw);
        // 已知标签与未知标签混排时，只保护已知的
        let mixed = "真<shake>好</shake>看<weather>天</weather>";
        let (protected2, tags2) = protect_effect_tags(mixed);
        assert_eq!(tags2.len(), 2);
        assert!(protected2.contains("\u{0}FX_TAG_0\u{0}"));
        assert!(protected2.contains("<weather>"), "未知标签保持原样: {protected2}");
    }

    /// 标签误入日语翻译块（LLM 格式错误时）：译文剔除标签、中文台词保留标签
    #[test]
    fn tags_inside_japanese_block_are_removed() {
        let mp = MessageProcessor::new(
            ProcessorOptions {
                enable_translate: true,
                ..Default::default()
            },
            None,
        );
        let input = "【认真】<emphasis>用户酱，你知道吗。</emphasis>每一次这样……\n<見ててね、<emphasis>一番の特技</emphasis>なんだから>";
        let segs = mp.parse_and_classify_emotional_segments(input);
        assert_eq!(segs.len(), 1);
        let s = &segs[0];
        assert_eq!(s.japanese_text, "見ててね、一番の特技なんだから", "译文剔除标签");
        assert_eq!(
            s.following_text,
            "<emphasis>用户酱，你知道吗。</emphasis>每一次这样……",
            "中文台词保留标签"
        );
    }
}
