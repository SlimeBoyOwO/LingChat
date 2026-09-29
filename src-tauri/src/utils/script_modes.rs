//! 剧本包里的「用户声明的模式」。
//!
//! 用户说过的两件事，决定**校验的松紧**：
//! - **素材模式**：只用已有 / 先预留（之后补）/ 零素材
//! - **角色卡模式**：只用已有 / 允许缺失（之后补）
//!
//! 声明写在剧本包的 `.agent/constraints.md` 里（模型问过用户之后写下来）。
//! 校验（整剧校验与章节自检）都读这里 —— **用户允许缺的东西，不该被判成错**；
//! 否则校验会一直红着，而那是用户自己允许的。
//!
//! 认不出的写法一律当「未声明」并**按最宽处理**：没问过就不该拦人，
//! 由章节自检去催模型问用户。

use std::path::Path;

/// 用户约束卡片。剧本包内的相对路径。
pub const CONSTRAINTS_REL_PATH: &str = ".agent/constraints.md";

/// 素材从哪来。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum AssetMode {
    /// 没问过 / 卡片里没写或写得不认识。
    #[default]
    Unspecified,
    /// 只用磁盘上已有的 —— 写了不存在的名字就是错，必须当场换掉。
    OnlyExisting,
    /// 之后补素材，先留位置 —— 缺失只登记，不判错。
    Reserve,
    /// 零素材：纯文字剧情，**连磁盘上有的也不引用**。
    None,
}

/// 角色卡从哪来。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum CastMode {
    /// 没问过 / 卡片里没写或写得不认识。
    #[default]
    Unspecified,
    /// 只用磁盘上已有的角色卡 —— 引用了找不到的角色就是错。
    OnlyExisting,
    /// 允许缺失（角色之后再加）—— 找不到的角色只提醒，不判错。
    AllowMissing,
}

/// 一个剧本包当前生效的模式。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct ScriptModes {
    pub asset: AssetMode,
    pub cast: CastMode,
}

impl ScriptModes {
    /// 从剧本包里读（读不到/没写 = 全是未声明）。
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

/// 卡片里对同一个模式给出了**互相矛盾**的说法时，返回一句提示，否则 `None`。
///
/// 用户改主意之后模型常常新写一条、旧的留着不删（真机踩过：卡片里
/// 「零素材」和「只用已有」并存，交接单说"未声明"，模型连着三轮追问按哪个走）。
/// 判定取**最后一条**，但必须把矛盾摊开让人看见。
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

/// 素材缺失算不算「错误」。
///
/// 只有用户**明确说过"只用现有的"**才算错。没声明时按最宽处理：
/// 没问过就不该拦人（章节自检会催模型去问）。
pub fn asset_missing_is_error(mode: AssetMode) -> bool {
    mode == AssetMode::OnlyExisting
}

impl AssetMode {
    /// 交接单与回执里那一行。
    pub fn describe(self) -> &'static str {
        match self {
            AssetMode::Unspecified => "未声明（先问用户：只用已有 / 先预留 / 零素材）",
            AssetMode::OnlyExisting => "只用已有（引用不存在的素材 = 错误，当场改）",
            AssetMode::Reserve => "先预留（缺素材只登记进 .agent/assets.md，不判错）",
            AssetMode::None => "零素材（不引用任何素材，纯文字剧情）",
        }
    }

