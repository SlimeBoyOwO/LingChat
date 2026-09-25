use std::collections::{HashMap, HashSet};
use std::fs;
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

use serde::{Deserialize, Serialize};
use serde_json::Value as JsonValue;
use tauri::{AppHandle, Manager};
use tokio_util::sync::CancellationToken;

use crate::AppState;
use crate::ai_service::types::{
    CharacterSettings, Live2dEyeBlinkBinding, Live2dMotionBinding, Live2dParameterBinding,
    Live2dSettings, Live2dVariant,
};
use crate::db::entities::role::RoleType;
use crate::db::managers::role_repo::RoleRepo;
use crate::utils::archive::extract_zip;
use crate::utils::yaml_file::write_json_as_yaml;

use super::game_data_dir;

#[derive(Debug, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Live2dSourceKind {
    Directory,
    Zip,
}

#[derive(Debug, Serialize)]
pub struct Live2dModelInfo {
    pub variant: String,
    pub model: String,
    pub expressions: Vec<String>,
    pub motions: HashMap<String, Vec<String>>,
}

#[derive(Debug, Serialize)]
pub struct Live2dImportResult {
    pub live2d: Live2dSettings,
    pub models: Vec<Live2dModelInfo>,
}

/// 运行时注入用的可用资源表：把 model3.json 自己没声明的表情/动作补回去。
///
/// `settings.yml` 只记「情绪 -> 表情名」这类绑定，不记名字对应的文件；而引擎查表情
/// 只能查 model3.json 的 `FileReferences.Expressions`，名字不在表里时 `setExpression`
/// 静默返回 false。所以 VTS 式导出的模型必须在加载时把声明注入进去，前端才有办法
/// 让用户绑的名字真正生效。这份清单是派生数据，不落盘。
#[derive(Debug, Serialize)]
pub struct Live2dVariantAssets {
    /// 表情名 -> 模型目录相对路径
    pub expressions: HashMap<String, String>,
    /// 动作组名 -> 文件列表（模型目录相对路径）
    pub motions: HashMap<String, Vec<String>>,
}

fn role_dir(
    role_type: &RoleType,
    folder: &str,
    script_key: Option<&str>,
) -> Result<PathBuf, String> {
    match role_type {
        RoleType::Main => Ok(super::resolve_character_dir(folder)),
        RoleType::Npc => script_key
            .map(|key| {
                game_data_dir()
                    .join("scripts")
                    .join(key)
                    .join("characters")
                    .join(folder)
            })
            .ok_or_else(|| "剧本角色缺少 script_key".to_string()),
        RoleType::System | RoleType::User => Err("系统角色不支持 Live2D 资源".to_string()),
    }
}

fn copy_directory(source: &Path, target: &Path) -> Result<(), String> {
    fs::create_dir_all(target).map_err(|e| format!("创建目录失败: {e}"))?;
    for entry in fs::read_dir(source).map_err(|e| format!("读取目录失败: {e}"))? {
        let entry = entry.map_err(|e| format!("读取目录项失败: {e}"))?;
        let destination = target.join(entry.file_name());
        let file_type = entry
            .file_type()
            .map_err(|e| format!("读取文件类型失败: {e}"))?;
        if file_type.is_dir() {
            copy_directory(&entry.path(), &destination)?;
        } else if file_type.is_file() {
            fs::copy(entry.path(), destination).map_err(|e| format!("复制文件失败: {e}"))?;
        }
    }
    Ok(())
}

fn collect_model_files(dir: &Path, files: &mut Vec<PathBuf>) -> Result<(), String> {
    for entry in fs::read_dir(dir).map_err(|e| format!("扫描 Live2D 目录失败: {e}"))? {
        let entry = entry.map_err(|e| format!("读取 Live2D 文件失败: {e}"))?;
        let path = entry.path();
        if entry.file_type().map_err(|e| e.to_string())?.is_dir() {
            collect_model_files(&path, files)?;
        } else if path
            .file_name()
            .and_then(|name| name.to_str())
            .map(|name| name.ends_with(".model3.json"))
            .unwrap_or(false)
        {
            files.push(path);
        }
    }
    Ok(())
}

fn find_import_manifest(dir: &Path) -> Result<Option<PathBuf>, String> {
    for entry in fs::read_dir(dir).map_err(|e| format!("扫描 Live2D 目录失败: {e}"))? {
        let entry = entry.map_err(|e| format!("读取 Live2D 文件失败: {e}"))?;
        let path = entry.path();
        if entry.file_type().map_err(|e| e.to_string())?.is_dir() {
            if let Some(found) = find_import_manifest(&path)? {
                return Ok(Some(found));
            }
        } else if entry.file_name() == "lingchat-live2d.json" {
            return Ok(Some(path));
        }
    }
    Ok(None)
}

fn has_url_scheme(value: &str) -> bool {
    let Some((scheme, _)) = value.split_once(':') else {
        return false;
    };
    let mut chars = scheme.chars();
    chars
        .next()
        .is_some_and(|first| first.is_ascii_alphabetic())
        && chars.all(|character| {
            character.is_ascii_alphanumeric() || matches!(character, '+' | '-' | '.')
        })
}

fn validate_relative_path(value: &str, label: &str) -> Result<(), String> {
    if has_url_scheme(value) || value.starts_with('/') || value.starts_with('\\') {
        return Err(format!("{label} 必须是相对路径: {value}"));
    }
    Ok(())
}

