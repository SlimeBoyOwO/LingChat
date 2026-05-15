//! 中文 → 日文翻译器。对标 Python `ling_chat.core.ai_service.translator.Translator`。
//!
//! 输入/输出结构：输入是 `parse_and_classify_emotional_segments` 产出的若干
//! [`crate::ai_service::message_system::processor::EmotionSegment`]，把每个 segment 的
//! `following_text`（中文）翻译后写回到 `japanese_text` 字段。
//!
//! 与 Python 版差异：
//! - 不再和 VoiceMaker 直接耦合（TTS 子系统尚未移植）。流式分支 / 非流式分支一律
//!   只做翻译，把结果写回 segment；上层再决定要不要做 TTS。
//! - provider 通过 [`crate::ai_service::llm::LlmClient`] 注入，不同环境可以用不同的小模型。

use anyhow::Result;

use crate::ai_service::llm::LlmClient;
use crate::ai_service::message_system::processor::EmotionSegment;
use crate::ai_service::types::LlmMessage;

const TRANSLATOR_SYSTEM_PROMPT: &str = "\n            你是一个二次元角色中文台词翻译师，任务是翻译二次元台词对话，\n            将中文翻译成日语，允许意译。确保你的翻译符合二次元的发言习惯，而不是生硬的直译，保持流畅自然生动。\n            除了翻译内容，你不提供额外的解释，并且你的翻译句子必须包裹在<>符号内，否则会导致严重错误。\n            比如，原文内容为：\n            <你好呀莱姆，今天过的怎么样呀？><哎？有点不高兴吗？没关系~>\n            那么你的回复内容为：\n            <はいはい、レムちゃん、今日はどうだった？><えっ？なんだかご機嫌ななめ？大丈夫だよ～>\n            ";

/// 翻译器。
pub struct Translator {
    /// 是否启用实时翻译（对应旧版 `ENABLE_TRANSLATE`）。禁用时 `translate` 直接返回。
    pub enable: bool,
    /// 翻译用 LLM。配置不可用时 translator 仍可构造，只是 translate 会提前返回。
    client: Option<LlmClient>,
}

impl Translator {
    pub fn new(client: Option<LlmClient>, enable: bool) -> Self {
        Self { enable, client }
    }

    /// 把 segments 中的中文翻译成日文，原地写回 `japanese_text`。
    ///
    /// `script`=`true` 时，即使 `enable=false` 也翻译（旧版剧本默认一定翻译）。
    pub async fn translate_segments(
        &self,
        segments: &mut [EmotionSegment],
        script: bool,
    ) -> Result<()> {
        if !self.enable && !script {
            return Ok(());
        }
        let Some(client) = self.client.as_ref() else {
            log::warn!("Translator: 未配置翻译 LLM，跳过翻译");
            return Ok(());
        };

        let full_chinese = collect_chinese_part(segments);
        if full_chinese.is_empty() {
            log::warn!("AI回复没有中文，跳过日语翻译");
            return Ok(());
        }

        let messages = vec![
            LlmMessage::system(TRANSLATOR_SYSTEM_PROMPT),
            LlmMessage::user(full_chinese),
        ];

        let japanese_response = client.complete(&messages).await?;
        log::info!("完整日语翻译结果: {japanese_response}");

        apply_translation_result(&japanese_response, segments);
        Ok(())
    }
}

fn collect_chinese_part(segments: &[EmotionSegment]) -> String {
    let mut out = String::new();
    for s in segments {
        out.push('<');
        out.push_str(&s.following_text);
        out.push('>');
    }
    out
}

/// 从翻译结果中依次抽取 `<...>` 片段并写回到每个 segment 的 `japanese_text`。
fn apply_translation_result(response: &str, segments: &mut [EmotionSegment]) {
    let mut cursor = response;
    let mut idx = 0usize;
    while let Some(start) = cursor.find('<') {
        let after = &cursor[start + 1..];
        let Some(end_rel) = after.find('>') else { break };
        let jp = &after[..end_rel];
        if idx >= segments.len() {
            break;
        }
        segments[idx].japanese_text = jp.to_string();
        idx += 1;
        cursor = &after[end_rel + 1..];
    }
}
