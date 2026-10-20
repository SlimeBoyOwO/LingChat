//! 文本预处理：可读字符、无意义空格、按标点切句、情绪 → 语速/停顿。
//!
//! 这一层完全不碰 sherpa：输入是模型要念的台词，输出是一条条可以直接送进
//! `generate_with_config` 的小句（以及每句之后该停多久）。

use crate::ai_service::tts::adapters::gsv::gsv_emo_category;

/// 句末标点：在这里切句，标点本身留在小句里（模型据此收尾），后面停 [`SENTENCE_PAUSE`]。
const SENTENCE_PUNCT: &[char] = &['。', '！', '？', '.', '!', '?', '…'];
/// 从句标点（逗号、顿号、分号、冒号）：也切句，后面停 [`CLAUSE_PAUSE`]。
///
/// 只想「按句号切」的话把这里清空即可：逗号会留在文本里交给模型自己处理，整句只做
/// 一次推理、语气更连贯，但逗号处的停顿长度就不可控了。
///
/// 数字/字母中间的这些标点不算断句（`9:30`、`3.14`、`1,000`），见 [`split_clauses`]。
const CLAUSE_PUNCT: &[char] = &['，', '、', '；', '：', ',', ';', ':', '—'];

/// 小句之间插多长静音（秒）。取值沿用先前实测的自然句读：句号后 0.4 秒左右、
/// 逗号后 0.2 秒左右（见 SILENCE_SCALE 的注释）。
const SENTENCE_PAUSE: f32 = 0.40;
const CLAUSE_PAUSE: f32 = 0.20;

/// 模型读得出的文字种类，决定 [`strip_unreadable`] 里哪些字符能原样留下。
///
/// 以前只区分「认不认拉丁字母」，假名和谚文跟着拉丁字母走：日语/韩语模型的词表里
/// 没有英文，于是被判定为不认拉丁字母，假名/谚文跟着一起被剔掉——日语模型只剩
/// 汉字、韩语模型整句变空。这三个开关互相独立，由目标语言与词表能力共同决定
/// （见 `SherpaOnnxAdapter::new`）。
#[derive(Clone, Copy, Debug, Default)]
pub(super) struct ReadableScripts {
    /// 拉丁字母（kokoro 多语言版带 `lexicon-us-en.txt`；中文 matcha / zipvoice 没有）。
    latin: bool,
    /// 日文假名（目标语言是日文时留着；中文模型会把假名一起剔掉）。
    kana: bool,
    /// 韩文谚文（目标语言是韩文时留着）。
    hangul: bool,
}

/// 目标语言是不是某个小写语言代码（设置里的取值是 `zh` / `en` / `ja` / `ko`，
/// 见 `fields.ts` 的 `sherpa_onnx_lang` 与 [`crate::ai_service::tts::text_language`]）。
fn is_language(language: &str, code: &str) -> bool {
    language.trim().eq_ignore_ascii_case(code)
}

/// 汉字里的数字/日期该按中文的读法改写吗（见 [`crate::ai_service::tts::chinese_digits`]）。
///
/// 日文/韩文的汉字读法跟中文不同：「2026年」在日文里念「にせんにじゅうろくねん」，
/// 套用中文改写会念成「二零二六年」；「9:30」是「9時30分」而不是「九点三十」。所以
/// 这两门语言把数字原样交给模型自己的前端，其余（中文、未设置、英文模型念中文文本）
/// 仍按中文改写。
pub(super) fn reads_han_as_chinese(language: &str) -> bool {
    !is_language(language, "ja") && !is_language(language, "ko")
}

/// 模型读得出哪几种文字：拉丁字母看词表（见 `model_reads_latin`），假名/谚文看
/// 目标语言——日文/韩文模型的词表里通常没有英文，但它们读自己的文字不需要英文词表。
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

/// 是不是日文假名（平假名、片假名、片假名音标扩展、半角片假名）。
///
/// 比 [`crate::ai_service::tts::text_language::detect`] 的范围宽：那里要排除「・」「ー」
/// 免得把中文译名当日语，这里只是判断「这个字日语模型读不读得出来」——长音符「ー」
/// 是日语拼写的一部分（「コーヒー」），漏掉它就念不完整。
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

