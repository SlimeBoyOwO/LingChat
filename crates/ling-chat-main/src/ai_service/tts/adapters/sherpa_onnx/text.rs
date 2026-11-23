//! 文本预处理：可读字符、空格、切句、情绪 → 语速/停顿。

use crate::ai_service::tts::adapters::gsv::{gsv_emo_category, known_emo_category};

/// 句末标点：在这里切句，标点本身留在小句里（模型据此收尾），后面停 [`SENTENCE_PAUSE`]。
const SENTENCE_PUNCT: &[char] = &['。', '！', '？', '.', '!', '?', '…'];
/// 从句标点：不切句，留给模型自己处理（模型的逗号停顿比外部拼接自然）；
/// 仅当句子超过 [`MAX_CLAUSE_CHARS`] 时作为兜底切分点。数字/字母中间的标点不算断句。
const CLAUSE_PUNCT: &[char] = &['，', '、', '；', '：', ',', ';', ':', '—'];

/// 小句间静音：句号后 0.30 秒、逗号后 0.20 秒。
const SENTENCE_PAUSE: f32 = 0.30;
pub(super) const CLAUSE_PAUSE: f32 = 0.20;

/// 单次推理的最大字数。超过后在从句标点处兜底切开，避免极长句子一次生成过久；
/// 正常对话句子远小于该值，逗号会原样交给模型。
const MAX_CLAUSE_CHARS: usize = 80;

/// 模型读得出的文字种类；三者独立，由目标语言与词表能力决定。
#[derive(Clone, Copy, Debug, Default)]
pub(super) struct ReadableScripts {
    latin: bool,
    kana: bool,
    hangul: bool,
}

/// 目标语言是否等于某个小写语言代码。
pub(super) fn is_language(language: &str, code: &str) -> bool {
    language.trim().eq_ignore_ascii_case(code)
}

/// 汉字里的数字/日期是否按中文读法改写；日文/韩文交给模型自己读。
pub(super) fn reads_han_as_chinese(language: &str) -> bool {
    !is_language(language, "ja") && !is_language(language, "ko")
}

/// 拉丁字母看词表，假名/谚文看目标语言。
pub(super) fn readable_scripts(language: &str, reads_latin: bool) -> ReadableScripts {
    ReadableScripts {
        latin: reads_latin,
        kana: reads_latin || is_language(language, "ja"),
        hangul: reads_latin || is_language(language, "ko"),
    }
}

/// 是不是汉字（范围与 `chinese_digits::has_han` 一致）。
fn is_han(c: char) -> bool {
    let cp = c as u32;
    (0x3400..=0x4DBF).contains(&cp)
        || (0x4E00..=0x9FFF).contains(&cp)
        || (0xF900..=0xFAFF).contains(&cp)
        || (0x2_0000..=0x3_FFFF).contains(&cp)
}

/// 是否为日文假名（含长音符「ー」，否则「コーヒー」念不全）。
fn is_kana(c: char) -> bool {
    let cp = c as u32;
    (0x3040..=0x309F).contains(&cp)
        || (0x30A0..=0x30FF).contains(&cp)
        || (0x31F0..=0x31FF).contains(&cp)
        || (0xFF65..=0xFF9F).contains(&cp)
}

/// 是不是韩文谚文（谚文音节、字母、兼容字母，以及扩展区）。
fn is_hangul(c: char) -> bool {
    let cp = c as u32;
    (0xAC00..=0xD7A3).contains(&cp)
        || (0x1100..=0x11FF).contains(&cp)
        || (0x3130..=0x318F).contains(&cp)
        || (0xA960..=0xA97F).contains(&cp)
        || (0xD7B0..=0xD7FF).contains(&cp)
}

