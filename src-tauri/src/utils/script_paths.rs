//! 剧本包定位与路径安全。
//!
//! 引擎接受三种磁盘布局（见 `ScriptManager::scan_scripts`）：
//!
//! ```text
//! scripts/character/<角色>/<剧本>/     两级，羁绊冒险用
//! scripts/standalone/<剧本>/           一级，独立剧本用
//! scripts/<剧本>/                      一级，兼容布局
//! ```
//!
//! 编辑器用「剧本 key」指代一个剧本包 —— 即相对 `scripts/` 的路径，统一用 `/`
//! 作分隔符，例如 `character/诺一钦灵/想出去玩啦`。key 由后端枚举产出，前端
//! 只做透传，任何进入文件系统的 key 都必须先过 [`resolve_script_dir`]。
//!
//! 原为 `api/script_editor/paths.rs`，迁到 utils 后剧本路径解析逻辑与 API
//! 命令层解耦，任何需要访问剧本包的模块都能复用。

use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

use crate::utils::path::validate_path_in_base;

/// 保留的一级目录名 —— 它们本身不是剧本包。
const RESERVED_TOP_LEVEL: [&str; 2] = ["character", "standalone"];

/// 剧本包在磁盘上的布局。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ScriptLayout {
    /// `scripts/character/<角色>/<剧本>/`
    Character,
    /// `scripts/standalone/<剧本>/`
    Standalone,
    /// `scripts/<剧本>/`
    Flat,
}

impl ScriptLayout {
    /// 该布局下 key 的段数。
    fn segments(self) -> usize {
        match self {
            ScriptLayout::Character => 3,
            ScriptLayout::Standalone => 2,
            ScriptLayout::Flat => 1,
        }
    }
}

fn scripts_root() -> PathBuf {
    crate::init::static_copy::get_data_dir()
        .join("game_data")
        .join("scripts")
}

/// 把 key 拆成段，同时拒绝一切可疑内容。
///
/// 这是唯一的入口校验点：`..`、绝对路径、空段、Windows 盘符、以及任何
/// 平台分隔符都在这里被挡掉，后面的代码可以安全地 `join`。
fn split_key(key: &str) -> Result<Vec<String>, String> {
    if key.trim().is_empty() {
        return Err("剧本 key 为空".to_string());
    }
    if key.contains('\\') {
        return Err("剧本 key 只能用 / 作分隔符".to_string());
    }
    let segs: Vec<String> = key.split('/').map(|s| s.trim().to_string()).collect();
    for seg in &segs {
        if seg.is_empty() {
            return Err(format!("剧本 key 含空路径段: '{}'", key));
        }
        if seg == "." || seg == ".." {
            return Err(format!("剧本 key 含非法路径段: '{}'", key));
        }
        if seg.contains(':') {
            return Err(format!("剧本 key 含非法字符: '{}'", key));
        }
    }
    if segs.len() > 3 {
        return Err(format!("剧本 key 层级过深: '{}'", key));
    }
    Ok(segs)
}

/// 由 key 推断布局。
pub fn layout_of(key: &str) -> Result<ScriptLayout, String> {
    let segs = split_key(key)?;
    match segs.len() {
        3 if segs[0] == "character" => Ok(ScriptLayout::Character),
        2 if segs[0] == "standalone" => Ok(ScriptLayout::Standalone),
        1 if !RESERVED_TOP_LEVEL.contains(&segs[0].as_str()) => Ok(ScriptLayout::Flat),
        _ => Err(format!("无法识别的剧本 key: '{}'", key)),
    }
}

/// key → 磁盘目录，并确认它确实在 `scripts/` 之内且已存在。
pub fn resolve_script_dir(key: &str) -> Result<PathBuf, String> {
    let layout = layout_of(key)?;
    let segs = split_key(key)?;
    debug_assert_eq!(segs.len(), layout.segments());

    let mut dir = scripts_root();
    for seg in &segs {
        dir.push(seg);
    }

    if !dir.is_dir() {
        return Err(format!("剧本不存在: '{}'", key));
    }
    // canonicalize 之后再确认前缀，防御符号链接
    validate_path_in_base(&dir, &scripts_root())?;
    Ok(dir)
}

