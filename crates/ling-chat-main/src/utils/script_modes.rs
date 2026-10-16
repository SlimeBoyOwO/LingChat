//! 剧本包里的「用户声明的模式」（素材模式 / 角色卡模式），写在 `.agent/constraints.md`。

use std::path::Path;

pub const CONSTRAINTS_REL_PATH: &str = ".agent/constraints.md";

/// 素材与角色卡共用的松紧：一个枚举、两个取值，判据只有一种。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Mode {
    /// 只用磁盘上已有的 —— 写了不存在的名字就是错，必须当场换掉。
    OnlyExisting,
    /// 允许缺失（之后补）：缺失只登记、只提醒，不判错。默认。
    #[default]
    AllowMissing,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct ScriptModes {
    pub asset: Mode,
    pub cast: Mode,
}

impl ScriptModes {
    pub fn read(script_dir: &Path) -> Self {
        std::fs::read_to_string(script_dir.join(CONSTRAINTS_REL_PATH))
            .map(|t| Self::parse(&t))
            .unwrap_or_default()
    }

    pub fn parse(text: &str) -> Self {
        Self {
            asset: parse_mode(text, ASSET_KEYS),
            cast: parse_mode(text, CAST_KEYS),
        }
    }
}

impl Mode {
    /// 交接单里的模式一行；`what` 是「素材」或「角色卡」。
    pub fn describe(self, what: &str) -> String {
        match self {
            Mode::OnlyExisting => {
                format!("{what}模式是「只用已有」：引用磁盘上没有的就是错，当场改")
            },
            Mode::AllowMissing => {
                format!("{what}模式是「允许缺失」：缺的只提醒、不判错，之后补上即可")
            },
        }
    }

    /// 换模式的话术：用户说一句就能改，已落盘的章节不会跟着重写。
    pub fn switch_hint(self, what: &str) -> String {
        match self {
            Mode::OnlyExisting => format!(
                "{what}模式是「只用已有」。若其实打算之后补，\
                 说一句「{what}我之后补，先留位置」，我就改成「允许缺失」，这些就不再拦；\
                 已落盘的章节不用重写。\n"
            ),
            Mode::AllowMissing => format!(
                "{what}模式是「允许缺失」：磁盘上还没有的只提醒、不判错，之后补上即可。\
                 若想改成「只用已有」，说一句就行；改成后已落盘章节里缺的会变成必须修的错。\n"
            ),
        }
    }
}

/// 卡片里对同一模式给出互相矛盾的说法时返回一句提示：判定取最后一条，但要把矛盾摊开让人看见。
pub fn conflict_note(text: &str) -> Option<String> {
    let mut asset: Option<Mode> = None;
    let mut cast: Option<Mode> = None;
    for (a, c) in statements(text) {
        if let Some(a) = a {
            asset = match asset {
                Some(prev) if prev != a => return Some(conflict_line("素材", prev, a)),
                _ => Some(a),
            };
        }
        if let Some(c) = c {
            cast = match cast {
                Some(prev) if prev != c => return Some(conflict_line("角色卡", prev, c)),
                _ => Some(c),
            };
        }
    }
    None
}

fn conflict_line(what: &str, prev: impl std::fmt::Debug, now: impl std::fmt::Debug) -> String {
    format!("卡片里「{what}模式」有两处互相矛盾的说法（{prev:?} 与 {now:?}），我按最后一条走")
}

/// 卡片里有没有提过这一类模式 —— 不看取值（取值缺省也按最宽走），只回答"提没提过"。
fn declares(text: &str, keys: &[&str]) -> bool {
    text.lines().any(|line| match field(line) {
        Some((key, value)) => key_matches(&key, keys) && value_mode(&value).is_some(),
        None => false,
    })
}

/// 素材模式声明过没有：没声明过才需要催用户定（章节自检里问这一句）。
pub fn declares_asset(text: &str) -> bool {
    declares(text, ASSET_KEYS)
}

/// 角色卡模式声明过没有：决定交接单带不带角色卡模式那一行。
pub fn declares_cast(text: &str) -> bool {
    declares(text, CAST_KEYS)
}