/// 把模型读不出来的字符换成空格；从句标点保留，其余符号/外语文字剔除，
/// 避免被 espeak 兜底念出来或带跑整句。
pub(super) fn strip_unreadable(text: &str, scripts: ReadableScripts) -> String {
    let chars: Vec<char> = text.chars().collect();
    let mut out = String::with_capacity(text.len());
    for (i, &c) in chars.iter().enumerate() {
        let speakable = is_han(c)
            // 假名/谚文看模型读不读得出来；读不了会被 espeak 兜底带跑。
            || (scripts.kana && is_kana(c))
            || (scripts.hangul && is_hangul(c))
            || c.is_ascii_digit()
            || matches!(c, '０'..='９')
            // 句末标点：切句与停顿由 split_clauses 决定，标点本身留给模型收尾。
            || SENTENCE_PUNCT.contains(&c)
            // 从句标点：合并短句时放回来的那个（见 Clause），别在这里又抹掉。
            || CLAUSE_PUNCT.contains(&c)
            || (scripts.latin
                && (c.is_alphanumeric()
                    // 英文缩写里的撇号，留在词里，别把 don't 拆成两个词。
                    || (matches!(c, '\'' | '’')
                        && i > 0
                        && i + 1 < chars.len()
                        && chars[i - 1].is_ascii_alphanumeric()
                        && chars[i + 1].is_ascii_alphanumeric())));
        if speakable && !c.is_whitespace() {
            // 标点紧贴前面的词：别把符号换出来的空格留在标点前（`别走 ,好嘛`）。
            if (SENTENCE_PUNCT.contains(&c) || CLAUSE_PUNCT.contains(&c)) && out.ends_with(' ') {
                out.pop();
            }
            out.push(c);
        } else if !out.ends_with(' ') {
            // 连续符号/空白只留一个空格，免得前端切出多个空词。
            out.push(' ');
        }
    }
    out.trim().to_string()
}

/// 去掉中文/日文里无意义的空格（kokoro 会把空格段当成独立一句，多出停顿）。
/// 拉丁字母/数字/谚文两侧的空格保留。
pub(super) fn collapse_cjk_spaces(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    let mut chars = text.chars().peekable();
    while let Some(c) = chars.next() {
        if !c.is_whitespace() {
            out.push(c);
            continue;
        }
        // 连续空白算一段，按两侧的字符决定留不留。
        while chars.peek().is_some_and(|next| next.is_whitespace()) {
            chars.next();
        }
        let prev = out.chars().next_back();
        let next = chars.peek().copied();
        if needs_word_space(prev) || needs_word_space(next) {
            out.push(' ');
        }
    }
    out.trim().to_string()
}

/// 这个字符是不是「靠空格分词、因此空格不能去掉」的文字：拉丁字母/数字、谚文。
fn needs_word_space(c: Option<char>) -> bool {
    c.is_some_and(|c| c.is_ascii_alphanumeric() || is_hangul(c))
}

/// 短句不单独送模型：少于该字数先并到下一句，合并处标点原样放回。
const MIN_CLAUSE_CHARS: usize = 3;

/// 情绪 →（语速倍率，停顿倍率）。这些引擎没有情绪输入，只能调语速与停顿，
/// 倍率限制在 ±12% / ±25% 内；未识别情绪按中立。
pub(super) fn emotion_prosody(emo: &str) -> (f32, f32) {
    // 六分类名本身也是合法输入，先原样认一次。
    let category = match emo {
        "吃惊" | "开心" | "恐惧" | "难过" | "生气" | "中立" => emo,
        other => gsv_emo_category(other),
    };

    match category {
        // 情绪越高涨，语速越快、标点停顿越短。
        "开心" => (1.05, 0.90),
        "生气" => (1.08, 0.80),
        "恐惧" => (1.10, 0.80),
        "吃惊" => (1.08, 0.75),
        // 低落时放慢、句读拉长。
        "难过" => (0.90, 1.25),
        _ => (1.0, 1.0),
    }
}

/// 解析本次情绪。
///
/// 情绪词表与 GSV 六分类共用（[`known_emo_category`]）：词表内的词直接用于韵律，
/// 词表外的标签一律按中立处理，不再另起一套情绪模型，避免两套分类结果互相打架。
pub(super) fn resolve_emotion(emo: &str) -> String {
    let emo = emo.trim();
    if !emo.is_empty() && known_emo_category(emo).is_some() {
        emo.to_string()
    } else {
        String::new()
    }
}

/// 小句：(文本, 之后停顿秒数, 被切掉的从句标点)。
/// 标点单独保留，短句合并时要放回文本。
type Clause = (String, f32, Option<char>);