/// 供「新建剧本」使用：只算路径、不要求已存在，但同样做前缀校验。
///
/// 目标目录还不存在时无法 canonicalize，所以校验它的父目录。
pub fn resolve_new_script_dir(key: &str) -> Result<PathBuf, String> {
    layout_of(key)?;
    let segs = split_key(key)?;

    let mut dir = scripts_root();
    for seg in &segs {
        dir.push(seg);
    }
    if dir.exists() {
        return Err(format!("剧本已存在: '{}'", key));
    }

    // 只做纯路径校验，不建任何目录 —— 「解析路径」不该有副作用。
    // 早先用 canonicalize 做前缀校验：它要求基目录已存在，而新建剧本时
    // scripts/（乃至 game_data/）很可能还没建，打包版里 data 目录就在 exe
    // 旁边、首次创建前根本没有 scripts/，于是 canonicalize 报 os error 3
    // 「系统找不到指定的路径」，新建剧本永远失败。
    // split_key 已经挡掉了 .. / 绝对路径 / 盘符 / 分隔符，拼出来的 dir 不可能
    // 逃出 scripts_root，所以词法 starts_with 校验就足够（与 resolve_chapter_file
    // 对不存在路径的处理一致），不再依赖任何目录必须已存在。
    let root = scripts_root();
    if !dir.starts_with(&root) {
        return Err(format!("剧本 key 逃出了 scripts/ 目录: '{}'", key));
    }
    Ok(dir)
}

/// 章节 id（相对 `Chapters/` 的路径，不含 `.yaml`）→ 磁盘文件。
///
/// 支持子目录，如 `Intro/intro2` → `Chapters/Intro/intro2.yaml`。
/// `require_exists` 为 false 时用于新建。
pub fn resolve_chapter_file(
    script_dir: &Path,
    chapter_id: &str,
    require_exists: bool,
) -> Result<PathBuf, String> {
    if chapter_id.trim().is_empty() {
        return Err("章节 id 为空".to_string());
    }
    if chapter_id.contains('\\') {
        return Err("章节 id 只能用 / 作分隔符".to_string());
    }
    if chapter_id.eq_ignore_ascii_case("end") {
        return Err("'end' 是保留字，不能作为章节名".to_string());
    }

    let chapters_dir = script_dir.join("Chapters");
    let mut file = chapters_dir.clone();
    for seg in chapter_id.split('/') {
        let seg = seg.trim();
        if seg.is_empty() || seg == "." || seg == ".." || seg.contains(':') {
            return Err(format!("章节 id 含非法路径段: '{}'", chapter_id));
        }
        file.push(seg);
    }
    file.set_extension("yaml");

    if require_exists {
        if !file.is_file() {
            return Err(format!("章节不存在: '{}'", chapter_id));
        }
        validate_path_in_base(&file, &chapters_dir)?;
    } else {
        // 同样不建目录。子目录可能还不存在，所以对 Chapters/ 本身做前缀校验，
        // 再用字符串前缀确认拼出来的路径没有逃出去（父目录不存在时无法 canonicalize）。
        if chapters_dir.is_dir() {
            validate_path_in_base(&chapters_dir, script_dir)?;
        }
        if !file.starts_with(&chapters_dir) {
            return Err(format!("章节 id 逃出了 Chapters/ 目录: '{}'", chapter_id));
        }
    }
    Ok(file)
}

/// 枚举 `scripts/` 下所有剧本包的 key。
///
/// 只把含 `story_config.yaml` 的目录算作剧本包 —— 与 `ScriptManager` 的判定
/// 一致，避免编辑器列出引擎看不见的目录。
pub fn enumerate_script_keys() -> Vec<String> {
    let root = scripts_root();
    let mut keys = Vec::new();

    let Ok(level1) = std::fs::read_dir(&root) else {
        return keys;
    };

    for e1 in level1.flatten() {
        if !e1.file_type().map(|t| t.is_dir()).unwrap_or(false) {
            continue;
        }
        let name1 = e1.file_name().to_string_lossy().to_string();
        // 跳过点号开头的目录（如 .tmp 临时目录、旧回收目录残留等）
        if name1.starts_with('.') {
            continue;
        }

        match name1.as_str() {
            "character" => {
                // character/<角色>/<剧本>/
                if let Ok(level2) = std::fs::read_dir(e1.path()) {
                    for e2 in level2.flatten() {
                        if !e2.file_type().map(|t| t.is_dir()).unwrap_or(false) {
                            continue;
                        }
                        let name2 = e2.file_name().to_string_lossy().to_string();
                        if name2.starts_with('.') {
                            continue;
                        }
                        if let Ok(level3) = std::fs::read_dir(e2.path()) {
                            for e3 in level3.flatten() {
                                if !e3.file_type().map(|t| t.is_dir()).unwrap_or(false) {
                                    continue;
                                }
                                if !e3.path().join("story_config.yaml").is_file() {
                                    continue;
                                }
                                let name3 = e3.file_name().to_string_lossy().to_string();
                                if name3.starts_with('.') {
                                    continue;
                                }
                                keys.push(format!("character/{}/{}", name2, name3));
                            }
                        }
                    }
                }
            }
            "standalone" => {
                if let Ok(level2) = std::fs::read_dir(e1.path()) {
                    for e2 in level2.flatten() {
                        if !e2.file_type().map(|t| t.is_dir()).unwrap_or(false) {
                            continue;
                        }
                        if !e2.path().join("story_config.yaml").is_file() {
                            continue;
                        }
                        let name2 = e2.file_name().to_string_lossy().to_string();
                        if name2.starts_with('.') {
                            continue;
                        }
                        keys.push(format!("standalone/{}", name2));
                    }
                }
            }
            _ => {
                if e1.path().join("story_config.yaml").is_file() {
                    keys.push(name1);
                }
            }
        }
    }

    keys.sort();
    keys
}