fn referenced_path(
    model_dir: &Path,
    resource_root: &Path,
    value: &JsonValue,
    label: &str,
) -> Result<(), String> {
    let Some(relative) = value.as_str() else {
        return Err(format!("{label} 引用不是字符串"));
    };
    validate_relative_path(relative, &format!("{label} 引用"))?;
    let resolved = model_dir.join(relative);
    if !resolved.is_file() {
        return Err(format!("缺少 {label} 文件: {relative}"));
    }
    let canonical = resolved
        .canonicalize()
        .map_err(|e| format!("解析 {label} 文件失败: {e}"))?;
    let canonical_root = resource_root
        .canonicalize()
        .map_err(|e| format!("解析 Live2D 资源目录失败: {e}"))?;
    canonical
        .strip_prefix(canonical_root)
        .map_err(|_| format!("{label} 文件必须位于本次导入的 Live2D 资源内: {relative}"))?;
    Ok(())
}

/// model3.json 引用的表情/动作后缀。VTube Studio 导出把资源散在模型目录里，
/// 靠这两个后缀识别。
const EXPRESSION_SUFFIX: &str = ".exp3.json";
const MOTION_SUFFIX: &str = ".motion3.json";

/// 读取 model3.json，返回它的 `FileReferences` 与模型文件所在目录。
fn read_model_references(
    model_file: &Path,
) -> Result<(serde_json::Map<String, JsonValue>, PathBuf), String> {
    let raw = fs::read_to_string(model_file).map_err(|e| format!("读取 model3 失败: {e}"))?;
    let json: JsonValue =
        serde_json::from_str(&raw).map_err(|e| format!("解析 model3 失败: {e}"))?;
    let refs = json
        .get("FileReferences")
        .and_then(JsonValue::as_object)
        .ok_or_else(|| "model3 缺少 FileReferences".to_string())?
        .clone();
    let model_dir = model_file
        .parent()
        .ok_or_else(|| "model3 路径无效".to_string())?
        .to_path_buf();
    Ok((refs, model_dir))
}

/// 递归收集 `dir` 下所有以 `suffix` 结尾的文件，返回 (去掉 suffix 的文件名, 相对 base 的路径)。
///
/// 用 `strip_suffix` 而不是 `file_stem`：后者只剥一层扩展名，`哭.exp3.json` 会得到
/// `哭.exp3`，既进不了设置界面的选项表，也匹配不上任何关键字。结果按相对路径排序，
/// 且排序必须在去重与重命名之前——导入期写进 `settings.yml` 的组名和运行期注入用的
/// 组名必须逐字相同，否则绑定会指向一个不存在的组。
fn scan_loose_files(base: &Path, suffix: &str) -> Result<Vec<(String, String)>, String> {
    let mut found = Vec::new();
    collect_loose_files(base, base, suffix, &mut found)?;
    found.sort_by(|left, right| left.1.cmp(&right.1));
    Ok(found)
}

fn collect_loose_files(
    base: &Path,
    dir: &Path,
    suffix: &str,
    found: &mut Vec<(String, String)>,
) -> Result<(), String> {
    for entry in fs::read_dir(dir).map_err(|e| format!("扫描 Live2D 资源目录失败: {e}"))? {
        let entry = entry.map_err(|e| format!("读取 Live2D 资源文件失败: {e}"))?;
        let path = entry.path();
        if entry.file_type().map_err(|e| e.to_string())?.is_dir() {
            collect_loose_files(base, &path, suffix, found)?;
            continue;
        }
        let Some(name) = path.file_name().and_then(|name| name.to_str()) else {
            continue;
        };
        let Some(stem) = name.strip_suffix(suffix) else {
            continue;
        };
        if stem.is_empty() {
            continue;
        }
        let relative = path
            .strip_prefix(base)
            .map_err(|_| "Live2D 资源不在模型目录内".to_string())?
            .to_string_lossy()
            .replace('\\', "/");
        found.push((stem.to_string(), relative));
    }
    Ok(())
}

/// 一个 variant 的可用表情/动作清单。既驱动设置界面的下拉框，也驱动运行时把声明
/// 注入回 model3.json——VTS 式导出的 model3.json 里没有 `FileReferences.Expressions`，
/// 名字不补进去的话引擎查不到，`setExpression` 会静默返回 false。
struct ResolvedAssets {
    /// 表情名，有序：来自声明时保持声明顺序，来自扫描时按文件路径排序
    expressions: Vec<String>,
    /// 表情名 -> 模型目录相对路径
    expression_files: HashMap<String, String>,
    /// 动作组名 -> 文件列表（模型目录相对路径）
    motions: HashMap<String, Vec<String>>,
    /// 表情清单是否来自扫描。扫描结果按字母序排列，不能沿用「取第一个」的默认表情回退
    scanned_expressions: bool,
}

