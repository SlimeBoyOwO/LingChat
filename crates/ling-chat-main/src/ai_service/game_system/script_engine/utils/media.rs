//! Media file resolution for script events.
//!
//! Searches the current script's `Assets/` subdirectories first,
//! then falls back to global `game_data/` directories.

use std::path::Path;

/// 引擎的「清空该轨」写法（`musicPath: none` 停音乐、`imagePath: none` 关掉插图），
pub const NONE_SENTINEL: &str = "none";

/// Media type determines which subdirectories and fallback to search.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MediaType {
    /// 媒体类型枚举，决定搜索的子目录和 fallback 目录
    Background,
    Music,
    Sound,
    Pic,
    /// 环境音（雨声、风声等持续场景音效）
    Ambient,
}

impl MediaType {
    /// Candidate subdirectory names under `Assets/` in the script directory.
    pub fn subdir_candidates(self) -> &'static [&'static str] {
        match self {
            MediaType::Background => &["Backgrounds", "Pics", "Pictures", "Pic", "Picture"],
            MediaType::Music => &["Musics", "BGMs", "Music", "BGM"],
            MediaType::Sound => &["Sounds", "SoundEffects", "Sound", "SoundEffect"],
            MediaType::Pic => &["Pics", "Pictures", "Pic", "Picture"],
            MediaType::Ambient => &["Ambients", "AmbientSounds", "Environment", "Ambient"],
        }
    }

    /// Fallback directory name under `game_data/`.
    pub fn fallback_dir(self) -> &'static str {
        match self {
            MediaType::Background | MediaType::Pic => "backgrounds",
            MediaType::Music | MediaType::Sound => "musics",
            MediaType::Ambient => "ambients",
        }
    }

    /// 该类素材允许的扩展名（小写、不含点）：编辑器列举素材与「换扩展名找同一个素材」
    pub fn allowed_extensions(self) -> &'static [&'static str] {
        match self {
            MediaType::Background | MediaType::Pic => &["png", "jpg", "jpeg", "webp", "bmp", "gif"],
            MediaType::Music | MediaType::Sound | MediaType::Ambient => {
                &["mp3", "wav", "ogg", "flac"]
            },
        }
    }
}

/// Resolve a script media file path from YAML event data.
///
/// Resolution order:
/// 1. If `script_path` is `Some`, search `{script_path}/Assets/{candidate}/{file_path}`
/// 2. Fallback: search `{data_dir}/game_data/{fallback_dir}/{file_path}`
/// 3. 以上都失败时，剥掉引用里多余的类型目录前缀（如 `backgrounds/夜晚.webp`）重试
///
/// 每一轮都是「先按完整文件名精确匹配，再换成同主干的其它扩展名」：引用写 `药店.png`
/// 而磁盘上只有 `药店.jpg` / `药店.webp` 时照样命中，见 [`resolve_with_extensions`]。
///
/// Returns the canonical absolute path if found, `None` otherwise.
pub fn resolve_script_media(
    data_dir: &Path,
    script_path: Option<&Path>,
    file_path: &str,
    media_type: MediaType,
) -> Option<String> {
    // 哨兵值按「没有素材」处理，不能被扩展名兜底当成同名文件（磁盘上有个 `none.png`
    // 就会把「停音乐」变成「放 none.png」）
    if file_path.eq_ignore_ascii_case(NONE_SENTINEL) {
        return None;
    }
    if let Some(found) = resolve_with_extensions(data_dir, script_path, file_path, media_type) {
        return Some(found);
    }
    // AI 生成/修改剧本时容易把类型目录写进引用（背景写成 backgrounds/夜晚.webp），
    // 解析器本来就会按类型拼目录，这种写法会变成双重目录而找不到；剥掉前缀兜底一次。
    let stripped = strip_type_dir_prefix(file_path, media_type)?;
    resolve_with_extensions(data_dir, script_path, stripped, media_type)
}