/// 送给 sherpa 之前，把「读不出来、还会被硬念出来或带跑整句」的字符换成空格。
///
/// 三件事：
///
/// 1. 从句标点（`，、；：,;:—`）要留着：短句合并后一片里有两个小句（见 [`Clause`]），
///    标点在 sherpa 前端有对应的 token（词表没收录的会退化成 pad），模型据此收语气、
///    给停顿；换成空格它只是个读不出来的空词（日志里 `Ignore OOV ' '`），停顿全靠模型
///    临场发挥。标点把这一片切成多份 token 序列也没关系——[`MAX_NUM_SENTENCES`] 之内
///    仍在一次推理里。
/// 2. 其余符号（`~`、`～`、`♪`、`♡`、`→`、emoji、孤立括号、引号……）既不在词表里、
///    也不是汉字，`MatchaTtsLexicon` 会走 **espeak（en-us）兜底**当英文念出来：ASCII
///    `~` 实测被念成 "tilde"（SenseVoice 反读「好~」得到「好挑的」）。
/// 3. 模型读不了的文字——英文、罗马音、假名、谚文——会被同一套 espeak 兜底音素化：
///    中文 matcha 的词表里一个英文词都没有，`hello` 这类要么被丢词、要么按字母念；
///    零样本的 zipvoice 更糟，夹一句英文会把整句带跑（SenseVoice 反读里冒出参考音频
///    的内容，听起来就是「换了个人、还念了参考音频里的话」）。所以模型读不出来哪种
///    文字（见 [`ReadableScripts`]）就整段剔掉——宁可少念一句英文，也不要整句走样。
pub(super) fn strip_unreadable(text: &str, scripts: ReadableScripts) -> String {
    let chars: Vec<char> = text.chars().collect();
    let mut out = String::with_capacity(text.len());
    for (i, &c) in chars.iter().enumerate() {
        let speakable = is_han(c)
            // 假名/谚文看模型读不读得出来：中文模型读不了「こんにちは」，
            // 交给 espeak 兜底会把整句带跑；日语/韩语模型则必须留着。
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

/// 去掉中文/日文里没有意义的空格。
///
/// kokoro 的多语言前端（`kokoro-multi-lang-lexicon.cc` 的 `ConvertTextToTokenIds`）按
/// 「汉字 / 非汉字」把文本切成若干段，非汉字段一律交给 espeak 音素化，而且**每段都
/// 当成独立的一句**（kokoro 的 batch 恒为 1，逐句 `Process` 后硬拼）。于是汉字中间夹
/// 一个空格就多出一个「空格段」→ 多出一整句的停顿：实测「今天 天气真不错」比
/// 「今天天气真不错」多 0.71 秒静音；「好~呀」经 [`strip_unreadable`] 变成「好 呀」、
/// 「好呀，😊今天」变成「好呀， 今天」时也一样（0.72 / 0.76 秒，每句都这样）。
///
/// 中文/日文本来不靠空格分词，所以只保留真正有用的空格：两侧任意一边是拉丁字母/
/// 数字（英文单词）或谚文（韩文按空格分词）时留一个，其余（汉字之间、CJK 标点之后、
/// 符号换出来、换行/制表符换出来的空格）全部去掉。
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

/// 小句太短时不要单独送进模型：实测两字的「对了」单独合成，SenseVoice 反读成
/// 「饿了」（同一句里连着念就是对的）——短句缺少上下文，模型容易读错、读得生硬。
/// 短于这个字数的先并到下一句里（末尾实在只剩一句短的，就并回前一句）。
///
/// 合并处那个标点会原样放回文本（见 [`Clause`] / [`merge_short_clauses`]），所以
/// 停顿不会被吃掉；实测同样一句「喂？你在听吗？」，并成「喂？你在听吗？」（标点留着）
/// 比并成「喂 你在听吗？」（空格）读得准——空格那版 SenseVoice 把「喂」听成「问」。
const MIN_CLAUSE_CHARS: usize = 3;

/// 情绪 →（语速倍率，停顿倍率）。
///
/// sherpa-onnx 的这几个引擎里只有 `vits` 能接 `emotion_id`，而且还要模型导出时带
/// `emotion_id` 输入 + `num_emotions` 元数据；matcha / kokoro / zipvoice 完全没有
/// 情绪入口。所以在这些模型上，情绪只能落成**不动音色**的两个参数：语速和标点
/// 停顿。倍率都收在 ±12% / ±25% 内，避免听成「变速播放」。
///
/// 分类沿用 GSV 的六分类（[`gsv_emo_category`]，与提示词里的情绪清单一致）：
/// 吃惊 / 开心 / 恐惧 / 难过 / 生气 / 中立；未识别的情绪一律按中立。
pub(super) fn emotion_prosody(emo: &str) -> (f32, f32) {
    // 六分类名本身（「开心」「恐惧」…）也是合法输入：提示词给的是「高兴 / 害怕 /
    // 惊讶」这类同义词，但分类器与立绘槽位会直接给分类名，所以先原样认一次。
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

/// 切出来的小句：文本、之后停多久（秒）、以及**被切掉的那个从句标点**（没有就是
/// `None`）。
///
/// 标点要单独留一份，是因为短句合并时得把它放回文本里（见 [`merge_short_clauses`]）：
/// 直接从文本里丢掉，合并处就只剩一个空格——那在 sherpa 前端是个读不出来的空词，
/// 模型既读不到标点、也拿不到标点该有的语气。句末标点（`。！？`）本来就写在小句里，
/// 所以这里只记从句标点。
type Clause = (String, f32, Option<char>);

/// 按标点把文本切成小句，并给出每句之后该停多久（秒；最后一句是 0）。
///
/// 为什么要自己切：sherpa 自己也会按标点把文本切成若干份 token 序列送进模型，但
/// **拼接处是硬接**——`offline-tts-matcha-impl.h`（v1.13.5）里多句/多批的结果就是
/// `ans.samples.insert(ans.samples.end(), ...)`，既没有淡入淡出也没有额外静音；两截
/// 波形在接缝处幅值不连续，听感上就是「咔哒」一声（零样本模型接缝两侧音色也会微微
/// 跳）。自己切之后每处停顿长度可控，接缝两侧再各做一次淡入淡出（见
/// [`apply_edge_fades`]）。
///
/// 数字/字母**中间**的标点不算断句：`9:30`、`3.14`、`1,000`、`www.bing.com` 都要整块
/// 念（模型自带的 rule FST 也认这些写法，见 [`crate::ai_service::tts::chinese_digits`]）。
/// 判定只看前后是不是紧挨着的 ASCII 字母数字，所以全角的 `1，000` 一样不会在那里切。
///
/// 所有语言都走这里（英文/日文/韩文一样），所以英文的缩写要单独保护，见
/// [`ENGLISH_ABBREVIATIONS`]。
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
            // `Mr.` / `Dr.` / `e.g.` 里的点不是句末；`U.S. now`、`Acme Inc. the` 这种
            // 后面接小写词的也不是。见 [`ends_with_english_abbreviation`] /
            // [`next_word_is_lowercase`]。
            let continuation = c == '.'
                && (ends_with_english_abbreviation(&chars, i) || next_word_is_lowercase(&chars, i));
            if !inside_word && !continuation {
                raw.push((std::mem::take(&mut cur), SENTENCE_PAUSE, None));
            }
        } else if CLAUSE_PUNCT.contains(&c) {
            if inside_word {
                cur.push(c);
            } else {
                raw.push((std::mem::take(&mut cur), CLAUSE_PAUSE, Some(c)));
            }
        } else {
            cur.push(c);
        }
    }
    if !cur.trim().is_empty() {
        raw.push((cur, 0.0, None));
    }

    // 连写的标点（「真的？！」）会切出空片段，只剩标点的片段（「！」「……」）也一起
    // 去掉；整段末尾不留停顿。
    raw.retain(|(clause, _, _)| clause.chars().any(char::is_alphanumeric));
    if let Some(last) = raw.last_mut() {
        last.1 = 0.0;
    }

    // 英文标点后面通常跟一个空格，切出来的小句会带前导空格：统一 trim，合并时按
    // 语言补回空格（见 [`join_clauses`]），也让模型少一个空词。
    for (clause, _, _) in raw.iter_mut() {
        *clause = clause.trim().to_string();
    }

    merge_short_clauses(raw)
}