/// 按句末标点切句并给出每句后的停顿（最后一句为 0）。
/// 句内的逗号/顿号不切，原样交给模型，避免逐词拼接把语流切碎（听起来一顿一顿）；
/// 只有句子超过 [`MAX_CLAUSE_CHARS`] 时才在从句标点处兜底切开。
/// 数字/字母中间的标点不算断句，英文缩写另有保护。
pub(super) fn split_clauses(text: &str) -> Vec<Clause> {
    let chars: Vec<char> = text.chars().collect();
    let mut raw: Vec<Clause> = Vec::new();
    let mut cur = String::new();

    for (i, &c) in chars.iter().enumerate() {
        let inside_word = i > 0
            && chars[i - 1].is_ascii_alphanumeric()
            && chars.get(i + 1).is_some_and(|n| n.is_ascii_alphanumeric());

        if SENTENCE_PUNCT.contains(&c) {
            cur.push(c);
            // Mr./Dr./e.g. 里的点不是句末；后接小写词的也不是。
            let continuation = c == '.'
                && (ends_with_english_abbreviation(&chars, i) || next_word_is_lowercase(&chars, i));
            if !inside_word && !continuation {
                raw.push((std::mem::take(&mut cur), SENTENCE_PAUSE, None));
            }
        } else if CLAUSE_PUNCT.contains(&c) && !inside_word {
            if speakable_len(&cur) >= MAX_CLAUSE_CHARS {
                raw.push((std::mem::take(&mut cur), CLAUSE_PAUSE, Some(c)));
            } else {
                cur.push(c);
            }
        } else {
            cur.push(c);
        }
    }
    if !cur.trim().is_empty() {
        raw.push((cur, 0.0, None));
    }

    // 丢掉只剩标点的空片段；整段末尾不留停顿。
    raw.retain(|(clause, _, _)| clause.chars().any(char::is_alphanumeric));
    if let Some(last) = raw.last_mut() {
        last.1 = 0.0;
    }

    // 统一 trim，合并时按语言补回空格。
    for (clause, _, _) in raw.iter_mut() {
        *clause = clause.trim().to_string();
    }

    merge_short_clauses(raw)
}

/// 点号前的常见英文缩写：这些点不是句末，别在那里切句。
const ENGLISH_ABBREVIATIONS: &[&str] = &[
    "mr", "mrs", "ms", "dr", "prof", "st", "jr", "sr", "vs", "fig", "no", "e.g", "i.e", "a.m",
    "p.m",
];

/// 句点前是不是英文缩写（只往前看字母和点）。
fn ends_with_english_abbreviation(chars: &[char], dot: usize) -> bool {
    let mut word: Vec<char> = Vec::new();
    let mut i = dot;
    while i > 0 {
        i -= 1;
        let c = chars[i];
        if c.is_ascii_alphabetic() {
            word.push(c.to_ascii_lowercase());
        } else if c == '.' && !word.is_empty() {
            word.push('.');
        } else {
            break;
        }
    }
    word.reverse();
    let word: String = word.iter().collect();
    ENGLISH_ABBREVIATIONS.contains(&word.as_str())
}

/// 句点后（跳过空格）首字符是否为小写字母；是则多半不是句末。
fn next_word_is_lowercase(chars: &[char], dot: usize) -> bool {
    chars[dot + 1..]
        .iter()
        .find(|c| !c.is_whitespace())
        .is_some_and(|c| c.is_ascii_lowercase())
}

/// 文本里是否有内部从句标点（标点后还有字）。
/// 有则模型会自己塞较长静音，调用处据此压回 [`CLAUSE_PAUSE`]。
pub(super) fn has_inner_clause_punct(text: &str) -> bool {
    let mut seen_punct = false;
    for c in text.chars() {
        if CLAUSE_PUNCT.contains(&c) {
            seen_punct = true;
        } else if seen_punct && c.is_alphanumeric() {
            return true;
        }
    }
    false
}

/// 可念的字数（汉字/字母/数字），不算标点与空白。
fn speakable_len(text: &str) -> usize {
    text.chars().filter(|c| c.is_alphanumeric()).count()
}