fn resolve_assets(
    refs: &serde_json::Map<String, JsonValue>,
    model_dir: &Path,
    resource_root: &Path,
) -> Result<ResolvedAssets, String> {
    let mut expressions = Vec::new();
    let mut expression_files = HashMap::new();
    if let Some(items) = refs.get("Expressions").and_then(JsonValue::as_array) {
        for item in items {
            if let Some(file) = item.get("File") {
                referenced_path(model_dir, resource_root, file, "Expression")?;
                if let Some(file) = file.as_str() {
                    if let Some(name) = item.get("Name").and_then(JsonValue::as_str) {
                        expression_files.insert(name.to_string(), file.to_string());
                    }
                }
            }
            if let Some(name) = item.get("Name").and_then(JsonValue::as_str) {
                expressions.push(name.to_string());
            }
        }
    }

    let mut motions = HashMap::new();
    if let Some(groups) = refs.get("Motions").and_then(JsonValue::as_object) {
        for (group, items) in groups {
            let items = items
                .as_array()
                .ok_or_else(|| format!("动作组 {group} 格式无效"))?;
            let mut files = Vec::new();
            for item in items {
                if let Some(file) = item.get("File") {
                    referenced_path(model_dir, resource_root, file, "Motion")?;
                    if let Some(file) = file.as_str() {
                        files.push(file.to_string());
                    }
                }
                if let Some(sound) = item.get("Sound") {
                    if sound.as_str() != Some("") {
                        referenced_path(model_dir, resource_root, sound, "Motion sound")?;
                    }
                }
            }
            motions.insert(group.clone(), files);
        }
    }

    // 声明缺失就退回扫描散装资源。VTS 导出这两段都没有，资源散落在模型目录下——
    // 可能在 `expressions/`，也可能在拼错的目录名里，或干脆平铺在模型目录根。
    let scanned_expressions = expressions.is_empty();
    if scanned_expressions {
        let mut seen = HashSet::new();
        for (name, file) in scan_loose_files(model_dir, EXPRESSION_SUFFIX)? {
            // 引擎按名字查表，重名只有第一个会命中，这里按排序后的路径首次出现者胜
            if seen.insert(name.clone()) {
                expressions.push(name.clone());
                expression_files.insert(name, file);
            }
        }
    }

    if motions.is_empty() {
        let mut used = HashSet::new();
        for (stem, file) in scan_loose_files(model_dir, MOTION_SUFFIX)? {
            // 散装动作没有组名，一个文件自成一组，组名取文件名去掉 .motion3.json。
            // 与 configureRuntimeIdle 往 Motions 里塞合成组是同一个路子。
            let group = unique_asset_name(&stem, &used);
            used.insert(group.clone());
            motions.insert(group, vec![file]);
        }
    }

    Ok(ResolvedAssets {
        expressions,
        expression_files,
        motions,
        scanned_expressions,
    })
}

fn unique_asset_name(base: &str, used: &HashSet<String>) -> String {
    if !used.contains(base) {
        return base.to_string();
    }
    for index in 2.. {
        let candidate = format!("{base}_{index}");
        if !used.contains(&candidate) {
            return candidate;
        }
    }
    unreachable!()
}

/// 待机组名。精确 `Idle` 优先（保持既有行为），再大小写不敏感——扫描出来的散装动作
/// 组名就是文件名，实际就是小写 `idle`——最后认中文「待机」。`sleep` 之类不算待机。
fn is_idle_group_name(group: &str) -> bool {
    group == "Idle" || group.eq_ignore_ascii_case("idle") || group == "待机"
}

/// 扫描出来的表情里哪些可以当默认表情。刻意只认明确的默认名：VTS 导出的表情名是
/// 「脸红」「墨镜」这类功能名，没有约定俗成的默认项，按字母序取第一个会凭空给角色
/// 换脸——DeepSeek 会取到 `love`，SailorDoggy 会取到 `blush`。
fn is_default_expression_name(name: &str) -> bool {
    name == "00_Default"
        || name.to_ascii_lowercase() == "default"
        || name == "默认"
        || name == "正常"
}