/// 精确匹配优先，不中再逐个换扩展名 —— 引用里写了正确扩展名时，行为与从前完全一致。
fn resolve_with_extensions(
    data_dir: &Path,
    script_path: Option<&Path>,
    file_path: &str,
    media_type: MediaType,
) -> Option<String> {
    if let Some(found) = resolve_exact(data_dir, script_path, file_path, media_type) {
        return Some(found);
    }
    extension_variants(file_path, media_type)
        .into_iter()
        .find_map(|candidate| resolve_exact(data_dir, script_path, &candidate, media_type))
}

/// 把引用里的扩展名换成该类型允许的其它扩展名，目录与文件名主干原样保留。
///
/// 磁盘上换了格式（png 转成 webp 省体积）而剧本没跟着改时靠它兜住。引用本身不带
/// 扩展名（`imagePath: 药店`）就是在补扩展名；带一个引擎不认识的扩展名也一并替换 ——
/// 后缀不参与匹配，磁盘上是哪种格式就用哪种。
fn extension_variants(file_path: &str, media_type: MediaType) -> Vec<String> {
    // 按原引用的分隔符切，别用 Path::join —— 那会把引用里的 `\` 在 Linux 上变成普通字符
    let name_start = file_path.rfind(['/', '\\']).map_or(0, |i| i + 1);
    let (dir, name) = file_path.split_at(name_start);
    let Some(stem) = Path::new(name).file_stem().and_then(|s| s.to_str()) else {
        return Vec::new();
    };
    if stem.is_empty() {
        return Vec::new();
    }
    media_type
        .allowed_extensions()
        .iter()
        .map(|ext| format!("{dir}{stem}.{ext}"))
        .collect()
}

/// 按原始引用精确查找：先剧本 Assets 候选目录，再全局 game_data 类型目录。
fn resolve_exact(
    data_dir: &Path,
    script_path: Option<&Path>,
    file_path: &str,
    media_type: MediaType,
) -> Option<String> {
    if file_path.is_empty() {
        return None;
    }

    // Step 1: search current script's Assets directory
    if let Some(sp) = script_path {
        for candidate in media_type.subdir_candidates() {
            let candidate_path = sp.join("Assets").join(candidate).join(file_path);
            if candidate_path.exists() {
                if let Ok(canon) = candidate_path.canonicalize() {
                    return Some(canon.to_string_lossy().into_owned());
                }
            }
        }
    }

    // Step 2: fallback to game_data/{fallback_dir}
    let fallback_path = data_dir
        .join("game_data")
        .join(media_type.fallback_dir())
        .join(file_path);
    if fallback_path.exists() {
        if let Ok(canon) = fallback_path.canonicalize() {
            return Some(canon.to_string_lossy().into_owned());
        }
    }

    None
}

/// 引用第一层是类型目录名（如 `backgrounds/`、`musics\\`）时返回剥掉前缀的部分，
/// 否则返回 None。只剥已知类型词，自定义子目录（如 `海边/白天.webp`）不受影响；
/// 同时拒绝绝对路径和 `..` 等不安全的剩余路径，避免兜底逻辑扩大原始查找范围。
fn strip_type_dir_prefix(file_path: &str, media_type: MediaType) -> Option<&str> {
    let separator = file_path.find(['/', '\\'])?;
    let (prefix, rest_with_separator) = file_path.split_at(separator);
    let rest = rest_with_separator.get(1..)?;
    let mut components = rest.split(['/', '\\']);
    let first = components.next()?;
    if first.is_empty()
        || first == "."
        || first == ".."
        || first.ends_with(':')
        || components.any(|part| part.is_empty() || part == "." || part == "..")
        || Path::new(rest).is_absolute()
    {
        return None;
    }
    let is_type_dir = prefix.eq_ignore_ascii_case(media_type.fallback_dir())
        || media_type
            .subdir_candidates()
            .iter()
            .any(|c| c.eq_ignore_ascii_case(prefix));
    is_type_dir.then_some(rest)
}