    /// 怎么换一个模式。用户决定没有一次性的，改法要随手说出来。
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

/// 从约束卡片里读素材模式。
///
/// 键认 `素材` 与 `素材模式`（模型两种都写过）；值取到第一个标点为止，
/// 于是 `素材：零素材（依据：用户说…）` 这种带解释的写法也读得出来。
pub fn parse_asset_mode(text: &str) -> AssetMode {
    let mut found = AssetMode::Unspecified;
    for (a, _) in statements(text) {
        if let Some(a) = a {
            found = a; // 有多处就以最后一条为准（用户后来改的那条）
        }
    }
    found
}

/// 从约束卡片里读角色卡模式。键认 `角色卡` 与 `角色卡模式`。
pub fn parse_cast_mode(text: &str) -> CastMode {
    let mut found = CastMode::Unspecified;
    for (_, c) in statements(text) {
        if let Some(c) = c {
            found = c;
        }
    }
    found
}

/// 逐行取出卡片里所有能认出来的模式声明（按出现顺序）。
///
/// 值认不出就**不算声明**：认错的代价是校验松紧反了。
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

/// 取一行的 `键：值`。
///
/// 认列表符号、加粗、反引号、表格行 —— 模型这几样都写过：
/// `- 素材：零素材（依据：…）`、`- **素材模式**：零素材`、`| 角色卡 | 允许缺失 |`。
fn field(line: &str) -> Option<(String, String)> {
    let raw = line.trim();
    if raw.is_empty() || raw.starts_with('#') {
        return None;
    }
    let mut cleaned = raw.trim_start_matches(['-', '*', ' ', '\t', '>']).trim();
    cleaned = cleaned.trim_matches('|').trim();
    let (key_raw, rest) = if cleaned.contains('|') {
        // 表格行：`| 素材 | 零素材 |`
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

/// 去掉 `*` / `_` / 反引号这类装饰。
fn strip_marks(s: &str) -> String {
    s.trim_matches(|c: char| c == '*' || c == '_' || c == '`' || c.is_whitespace())
        .trim()
        .to_string()
}

/// 取到第一个标点为止：`零素材（依据：…）` → `零素材`。
fn cut_at_punctuation(s: &str) -> String {
    let end = s
        .find(['（', '(', '，', ',', '；', ';', '。', '：', ':', '、'])
        .unwrap_or(s.len());
    s[..end].trim().to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reads_both_modes_from_one_card() {
        let text = "# 约束\n\n- 素材模式：先预留\n- 角色卡：允许缺失\n- 玩家扮演：风雪\n";
        let modes = ScriptModes::parse(text);
        assert_eq!(modes.asset, AssetMode::Reserve);
        assert_eq!(modes.cast, CastMode::AllowMissing);
    }

    #[test]
    fn asset_mode_synonyms_and_unknown_values() {
        let read = |s: &str| parse_asset_mode(&format!("# x\n\n{}", s));
        assert_eq!(read("- 素材模式：只用已有"), AssetMode::OnlyExisting);
        assert_eq!(read("* 素材模式：仅用已有"), AssetMode::OnlyExisting);
        assert_eq!(read("素材模式：之后补充"), AssetMode::Reserve);
        // 真人真的这么说过：「素材的话就零素材」
        assert_eq!(read("- 素材模式：零素材"), AssetMode::None);
        assert_eq!(read("- 素材模式：不要素材"), AssetMode::None);
        // 认不出 → 未声明（并按最宽处理），不猜
        assert_eq!(read("- 素材模式：尽量用已有的"), AssetMode::Unspecified);
        assert_eq!(read("- 玩家扮演：风雪"), AssetMode::Unspecified);
        assert_eq!(parse_asset_mode(""), AssetMode::Unspecified);
    }

    #[test]
    fn cast_mode_synonyms() {
        let read = |s: &str| parse_cast_mode(&format!("# x\n\n{}", s));
        assert_eq!(read("- 角色卡：只用已有"), CastMode::OnlyExisting);
        assert_eq!(read("- 角色卡：允许缺失"), CastMode::AllowMissing);
        assert_eq!(read("- 角色卡：之后再加"), CastMode::AllowMissing);
        assert_eq!(read("- 角色卡：看情况"), CastMode::Unspecified);
        // 模型两处手册里都写作「角色卡模式」，真人也会跟着这么写
        assert_eq!(read("- 角色卡模式：允许缺失"), CastMode::AllowMissing);
    }

    #[test]
    fn only_explicit_use_existing_is_strict() {
        // 用户明确说"只用现有的"才算错；没声明按最宽（章节自检会去催问）
        assert!(asset_missing_is_error(AssetMode::OnlyExisting));
        assert!(!asset_missing_is_error(AssetMode::Reserve));
        assert!(!asset_missing_is_error(AssetMode::Unspecified));
        // 零素材是另一套判定（引用本身就不该有），不在这里
        assert!(!asset_missing_is_error(AssetMode::None));
    }

    #[test]
    fn every_mode_tells_how_to_switch() {
        for mode in [
            AssetMode::Unspecified,
            AssetMode::OnlyExisting,
            AssetMode::Reserve,
            AssetMode::None,
        ] {
            let hint = mode.switch_hint();
            assert!(hint.contains("素材模式"), "{mode:?}: {hint}");
            assert!(hint.contains("说一句"), "{mode:?}: {hint}");
        }
        for mode in [
            CastMode::Unspecified,
            CastMode::OnlyExisting,
            CastMode::AllowMissing,
        ] {
            let hint = mode.switch_hint();
            assert!(hint.contains("角色卡"), "{mode:?}: {hint}");
        }
    }

    #[test]
    fn missing_card_reads_as_unspecified() {
        assert_eq!(
            ScriptModes::read(Path::new("/nonexistent-script-dir")),
            ScriptModes::default()
        );
    }

    #[test]
    fn reads_the_forms_the_model_actually_writes() {
        // 真机实录：卡片里写的是 `- 素材：零素材（依据：…）`，
        // 解析只认「素材模式」于是判成未声明，交接单跟磁盘对不上，模型连着追问三轮。
        let real = "# 约束（素材 / 角色卡模式）\n\n\
                    - 素材：零素材（依据：用户本轮「算了，改成零素材吧，纯文字」。自本轮起，章节里不许出现任何素材事件…）\n\
                    - 角色卡：允许缺失\n";
        let modes = ScriptModes::parse(real);
        assert_eq!(modes.asset, AssetMode::None);
        assert_eq!(modes.cast, CastMode::AllowMissing);

        let read = |s: &str| parse_asset_mode(&format!("# x\n\n{s}"));
        // 加粗、反引号、表格、条目不带冒号空格
        assert_eq!(read("- **素材模式**：零素材"), AssetMode::None);
        assert_eq!(read("- `素材模式`：先预留"), AssetMode::Reserve);
        assert_eq!(read("| 素材 | 只用已有 |"), AssetMode::OnlyExisting);
        assert_eq!(read("素材模式:只用已有"), AssetMode::OnlyExisting);
        assert_eq!(
            read("- 素材模式：先预留（缺的登记进 assets.md）"),
            AssetMode::Reserve
        );
    }

    #[test]
    fn later_statement_wins_and_conflicts_are_reported() {
        // 用户改主意时模型会新写一条、旧的留着一部分：以最后一条为准，但要把矛盾说出来
        let text =
            "# 约束\n\n- 素材模式：只用已有\n- 角色卡：允许缺失\n- 素材：零素材（用户改口）\n";
        assert_eq!(parse_asset_mode(text), AssetMode::None);
        let note = conflict_note(text).expect("两处矛盾要报出来");
        assert!(note.contains("素材"), "{note}");
        assert!(note.contains("最后一条"), "{note}");

        // 说法一致时不该噪声
        let same = "# 约束\n\n- 素材模式：零素材\n- 素材：零素材\n";
        assert_eq!(conflict_note(same), None);
        assert_eq!(conflict_note("- 素材模式：先预留\n"), None);
    }

    #[test]
    fn values_that_only_look_like_modes_are_ignored() {
        // 「尽量用已有的」不是「只用已有」；认错的代价是校验松紧反了
        let read = |s: &str| parse_asset_mode(&format!("# x\n\n{s}"));
        assert_eq!(read("- 素材模式：尽量用已有的"), AssetMode::Unspecified);
        assert_eq!(read("- 素材模式：看情况"), AssetMode::Unspecified);
        assert_eq!(read("- 素材模式：还没想好"), AssetMode::Unspecified);
        // 键认不出的一律不算（玩家扮演不是模式）
        assert_eq!(read("- 玩家扮演：风雪"), AssetMode::Unspecified);
        assert_eq!(parse_asset_mode(""), AssetMode::Unspecified);
        // 标题行不是声明
        assert_eq!(
            parse_asset_mode("# 素材模式：零素材\n"),
            AssetMode::Unspecified
        );
    }
}
