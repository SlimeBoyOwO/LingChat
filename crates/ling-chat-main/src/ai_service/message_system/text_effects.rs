use serde::{Deserialize, Serialize};

use crate::ai_service::types::LlmMessage;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum TextEffect {
    Emphasis,
    Shake,
    Blur,
    Float,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TextEffectSpan {
    // 下标与前端字符串一致，使用 UTF-16，左闭右开。
    pub start: usize,
    pub end: usize,
    pub effect: TextEffect,
}

/// 只解析完整、非嵌套的演出标记；其他内容逐字保留。
pub fn parse_text_effects(raw: &str) -> (String, Vec<TextEffectSpan>) {
    let mut text = String::with_capacity(raw.len());
    let mut spans = Vec::new();
    let mut offset = 0;
    let mut utf16_len = 0;
    while offset < raw.len() {
        let rest = &raw[offset..];
        let mut consumed = 0;
        if let Some(after_colon) = rest.strip_prefix(':') {
            let name_len = after_colon
                .bytes()
                .take_while(|b| b.is_ascii_alphabetic() || *b == b'-')
                .count();
            if after_colon.as_bytes().get(name_len) == Some(&b'[') {
                let content_start = name_len + 2;
                let mut depth = 1;
                let mut nested = false;
                for (i, ch) in rest[content_start..].char_indices() {
                    match ch {
                        '[' => {
                            depth += 1;
                            nested = true;
                        },
                        ']' => depth -= 1,
                        _ => {},
                    }
                    if depth != 0 {
                        continue;
                    }
                    let end = content_start + i;
                    let content = &rest[content_start..end];
                    let effect = match &after_colon[..name_len] {
                        "emphasis" => Some(TextEffect::Emphasis),
                        "shake" => Some(TextEffect::Shake),
                        "blur" => Some(TextEffect::Blur),
                        "float" => Some(TextEffect::Float),
                        _ => None,
                    };
                    consumed = end + 1;
                    if let Some(effect) = effect.filter(|_| {
                        !nested
                            && (1..=30).contains(&content.chars().count())
                            && !content.contains(['\r', '\n'])
                    }) {
                        let start = utf16_len;
                        text.push_str(content);
                        utf16_len += content.encode_utf16().count();
                        spans.push(TextEffectSpan {
                            start,
                            end: utf16_len,
                            effect,
                        });
                    } else {
                        text.push_str(&rest[..consumed]);
                        utf16_len += rest[..consumed].encode_utf16().count();
                    }
                    break;
                }
                // 未闭合的外层不能使内部标记意外生效。
                if consumed == 0 {
                    text.push_str(rest);
                    break;
                }
            }
        }
        if consumed == 0 {
            let ch = rest.chars().next().unwrap();
            text.push(ch);
            utf16_len += ch.len_utf16();
            consumed = ch.len_utf8();
        }
        offset += consumed;
    }
    (text, spans)
}

const PROMPT: &str = "\n\n可选的台词文字演出：:emphasis[文字] 表示加重强调，:shake[文字] 表示抖动，:blur[文字] 表示模糊，:float[文字] 表示轻轻飘动。\n只在确有演出需要时使用，可以完全不用，每句最多一处；标记内限1到30字，不跨行、不嵌套，不使用参数。只标记台词，不能标记【情绪】、（动作）或<日语翻译>。继续遵守原有对话格式。\n示例：\n【紧张】你:shake[别过来！]（退后一步）\n【开心】今天真是:float[美好的一天～]\n【认真】这件事:emphasis[非常重要]。<これはとても大切なことだ。>\n【困惑】那天的事，已经:blur[记不清了……]";

pub fn inject_text_effects(context: &mut Vec<LlmMessage>, enabled: bool) {
    if !enabled || context.is_empty() {
        return;
    }
    if let Some(system) = context.iter_mut().find(|message| message.role == "system") {
        system.content.push_str(PROMPT);
    } else {
        context.insert(0, LlmMessage::system(PROMPT));
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn text_effects_preserve_unicode_offsets_and_multiple_spans() {
        let (text, spans) = parse_text_effects("😀你:shake[别过来！] :float[好🌟]");
        assert_eq!(text, "😀你别过来！ 好🌟");
        assert_eq!(
            spans,
            vec![
                TextEffectSpan {
                    start: 3,
                    end: 7,
                    effect: TextEffect::Shake
                },
                TextEffectSpan {
                    start: 8,
                    end: 11,
                    effect: TextEffect::Float
                },
            ]
        );
    }

    #[test]
    fn text_effects_reject_invalid_markup_without_losing_text() {
        for raw in [
            ":shake[]",
            ":shake[未闭合",
            ":unknown[文字]",
            ":shake-high[文字]",
            ":Shake[文字]",
            ":shake[跨\n行]",
            ":float[跨\r行]",
            ":shake[外:blur[内]]",
            ":unknown[:shake[内]]",
            ":shake[外:blur[内]",
        ] {
            assert_eq!(parse_text_effects(raw), (raw.to_string(), vec![]));
        }
        let raw = format!(":shake[{}]", "字".repeat(31));
        assert_eq!(parse_text_effects(&raw), (raw, vec![]));
    }

    #[test]
    fn text_effects_accept_all_effects_and_length_boundary() {
        for name in ["emphasis", "shake", "blur", "float"] {
            let content = "🌟".repeat(30);
            let (text, spans) = parse_text_effects(&format!(":{name}[{content}]"));
            assert_eq!(text, content);
            assert_eq!(spans[0].end, 60);
        }
    }

    #[test]
    fn text_effects_prompt_is_request_local_and_optional() {
        let original = vec![LlmMessage::system("角色人设"), LlmMessage::user("你好")];
        let mut disabled = original.clone();
        inject_text_effects(&mut disabled, false);
        assert_eq!(disabled, original);
        let mut enabled = original.clone();
        inject_text_effects(&mut enabled, true);
        assert!(enabled[0].content.contains(":shake[文字]"));
        assert_eq!(enabled.len(), original.len());
        assert_eq!(original[0].content, "角色人设");
    }

    #[test]
    fn text_effects_survive_normalization_and_leave_tts_plain() {
        use super::super::processor::{MessageProcessor, ProcessorOptions, fix_ai_generated_text};
        let input = "【紧张】你:shake[别过来！]（:float[退后一步]）<:emphasis[来ないで！]>";
        let normalized = fix_ai_generated_text(input);
        assert!(normalized.contains(":shake[别过来！]"));
        for translate in [false, true] {
            let processor = MessageProcessor::new(
                ProcessorOptions {
                    enable_translate: translate,
                    ..Default::default()
                },
                None,
            );
            let segments = processor.parse_and_classify_emotional_segments(&normalized);
            assert_eq!(segments.len(), 1);
            let segment = &segments[0];
            assert_eq!(segment.following_text, "你别过来！");
            assert_eq!(segment.motion_text, "退后一步");
            assert_eq!(
                segment.japanese_text,
                if translate {
                    "来ないで！"
                } else {
                    "你别过来！"
                }
            );
            assert_eq!(
                segment.text_effects,
                vec![TextEffectSpan {
                    start: 1,
                    end: 5,
                    effect: TextEffect::Shake,
                }]
            );
        }
    }

    #[test]
    fn text_effects_reply_payload_has_optional_camel_case_metadata() {
        use super::super::responses::ReplyResponse;
        let mut response = ReplyResponse::new_reply();
        let plain = serde_json::to_value(&response).unwrap();
        assert!(plain.get("textEffects").is_none());
        assert!(
            serde_json::from_value::<ReplyResponse>(plain)
                .unwrap()
                .text_effects
                .is_empty()
        );
        (response.message, response.text_effects) = parse_text_effects("你:shake[好]");
        let payload = serde_json::to_value(response).unwrap();
        assert_eq!(payload["message"], "你好");
        assert_eq!(payload["textEffects"][0]["effect"], "shake");
        assert_eq!(payload["textEffects"][0]["start"], 1);
    }
}