/// 列出一个剧本包内的全部章节 id（相对 `Chapters/`，不含扩展名，用 `/` 分隔）。
///
/// 只认 `.yaml`，与引擎一致（`.yml` 引擎找不到）。
pub fn enumerate_chapter_ids(script_dir: &Path) -> Vec<String> {
    let chapters_dir = script_dir.join("Chapters");
    let mut out = Vec::new();
    walk_chapters(&chapters_dir, "", &mut out);
    out.sort();
    out
}

fn walk_chapters(dir: &Path, prefix: &str, out: &mut Vec<String>) {
    let Ok(entries) = std::fs::read_dir(dir) else {
        return;
    };
    for e in entries.flatten() {
        let path = e.path();
        let name = e.file_name().to_string_lossy().to_string();

        // 跳过点号开头的一切：编辑器写盘用的 .<name>.tmp 临时文件、
        // 旧回收目录的残留等，不应被当成正经章节。
        if name.starts_with('.') {
            continue;
        }

        if path.is_dir() {
            let next = if prefix.is_empty() {
                name
            } else {
                format!("{}/{}", prefix, name)
            };
            walk_chapters(&path, &next, out);
        } else if path.extension().and_then(|s| s.to_str()) == Some("yaml") {
            let stem = path
                .file_stem()
                .map(|s| s.to_string_lossy().to_string())
                .unwrap_or_default();
            if stem.is_empty() {
                continue;
            }
            out.push(if prefix.is_empty() {
                stem
            } else {
                format!("{}/{}", prefix, stem)
            });
        }
    }
}

/// 字符层面的公共校验，目录名和文件名共用。
fn check_name_chars(name: &str) -> Result<(), String> {
    const FORBIDDEN: [char; 9] = ['/', '\\', ':', '*', '?', '"', '<', '>', '|'];
    if let Some(bad) = name.chars().find(|c| FORBIDDEN.contains(c)) {
        return Err(format!("名称不能包含字符 '{}'", bad));
    }
    if name.chars().any(|c| c.is_control()) {
        return Err("名称不能包含控制字符".to_string());
    }
    if name.starts_with('.') {
        // 点号开头的文件会被 walk_chapters / list_asset_dir 跳过，
        // 允许创建等于制造「存了但看不见」的困惑
        return Err("名称不能以点号开头".to_string());
    }
    if name.ends_with('.') || name.ends_with(' ') {
        return Err("名称不能以点号或空格结尾（Windows 不允许）".to_string());
    }
    const RESERVED: [&str; 22] = [
        "CON", "PRN", "AUX", "NUL", "COM1", "COM2", "COM3", "COM4", "COM5", "COM6", "COM7", "COM8",
        "COM9", "LPT1", "LPT2", "LPT3", "LPT4", "LPT5", "LPT6", "LPT7", "LPT8", "LPT9",
    ];
    // Windows 的保留名对带扩展名的文件同样生效（CON.txt 也不行）
    let stem = name.split('.').next().unwrap_or(name).to_uppercase();
    if RESERVED.contains(&stem.as_str()) {
        return Err(format!("'{}' 是系统保留名，不能使用", stem));
    }
    Ok(())
}