fn inspect_model(
    model_file: &Path,
    resource_root: &Path,
    role_root: &Path,
    variant: String,
) -> Result<(Live2dModelInfo, Live2dVariant), String> {
    let (refs, model_dir) = read_model_references(model_file)?;
    let model_dir = model_dir.as_path();

    referenced_path(
        model_dir,
        resource_root,
        refs.get("Moc").unwrap_or(&JsonValue::Null),
        "Moc",
    )?;
    if let Some(textures) = refs.get("Textures").and_then(JsonValue::as_array) {
        for texture in textures {
            referenced_path(model_dir, resource_root, texture, "Texture")?;
        }
    }
    for key in ["Physics", "Pose", "UserData", "DisplayInfo"] {
        if let Some(reference) = refs.get(key) {
            referenced_path(model_dir, resource_root, reference, key)?;
        }
    }

    let ResolvedAssets {
        expressions,
        expression_files: _,
        motions,
        scanned_expressions,
    } = resolve_assets(&refs, model_dir, resource_root)?;

    let relative = model_file
        .strip_prefix(role_root)
        .map_err(|_| "model3 不在角色目录内".to_string())?
        .to_string_lossy()
        .replace('\\', "/");
    let idle_group = motions
        .keys()
        .find(|group| group.as_str() == "Idle")
        .or_else(|| {
            motions
                .keys()
                .find(|group| is_idle_group_name(group.as_str()))
        })
        .filter(|group| motions.get(*group).is_some_and(|files| !files.is_empty()))
        .cloned();
    let idle = idle_group.map(|group| Live2dMotionBinding {
        group,
        index: 0,
        loop_motion: true,
        extra: HashMap::new(),
    });
    let default_expression = if scanned_expressions {
        expressions
            .iter()
            .find(|name| is_default_expression_name(name.as_str()))
            .cloned()
    } else {
        expressions
            .iter()
            .find(|name| name.as_str() == "00_Default")
            .cloned()
            .or_else(|| expressions.first().cloned())
    };
    // 自动绑定只是给个起点，用户可以在设置界面改。中文关键字是给 VTS 式散装资源用的：
    // 那些模型的表情名是「脸红」「星星眼」这类中文功能名，纯英文关键字一条都匹配不上。
    let mut expression_bindings = HashMap::new();
    for (emotion, keywords) in [
        ("正常", &["default", "normal", "正常", "默认"][..]),
        ("平静", &["calm", "default", "normal", "平静", "淡定"]),
        ("高兴", &["happy", "smile", "高兴", "开心", "笑"]),
        ("兴奋", &["kira", "waku", "happy", "兴奋", "星星眼"]),
        ("生气", &["angry", "生气", "怒"]),
        ("害羞", &["shy", "blush", "害羞", "脸红", "羞"]),
        ("疑惑", &["doubt", "ask", "疑惑", "疑问", "问号"]),
        ("哭泣", &["tear", "sad", "cry", "哭", "泪", "悲伤"]),
        ("惊讶", &["surpris", "惊讶", "震惊", "感叹号"]),
        ("厌恶", &["disgust", "厌恶", "嫌弃", "反感"]),
        ("担心", &["troubled", "worry", "担心", "忧", "流汗"]),
        ("认真", &["serious", "认真", "正经"]),
        ("紧张", &["nervous", "紧张", "冷汗"]),
        ("害怕", &["scared", "fear", "害怕", "恐惧"]),
        ("慌张", &["panic", "慌张", "慌乱"]),
        ("无奈", &["speechless", "无奈", "叹气", "无语"]),
        ("心动", &["heart", "love", "心动", "心跳", "爱心"]),
        ("调皮", &["playful", "调皮", "吐舌", "恶作剧"]),
        ("难为情", &["embarrass", "难为情", "尴尬"]),
        ("自信", &["confident", "自信", "得意"]),
    ] {
        if let Some(name) = expressions.iter().find(|name| {
            let lower = name.to_ascii_lowercase();
            keywords.iter().any(|keyword| lower.contains(keyword))
        }) {
            expression_bindings.insert(emotion.to_string(), name.clone());
        }
    }
    // 组名顺序必须固定：命中是「先到先得」，跟着 HashMap 的随机顺序走会让导入期
    // 写进 settings.yml 的绑定每次都不一样
    let mut motion_group_names: Vec<String> = motions.keys().cloned().collect();
    motion_group_names.sort();
    let mut motion_bindings = HashMap::new();
    for (emotion, keywords) in [
        ("高兴", &["waku", "happy", "高兴", "开心", "笑"] as &[&str]),
        ("兴奋", &["waku", "happy", "兴奋", "欢呼"]),
        ("生气", &["angry", "生气", "怒"]),
        ("疑惑", &["doubt", "疑惑", "疑问"]),
        ("担心", &["troubled", "担心", "忧"]),
        ("晕", &["dizzy", "晕"]),
    ] {
        'groups: for group in &motion_group_names {
            // 待机组和背景组不参与情绪反应，否则待机动作会被某个情绪抢走。
            // 扫描出来的组名是小写 idle 或中文「待机」，跳过判断必须大小写不敏感
            if is_idle_group_name(group) || group == "Background" {
                continue;
            }
            let files = &motions[group];
            if files.is_empty() {
                continue;
            }
            let group_lower = group.to_ascii_lowercase();
            // 散装动作的组名就是文件名，本身即语义名；先按文件路径找，找不到再退回
            // 「组名命中就用该组第一个动作」，否则文件名不透明的包（m01.motion3.json）永远绑不上
            let index = files
                .iter()
                .position(|file| {
                    let lower = file.to_ascii_lowercase();
                    keywords.iter().any(|keyword| lower.contains(keyword))
                })
                .or_else(|| {
                    keywords
                        .iter()
                        .any(|keyword| group_lower.contains(keyword))
                        .then_some(0)
                });
            if let Some(index) = index {
                motion_bindings.insert(
                    emotion.to_string(),
                    Live2dMotionBinding {
                        group: group.clone(),
                        index,
                        loop_motion: false,
                        extra: HashMap::new(),
                    },
                );
                break 'groups;
            }
        }
    }
    let variant_settings = Live2dVariant {
        model: relative.clone(),
        default_expression,
        expressions: expression_bindings,
        motions: motion_bindings,
        idle,
        eye_blink: Some(Live2dEyeBlinkBinding {
            left: "ParamEyeLOpen".to_string(),
            right: "ParamEyeROpen".to_string(),
            extra: HashMap::new(),
        }),
        focus_anchor: None,
        lip_sync: Some(Live2dParameterBinding {
            parameter: "ParamMouthOpenY".to_string(),
            gain: 1.0,
            extra: HashMap::new(),
        }),
        extra: HashMap::new(),
    };
    Ok((
        Live2dModelInfo {
            variant,
            model: relative,
            expressions,
            motions,
        },
        variant_settings,
    ))
}

fn unique_variant_name(model_file: &Path, existing: &HashMap<String, Live2dVariant>) -> String {
    let base = model_file
        .file_stem()
        .and_then(|name| name.to_str())
        .unwrap_or("model")
        .trim_end_matches(".model3")
        .to_string();
    if !existing.contains_key(&base) {
        return base;
    }
    for index in 2.. {
        let candidate = format!("{base}_{index}");
        if !existing.contains_key(&candidate) {
            return candidate;
        }
    }
    unreachable!()
}

fn validate_motion_binding(
    variant_name: &str,
    label: &str,
    binding: &Live2dMotionBinding,
    info: &Live2dModelInfo,
) -> Result<(), String> {
    let files = info.motions.get(&binding.group).ok_or_else(|| {
        format!(
            "variant {variant_name} 的 {label} 引用了不存在的动作组 {}",
            binding.group
        )
    })?;
    if binding.index >= files.len() {
        return Err(format!(
            "variant {variant_name} 的 {label} 动作索引 {} 越界（组 {} 共 {} 个）",
            binding.index,
            binding.group,
            files.len()
        ));
    }
    Ok(())
}

