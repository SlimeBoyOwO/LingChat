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

use crate::ai_service::llm::{slot_snapshot, LlmSlot};
use crate::ai_service::message_system::processor::EmotionSegment;
use crate::ai_service::types::LlmMessage;

fn translator_system_prompt(target_lang: &str) -> Option<String> {
    let (language_name, extra_instruction) = match target_lang {
        "ja" => ("日语", "使用自然、口语化且符合二次元角色语气的日语表达。"),
        "en" => ("英语", "使用自然、口语化的英语表达。"),
        "ko" => ("韩语", "使用自然、口语化且符合角色语气的韩语表达。"),
        _ => return None,
    };

    Some(format!(
        "你是一个二次元角色中文台词翻译师。请将每个中文台词片段翻译成{language_name}，允许意译，保持角色语气自然生动。{extra_instruction}\n\
         只返回翻译结果，不要解释；输出片段数量必须与输入一致，并且每个片段都必须分别包裹在 < 和 > 中。"
    ))
}

/// 翻译器。
pub struct Translator {
    /// 是否启用实时翻译（对应旧版 `ENABLE_TRANSLATE`）。禁用时 `translate` 直接返回。
    pub enable: bool,
    /// 翻译用 LLM 槽位（支持运行时热切换）。
    /// 槽位本身始终存在，内部值为 None 时表示未配置翻译模型。
    client: LlmSlot,
}

impl Translator {
    pub fn new(client: LlmSlot, enable: bool) -> Self {
        Self { enable, client }
    }

    /// 返回翻译 LLM 槽位引用（用于热切换）。
    pub fn slot(&self) -> &LlmSlot {
        &self.client
    }

    /// 把 segments 中的中文翻译成日文，原地写回 `japanese_text`。
    ///
    /// `script`=`true` 时，即使 `enable=false` 也翻译（旧版剧本默认一定翻译）。
    pub async fn translate_segments(
        &self,
        segments: &mut [EmotionSegment],
        script: bool,
    ) -> Result<()> {
        self.translate_segments_to(segments, script, "ja")
            .await
            .map(|_| ())
    }

    /// 将中文台词翻译成指定语言，并返回是否取得了全部片段的译文。
    pub async fn translate_segments_to(
        &self,
        segments: &mut [EmotionSegment],
        script: bool,
        target_lang: &str,
    ) -> Result<bool> {
        if !self.enable && !script {
            return Ok(false);
        }
        let Some(system_prompt) = translator_system_prompt(target_lang) else {
            tracing::warn!("Translator: 不支持的目标语言 {target_lang}，跳过翻译");
            return Ok(false);
        };
        let Some(client) = slot_snapshot(&self.client).await else {
            tracing::warn!("Translator: 翻译 LLM 槽位为空，跳过翻译");
            return Ok(false);
        };

        let full_chinese = collect_chinese_part(segments);
        if full_chinese.is_empty() {
            tracing::warn!("AI回复没有可翻译文本，跳过 {target_lang} 翻译");
            return Ok(false);
        }

        let messages = vec![
            LlmMessage::system(system_prompt),
            LlmMessage::user(full_chinese),
        ];

        let translated_response = client.complete(&messages).await?;
        tracing::info!("完整 {target_lang} 翻译结果: {translated_response}");

        let translated_count = apply_translation_result(&translated_response, segments);
        if translated_count != segments.len() {
            tracing::warn!(
                "Translator: {target_lang} 译文片段不完整: {translated_count}/{}",
                segments.len()
            );
        }
        Ok(translated_count == segments.len())
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
fn apply_translation_result(response: &str, segments: &mut [EmotionSegment]) -> usize {
    let mut cursor = response;
    let mut idx = 0usize;
    while let Some(start) = cursor.find('<') {
        let after = &cursor[start + 1..];
        let Some(end_rel) = after.find('>') else {
            break;
        };
        let jp = &after[..end_rel];
        if idx >= segments.len() {
            break;
        }
        segments[idx].japanese_text = jp.to_string();
        idx += 1;
        cursor = &after[end_rel + 1..];
    }
    idx
}