/// `.` 前的常见英文缩写：这里的点不是句末，别在那儿切句。
///
/// 以前英文整段不切，认不认缩写都无所谓；现在英文也按标点切，不保护的话
/// 「Mr. Smith」「Dr. Wang」会在中间多出 0.4 秒停顿、前后还各起一段语气，`e.g.` /
/// `i.e.` 也会被切成两半。只收「后面一定还有下文」的称呼/编号类；`etc.` / `Inc.`
/// 这类通常就是句末的照常切——宁可多一个停顿，也别把两句黏成一句。
const ENGLISH_ABBREVIATIONS: &[&str] = &[
    "mr", "mrs", "ms", "dr", "prof", "st", "jr", "sr", "vs", "fig", "no", "e.g", "i.e", "a.m",
    "p.m",
];

/// `chars[dot]` 这个句点前面是不是 [`ENGLISH_ABBREVIATIONS`] 里的缩写。
///
/// 只往前看字母和点（`e.g` / `a.m` 这种缩写内部还有一个点），遇到空格/汉字/数字就停。
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

/// `chars[dot]` 后面（跳过空格）第一个字符是不是小写拉丁字母。
///
/// 英文里句号后面跟小写词基本不会是句末：`I live in the U.S. now`、`Acme Inc. the
/// company`。真断句后面跟的是大写开头或数字，所以这里不切——否则 `U.S. now` 会被
/// 切成两句、中间多出 0.4 秒停顿。
fn next_word_is_lowercase(chars: &[char], dot: usize) -> bool {
    chars[dot + 1..]
        .iter()
        .find(|c| !c.is_whitespace())
        .is_some_and(|c| c.is_ascii_lowercase())
}