fn validate_variant_bindings(
    variant_name: &str,
    variant: &Live2dVariant,
    info: &Live2dModelInfo,
) -> Result<(), String> {
    if let Some(anchor) = &variant.focus_anchor {
        if !anchor.x.is_finite()
            || !anchor.y.is_finite()
            || !(0.0..=1.0).contains(&anchor.x)
            || !(0.0..=1.0).contains(&anchor.y)
        {
            return Err(format!(
                "variant {variant_name} 的 focus_anchor x/y 必须是 0 到 1 之间的有限数值"
            ));
        }
    }
    if let Some(expression) = &variant.default_expression {
        if !info.expressions.contains(expression) {
            return Err(format!(
                "variant {variant_name} 的默认表情不存在: {expression}"
            ));
        }
    }
    for (emotion, expression) in &variant.expressions {
        if !info.expressions.contains(expression) {
            return Err(format!(
                "variant {variant_name} 的情绪 {emotion} 引用了不存在的表情: {expression}"
            ));
        }
    }
    if let Some(idle) = &variant.idle {
        validate_motion_binding(variant_name, "idle", idle, info)?;
    }
    for (emotion, motion) in &variant.motions {
        validate_motion_binding(variant_name, &format!("情绪 {emotion}"), motion, info)?;
    }
    Ok(())
}

