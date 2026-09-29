//! 剧本包里的「用户声明的模式」（素材模式 / 角色卡模式），写在 `.agent/constraints.md`。
//!
//! 它们只决定**校验的松紧**：用户允许缺的东西不该被判成错，否则校验会一直红着，而那是
//! 用户自己允许的。认不出的写法一律当「未声明」并**按最宽处理** —— 没问过就不该拦人。

use std::path::Path;

pub const CONSTRAINTS_REL_PATH: &str = ".agent/constraints.md";

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum AssetMode {
    #[default]
    Unspecified,
    /// 只用磁盘上已有的 —— 写了不存在的名字就是错，必须当场换掉。
    OnlyExisting,
    /// 之后补素材，先留位置 —— 缺失只登记，不判错。
    Reserve,
    /// 零素材：纯文字剧情，**连磁盘上有的也不引用**。
    None,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum CastMode {
    #[default]
    Unspecified,
    /// 只用磁盘上已有的角色卡 —— 引用了找不到的角色就是错。
    OnlyExisting,
    /// 允许缺失（角色之后再加）—— 找不到的角色只提醒，不判错。
    AllowMissing,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct ScriptModes {
    pub asset: AssetMode,
    pub cast: CastMode,
}

impl ScriptModes {
    pub fn read(script_dir: &Path) -> Self {
        std::fs::read_to_string(script_dir.join(CONSTRAINTS_REL_PATH))
            .map(|t| Self::parse(&t))
            .unwrap_or_default()
    }

    pub fn parse(text: &str) -> Self {
        Self {
            asset: parse_asset_mode(text),
            cast: parse_cast_mode(text),
        }
    }
}

/// 卡片里对同一模式给出**互相矛盾**的说法时返回一句提示。判定取**最后一条**，
/// 但必须把矛盾摊开让人看见 —— 用户改主意后模型常新写一条、旧的留着不删（真机踩过）。
pub fn conflict_note(text: &str) -> Option<String> {
    let mut asset: Option<AssetMode> = None;
    let mut cast: Option<CastMode> = None;
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

/// 素材缺失只有用户**明确说过"只用现有的"**才算错；没声明时按最宽处理（没问过就不该拦人）。
pub fn asset_missing_is_error(mode: AssetMode) -> bool {
    mode == AssetMode::OnlyExisting
}

impl AssetMode {
    pub fn describe(self) -> &'static str {
        match self {
            AssetMode::Unspecified => "未声明（先问用户：只用已有 / 先预留 / 零素材）",
            AssetMode::OnlyExisting => "只用已有（引用不存在的素材 = 错误，当场改）",
            AssetMode::Reserve => "先预留（缺素材只登记进 .agent/assets.md，不判错）",
            AssetMode::None => "零素材（不引用任何素材，纯文字剧情）",
        }
    }

    pub fn switch_hint(self) -> &'static str {
        match self {
            AssetMode::Unspecified => {
                "素材模式还没定：问用户要「只用已有」「先预留」还是「零素材」，\
                 写进 .agent/constraints.md（`- 素材模式：只用已有`）。\
                 定下来之后想换，说一句就行。\n"
            },
            AssetMode::OnlyExisting => {
                "素材模式是「只用已有」。若其实打算之后补素材，\
                 说一句「素材我之后补，先留位置」，\
                 我就改成「先预留」，这些就不再拦；已落盘的章节不用重写。\n"
            },
            AssetMode::Reserve => {
                "素材模式是「先预留」。若想改成「只用已有」，说一句就行；\
                 改成后已落盘章节里留空的素材会变成必须修的错。\n"
            },
            AssetMode::None => {
                "素材模式是「零素材」：不该引用任何素材（连磁盘上有的也不用），\
                 画面全靠旁白与对白写出来。若想改成「只用已有」或「先预留」，说一句就行。\n"
            },
        }
    }
}

impl CastMode {
    pub fn describe(self) -> &'static str {
        match self {
            CastMode::Unspecified => "未声明（先问用户：只用已有 / 允许缺失）",
            CastMode::OnlyExisting => "只用已有（引用了找不到的角色 = 错误）",
            CastMode::AllowMissing => "允许缺失（角色之后再加，找不到只提醒）",
        }
    }

    pub fn switch_hint(self) -> &'static str {
        match self {
            CastMode::Unspecified => {
                "角色卡模式还没定：问用户「只用已有的角色卡」还是「允许缺失、之后再加」，\
                 写进 .agent/constraints.md（`- 角色卡：允许缺失`）。\n"
            },
            CastMode::OnlyExisting => {
                "角色卡模式是「只用已有」。若那些角色之后才加，\
                 说一句「角色我之后加」，我就改成「允许缺失」，校验不再拦。\n"
            },
            CastMode::AllowMissing => {
                "角色卡模式是「允许缺失」：还没建的角色卡只提醒、不判错。\
                 若想改成「只用已有」，说一句就行。\n"
            },
        }
    }
}