/// 校验用户上传的**素材文件名**。
///
/// 与目录名分开，因为规则不同：长度按字符数而不是字节数算（`sanitize_folder_name`
/// 的 64 字节上限换成中文只有 21 个字，正常的背景图文件名都过不了），
/// 也不需要排除 `character` / `standalone` 这两个目录保留名。
pub fn sanitize_file_name(raw: &str) -> Result<String, String> {
    let name = raw.trim();
    if name.is_empty() {
        return Err("文件名不能为空".to_string());
    }
    if name.chars().count() > 120 {
        return Err("文件名过长（上限 120 个字符）".to_string());
    }
    check_name_chars(name)?;
    Ok(name.to_string())
}

/// 校验用户输入的目录/文件名，返回可安全落盘的名字。
///
/// 拒绝路径分隔符、Windows 保留名、控制字符、以及首尾空白/点号 ——
/// 这些在 Windows 上会创建失败或产生诡异结果。
pub fn sanitize_folder_name(raw: &str) -> Result<String, String> {
    let name = raw.trim();
    if name.is_empty() {
        return Err("名称不能为空".to_string());
    }
    // 按字符数而不是字节数 —— 原先的 64 字节换成中文只有 21 个字
    if name.chars().count() > 64 {
        return Err("名称过长（上限 64 个字符）".to_string());
    }
    check_name_chars(name)?;
    if RESERVED_TOP_LEVEL.contains(&name) {
        return Err(format!("'{}' 是保留目录名，不能作为剧本名", name));
    }
    Ok(name.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn layout_inference() {
        assert_eq!(layout_of("character/风雪/试着仰望星空"), Ok(ScriptLayout::Character));
        assert_eq!(layout_of("standalone/我的剧本"), Ok(ScriptLayout::Standalone));
        assert_eq!(layout_of("我的剧本"), Ok(ScriptLayout::Flat));
    }

    #[test]
    fn layout_rejects_reserved_and_malformed() {
        // 保留目录本身不是剧本包
        assert!(layout_of("character").is_err());
        assert!(layout_of("standalone").is_err());
        // 段数与前缀不匹配
        assert!(layout_of("character/风雪").is_err());
        assert!(layout_of("standalone/a/b").is_err());
        assert!(layout_of("a/b").is_err());
    }

    #[test]
    fn key_rejects_traversal() {
        for bad in [
            "..",
            "../etc",
            "character/../../etc",
            "a/./b",
            "",
            "   ",
            "a//b",
            "C:/windows",
            "a\\b",
            "a/b/c/d",
        ] {
            assert!(layout_of(bad).is_err(), "应拒绝: {:?}", bad);
        }
    }

    #[test]
    fn sanitize_accepts_cjk_and_rejects_hostile() {
        assert_eq!(sanitize_folder_name(" 想出去玩啦 ").unwrap(), "想出去玩啦");
        for bad in [
            "", "   ", "a/b", "a\\b", "a:b", "a*b", "a?b", "a\"b", "a<b", "a>b", "a|b", "名字.",
            "名字 ", "CON", "nul", "character", "standalone",
        ] {
            assert!(sanitize_folder_name(bad).is_err(), "应拒绝: {:?}", bad);
        }
    }

    #[test]
    fn sanitize_allows_dots_inside() {
        // 原型编辑器错误地禁止了所有点号，Windows 其实允许中间的点
        assert_eq!(sanitize_folder_name("v1.2 试作").unwrap(), "v1.2 试作");
    }

    #[test]
    fn sanitize_rejects_leading_dot() {
        // 点号开头的东西会被 walk_chapters / list_asset_dir 跳过，
        // 允许创建等于制造「存了但看不见」
        assert!(sanitize_folder_name(".hidden").is_err());
        assert!(sanitize_file_name(".bg.png").is_err());
    }

    #[test]
    fn file_name_allows_long_cjk_and_extensions() {
        // 目录名的 64 字节上限会把正常的中文素材名拒掉，文件名按字符数算
        let long = "樱花盛开的公园背景图片最终修正版二.png";
        assert!(sanitize_file_name(long).is_ok());
        assert!(sanitize_folder_name(long).is_ok());
        // 带扩展名的 Windows 保留名同样要挡
        assert!(sanitize_file_name("CON.png").is_err());
        assert!(sanitize_file_name("nul.wav").is_err());
        // 但正常带扩展名的文件不受影响
        assert_eq!(sanitize_file_name("character.png").unwrap(), "character.png");
    }
}