#[tauri::command]
pub async fn import_live2d(
    app: AppHandle,
    role_id: i32,
    source_path: String,
    source_kind: Live2dSourceKind,
) -> Result<Live2dImportResult, String> {
    let state = app.state::<AppState>();
    let role = RoleRepo::get_role_by_id(&state.db, role_id)
        .await
        .map_err(|e| format!("查询角色失败: {e}"))?
        .ok_or_else(|| format!("角色 {role_id} 不存在"))?;
    let folder = role
        .resource_folder
        .as_deref()
        .ok_or_else(|| "角色资源目录不存在".to_string())?;
    let root = role_dir(&role.role_type, folder, role.script_key.as_deref())?;
    let source = PathBuf::from(source_path);
    if !source.exists() {
        return Err("Live2D 来源不存在".to_string());
    }
    let canonical_source = source
        .canonicalize()
        .map_err(|e| format!("解析 Live2D 来源失败: {e}"))?;
    let canonical_root = root
        .canonicalize()
        .map_err(|e| format!("解析角色目录失败: {e}"))?;
    if canonical_root.starts_with(&canonical_source) {
        return Err("不能从包含当前角色目录的上级目录导入 Live2D".to_string());
    }

    let nonce = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_err(|e| e.to_string())?
        .as_millis();
    let staging = root.join(format!(".live2d-staging-{nonce}"));
    fs::create_dir_all(&staging).map_err(|e| format!("创建临时目录失败: {e}"))?;

    let import_result = (|| -> Result<(), String> {
        match source_kind {
            Live2dSourceKind::Directory => {
                if !source.is_dir() {
                    return Err("选择的 Live2D 来源不是目录".to_string());
                }
                copy_directory(&source, &staging)
            },
            Live2dSourceKind::Zip => {
                if !source.is_file() {
                    return Err("选择的 Live2D 来源不是 ZIP 文件".to_string());
                }
                extract_zip(&source, &staging, &CancellationToken::new(), &|_| {})
                    .map(|_| ())
                    .map_err(|e| e.to_string())
            },
        }
    })();
    if let Err(error) = import_result {
        let _ = fs::remove_dir_all(&staging);
        return Err(error);
    }

    let mut model_files = Vec::new();
    if let Err(error) = collect_model_files(&staging, &mut model_files) {
        let _ = fs::remove_dir_all(&staging);
        return Err(error);
    }
    model_files.sort();
    if model_files.is_empty() {
        let _ = fs::remove_dir_all(&staging);
        return Err("未找到 .model3.json".to_string());
    }

    let manifest_relative = match find_import_manifest(&staging) {
        Ok(manifest) => manifest
            .map(|path| path.strip_prefix(&staging).map(Path::to_path_buf))
            .transpose()
            .map_err(|e| e.to_string())?,
        Err(error) => {
            let _ = fs::remove_dir_all(&staging);
            return Err(error);
        },
    };
    let target = root.join("live2d").join(format!("import-{nonce}"));
    if let Err(error) = fs::create_dir_all(target.parent().unwrap()) {
        let _ = fs::remove_dir_all(&staging);
        return Err(error.to_string());
    }
    if let Err(error) = fs::rename(&staging, &target) {
        let _ = fs::remove_dir_all(&staging);
        return Err(format!("保存 Live2D 资源失败: {error}"));
    }
    let canonical_target = match target.canonicalize() {
        Ok(path) => path,
        Err(error) => {
            let _ = fs::remove_dir_all(&target);
            return Err(format!("解析 Live2D 导入目录失败: {error}"));
        },
    };

    let (live2d, models) = if let Some(manifest_relative) = manifest_relative {
        let manifest_path = target.join(manifest_relative);
        let manifest_dir = manifest_path
            .parent()
            .ok_or_else(|| "Live2D 导入清单路径无效".to_string())?;
        let configuration_result =
            (|| -> Result<(Live2dSettings, Vec<Live2dModelInfo>), String> {
                let raw = fs::read_to_string(&manifest_path)
                    .map_err(|e| format!("读取 lingchat-live2d.json 失败: {e}"))?;
                let mut configured: Live2dSettings = serde_json::from_str(&raw)
                    .map_err(|e| format!("解析 lingchat-live2d.json 失败: {e}"))?;
                if configured.version != 1
                    || !configured
                        .variants
                        .contains_key(&configured.default_variant)
                {
                    return Err(
                        "lingchat-live2d.json 必须是 version 1 且包含 default_variant".to_string(),
                    );
                }
                let mut inspected = Vec::new();
                for (variant_name, variant) in &mut configured.variants {
                    validate_relative_path(
                        &variant.model,
                        &format!("variant {variant_name} 的模型路径"),
                    )?;
                    let source_model = manifest_dir
                        .join(&variant.model)
                        .canonicalize()
                        .map_err(|e| format!("解析导入清单模型路径失败: {e}"))?;
                    source_model
                        .strip_prefix(&canonical_target)
                        .map_err(|_| "导入清单中的模型必须位于本次导入目录内".to_string())?;
                    let relative = source_model
                        .strip_prefix(&canonical_root)
                        .map_err(|_| "导入清单中的模型路径无效".to_string())?
                        .to_string_lossy()
                        .replace('\\', "/");
                    variant.model = relative;
                    let (info, _) = inspect_model(
                        &source_model,
                        &canonical_target,
                        &root,
                        variant_name.clone(),
                    )?;
                    validate_variant_bindings(variant_name, variant, &info)?;
                    inspected.push(info);
                }
                for (clothes, variant_name) in &configured.clothes_variants {
                    if !configured.variants.contains_key(variant_name) {
                        return Err(format!(
                            "服装 {clothes} 映射到不存在的 variant: {variant_name}"
                        ));
                    }
                }
                inspected.sort_by(|left, right| left.variant.cmp(&right.variant));
                Ok((configured, inspected))
            })();
        match configuration_result {
            Ok(result) => result,
            Err(error) => {
                let _ = fs::remove_dir_all(&target);
                return Err(error);
            },
        }
    } else {
        let generation_result = (|| -> Result<(Live2dSettings, Vec<Live2dModelInfo>), String> {
            let mut variants = HashMap::new();
            let mut inspected = Vec::new();
            for staged_model in model_files {
                let relative_in_staging = staged_model
                    .strip_prefix(&staging)
                    .map_err(|e| e.to_string())?;
                let model_file = target.join(relative_in_staging);
                let variant_name = unique_variant_name(&model_file, &variants);
                let (info, variant) =
                    inspect_model(&model_file, &canonical_target, &root, variant_name.clone())?;
                variants.insert(variant_name, variant);
                inspected.push(info);
            }
            let default_variant = inspected[0].variant.clone();
            Ok((
                Live2dSettings {
                    version: 1,
                    default_variant: default_variant.clone(),
                    variants,
                    clothes_variants: HashMap::from([("default".to_string(), default_variant)]),
                    extra: HashMap::new(),
                },
                inspected,
            ))
        })();
        match generation_result {
            Ok(result) => result,
            Err(error) => {
                let _ = fs::remove_dir_all(&target);
                return Err(error);
            },
        }
    };

    let mut settings =
        match RoleRepo::get_role_settings_by_id(&state.db, &super::data_dir(), role_id).await {
            Ok(settings) => settings.unwrap_or_else(CharacterSettings::default),
            Err(error) => {
                let _ = fs::remove_dir_all(&target);
                return Err(format!("读取角色配置失败: {error}"));
            },
        };
    settings.live2d = Some(live2d.clone());
    let mut value = match serde_json::to_value(&settings) {
        Ok(value) => value,
        Err(error) => {
            let _ = fs::remove_dir_all(&target);
            return Err(error.to_string());
        },
    };
    if let Some(object) = value.as_object_mut() {
        for transient in [
            "character_id",
            "resource_path",
            "character_folder",
            "script_key",
            "script_role_key",
        ] {
            object.remove(transient);
        }
    }
    if let Err(error) = write_json_as_yaml(&root.join("settings.yml"), &value) {
        let _ = fs::remove_dir_all(&target);
        return Err(format!("保存 Live2D 配置失败: {error}"));
    }

    {
        let service = state.ai_service.lock().await;
        let mut game_status = service.game_status.lock().await;
        game_status
            .role_manager
            .update_role_live2d_settings(role_id, &settings);
    }

    Ok(Live2dImportResult { live2d, models })
}

#[tauri::command]
pub async fn get_live2d_file(
    app: AppHandle,
    role_id: i32,
    file_path: String,
) -> Result<String, String> {
    let state = app.state::<AppState>();
    let role = RoleRepo::get_role_by_id(&state.db, role_id)
        .await
        .map_err(|e| format!("查询角色失败: {e}"))?
        .ok_or_else(|| format!("角色 {role_id} 不存在"))?;
    let folder = role
        .resource_folder
        .as_deref()
        .ok_or_else(|| "角色资源目录不存在".to_string())?;
    let root = role_dir(&role.role_type, folder, role.script_key.as_deref())?;
    let resolved = root.join(file_path);
    crate::utils::path::validate_path_in_base(&resolved, &root)?;
    if !resolved.is_file() {
        return Err("Live2D 文件不存在".to_string());
    }
    resolved
        .canonicalize()
        .map(|path| path.to_string_lossy().into_owned())
        .map_err(|e| format!("解析 Live2D 文件路径失败: {e}"))
}