/// 从卡片里读一类模式（`keys` 是这一类认的键名表）：值取到第一个标点为止，带解释的写法也读得出来。
fn parse_mode(text: &str, keys: &[&str]) -> Mode {
    let mut found = Mode::default();
    for line in text.lines() {
        let Some((key, value)) = field(line) else {
            continue;
        };
        if key_matches(&key, keys) {
            if let Some(mode) = value_mode(&value) {
                found = mode;
            }
        }
    }
    found
}

/// 逐行取出卡片里所有能认出来的模式声明；值认不出就不算声明（认错的代价是校验松紧反了）。
fn statements(text: &str) -> Vec<(Option<Mode>, Option<Mode>)> {
    let mut out = Vec::new();
    for line in text.lines() {
        let Some((key, value)) = field(line) else {
            continue;
        };
        let Some(mode) = value_mode(&value) else {
            continue;
        };
        if key_matches(&key, ASSET_KEYS) {
            out.push((Some(mode), None));
        } else if key_matches(&key, CAST_KEYS) {
            out.push((None, Some(mode)));
        }
    }
    out
}

const ASSET_KEYS: &[&str] = &["素材", "素材模式", "素材来源"];
const CAST_KEYS: &[&str] = &["角色卡", "角色卡模式", "人物卡", "人物卡模式"];

/// 认得出的写法 → 取值，一张值表两类模式共用：除了"只用已有"，其余（含全部旧写法）一律收进 `AllowMissing`，按最宽走。
const VALUES: &[(&str, Mode)] = &[
    ("只用已有", Mode::OnlyExisting),
    ("仅用已有", Mode::OnlyExisting),
    ("先预留", Mode::AllowMissing),
    ("之后补充", Mode::AllowMissing),
    ("之后补", Mode::AllowMissing),
    ("零素材", Mode::AllowMissing),
    ("不要素材", Mode::AllowMissing),
    ("不用素材", Mode::AllowMissing),
    ("无素材", Mode::AllowMissing),
    ("允许缺失", Mode::AllowMissing),
    ("之后再加", Mode::AllowMissing),
    ("之后加", Mode::AllowMissing),
    ("允许后补", Mode::AllowMissing),
];

/// 值 → 取值；表里一条都对不上就返回 `None`（不按声明算，于是走默认的最宽）。
fn value_mode(value: &str) -> Option<Mode> {
    VALUES
        .iter()
        .find(|(k, _)| value.starts_with(k))
        .map(|(_, mode)| *mode)
}

fn key_matches(key: &str, names: &[&str]) -> bool {
    names.contains(&key)
}

/// 取一行的 `键：值`：认列表符号、加粗、反引号与表格行（模型这几样都写过）。
fn field(line: &str) -> Option<(String, String)> {
    let raw = line.trim();
    if raw.is_empty() || raw.starts_with('#') {
        return None;
    }
    let mut cleaned = raw.trim_start_matches(['-', '*', ' ', '\t', '>']).trim();
    cleaned = cleaned.trim_matches('|').trim();
    let (key_raw, rest) = if cleaned.contains('|') {
        let mut cells = cleaned.split('|').map(str::trim).filter(|c| !c.is_empty());
        (
            cells.next()?.to_string(),
            cells.next().unwrap_or("").to_string(),
        )
    } else {
        let pos = cleaned.find([':', '：'])?;
        (cleaned[..pos].to_string(), cleaned[pos..].to_string())
    };
    let key = strip_marks(&key_raw);
    let key = key.strip_suffix("模式").unwrap_or(&key).to_string();
    let value = strip_marks(rest.trim_start_matches([':', '：']).trim());
    let value = cut_at_punctuation(&value);
    Some((key, value))
}

fn strip_marks(s: &str) -> String {
    s.trim_matches(|c: char| c == '*' || c == '_' || c == '`' || c.is_whitespace())
        .trim()
        .to_string()
}

fn cut_at_punctuation(s: &str) -> String {
    let end = s
        .find(['（', '(', '，', ',', '；', ';', '。', '：', ':', '、'])
        .unwrap_or(s.len());
    s[..end].trim().to_string()
}
