//! 阿拉伯数字 → 汉字读法。
//!
//! 中文模型**只有在模型目录里带规则 FST 时**才由 sherpa 自己改写数字
//! （matcha-zh 的 `number.fst`、kokoro 的 `number-zh.fst`，见适配器里的
//! `with_rule_files`）。zipvoice / supertonic 的实现压根不读 `rule_fsts`
//! （sherpa-onnx 1.13.5 的 `offline-tts-{zipvoice,supertonic}-impl.h` 里没有
//! `tn_list_`），别的中文模型也可能没随包带 FST。这些模型下数字的读法都不对：
//!
//! - matcha/vits 这类走 CharacterLexicon 的，非汉字直接丢词——日志里是
//!   `Ignore OOV '3'`，用户听到「第 3 章」变成「第 章」；
//! - zipvoice 的 tokens.txt 里虽然有字面数字 token（`133 = "3"`），不会被丢，
//!   但念出来不是「三」。用 sense-voice 反读同一段音频实测：原样送进去读成
//!   「比如**对**章的那些题目」，改写后是「比如**第三章**的那些题目」。
//!
//! 所以这里在 Rust 侧补一层改写。读法对齐模型自带 `number.fst` 的输出（用 kaldifst
//! 跑同一批样例逐条核对过）：
//!
//! | 输入          | 输出                     |
//! |---------------|--------------------------|
//! | `第 3 章`     | `第 三 章`                |
//! | `12` / `105`  | `十二` / `一百零五`        |
//! | `1500`        | `一千五百`                 |
//! | `007`         | `零零七`（前导零逐个念）     |
//! | `2026年`      | `二零二六年`（四位年份逐位念）|
//! | `3.14`        | `三点一四`                 |
//! | `13800138000` | `幺三八零零一三八零零零`     |
//!
//! 刻意**不跟**的部分：模型还带 `phone.fst`，会把 `1100` 念成「幺幺零零」、
//! `11000` 念成「幺幺零零零」——那是按短号码规则硬套的，做数量词时会读错，
//! 所以这里只有 11 位以上的长号码才逐位念。另外模型自带 FST 对部分八位以上
//! 整数有 bug（`100000001` 被念成「一千万一」），这里按标准读法输出
//! 「一亿零一」。

/// 数字 0-9 的汉字。
const DIGITS: [char; 10] = ['零', '一', '二', '三', '四', '五', '六', '七', '八', '九'];

/// 组内位权：个、十、百、千。
const UNITS: [&str; 4] = ["", "十", "百", "千"];

/// 节权：个、万、亿（10 位以内最多三节）。
const GROUP_UNITS: [&str; 3] = ["", "万", "亿"];

/// 到这个长度以上就按长号码逐位念（手机号、身份证号）。
const PHONE_LEN: usize = 11;

/// 把文本里的阿拉伯数字改写成汉字读法，其余字符原样保留。
pub(crate) fn normalize(text: &str) -> String {
    let chars: Vec<char> = text.chars().collect();
    let mut out = String::with_capacity(text.len() * 2);
    let mut i = 0;

    while i < chars.len() {
        if !chars[i].is_ascii_digit() {
            out.push(chars[i]);
            i += 1;
            continue;
        }

        let start = i;
        while i < chars.len() && chars[i].is_ascii_digit() {
            i += 1;
        }
        let int_part: String = chars[start..i].iter().collect();

        // 小数点后逐位念：3.14 → 三点一四，3.50 → 三点五零。
        let mut fraction: Option<String> = None;
        if i + 1 < chars.len() && chars[i] == '.' && chars[i + 1].is_ascii_digit() {
            let mut j = i + 1;
            while j < chars.len() && chars[j].is_ascii_digit() {
                j += 1;
            }
            fraction = Some(chars[i + 1..j].iter().collect());
            i = j;
        }

        // 「2026年」按年份逐位念，「20年」还是按数量念「二十年」——与模型自带的
        // date.fst 一致（实测 1998年 → 一九九八年、98年 → 九十八年）。
        let before_nian = fraction.is_none() && chars.get(i) == Some(&'年');
        out.push_str(&read_number(&int_part, before_nian));

        if let Some(digits) = fraction {
            out.push('点');
            out.extend(digits.chars().map(digit_char));
        }
    }

    out
}