#[tauri::command]
pub async fn inspect_live2d(app: AppHandle, role_id: i32) -> Result<Live2dImportResult, String> {
    let state = app.state::<AppState>();
    let role = RoleRepo::get_role_by_id(&state.db, role_id)
        .await
        .map_err(|e| format!("查询角色失败: {e}"))?
        .ok_or_else(|| format!("角色 {role_id} 不存在"))?;
    let folder = role
        .resource_folder
        .as_deref()
        .ok_or_else(|| "角色资源目录不存在".to_string())?;
    let root = role_dir(&role.role_type, folder, role.script_key.as_deref())?;
    let settings = RoleRepo::get_role_settings_by_id(&state.db, &super::data_dir(), role_id)
        .await
        .map_err(|e| format!("读取角色配置失败: {e}"))?
        .ok_or_else(|| "角色配置不存在".to_string())?;
    let live2d = settings
        .live2d
        .ok_or_else(|| "角色未配置 Live2D".to_string())?;
    let mut models = Vec::new();
    for (variant_name, variant) in &live2d.variants {
        let model_file = root.join(&variant.model);
        let (info, _) = inspect_model(&model_file, &root, &root, variant_name.clone())?;
        models.push(info);
    }
    models.sort_by(|left, right| left.variant.cmp(&right.variant));
    Ok(Live2dImportResult { live2d, models })
}