/// 把短于 [`MIN_CLAUSE_CHARS`] 的小句并到下一句（末尾并回前一句），合并处标点放回。
fn merge_short_clauses(raw: Vec<Clause>) -> Vec<Clause> {
    let mut out: Vec<Clause> = Vec::new();
    let mut pending: Option<Clause> = None;

    for (clause, pause, tail_punct) in raw {
        let merged = match pending.take() {
            Some((prev, _, prev_punct)) => {
                (join_clauses(&prev, prev_punct, &clause), pause, tail_punct)
            },
            None => (clause, pause, tail_punct),
        };
        if speakable_len(&merged.0) < MIN_CLAUSE_CHARS {
            pending = Some(merged);
        } else {
            out.push(merged);
        }
    }

    if let Some((tail, _, _)) = pending {
        match out.last_mut() {
            Some(last) => {
                last.0 = join_clauses(&last.0, last.2, &tail);
                last.2 = None;
                last.1 = 0.0; // 并完就是整段最后一句了，后面不留停顿
            },
            None => out.push((tail, 0.0, None)),
        }
    }
    out
}

/// 合并两个小句：放回被切掉的标点，拉丁文字后按需补空格。
fn join_clauses(prev: &str, prev_punct: Option<char>, next: &str) -> String {
    let mut out = String::with_capacity(prev.len() + next.len() + 4);
    out.push_str(prev);
    if let Some(punct) = prev_punct {
        out.push(punct);
    }
    if next.starts_with(|c: char| c.is_ascii_alphanumeric()) {
        out.push(' ');
    }
    out.push_str(next);
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    /// 符号换成空格：保留标点，剔除模型读不了的文字。
    #[test]
    fn strips_unreadable_chars_but_keeps_sentence_ends() {
        // 认得拉丁字母的模型（kokoro 多语言）：英文原样留着，标点都留着。
        let latin = ReadableScripts {
            latin: true,
            ..Default::default()
        };
        assert_eq!(
            strip_unreadable("他说，你好、今天：很好；真的？", latin),
            "他说，你好、今天：很好；真的？"
        );
        // 句末标点原样保留（切句与停顿由 split_clauses 负责）。
        assert_eq!(
            strip_unreadable("你好。真的吗！好呀？", latin),
            "你好。真的吗！好呀？"
        );
        // 波浪号、音乐符号、箭头、装饰符号一律换成空格。
        assert_eq!(strip_unreadable("好~呀", latin), "好 呀");
        assert_eq!(strip_unreadable("好～♪♡→呀", latin), "好 呀");
        // 英文缩写里的撇号要留着，别把 don't 拆开。
        assert_eq!(strip_unreadable("I don't ~know~", latin), "I don't know");
        // 连续符号只留一个空格；符号换出来的空格不留在标点前面。
        assert_eq!(
            strip_unreadable("他说,\"别走\",好嘛.", latin),
            "他说, 别走,好嘛."
        );

        // 中文模型（matcha / zipvoice）读不了英文、罗马音、假名：连着符号一起剔掉，
        // 否则要么被丢词，要么被 espeak 按字母念、把整句带跑。
        let no_latin = ReadableScripts::default();
        assert_eq!(strip_unreadable("今天真开心 hello", no_latin), "今天真开心");
        assert_eq!(strip_unreadable("hello 今天真开心", no_latin), "今天真开心");
        assert_eq!(
            strip_unreadable("おはよう 今日はいい天気", no_latin),
            "今日 天気"
        );
        assert_eq!(
            strip_unreadable("AI 帮你查了 wiki，好呀~", no_latin),
            "帮你查了，好呀"
        );
        // 数字和标点必须留着：数字交给 FST / 数字改写，标点决定切句与停顿
        // （从句标点是短句合并时放回来的，绝不能在这里又抹掉）。
        assert_eq!(strip_unreadable("第 3 章。", no_latin), "第 3 章。");
        assert_eq!(strip_unreadable("嗯，好的呀", no_latin), "嗯，好的呀");
    }

    /// 日语/韩语模型的词表里没有英文，但假名/谚文必须留着。
    #[test]
    fn keeps_kana_and_hangul_for_their_own_models() {
        let japanese = ReadableScripts {
            kana: true,
            ..Default::default()
        };
        // 长音符「ー」、片假名中点「・」也得留着（「コーヒー」缺了「ー」就念不全）。
        assert_eq!(
            strip_unreadable("おはよう、今日はいい天気ですね。コーヒー・ミルク", japanese),
            "おはよう、今日はいい天気ですね。コーヒー・ミルク"
        );
        // 日文模型同样读不了英文和谚文。
        assert_eq!(
            strip_unreadable("hello こんにちは 안녕하세요", japanese),
            "こんにちは"
        );

        let korean = ReadableScripts {
            hangul: true,
            ..Default::default()
        };
        assert_eq!(
            strip_unreadable("안녕하세요, 오늘 날씨가 좋네요.", korean),
            "안녕하세요, 오늘 날씨가 좋네요."
        );
        assert_eq!(
            strip_unreadable("hello 안녕하세요 こんにちは", korean),
            "안녕하세요"
        );

        // 目标语言是中文时两者都剔掉（中文模型读不了，交给 espeak 会把整句带跑）。
        let chinese = ReadableScripts::default();
        assert_eq!(
            strip_unreadable("好呀 こんにちは 안녕하세요", chinese),
            "好呀"
        );
    }

    /// 目标语言决定假名/谚文留不留。
    #[test]
    fn decides_readable_scripts_by_language() {
        // 中文：只留汉字/数字/标点（模型读不了英文、假名、谚文）。
        let zh = readable_scripts("zh", false);
        assert!(!zh.latin && !zh.kana && !zh.hangul);
        // 日文/韩文：自己的文字留着，英文和另一种文字仍剔掉。
        let ja = readable_scripts("ja", false);
        assert!(ja.kana && !ja.latin && !ja.hangul);
        let ko = readable_scripts("ko", false);
        assert!(ko.hangul && !ko.latin && !ko.kana);
        // 带英文词表的多语言模型（kokoro）：三种都认得，语言字段不再限制。
        let multi = readable_scripts("zh", true);
        assert!(multi.latin && multi.kana && multi.hangul);
        // 大小写/前后空格不影响判断。
        assert!(readable_scripts(" JA ", false).kana);

        // 汉字数字/日期：中文（含未设置语言）按中文读，日文/韩文交给模型自己读。
        assert!(reads_han_as_chinese("zh"));
        assert!(reads_han_as_chinese(""));
        assert!(reads_han_as_chinese("en"));
        assert!(!reads_han_as_chinese("ja"));
        assert!(!reads_han_as_chinese(" ko "));
    }

    /// 中文/日文里的空格去掉；英文/韩文靠空格分词，必须留着。
    #[test]
    fn drops_spaces_that_cjk_does_not_need() {
        // 汉字之间、CJK 标点之后、换行/连续空白：全部去掉。
        assert_eq!(collapse_cjk_spaces("今天 天气 真不错"), "今天天气真不错");
        assert_eq!(collapse_cjk_spaces("好 呀， 今天"), "好呀，今天");
        assert_eq!(
            collapse_cjk_spaces("你好。 今天\n天气\t不错"),
            "你好。今天天气不错"
        );
        assert_eq!(collapse_cjk_spaces("  今天 天气  "), "今天天气");
        // 日文同样不靠空格分词（长音符「ー」也不会被当成空白）。
        assert_eq!(collapse_cjk_spaces("こんにちは 世界"), "こんにちは世界");
        assert_eq!(
            collapse_cjk_spaces("コーヒー を ください"),
            "コーヒーをください"
        );
        // 英文/韩文的词间空格留着（韩文按空格分词，去掉就是病句）。
        assert_eq!(
            collapse_cjk_spaces("hello  brave  world"),
            "hello brave world"
        );
        assert_eq!(
            collapse_cjk_spaces("안녕하세요 오늘 날씨가 좋네요"),
            "안녕하세요 오늘 날씨가 좋네요"
        );
        // 汉字与英文/数字之间也要留一格：不然「今天hello」「第3章」会粘成一个词。
        assert_eq!(collapse_cjk_spaces("今天 hello 天气"), "今天 hello 天气");
        assert_eq!(collapse_cjk_spaces("第 3 章"), "第 3 章");
        // 英文标点后跟英文单词：空格留着（`hello,world` 会被读成一个词）。
        assert_eq!(collapse_cjk_spaces("hello, world"), "hello, world");
        // 只有标点的串不去动它。
        assert_eq!(collapse_cjk_spaces(""), "");
        assert_eq!(collapse_cjk_spaces("  "), "");
    }

    /// 切句、停顿、短句合并与数字内标点保护。
    #[test]
    fn splits_clauses_with_pauses() {
        // 只在句号/问号/感叹号处切；句内逗号原样留给模型，避免逐小句拼接把语流切碎。
        assert_eq!(
            split_clauses("他说今天很好，你好呀我的朋友。今天天气真好，我们一起出去玩吧！"),
            vec![
                (
                    "他说今天很好，你好呀我的朋友。".to_string(),
                    SENTENCE_PAUSE,
                    None
                ),
                ("今天天气真好，我们一起出去玩吧！".to_string(), 0.0, None),
            ]
        );
        // 两字的小句没有上下文，并到下一句；合并处的标点原样放回去，停顿才不会丢。
        assert_eq!(
            split_clauses("对了，你吃饭了吗？"),
            vec![("对了，你吃饭了吗？".to_string(), 0.0, None)]
        );
        // 末尾剩一句短的：并回前一句。
        assert_eq!(
            split_clauses("今天天气真不错。好吧。"),
            vec![("今天天气真不错。好吧。".to_string(), 0.0, None)]
        );
        // 带逗号的整句只合成一次，逗号停顿交给模型（原来是逗号处切一刀、外部补静音）。
        assert_eq!(
            split_clauses("嗯，好的呀，那我们走吧。"),
            vec![("嗯，好的呀，那我们走吧。".to_string(), 0.0, None)]
        );
        assert_eq!(
            split_clauses("我觉得，还是算了吧。"),
            vec![("我觉得，还是算了吧。".to_string(), 0.0, None)]
        );
        // 没有标点的整句只有一片。
        assert_eq!(
            split_clauses("你好呀"),
            vec![("你好呀".to_string(), 0.0, None)]
        );
        // 连写标点切出来的空片段（只剩「！」）要丢掉。
        assert_eq!(
            split_clauses("真的？！"),
            vec![("真的？".to_string(), 0.0, None)]
        );

        // 数字/字母中间的标点不算断句：时刻、小数、千分位、域名都要整块念
        // （以前 `9:30` 会在冒号处切一刀，停顿插进时间中间，读成「上午九，三十开会」）。
        assert_eq!(
            split_clauses("上午 9:30 开会，别迟到了。"),
            vec![("上午 9:30 开会，别迟到了。".to_string(), 0.0, None)]
        );
        assert_eq!(
            split_clauses("圆周率是 3.14。"),
            vec![("圆周率是 3.14。".to_string(), 0.0, None)]
        );
        assert_eq!(
            split_clauses("一共 1,000 块，别忘了。"),
            vec![("一共 1,000 块，别忘了。".to_string(), 0.0, None)]
        );
        assert_eq!(
            split_clauses("我打开了 www.bing.com。"),
            vec![("我打开了 www.bing.com。".to_string(), 0.0, None)]
        );
    }

    /// 超长句子才在逗号处兜底切开，避免单次推理过久；切点后的标点由合并逻辑放回。
    #[test]
    fn splits_overlong_sentence_at_clause_punct() {
        let long = format!("{}，{}。", "很".repeat(MAX_CLAUSE_CHARS), "好".repeat(10));
        let clauses = split_clauses(&long);
        assert_eq!(clauses.len(), 2);
        assert_eq!(clauses[0].1, CLAUSE_PAUSE);
        assert_eq!(clauses[0].2, Some('，'));
        assert_eq!(clauses[1].1, 0.0);
        assert!(clauses[1].0.ends_with('。'));

        // 没到阈值时不在逗号处切。
        let short = format!(
            "{}，{}。",
            "很".repeat(MAX_CLAUSE_CHARS - 1),
            "好".repeat(10)
        );
        assert_eq!(split_clauses(&short).len(), 1);
    }

    /// 英文/日文/韩文走同一套切句；英文缩写里的点不算句末。
    #[test]
    fn splits_other_languages_by_punctuation() {
        // 英文：只在句末切，句间停 0.3 秒；切出来的小句不带前导空格。
        assert_eq!(
            split_clauses("Hello there. How are you today? I am fine, thank you."),
            vec![
                ("Hello there.".to_string(), SENTENCE_PAUSE, None),
                ("How are you today?".to_string(), SENTENCE_PAUSE, None),
                ("I am fine, thank you.".to_string(), 0.0, None),
            ]
        );
        // 缩写不切：「Mr. Smith」「Dr. Wang」「e.g.」都在一句里。
        assert_eq!(
            split_clauses("Mr. Smith met Dr. Wang at 9 a.m. today."),
            vec![(
                "Mr. Smith met Dr. Wang at 9 a.m. today.".to_string(),
                0.0,
                None
            )]
        );
        assert_eq!(
            split_clauses("It looks like e.g. a cat. Then it runs."),
            vec![
                (
                    "It looks like e.g. a cat.".to_string(),
                    SENTENCE_PAUSE,
                    None
                ),
                ("Then it runs.".to_string(), 0.0, None),
            ]
        );
        // 小数、千分位、域名、`U.S.` 这种带点的词都不在中间切。
        assert_eq!(
            split_clauses("It costs 1,234.56 dollars. I live in the U.S. now."),
            vec![
                (
                    "It costs 1,234.56 dollars.".to_string(),
                    SENTENCE_PAUSE,
                    None
                ),
                ("I live in the U.S. now.".to_string(), 0.0, None),
            ]
        );
        // 太短的小句并到下一句，合并处补一个空格（`No,I` 挤在一起会被读成一个词）。
        assert_eq!(
            split_clauses("No, I don't."),
            vec![("No, I don't.".to_string(), 0.0, None)]
        );

        // 日文：只按句号/问号切，顿号留在句内交给模型。
        assert_eq!(
            split_clauses("こんにちは、今日はいい天気ですね。散歩でもしましょうか？"),
            vec![
                (
                    "こんにちは、今日はいい天気ですね。".to_string(),
                    SENTENCE_PAUSE,
                    None
                ),
                ("散歩でもしましょうか？".to_string(), 0.0, None),
            ]
        );

        // 韩文：句末是 ASCII 的句号/问号，逗号留在句内。
        assert_eq!(
            split_clauses("안녕하세요, 오늘 날씨가 좋네요. 같이 산책할까요?"),
            vec![
                (
                    "안녕하세요, 오늘 날씨가 좋네요.".to_string(),
                    SENTENCE_PAUSE,
                    None
                ),
                ("같이 산책할까요?".to_string(), 0.0, None),
            ]
        );
    }

    /// 情绪只落在语速/停顿上，分类走 GSV 六分类。
    #[test]
    fn maps_emotion_to_prosody() {
        // 中立与未识别（含空标签）不改变任何参数。
        for emo in ["", "中立", "平静", "正常", "疑惑"] {
            assert_eq!(emotion_prosody(emo), (1.0, 1.0), "{emo}");
        }
        // 高涨：语速更快、标点停顿更短。
        for emo in ["开心", "生气", "恐惧", "吃惊"] {
            let (speed, pause) = emotion_prosody(emo);
            assert!(speed > 1.0 && pause < 1.0, "{emo}: {speed}/{pause}");
        }
        // 低落：语速更慢、标点停顿更长。
        let (speed, pause) = emotion_prosody("难过");
        assert!(speed < 1.0 && pause > 1.0, "难过: {speed}/{pause}");
        // 同义词归到同一类，避免提示词/分类器的不同叫法失效。
        assert_eq!(emotion_prosody("高兴"), emotion_prosody("开心"));
        assert_eq!(emotion_prosody("害怕"), emotion_prosody("恐惧"));
        assert_eq!(emotion_prosody("惊讶"), emotion_prosody("吃惊"));
        assert_eq!(emotion_prosody("哭泣"), emotion_prosody("难过"));
    }

    /// 情绪词统一走 GSV 词典；词典外（含英文与空标签）按中立处理。
    #[test]
    fn resolves_emotion_through_gsv_lexicon() {
        // 六分类名、情绪模型标签与立绘别名都算已知情绪。
        assert_eq!(resolve_emotion("开心"), "开心");
        assert_eq!(resolve_emotion(" 平静 "), "平静");
        assert_eq!(resolve_emotion("伤心"), "伤心");
        // 未知标签清空 → emotion_prosody 回中立，歌词式文本也不会 panic。
        assert_eq!(resolve_emotion(""), "");
        assert_eq!(resolve_emotion("neutral"), "");
        assert_eq!(emotion_prosody(&resolve_emotion("")), (1.0, 1.0));
        // 词典内的词一定映射到六分类。
        for label in ["开心", "平静", "伤心", "生气", "害怕"] {
            let normalized = resolve_emotion(label);
            assert_eq!(normalized, label);
            assert!(
                known_emo_category(&normalized).is_some(),
                "{label} 应能映射到六分类"
            );
        }
    }
}