/// 读一个不含小数点的数字串。
fn read_number(digits: &str, before_nian: bool) -> String {
    if before_nian && digits.len() == 4 {
        return digits.chars().map(digit_char).collect();
    }

    if digits.len() >= PHONE_LEN {
        // 长号码逐位念，首位的 1 用「幺」。
        return digits
            .chars()
            .enumerate()
            .map(|(idx, c)| {
                if idx == 0 && c == '1' {
                    '幺'
                } else {
                    digit_char(c)
                }
            })
            .collect();
    }

    let trimmed = digits.trim_start_matches('0');
    if trimmed.is_empty() {
        // 0 / 00 / 000：全是零，逐个念。
        return digits.chars().map(digit_char).collect();
    }

    // 前导零逐个念，剩下的按数量念：007 → 零零七，0086 → 零零八十六。
    let mut out = String::new();
    out.extend(std::iter::repeat_n('零', digits.len() - trimmed.len()));
    out.push_str(&cardinal(trimmed));
    out
}

/// 按数量读一个不含前导零的整数（10 位以内）。
fn cardinal(digits: &str) -> String {
    // 从右往左按 4 位切节，高位那节可以不足 4 位。
    let mut groups: Vec<&str> = Vec::new();
    let mut end = digits.len();
    while end > 4 {
        groups.push(&digits[end - 4..end]);
        end -= 4;
    }
    groups.push(&digits[..end]);
    groups.reverse();

    let mut out = String::new();
    let mut need_zero = false;
    for (idx, group) in groups.iter().enumerate() {
        if group.bytes().all(|b| b == b'0') {
            // 整节为零：先记着，等后面还有非零节时补一个「零」。
            if !out.is_empty() {
                need_zero = true;
            }
            continue;
        }
        if need_zero || (!out.is_empty() && group.starts_with('0')) {
            out.push('零');
        }
        need_zero = false;
        out.push_str(&read_group(group));
        out.push_str(GROUP_UNITS[groups.len() - 1 - idx]);
    }

    // 开头是「一十…」时去掉「一」：10 → 十、十万 → 十万、十亿 → 十亿。
    match out.strip_prefix("一十") {
        Some(rest) => format!("十{rest}"),
        None => out,
    }
}

/// 读一节（最多 4 位，允许前导零），不带节权。
fn read_group(group: &str) -> String {
    let ds: Vec<u8> = group.bytes().map(|b| b - b'0').collect();
    let mut out = String::new();
    let mut pending_zero = false;

    for (i, d) in ds.iter().enumerate() {
        let pos = ds.len() - 1 - i;
        if *d == 0 {
            if !out.is_empty() {
                pending_zero = true;
            }
            continue;
        }
        if pending_zero {
            out.push('零');
            pending_zero = false;
        }
        out.push(DIGITS[*d as usize]);
        out.push_str(UNITS[pos]);
    }

    out
}

fn digit_char(c: char) -> char {
    DIGITS[(c as u8 - b'0') as usize]
}