/// 取某个 variant 的可用资源表，供运行时在加载模型前把声明注入 model3.json。
///
/// 与 `inspect_live2d` 分开：那个是给设置界面渲染下拉框用的（每次开设置页跑一次、
/// 覆盖全部 variant），这个是给渲染路径用的（每次加载模型跑一次、只要一个 variant）。
#[tauri::command]
pub async fn get_live2d_variant_assets(
    app: AppHandle,
    role_id: i32,
    variant_name: String,
) -> Result<Live2dVariantAssets, String> {
    let state = app.state::<AppState>();
    let role = RoleRepo::get_role_by_id(&state.db, role_id)
        .await
        .map_err(|e| format!("查询角色失败: {e}"))?
        .ok_or_else(|| format!("角色 {role_id} 不存在"))?;
    let folder = role
        .resource_folder
        .as_deref()
        .ok_or_else(|| "角色资源目录不存在".to_string())?;
    let root = role_dir(&role.role_type, folder, role.script_key.as_deref())?;
    let settings = RoleRepo::get_role_settings_by_id(&state.db, &super::data_dir(), role_id)
        .await
        .map_err(|e| format!("读取角色配置失败: {e}"))?
        .ok_or_else(|| "角色配置不存在".to_string())?;
    let live2d = settings
        .live2d
        .ok_or_else(|| "角色未配置 Live2D".to_string())?;
    let variant = live2d
        .variants
        .get(&variant_name)
        .ok_or_else(|| format!("variant {variant_name} 不存在"))?;
    let model_file = root.join(&variant.model);
    // variant.model 来自用户可编辑的 settings.yml，必须挡住越界路径
    crate::utils::path::validate_path_in_base(&model_file, &root)?;
    let (refs, model_dir) = read_model_references(&model_file)?;
    let assets = resolve_assets(&refs, &model_dir, &root)?;
    Ok(Live2dVariantAssets {
        expressions: assets.expression_files,
        motions: assets.motions,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    /// 造一个最小可解析的 model3.json。`referenced_path` 强制要求 Moc 指向真实存在的
    /// 文件，所以每个夹具都得配一个真的 .moc3 兄弟文件。
    fn write_model(dir: &Path, extra_references: &str) -> PathBuf {
        fs::create_dir_all(dir).unwrap();
        fs::write(dir.join("model.moc3"), b"moc").unwrap();
        let path = dir.join("model.model3.json");
        fs::write(
            &path,
            format!(r#"{{"Version":3,"FileReferences":{{"Moc":"model.moc3"{extra_references}}}}}"#),
        )
        .unwrap();
        path
    }

    fn write_asset(path: &Path) {
        fs::create_dir_all(path.parent().unwrap()).unwrap();
        fs::write(path, b"{}").unwrap();
    }

    /// 夹具里 role_root 与 resource_root 都取 tempdir 根，模型就放在根下
    fn inspect(root: &Path, model_file: &Path) -> (Live2dModelInfo, Live2dVariant) {
        inspect_model(model_file, root, root, "test".to_string()).unwrap()
    }

    fn sorted_names(values: impl IntoIterator<Item = String>) -> Vec<String> {
        let mut names: Vec<String> = values.into_iter().collect();
        names.sort();
        names
    }

    #[test]
    fn scans_loose_expressions_from_nested_typo_and_flat_directories() {
        let temp = tempfile::tempdir().unwrap();
        let root = temp.path();
        let model = write_model(root, "");
        write_asset(&root.join("expressions/脸红.exp3.json"));
        // VTS 包真实见过拼错的目录名，扫描认的是后缀不是目录名
        write_asset(&root.join("experssions/生气.exp3.json"));
        // 也有把 exp3 直接平铺在模型目录根的包
        write_asset(&root.join("调皮.exp3.json"));

        let (info, _) = inspect(root, &model);

        // 顺序 = 按模型目录相对路径排序：experssions < expressions < 中文
        assert_eq!(info.expressions, vec!["生气", "脸红", "调皮"]);
    }

    #[test]
    fn scans_loose_motions_as_one_group_per_file_named_by_stem() {
        let temp = tempfile::tempdir().unwrap();
        let root = temp.path();
        let model = write_model(root, "");
        write_asset(&root.join("motions/idle.motion3.json"));
        write_asset(&root.join("motions/喷水.motion3.json"));
        write_asset(&root.join("aidale.motion3.json"));

        let (info, variant) = inspect(root, &model);

        assert_eq!(
            sorted_names(info.motions.keys().cloned()),
            ["aidale", "idle", "喷水"]
        );
        assert_eq!(info.motions["idle"], vec!["motions/idle.motion3.json"]);
        assert_eq!(info.motions["aidale"], vec!["aidale.motion3.json"]);
        // 散装组名是小写 idle，待机推导必须认
        assert_eq!(
            variant.idle.map(|idle| idle.group),
            Some("idle".to_string())
        );
    }

    #[test]
    fn sleep_is_not_treated_as_an_idle_group() {
        let temp = tempfile::tempdir().unwrap();
        let root = temp.path();
        let model = write_model(root, "");
        write_asset(&root.join("motions/sleep.motion3.json"));

        let (_, variant) = inspect(root, &model);

        assert_eq!(variant.idle, None);
    }

    #[test]
    fn declared_references_win_over_loose_scan() {
        let temp = tempfile::tempdir().unwrap();
        let root = temp.path();
        let model = write_model(
            root,
            concat!(
                r#","Expressions":[{"Name":"Happy","File":"expressions/happy.exp3.json"}],"#,
                r#""Motions":{"Idle":[{"File":"motions/idle.motion3.json"}]}"#,
            ),
        );
        write_asset(&root.join("expressions/happy.exp3.json"));
        write_asset(&root.join("motions/idle.motion3.json"));
        // 已声明时不该把散装文件并进来
        write_asset(&root.join("expressions/Decoy.exp3.json"));

        let (info, variant) = inspect(root, &model);

        assert_eq!(info.expressions, vec!["Happy"]);
        assert_eq!(sorted_names(info.motions.keys().cloned()), ["Idle"]);
        // 声明分支保持既有行为：没有 00_Default 时取第一个
        assert_eq!(variant.default_expression.as_deref(), Some("Happy"));
        assert_eq!(
            variant.idle.map(|idle| idle.group),
            Some("Idle".to_string())
        );
    }

    #[test]
    fn scanned_lists_do_not_invent_a_default_expression() {
        let temp = tempfile::tempdir().unwrap();
        let root = temp.path();
        let model = write_model(root, "");
        // 按字母序取第一个会得到 blush / love，等于凭空给角色换脸
        write_asset(&root.join("expressions/blush.exp3.json"));
        write_asset(&root.join("expressions/love.exp3.json"));

        let (_, variant) = inspect(root, &model);

        assert_eq!(variant.default_expression, None);
    }

    #[test]
    fn scanned_default_expression_is_used_when_explicitly_named() {
        let temp = tempfile::tempdir().unwrap();
        let root = temp.path();
        let model = write_model(root, "");
        write_asset(&root.join("expressions/脸红.exp3.json"));
        write_asset(&root.join("expressions/默认.exp3.json"));

        let (_, variant) = inspect(root, &model);

        assert_eq!(variant.default_expression.as_deref(), Some("默认"));
    }

    #[test]
    fn chinese_keywords_bind_scanned_expression_names() {
        let temp = tempfile::tempdir().unwrap();
        let root = temp.path();
        let model = write_model(root, "");
        write_asset(&root.join("expressions/脸红.exp3.json"));
        write_asset(&root.join("expressions/生气.exp3.json"));
        write_asset(&root.join("expressions/星星眼.exp3.json"));

        let (_, variant) = inspect(root, &model);

        assert_eq!(
            variant.expressions.get("害羞").map(String::as_str),
            Some("脸红")
        );
        assert_eq!(
            variant.expressions.get("生气").map(String::as_str),
            Some("生气")
        );
        assert_eq!(
            variant.expressions.get("兴奋").map(String::as_str),
            Some("星星眼")
        );
    }

    #[test]
    fn declared_motion_groups_can_bind_by_group_name_when_files_are_opaque() {
        let temp = tempfile::tempdir().unwrap();
        let root = temp.path();
        let model = write_model(
            root,
            concat!(
                r#","Motions":{"Idle":[{"File":"motions/idle.motion3.json"}],"#,
                r#""Happy":[{"File":"motions/m01.motion3.json"}]}"#,
            ),
        );
        write_asset(&root.join("motions/idle.motion3.json"));
        write_asset(&root.join("motions/m01.motion3.json"));

        let (_, variant) = inspect(root, &model);

        // 文件名不透明时按组名命中，取该组第一个动作
        let happy = variant.motions.get("高兴").unwrap();
        assert_eq!(happy.group, "Happy");
        assert_eq!(happy.index, 0);
        assert_eq!(
            variant.idle.map(|idle| idle.group),
            Some("Idle".to_string())
        );
    }

    #[test]
    fn duplicate_motion_stems_are_uniquified_in_sorted_path_order() {
        let temp = tempfile::tempdir().unwrap();
        let root = temp.path();
        let model = write_model(root, "");
        write_asset(&root.join("motions/idle.motion3.json"));
        write_asset(&root.join("motions/extra/idle.motion3.json"));

        let (info, _) = inspect(root, &model);

        // 排序后 "motions/extra/..." 在前，先到者拿到 "idle"
        assert_eq!(
            info.motions["idle"],
            vec!["motions/extra/idle.motion3.json"]
        );
        assert_eq!(info.motions["idle_2"], vec!["motions/idle.motion3.json"]);
    }
}