/// 可念的字数（汉字/字母/数字），不算标点与空白。
fn speakable_len(text: &str) -> usize {
    text.chars().filter(|c| c.is_alphanumeric()).count()
}

/// 把短于 [`MIN_CLAUSE_CHARS`] 的小句并到下一句（末尾的短句并回前一句）。
///
/// 合并处原来那个标点要放回去（见 [`Clause`]）：留着它 sherpa 就把这一片切成两份
/// token 序列——只要不撞上 [`MAX_NUM_SENTENCES`] 就还是一次推理——模型既读得到标点
/// 带来的语气和停顿，短句也从后半句拿到了上下文。
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

/// 合并两个小句：把前一句被切掉的标点放回去，而不是塞一个空格。
///
/// 拉丁文字标点后面本来要空一格：`No, I don't.` 合并成 `No, I don't.` 而不是
/// `No,I don't.`（挤在一起连 espeak 都容易把 `No,I` 读成一个词）。中文/日文/韩文
/// 的标点后面不空格，所以只在后一句以 ASCII 字母数字开头时补一个空格。
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

    /// 符号换成空格：句末标点保留，从句标点（逗号/顿号/分号/冒号）也保留（短句合并
    /// 之后要靠它给停顿），`~`/`♪`/emoji 不再被 espeak 兜底念成英文（ASCII `~` 会被
    /// 念成 "tilde"）；模型读不了的英文/假名/谚文整段剔掉。
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

    /// 日语/韩语模型的词表里没有英文，但假名/谚文必须留着：以前它们跟着拉丁字母一起被
    /// 剔掉，日语模型只剩汉字、韩语模型整句变空（「生成的音频为空」）。
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

    /// 目标语言决定假名/谚文留不留、汉字数字按谁读：日文/韩文模型没有英文词表也得保留
    /// 自己的文字，中文模型则把它们剔掉。
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

    /// 中文/日文里的空格没有意义，去掉后 kokoro 才不会有「空格段」的假停顿；
    /// 英文/韩文靠空格分词，必须留着。这是 kokoro 中文语音里「莫名多停一拍」的根因。
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

    /// 按标点切小句时，句末停顿要长于逗号停顿，末尾不留停顿，空片段丢掉；太短的小句
    /// 要并到下一句（「对了」单独念会被读成「饿了」）、且合并处的标点要放回去；数字/
    /// 字母中间的标点不算断句（`9:30`、`3.14`、`1,000`）。
    #[test]
    fn splits_clauses_with_pauses() {
        assert_eq!(
            split_clauses("他说今天很好，你好呀我的朋友。今天天气真好，我们一起出去玩吧！"),
            vec![
                ("他说今天很好".to_string(), CLAUSE_PAUSE, Some('，')),
                ("你好呀我的朋友。".to_string(), SENTENCE_PAUSE, None),
                ("今天天气真好".to_string(), CLAUSE_PAUSE, Some('，')),
                ("我们一起出去玩吧！".to_string(), 0.0, None),
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
        // 3 字以上就单独成句、保留标点停顿（阈值取 6 会把这些并成一片，标点处全没停顿）。
        assert_eq!(
            split_clauses("嗯，好的呀，那我们走吧。"),
            vec![
                ("嗯，好的呀".to_string(), CLAUSE_PAUSE, Some('，')),
                ("那我们走吧。".to_string(), 0.0, None),
            ]
        );
        assert_eq!(
            split_clauses("我觉得，还是算了吧。"),
            vec![
                ("我觉得".to_string(), CLAUSE_PAUSE, Some('，')),
                ("还是算了吧。".to_string(), 0.0, None),
            ]
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
            vec![
                ("上午 9:30 开会".to_string(), CLAUSE_PAUSE, Some('，')),
                ("别迟到了。".to_string(), 0.0, None),
            ]
        );
        assert_eq!(
            split_clauses("圆周率是 3.14。"),
            vec![("圆周率是 3.14。".to_string(), 0.0, None)]
        );
        assert_eq!(
            split_clauses("一共 1,000 块，别忘了。"),
            vec![
                ("一共 1,000 块".to_string(), CLAUSE_PAUSE, Some('，')),
                ("别忘了。".to_string(), 0.0, None),
            ]
        );
        // 末尾的短句并回前一句时，那句被切掉的逗号同样要放回去。
        assert_eq!(
            split_clauses("一共 1,000 块，收好。"),
            vec![("一共 1,000 块，收好。".to_string(), 0.0, None)]
        );
        assert_eq!(
            split_clauses("我打开了 www.bing.com。"),
            vec![("我打开了 www.bing.com。".to_string(), 0.0, None)]
        );
    }

    /// 英文/日文/韩文也走同一套切句：句间停顿、逗号停顿、接缝淡入淡出都一致。
    /// 英文缩写（`Mr.` / `Dr.` / `e.g.`）里的点不算句末；小数、千分位、域名照旧整块念。
    #[test]
    fn splits_other_languages_by_punctuation() {
        // 英文：句号后 0.4 秒、逗号后 0.2 秒，切出来的小句不带前导空格。
        assert_eq!(
            split_clauses("Hello there. How are you today? I am fine, thank you."),
            vec![
                ("Hello there.".to_string(), SENTENCE_PAUSE, None),
                ("How are you today?".to_string(), SENTENCE_PAUSE, None),
                ("I am fine".to_string(), CLAUSE_PAUSE, Some(',')),
                ("thank you.".to_string(), 0.0, None),
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

        // 日文：全角句号/顿号/问号，假名与汉字一样按标点切。
        assert_eq!(
            split_clauses("こんにちは、今日はいい天気ですね。散歩でもしましょうか？"),
            vec![
                ("こんにちは".to_string(), CLAUSE_PAUSE, Some('、')),
                ("今日はいい天気ですね。".to_string(), SENTENCE_PAUSE, None),
                ("散歩でもしましょうか？".to_string(), 0.0, None),
            ]
        );

        // 韩文：句末是 ASCII 的句号/问号，逗号后面补空格。
        assert_eq!(
            split_clauses("안녕하세요, 오늘 날씨가 좋네요. 같이 산책할까요?"),
            vec![
                ("안녕하세요".to_string(), CLAUSE_PAUSE, Some(',')),
                ("오늘 날씨가 좋네요.".to_string(), SENTENCE_PAUSE, None),
                ("같이 산책할까요?".to_string(), 0.0, None),
            ]
        );
    }

    /// 情绪只落在语速/停顿两个参数上（这些模型没有 emotion 输入），分类走 GSV 六分类。
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
}