/// 文本里有没有汉字（只认汉字，不含假名/谚文）。
///
/// 用来判断「这次要不要把阿拉伯数字改写成汉字」：中文句子里念「三」，纯英文台词
/// 里该按英文念 three（zipvoice 是中英双语模型，英文台词很常见），韩文用的是韩语
/// 固有词/汉字词读法，也不该塞汉字进去。范围与 sherpa 的 `ContainsCJK` 不同——那边
/// 把谚文、假名也算 CJK。
pub(crate) fn has_han(text: &str) -> bool {
    text.chars().any(|c| {
        let cp = c as u32;
        (0x3400..=0x4DBF).contains(&cp)      // 汉字扩展 A
            || (0x4E00..=0x9FFF).contains(&cp)  // 基本汉字
            || (0xF900..=0xFAFF).contains(&cp)  // 兼容汉字
            || (0x2_0000..=0x3_FFFF).contains(&cp) // 扩展 B 及以后
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    /// 期望值来自模型自带的 `number.fst`（kaldifst 实跑），逐条对照。
    #[test]
    fn matches_model_number_fst() {
        let cases: &[(&str, &str)] = &[
            ("0", "零"),
            ("1", "一"),
            ("9", "九"),
            ("10", "十"),
            ("11", "十一"),
            ("20", "二十"),
            ("99", "九十九"),
            ("100", "一百"),
            ("101", "一百零一"),
            ("105", "一百零五"),
            ("107", "一百零七"),
            ("120", "一百二十"),
            ("170", "一百七十"),
            ("200", "二百"),
            ("1000", "一千"),
            ("1001", "一千零一"),
            ("1007", "一千零七"),
            ("1010", "一千零一十"),
            ("1200", "一千二百"),
            ("1500", "一千五百"),
            ("2000", "二千"),
            ("2026", "二千零二十六"),
            ("10000", "一万"),
            ("10001", "一万零一"),
            ("10007", "一万零七"),
            ("10010", "一万零一十"),
            ("10100", "一万零一百"),
            ("12345", "一万二千三百四十五"),
            ("20000", "二万"),
            ("54321", "五万四千三百二十一"),
            ("100000", "十万"),
            ("100001", "十万零一"),
            ("100007", "十万零七"),
            ("1000000", "一百万"),
            ("1000001", "一百万零一"),
            ("10000000", "一千万"),
            ("10000100", "一千万零一百"),
            ("100000000", "一亿"),
            ("1000000000", "十亿"),
            ("123456789", "一亿二千三百四十五万六千七百八十九"),
            ("007", "零零七"),
            ("09", "零九"),
            ("0086", "零零八十六"),
            ("3.14", "三点一四"),
            ("0.5", "零点五"),
            ("3.50", "三点五零"),
            ("12.5", "十二点五"),
            ("13800138000", "幺三八零零一三八零零零"),
            ("10000000000", "幺零零零零零零零零零零"),
        ];
        for (input, expected) in cases {
            assert_eq!(&normalize(input), expected, "读错：{input}");
        }
    }

    /// 句子里的改写：只有数字被动，标点、字母、汉字原样保留。
    #[test]
    fn rewrites_digits_in_sentences() {
        let cases: &[(&str, &str)] = &[
            ("第 3 章很重要。", "第 三 章很重要。"),
            ("第3章很重要。", "第三章很重要。"),
            ("我有12本书。", "我有十二本书。"),
            ("一共105个人。", "一共一百零五个人。"),
            ("2026年10月19日。", "二零二六年十月十九日。"),
            ("1998年生", "一九九八年生"),
            ("98年", "九十八年"),
            ("第007号", "第零零七号"),
            ("他今年20岁", "他今年二十岁"),
            ("50%的人", "五十%的人"),
            ("12点半", "十二点半"),
            ("3号线", "三号线"),
            ("1/3", "一/三"),
            ("3-5个", "三-五个"),
            ("GPT-5 出来了", "GPT-五 出来了"),
            ("1+1=2", "一+一=二"),
            ("abc123def", "abc一百二十三def"),
            (
                "我的手机号是13800138000",
                "我的手机号是幺三八零零一三八零零零",
            ),
            ("一二三", "一二三"),
            ("没有数字", "没有数字"),
        ];
        for (input, expected) in cases {
            assert_eq!(&normalize(input), expected, "改写错：{input}");
        }
    }

    /// 与模型自带 FST 的两处刻意偏差。
    #[test]
    fn deviates_from_fst_on_purpose() {
        // phone.fst 会把短号码念成「幺…」，做数量词时读错，这里按数量念。
        assert_eq!(normalize("1100"), "一千一百");
        assert_eq!(normalize("11000"), "一万一千");
        // 自带 FST 对 9 位整数有 bug（一千万一），这里按标准读法。
        assert_eq!(normalize("100000001"), "一亿零一");
        assert_eq!(normalize("1000000010"), "十亿零一十");
    }

    /// 只有文本里有汉字才该改写：英文台词要按英文念数字，谚文/假名不算汉字。
    #[test]
    fn decides_by_han_characters() {
        assert!(has_han("第 3 章"));
        assert!(has_han("GPT-5 出来了"));
        assert!(has_han("マイケル・乔丹"));

        assert!(!has_han("I have 3 apples"));
        assert!(!has_han("hello 3q"));
        assert!(!has_han("3.14"));
        assert!(
            !has_han("こんにちは 3"),
            "假名不是汉字，日文台词不该被塞进汉字数字"
        );
        assert!(!has_han("안녕하세요 3"), "谚文不是汉字");
    }
}