/// 从约束卡片里读素材模式：键认 `素材` 与 `素材模式`；值取到第一个标点为止，带解释的写法也读得出来。
pub fn parse_asset_mode(text: &str) -> AssetMode {
    let mut found = AssetMode::Unspecified;
    for (a, _) in statements(text) {
        if let Some(a) = a {
            found = a; // 有多处就以最后一条为准（用户后来改的那条）
        }
    }
    found
}

pub fn parse_cast_mode(text: &str) -> CastMode {
    let mut found = CastMode::Unspecified;
    for (_, c) in statements(text) {
        if let Some(c) = c {
            found = c;
        }
    }
    found
}

/// 逐行取出卡片里所有能认出来的模式声明；值认不出就**不算声明**（认错的代价是校验松紧反了）。
fn statements(text: &str) -> Vec<(Option<AssetMode>, Option<CastMode>)> {
    let mut out = Vec::new();
    for line in text.lines() {
        let Some((key, value)) = field(line) else {
            continue;
        };
        if key_matches(&key, &["素材", "素材模式", "素材来源"])
            && value_matches(&value, ASSET_VALUES)
        {
            out.push((Some(key_value_asset(&value)), None));
        } else if key_matches(&key, &["角色卡", "角色卡模式", "人物卡", "人物卡模式"])
            && value_matches(&value, CAST_VALUES)
        {
            out.push((None, Some(key_value_cast(&value))));
        }
    }
    out
}

const ASSET_VALUES: &[&str] = &[
    "只用已有",
    "仅用已有",
    "先预留",
    "之后补充",
    "之后补",
    "零素材",
    "不要素材",
    "不用素材",
    "无素材",
];

const CAST_VALUES: &[&str] = &[
    "只用已有",
    "仅用已有",
    "允许缺失",
    "之后补充",
    "之后再加",
    "之后加",
    "允许后补",
];

fn value_matches(value: &str, known: &[&str]) -> bool {
    known.iter().any(|k| value.starts_with(k))
}

fn key_value_asset(value: &str) -> AssetMode {
    if value.starts_with("只用已有") || value.starts_with("仅用已有") {
        AssetMode::OnlyExisting
    } else if value.starts_with("先预留") || value.starts_with("之后补") {
        AssetMode::Reserve
    } else {
        AssetMode::None
    }
}

fn key_value_cast(value: &str) -> CastMode {
    if value.starts_with("只用已有") || value.starts_with("仅用已有") {
        CastMode::OnlyExisting
    } else {
        CastMode::AllowMissing
    }
}

fn key_matches(key: &str, names: &[&str]) -> bool {
    names.contains(&key)
}

/// 取一行的 `键：值`。认列表符号、加粗、反引号、表格行 —— 模型这几样都写过
/// （`- **素材模式**：零素材`、`| 角色卡 | 允许缺失 |`）。
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
