//! 试听文本的语言判断。
//!
//! 中文、英文、中英混合（含数字）都不用显式指定语言：sherpa/espeak 与各远程服务
//! 自己就能处理。日文不行——kokoro 不指定语言时会把假名逐字当英文/中文念，实测
//! 「こんにちは、これはテストです。」(14 字) 要 9.9 秒且完全听不出是日文；显式指定
//! `ja` 后降到 2.4 秒的正常日文（用 sense-voice 反读确认）。
//!
//! 所以试听时按文本里的假名/谚文自动换语言：把自己要试的文本直接粘进来就行，不必先
//! 去改角色或全局的语言设置（那个设置会影响聊天时的语音合成）。

use crate::ai_service::types::CharacterSettings;

/// 从试听文本判断语言；判断不出来返回 `None`，表示沿用设置里的语言。
///
/// 只认「假名」「谚文」这类能定位到单一语言的字符。中文/英文/数字不参与判断：
/// 显式写 `zh` 反而会让中英混合文本和数字的读法变差（适配器里也是这么处理的）。
pub(crate) fn detect(text: &str) -> Option<&'static str> {
    let mut hangul = false;
    for c in text.chars() {
        let cp = c as u32;
        // 平假名、片假名、片假名音标扩展：只有日文会用。
        // 刻意不含 U+30FB「・」与 U+30FC「ー」——中文里也会用它们拼外文译名。
        if (0x3041..=0x3096).contains(&cp)
            || (0x30a1..=0x30fa).contains(&cp)
            || (0x31f0..=0x31ff).contains(&cp)
        {
            return Some("ja");
        }
        // 谚文音节、谚文字母、谚文兼容字母：只有韩文会用。
        if (0xac00..=0xd7a3).contains(&cp)
            || (0x1100..=0x11ff).contains(&cp)
            || (0x3131..=0x318e).contains(&cp)
        {
            hangul = true;
        }
    }
    hangul.then_some("ko")
}

/// 用试听文本判断出的语言覆盖设置里的语言字段，返回实际采用的语言。
///
/// 只动语言字段，其余配置（音色、参考音频、语速…）原样保留。`sherpa`/`sherpa-onnx`
/// 走 `voice_models.sherpa_onnx_lang`，其余服务走顶层 `voice_lang`；两个都写上，免得
/// 服务类型判断不准时语言丢失（`VoiceMaker` 里 sherpa 以 `sherpa_onnx_lang` 优先，
/// 留空才回落到 `voice_lang`）。
pub(crate) fn apply_to_settings(
    settings: &mut CharacterSettings,
    text: &str,
) -> Option<&'static str> {
    let language = detect(text)?;
    settings.voice_lang = Some(language.to_string());
    let tts_type = settings.tts_type.as_deref().unwrap_or("").trim();
    if matches!(tts_type, "sherpa" | "sherpa-onnx") {
        settings
            .voice_models
            .get_or_insert_with(Default::default)
            .sherpa_onnx_language = Some(language.to_string());
    }
    Some(language)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn detects_japanese_by_kana() {
        assert_eq!(detect("こんにちは、これはテストです。"), Some("ja"));
        assert_eq!(detect("カタカナだけ"), Some("ja"));
        assert_eq!(detect("你好，こんにちは"), Some("ja"));
    }

    #[test]
    fn does_not_mistake_chinese_punctuation_for_kana() {
        // 「・」「ー」在中文里也用来拼外文译名，不能当成日文
        assert_eq!(detect("迈克尔・乔丹"), None);
        assert_eq!(detect("咖啡ー杯"), None);
    }

    #[test]
    fn detects_korean_by_hangul() {
        assert_eq!(detect("안녕하세요, 반갑습니다."), Some("ko"));
        assert_eq!(detect("한국어"), Some("ko"));
    }

    #[test]
    fn leaves_chinese_english_and_numbers_alone() {
        for text in [
            "你好，这是一段测试语音。",
            "Hello, world! 123",
            "中英 mixed 123",
            "",
            "   ",
        ] {
            assert_eq!(detect(text), None, "{text:?}");
        }
    }

    #[test]
    fn japanese_wins_when_both_scripts_present() {
        assert_eq!(detect("안녕하세요 こんにちは"), Some("ja"));
    }

    #[test]
    fn apply_rewrites_sherpa_language_fields() {
        let mut settings = CharacterSettings {
            tts_type: Some("sherpa-onnx".into()),
            voice_lang: Some("zh".into()),
            voice_models: Some(Default::default()),
            ..Default::default()
        };

        assert_eq!(apply_to_settings(&mut settings, "こんにちは"), Some("ja"));
        assert_eq!(settings.voice_lang.as_deref(), Some("ja"));
        assert_eq!(
            settings
                .voice_models
                .as_ref()
                .unwrap()
                .sherpa_onnx_language
                .as_deref(),
            Some("ja")
        );
    }

    #[test]
    fn apply_rewrites_only_voice_lang_for_other_services() {
        let mut settings = CharacterSettings {
            tts_type: Some("cosyvoice".into()),
            voice_lang: Some("zh".into()),
            ..Default::default()
        };

        assert_eq!(apply_to_settings(&mut settings, "안녕하세요"), Some("ko"));
        assert_eq!(settings.voice_lang.as_deref(), Some("ko"));
        assert!(settings.voice_models.is_none());
    }

    #[test]
    fn apply_keeps_configured_language_for_neutral_text() {
        let mut settings = CharacterSettings {
            tts_type: Some("sherpa-onnx".into()),
            voice_lang: Some("zh".into()),
            ..Default::default()
        };

        assert_eq!(apply_to_settings(&mut settings, "你好"), None);
        assert_eq!(settings.voice_lang.as_deref(), Some("zh"));
    }
}
