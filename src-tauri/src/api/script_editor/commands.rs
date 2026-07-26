//! 剧本编辑器的 Tauri 命令。
//!
//! 这一层之前完全不存在 —— `api/script.rs` 只有 5 个只读命令，剧本从前端视角
//! 是只读的，而 `fs` 插件的 scope 也覆盖不到剧本目录。所有写入都必须走这里。
//!
//! 命名统一 `editor_` 前缀，避免与既有的 `list_scripts` 等混淆。

use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};
use serde_json::{Map, Value as JsonValue};
use tauri::{AppHandle, Manager};

use crate::ai_service::game_system::game_status::GameStatus;
use crate::ai_service::types::ScriptStatus;
use crate::api::{data_dir, game_data_dir};
use crate::db::managers::role_repo::RoleRepo;
use crate::AppState;

use sea_orm::DatabaseConnection;
use std::sync::Arc;
use tokio::sync::Mutex;

use super::io::{self, ChapterDoc};
use super::paths::{self, ScriptLayout};
use super::schema::{build_schema, ScriptSchema};
use super::validate::{self, ValidationReport};

// ============================================================
// 返回类型
// ============================================================

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ScriptPackage {
    /// 相对 scripts/ 的 key，用 / 分隔
    pub key: String,
    pub layout: ScriptLayout,
    /// 叶子目录名（羁绊冒险的 folder_key 就是它）
    pub folder_name: String,
    /// character/<角色>/ 布局下的角色目录名
    #[serde(skip_serializing_if = "Option::is_none")]
    pub bound_character_folder: Option<String>,
    pub script_name: String,
    pub description: String,
    pub is_adventure: bool,
    pub chapter_count: usize,
    /// 该剧本是否已被引擎加载（未加载表示需要重启或 rescan）
    pub loaded_by_engine: bool,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ChapterSummary {
    /// 相对 Chapters/ 的 id，不含扩展名，用 / 分隔
    pub id: String,
    /// story 里的显示名，缺省时为 None（引擎会回落成 id）
    #[serde(skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
    /// 子目录（用于流程图分组），顶层章节为 None
    #[serde(skip_serializing_if = "Option::is_none")]
    pub group: Option<String>,
    pub event_count: usize,
}

#[derive(Debug, Clone, Default, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AssetIndex {
    pub background: Vec<String>,
    pub music: Vec<String>,
    pub sound: Vec<String>,
    pub ambient: Vec<String>,
    pub pic: Vec<String>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ScriptCharacter {
    /// 目录名
    pub folder: String,
    /// 剧本里 `character:` 应该写的值（settings.yml 的 script_role_key，缺省为目录名）
    pub role_key: String,
    pub ai_name: String,
    /// avatar/ 下能找到的情绪名（不含扩展名）
    pub emotions: Vec<String>,
    /// avatar/ 下的服装子目录
    pub clothes: Vec<String>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ScriptDetail {
    pub package: ScriptPackage,
    /// story_config.yaml 原样转成的 JSON
    pub story_config: JsonValue,
    pub chapters: Vec<ChapterSummary>,
    pub assets: AssetIndex,
    pub characters: Vec<ScriptCharacter>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ChapterContent {
    pub id: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
    pub events: Vec<JsonValue>,
    /// 除 name / events 之外的顶层键，写回时原样保留
    pub extra: Map<String, JsonValue>,
}

// ============================================================
// 请求类型
// ============================================================

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CreateScriptRequest {
    /// 剧本目录名
    pub folder_name: String,
    /// 显示名，留空则用目录名
    #[serde(default)]
    pub script_name: String,
    #[serde(default)]
    pub description: String,
    /// 开场章节 id，留空默认 "main"
    #[serde(default)]
    pub intro_chapter: String,
    /// 是否建成羁绊冒险；true 时必须给 bound_character_folder
    #[serde(default)]
    pub is_adventure: bool,
    #[serde(default)]
    pub bound_character_folder: String,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct WriteChapterRequest {
    pub key: String,
    pub chapter_id: String,
    #[serde(default)]
    pub name: Option<String>,
    pub events: Vec<JsonValue>,
    #[serde(default)]
    pub extra: Map<String, JsonValue>,
}

// ============================================================
// 内部辅助
// ============================================================

fn read_package(key: &str, loaded_names: &HashSet<String>) -> Result<ScriptPackage, String> {
    let dir = paths::resolve_script_dir(key)?;
    let layout = paths::layout_of(key)?;
    let config = io::read_story_config(&dir)?;

    let folder_name = dir
        .file_name()
        .map(|s| s.to_string_lossy().to_string())
        .unwrap_or_default();

    let script_name = config
        .get("script_name")
        .and_then(|v| v.as_str())
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty())
        .unwrap_or_else(|| folder_name.clone());

    let adventure = config.get("adventure");
    let is_adventure = adventure
        .and_then(|a| a.get("is_adventure"))
        .and_then(|v| v.as_bool())
        .unwrap_or(false);

    let bound_character_folder = if layout == ScriptLayout::Character {
        key.split('/').nth(1).map(|s| s.to_string())
    } else {
        adventure
            .and_then(|a| a.get("bound_character_folder"))
            .and_then(|v| v.as_str())
            .map(|s| s.to_string())
            .filter(|s| !s.is_empty())
    };

    Ok(ScriptPackage {
        key: key.to_string(),
        layout,
        folder_name,
        bound_character_folder,
        loaded_by_engine: loaded_names.contains(&script_name),
        script_name,
        description: config
            .get("description")
            .and_then(|v| v.as_str())
            .unwrap_or("")
            .to_string(),
        is_adventure,
        chapter_count: paths::enumerate_chapter_ids(&dir).len(),
    })
}

/// 引擎当前内存里已加载的剧本名。
///
/// 引擎只在启动时扫一次目录，所以「磁盘上有」与「引擎能跑」是两件事。
/// 编辑器把这个差异显式暴露出来，而不是让作者困惑于「我明明存了却试玩不了」。
async fn loaded_script_names(app: &AppHandle) -> HashSet<String> {
    let state = app.state::<AppState>();
    let service = state.ai_service.lock().await;
    service.script_manager.all_scripts.keys().cloned().collect()
}

fn list_asset_dir(script_dir: &Path, subdirs: &[&str]) -> Vec<String> {
    let mut out: Vec<String> = Vec::new();
    for sub in subdirs {
        let dir = script_dir.join("Assets").join(sub);
        if let Ok(entries) = std::fs::read_dir(&dir) {
            for e in entries.flatten() {
                if e.file_type().map(|t| t.is_file()).unwrap_or(false) {
                    let name = e.file_name().to_string_lossy().to_string();
                    if !name.starts_with('.') && !out.contains(&name) {
                        out.push(name);
                    }
                }
            }
        }
    }
    out.sort();
    out
}

fn read_asset_index(script_dir: &Path) -> AssetIndex {
    // 子目录候选与 media.rs 的 subdir_candidates 保持一致
    AssetIndex {
        background: list_asset_dir(
            script_dir,
            &["Backgrounds", "Pics", "Pictures", "Pic", "Picture"],
        ),
        music: list_asset_dir(script_dir, &["Musics", "BGMs", "Music", "BGM"]),
        sound: list_asset_dir(
            script_dir,
            &["Sounds", "SoundEffects", "Sound", "SoundEffect"],
        ),
        ambient: list_asset_dir(
            script_dir,
            &["Ambients", "AmbientSounds", "Environment", "Ambient"],
        ),
        pic: list_asset_dir(script_dir, &["Pics", "Pictures", "Pic", "Picture"]),
    }
}

fn read_characters(script_dir: &Path) -> Vec<ScriptCharacter> {
    let mut out = Vec::new();
    let dir = script_dir.join("characters");
    let Ok(entries) = std::fs::read_dir(&dir) else {
        return out;
    };

    for e in entries.flatten() {
        if !e.file_type().map(|t| t.is_dir()).unwrap_or(false) {
            continue;
        }
        let folder = e.file_name().to_string_lossy().to_string();
        let settings: JsonValue = std::fs::read_to_string(e.path().join("settings.yml"))
            .ok()
            .and_then(|s| serde_yaml::from_str(&s).ok())
            .unwrap_or(JsonValue::Null);

        let role_key = settings
            .get("script_role_key")
            .and_then(|v| v.as_str())
            .map(|s| s.trim().to_string())
            .filter(|s| !s.is_empty())
            .unwrap_or_else(|| folder.clone());

        let ai_name = settings
            .get("ai_name")
            .and_then(|v| v.as_str())
            .unwrap_or(&folder)
            .to_string();

        let avatar = e.path().join("avatar");
        let mut emotions: Vec<String> = Vec::new();
        let mut clothes: Vec<String> = Vec::new();
        if let Ok(files) = std::fs::read_dir(&avatar) {
            for f in files.flatten() {
                let name = f.file_name().to_string_lossy().to_string();
                if f.file_type().map(|t| t.is_dir()).unwrap_or(false) {
                    clothes.push(name);
                } else if let Some(stem) = Path::new(&name).file_stem() {
                    emotions.push(stem.to_string_lossy().to_string());
                }
            }
        }
        emotions.sort();
        emotions.dedup();
        clothes.sort();

        out.push(ScriptCharacter {
            folder,
            role_key,
            ai_name,
            emotions,
            clothes,
        });
    }
    out.sort_by(|a, b| a.folder.cmp(&b.folder));
    out
}

fn chapter_summaries(script_dir: &Path) -> Vec<ChapterSummary> {
    paths::enumerate_chapter_ids(script_dir)
        .into_iter()
        .map(|id| {
            let (name, event_count) = match paths::resolve_chapter_file(script_dir, &id, true)
                .and_then(|f| io::read_yaml_as_json(&f))
                .and_then(ChapterDoc::from_json)
            {
                Ok(doc) => (doc.name, doc.events.len()),
                Err(_) => (None, 0),
            };
            let group = if id.contains('/') {
                id.rsplit_once('/').map(|(g, _)| g.to_string())
            } else {
                None
            };
            ChapterSummary {
                id,
                name,
                group,
                event_count,
            }
        })
        .collect()
}

// ============================================================
// 命令：读
// ============================================================

/// 事件 schema。前端的表单与校验全部由它驱动。
#[tauri::command]
pub fn editor_get_schema() -> ScriptSchema {
    build_schema()
}

#[tauri::command]
pub async fn editor_list_scripts(app: AppHandle) -> Result<Vec<ScriptPackage>, String> {
    let loaded = loaded_script_names(&app).await;
    let mut out = Vec::new();
    for key in paths::enumerate_script_keys() {
        match read_package(&key, &loaded) {
            Ok(p) => out.push(p),
            Err(e) => tracing::warn!("[ScriptEditor] 跳过无效剧本 {}: {}", key, e),
        }
    }
    Ok(out)
}

#[tauri::command]
pub async fn editor_read_script(app: AppHandle, key: String) -> Result<ScriptDetail, String> {
    let loaded = loaded_script_names(&app).await;
    let dir = paths::resolve_script_dir(&key)?;
    Ok(ScriptDetail {
        package: read_package(&key, &loaded)?,
        story_config: io::read_story_config(&dir)?,
        chapters: chapter_summaries(&dir),
        assets: read_asset_index(&dir),
        characters: read_characters(&dir),
    })
}

#[tauri::command]
pub fn editor_read_chapter(key: String, chapter_id: String) -> Result<ChapterContent, String> {
    let dir = paths::resolve_script_dir(&key)?;
    let file = paths::resolve_chapter_file(&dir, &chapter_id, true)?;
    let doc = ChapterDoc::from_json(io::read_yaml_as_json(&file)?)?;
    Ok(ChapterContent {
        id: chapter_id,
        name: doc.name,
        events: doc.events,
        extra: doc.extra,
    })
}

#[tauri::command]
pub fn editor_validate_script(key: String) -> Result<ValidationReport, String> {
    let dir = paths::resolve_script_dir(&key)?;

    // 收集其他剧本的 script_name 用于查重
    let mut names: HashMap<String, String> = HashMap::new();
    for other in paths::enumerate_script_keys() {
        if let Ok(d) = paths::resolve_script_dir(&other) {
            if let Ok(cfg) = io::read_story_config(&d) {
                if let Some(n) = cfg.get("script_name").and_then(|v| v.as_str()) {
                    let n = n.trim();
                    if !n.is_empty() {
                        names.insert(n.to_string(), other.clone());
                    }
                }
            }
        }
    }

    Ok(validate::validate(&data_dir(), &dir, &key, &names))
}

// ============================================================
// 命令：写
// ============================================================

#[tauri::command]
pub fn editor_write_chapter(req: WriteChapterRequest) -> Result<(), String> {
    let dir = paths::resolve_script_dir(&req.key)?;
    let file = paths::resolve_chapter_file(&dir, &req.chapter_id, true)?;
    let doc = ChapterDoc {
        name: req.name,
        events: req.events,
        extra: req.extra,
    };
    io::write_json_as_yaml(&file, &doc.to_json())
}

#[tauri::command]
pub fn editor_write_story_config(key: String, config: JsonValue) -> Result<(), String> {
    let dir = paths::resolve_script_dir(&key)?;
    io::write_story_config(&dir, &config)
}

#[tauri::command]
pub fn editor_create_chapter(
    key: String,
    chapter_id: String,
    name: String,
) -> Result<ChapterContent, String> {
    let dir = paths::resolve_script_dir(&key)?;

    // 逐段过 sanitize，子目录也要挡住非法字符
    for seg in chapter_id.split('/') {
        paths::sanitize_folder_name(seg)?;
    }

    let file = paths::resolve_chapter_file(&dir, &chapter_id, false)?;
    if file.exists() {
        return Err(format!("章节已存在: '{}'", chapter_id));
    }
    io::ensure_parent_dir(&file)?;

    let trimmed = name.trim();
    let doc = ChapterDoc {
        name: if trimmed.is_empty() {
            None
        } else {
            Some(trimmed.to_string())
        },
        // 新章节自带一条 chapter_end，否则一保存就是「缺少章节结束」的错误
        events: vec![serde_json::json!({
            "type": "chapter_end",
            "end_type": "linear",
            "next_chapter": "end"
        })],
        extra: Map::new(),
    };
    io::write_json_as_yaml(&file, &doc.to_json())?;

    Ok(ChapterContent {
        id: chapter_id,
        name: doc.name,
        events: doc.events,
        extra: doc.extra,
    })
}

#[tauri::command]
pub fn editor_delete_chapter(key: String, chapter_id: String) -> Result<(), String> {
    let dir = paths::resolve_script_dir(&key)?;
    let file = paths::resolve_chapter_file(&dir, &chapter_id, true)?;

    // 不真删：移到 Chapters/.trash/ 下带时间戳的副本。
    // 章节里可能是作者几个小时的工作量，误删不可逆是不可接受的。
    let trash = dir.join("Chapters").join(".trash");
    std::fs::create_dir_all(&trash).map_err(|e| format!("无法创建回收目录: {}", e))?;

    let stamp = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0);
    let flat = chapter_id.replace('/', "__");
    let dest = trash.join(format!("{}.{}.yaml", flat, stamp));

    std::fs::rename(&file, &dest)
        .or_else(|_| std::fs::copy(&file, &dest).map(|_| ()).and_then(|_| std::fs::remove_file(&file)))
        .map_err(|e| format!("删除章节失败: {}", e))?;
    Ok(())
}

// 这里原本有一个 editor_rename_chapter（改章节**文件名**）。已删除，理由：
//
// 1. 章节 id 会被别的章节的 `chapter_end.next_chapter` / `next` 以及
//    `story_config.yaml` 的 `intro_chapter` 引用。只改文件名不重写引用，
//    等于把作者的剧本悄悄改成断链——这正是校验器要报的 `graph.missing_target`。
// 2. 作者真正想改的是**显示名**，也就是章节 YAML 里的 `name:`。那个已经能在
//    章节编辑页顶部直接改（`setChapterName`），走正常的自动保存。
//
// 换句话说：真实需求已被覆盖，剩下的只是一个会破坏数据的入口。要是以后确实
// 需要改 id，得把所有引用它的 next_chapter / intro_chapter 一起重写。

#[tauri::command]
pub async fn editor_create_script(
    app: AppHandle,
    req: CreateScriptRequest,
) -> Result<ScriptPackage, String> {
    let folder = paths::sanitize_folder_name(&req.folder_name)?;

    let key = if req.is_adventure {
        let bound = paths::sanitize_folder_name(&req.bound_character_folder)
            .map_err(|e| format!("绑定角色目录名无效: {}", e))?;
        format!("character/{}/{}", bound, folder)
    } else {
        format!("standalone/{}", folder)
    };

    // folder_key 在羁绊冒险体系里是全局主键，重名会互相覆盖
    let existing = paths::enumerate_script_keys();
    if existing
        .iter()
        .any(|k| k.rsplit('/').next() == Some(folder.as_str()))
    {
        return Err(format!(
            "已存在同名剧本目录「{}」。羁绊冒险用目录名作全局主键，不能重名",
            folder
        ));
    }

    let script_name = if req.script_name.trim().is_empty() {
        folder.clone()
    } else {
        req.script_name.trim().to_string()
    };

    let intro = {
        let raw = req.intro_chapter.trim();
        let v = if raw.is_empty() { "main" } else { raw };
        for seg in v.split('/') {
            paths::sanitize_folder_name(seg)
                .map_err(|e| format!("开场章节名无效: {}", e))?;
        }
        v.to_string()
    };

    let dir = paths::resolve_new_script_dir(&key)?;

    // 目录骨架。注意 characters 是小写 —— 引擎读的是小写，原型编辑器建的是
    // 大写 Characters，Windows 上侥幸能跑，Linux/Android 上直接断裂。
    for sub in [
        "Chapters",
        "characters",
        "Assets/Backgrounds",
        "Assets/Musics",
        "Assets/Sounds",
        "Assets/Ambients",
        "Assets/Pics",
    ] {
        let mut p = dir.clone();
        for seg in sub.split('/') {
            p.push(seg);
        }
        std::fs::create_dir_all(&p).map_err(|e| format!("创建目录 {:?} 失败: {}", p, e))?;
    }

    // story_config.yaml
    let mut cfg = Map::new();
    cfg.insert("script_name".into(), JsonValue::String(script_name));
    cfg.insert("intro_chapter".into(), JsonValue::String(intro.clone()));
    cfg.insert(
        "description".into(),
        JsonValue::String(req.description.trim().to_string()),
    );
    cfg.insert("recommand_start".into(), JsonValue::String(String::new()));
    if req.is_adventure {
        let mut adv = Map::new();
        adv.insert("is_adventure".into(), JsonValue::Bool(true));
        adv.insert(
            "bound_character_folder".into(),
            JsonValue::String(req.bound_character_folder.trim().to_string()),
        );
        adv.insert("order".into(), JsonValue::Number(0.into()));
        adv.insert("unlock_conditions".into(), JsonValue::Array(Vec::new()));
        cfg.insert("adventure".into(), JsonValue::Object(adv));
    }
    let mut settings = Map::new();
    settings.insert("user_name".into(), JsonValue::String(String::new()));
    cfg.insert("script_settings".into(), JsonValue::Object(settings));

    io::write_story_config(&dir, &JsonValue::Object(cfg))?;

    // 开场章节
    let intro_file = paths::resolve_chapter_file(&dir, &intro, false)?;
    io::ensure_parent_dir(&intro_file)?;
    let first = ChapterDoc {
        name: Some("第一章".to_string()),
        events: vec![
            serde_json::json!({ "type": "narration", "text": "在这里写下第一句旁白。" }),
            serde_json::json!({
                "type": "chapter_end",
                "end_type": "linear",
                "next_chapter": "end"
            }),
        ],
        extra: Map::new(),
    };
    io::write_json_as_yaml(&intro_file, &first.to_json())?;

    let loaded = loaded_script_names(&app).await;
    read_package(&key, &loaded)
}

#[tauri::command]
pub fn editor_delete_script(key: String) -> Result<(), String> {
    let dir = paths::resolve_script_dir(&key)?;

    // 同样不真删：整包移到 game_data/.script_trash/
    let trash = game_data_dir().join(".script_trash");
    std::fs::create_dir_all(&trash).map_err(|e| format!("无法创建回收目录: {}", e))?;

    let stamp = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0);
    let dest = trash.join(format!("{}.{}", key.replace('/', "__"), stamp));

    // 与 editor_delete_chapter 保持一致：rename 跨设备会失败，退回「复制目录树 + 删原目录」
    if std::fs::rename(&dir, &dest).is_err() {
        copy_dir_recursive(&dir, &dest)
            .map_err(|e| format!("复制剧本到回收目录失败: {}。剧本仍在原处", e))?;
        std::fs::remove_dir_all(&dir)
            .map_err(|e| format!("剧本已复制到回收目录，但删除原目录失败: {}", e))?;
    }
    Ok(())
}

/// 素材落点。
///
/// 引擎的查找顺序是「先本剧本 `Assets/`，再全局 `game_data/`」
/// （见 `media.rs::resolve_script_media`），所以两种落点都能被找到，区别是：
/// - `script`：只有这个剧本用，随剧本一起分发，别的剧本看不到
/// - `global`：所有剧本共享，但导出剧本时不会带走
#[derive(Debug, Clone, Copy, PartialEq, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AssetScope {
    Script,
    Global,
}

/// 素材类别 → 剧本内子目录 / 全局目录。
///
/// 剧本内一律落在 `media.rs` 候选列表的**第一个**目录，保证引擎一定能找到；
/// 全局目录直接用 `MediaType::fallback_dir()` 的同一套值，避免又写一份会发散的映射。
fn asset_dirs(kind: &str) -> Result<(&'static str, PathBuf), String> {
    use crate::ai_service::game_system::script_engine::utils::media::MediaType;
    let (subdir, media) = match kind {
        "background" => ("Backgrounds", MediaType::Background),
        "music" => ("Musics", MediaType::Music),
        "sound" => ("Sounds", MediaType::Sound),
        "ambient" => ("Ambients", MediaType::Ambient),
        "pic" => ("Pics", MediaType::Pic),
        other => return Err(format!("未知素材类别: {}", other)),
    };
    Ok((subdir, game_data_dir().join(media.fallback_dir())))
}

fn allowed_extensions(kind: &str) -> &'static [&'static str] {
    match kind {
        "background" | "pic" => &["png", "jpg", "jpeg", "webp", "bmp", "gif"],
        _ => &["mp3", "wav", "ogg", "flac", "m4a"],
    }
}

/// 列出全局素材（`game_data/backgrounds` / `musics` / `ambient`）。
///
/// 注意 background 与 pic、music 与 sound 在全局层共享同一个目录 ——
/// 这是 `MediaType::fallback_dir()` 的既有行为，不是这里的简化。
#[tauri::command]
pub fn editor_list_global_assets() -> Result<AssetIndex, String> {
    let one = |kind: &str| -> Vec<String> {
        let Ok((_, dir)) = asset_dirs(kind) else {
            return Vec::new();
        };
        let allowed = allowed_extensions(kind);
        let mut out: Vec<String> = Vec::new();
        if let Ok(entries) = std::fs::read_dir(&dir) {
            for e in entries.flatten() {
                if !e.file_type().map(|t| t.is_file()).unwrap_or(false) {
                    continue;
                }
                let name = e.file_name().to_string_lossy().to_string();
                if name.starts_with('.') {
                    continue;
                }
                let ext = Path::new(&name)
                    .extension()
                    .map(|x| x.to_string_lossy().to_lowercase())
                    .unwrap_or_default();
                if allowed.contains(&ext.as_str()) {
                    out.push(name);
                }
            }
        }
        out.sort();
        out
    };

    Ok(AssetIndex {
        background: one("background"),
        music: one("music"),
        sound: one("sound"),
        ambient: one("ambient"),
        pic: one("pic"),
    })
}

/// 导入素材。
///
/// 只收**源文件路径**，由 Rust 自己 `fs::copy` —— 与 `api/font.rs::import_font`
/// 和 `import_role_from_path` 的既有做法一致。早先的实现让前端用
/// `plugin-fs::readFile` 读成字节再走 IPC，两个问题：用户从任意位置选的文件
/// 不在 `capabilities` 的 `fs:scope` 里会被直接拒绝；而且一个 64MB 的图会先
/// 变成 6700 万元素的 JS 数组再 JSON 序列化进 IPC。
///
/// `scope` 决定落点，见 [`AssetScope`]。
#[tauri::command]
pub fn editor_upload_asset(
    key: String,
    kind: String,
    scope: AssetScope,
    src_path: String,
) -> Result<String, String> {
    let src = Path::new(&src_path);
    if !src.is_file() {
        return Err(format!("源文件不存在: {}", src_path));
    }

    // 只取文件名，杜绝用源路径拼出目标路径
    let raw_name = src
        .file_name()
        .map(|s| s.to_string_lossy().to_string())
        .ok_or_else(|| "无法从源路径取出文件名".to_string())?;
    let name = paths::sanitize_file_name(&raw_name)?;

    let ext = Path::new(&name)
        .extension()
        .map(|e| e.to_string_lossy().to_lowercase())
        .unwrap_or_default();
    let allowed = allowed_extensions(&kind);
    if !allowed.contains(&ext.as_str()) {
        return Err(format!(
            "不支持的文件类型 .{}；{} 支持: {}",
            ext,
            kind,
            allowed.join(" / ")
        ));
    }

    let (subdir, global_dir) = asset_dirs(&kind)?;
    let target_dir = match scope {
        AssetScope::Script => paths::resolve_script_dir(&key)?.join("Assets").join(subdir),
        AssetScope::Global => global_dir,
    };

    std::fs::create_dir_all(&target_dir).map_err(|e| format!("无法创建素材目录: {}", e))?;
    let target = target_dir.join(&name);
    if target.exists() {
        return Err(format!("同名素材「{}」已存在", name));
    }
    std::fs::copy(src, &target).map_err(|e| format!("复制素材失败: {}", e))?;
    Ok(name)
}

/// 递归复制目录，供 rename 跨设备失败时兜底。
fn copy_dir_recursive(from: &Path, to: &Path) -> std::io::Result<()> {
    std::fs::create_dir_all(to)?;
    for entry in std::fs::read_dir(from)? {
        let entry = entry?;
        let src = entry.path();
        let dst = to.join(entry.file_name());
        if entry.file_type()?.is_dir() {
            copy_dir_recursive(&src, &dst)?;
        } else {
            std::fs::copy(&src, &dst)?;
        }
    }
    Ok(())
}

#[tauri::command]
pub fn editor_create_character(
    key: String,
    folder: String,
    ai_name: String,
    system_prompt: String,
) -> Result<ScriptCharacter, String> {
    let dir = paths::resolve_script_dir(&key)?;
    let folder = paths::sanitize_folder_name(&folder)?;

    let char_dir = dir.join("characters").join(&folder);
    if char_dir.exists() {
        return Err(format!("角色「{}」已存在", folder));
    }
    std::fs::create_dir_all(char_dir.join("avatar"))
        .map_err(|e| format!("创建角色目录失败: {}", e))?;

    let name = if ai_name.trim().is_empty() {
        folder.clone()
    } else {
        ai_name.trim().to_string()
    };

    // script_role_key 必须显式写入。缺了它，引擎的 register_script_roles 会
    // 每次启动都新建一个重复角色，而剧本里的 character: 又永远查不到
    // （PR1 已修键不一致的问题，这里仍然显式写，避免依赖回落行为）。
    let mut settings = Map::new();
    settings.insert("ai_name".into(), JsonValue::String(name.clone()));
    settings.insert("script_role_key".into(), JsonValue::String(folder.clone()));
    settings.insert(
        "system_prompt".into(),
        JsonValue::String(system_prompt.trim().to_string()),
    );

    io::write_json_as_yaml(
        &char_dir.join("settings.yml"),
        &JsonValue::Object(settings),
    )?;

    Ok(ScriptCharacter {
        folder: folder.clone(),
        role_key: folder,
        ai_name: name,
        emotions: Vec::new(),
        clothes: Vec::new(),
    })
}

/// 全局角色库里的一个角色（`game_data/characters/<目录>`）。
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct GlobalCharacter {
    pub folder: String,
    pub ai_name: String,
    /// 该角色在**当前剧本**里是否已经导入过
    pub already_in_script: bool,
    /// 全局目录里有没有 avatar/，没有的话导入后也不会有立绘
    pub has_avatar: bool,
}

/// 列出全局角色库，并标出哪些已经导入到当前剧本。
#[tauri::command]
pub fn editor_list_global_characters(key: String) -> Result<Vec<GlobalCharacter>, String> {
    let existing: HashSet<String> = paths::resolve_script_dir(&key)
        .map(|d| {
            read_characters(&d)
                .into_iter()
                .map(|c| c.folder)
                .collect::<HashSet<_>>()
        })
        .unwrap_or_default();

    let base = crate::api::characters_dir();
    let mut out = Vec::new();
    let Ok(entries) = std::fs::read_dir(&base) else {
        return Ok(out);
    };
    for e in entries.flatten() {
        if !e.file_type().map(|t| t.is_dir()).unwrap_or(false) {
            continue;
        }
        let folder = e.file_name().to_string_lossy().to_string();
        if folder.starts_with('.') {
            continue;
        }
        let settings: JsonValue = std::fs::read_to_string(e.path().join("settings.yml"))
            .ok()
            .and_then(|s| serde_yaml::from_str(&s).ok())
            .unwrap_or(JsonValue::Null);
        out.push(GlobalCharacter {
            ai_name: settings
                .get("ai_name")
                .and_then(|v| v.as_str())
                .unwrap_or(&folder)
                .to_string(),
            already_in_script: existing.contains(&folder),
            has_avatar: e.path().join("avatar").is_dir(),
            folder,
        });
    }
    out.sort_by(|a, b| a.folder.cmp(&b.folder));
    Ok(out)
}

/// 把一个全局角色导入当前剧本。
///
/// **为什么是「复制 settings.yml」而不是「直接引用」**：引擎解析 `character:`
/// 只有两条路（见 `script_function::get_role`）—— `MAIN` 走当前主角，其余一律
/// 按「剧本 key + 角色 key」在剧本自己的 `characters/` 里找。全局角色库不在这
/// 条路径上，所以剧本里写一个全局角色名，运行时必然解析不到人。
///
/// 但作者真正的诉求是「别让我把已有的人设再敲一遍」，那复制一份就够了：
/// 复制之后 `register_script_roles` 能正常注册，剧本也仍然是自包含的。
///
/// **立绘默认不复制**：`get_avatar_file` 的查找顺序本来就是「先
/// `game_data/characters/<目录>/avatar`，再各剧本的 `characters/<目录>/avatar`」，
/// 同名目录的立绘会自动落到全局那份上，白复制一遍只是让剧本目录凭空变大。
/// 只有作者打算把剧本单独分发给没有这个角色的人时，才需要 `with_avatar`。
#[tauri::command]
pub fn editor_import_global_character(
    key: String,
    folder: String,
    with_avatar: bool,
) -> Result<ScriptCharacter, String> {
    let dir = paths::resolve_script_dir(&key)?;
    let folder = paths::sanitize_folder_name(&folder)?;

    let src = crate::api::characters_dir().join(&folder);
    if !src.is_dir() {
        return Err(format!("全局角色库里没有「{}」", folder));
    }
    let src_settings = src.join("settings.yml");
    if !src_settings.is_file() {
        return Err(format!("角色「{}」缺少 settings.yml，无法导入", folder));
    }

    let dest = dir.join("characters").join(&folder);
    if dest.exists() {
        return Err(format!("本剧本里已经有角色「{}」了", folder));
    }
    std::fs::create_dir_all(&dest).map_err(|e| format!("创建角色目录失败: {}", e))?;

    // 不直接 copy 文件：要补写 script_role_key，并摘掉只对全局角色有意义的字段
    let raw = std::fs::read_to_string(&src_settings)
        .map_err(|e| format!("读取角色设定失败: {}", e))?;
    let mut settings: JsonValue =
        serde_yaml::from_str(&raw).map_err(|e| format!("角色设定不是合法 YAML: {}", e))?;
    let obj = settings
        .as_object_mut()
        .ok_or_else(|| "角色设定顶层必须是键值映射".to_string())?;
    obj.remove("character_id");
    obj.remove("resource_path");
    obj.remove("script_key");
    obj.insert("script_role_key".into(), JsonValue::String(folder.clone()));

    io::write_json_as_yaml(&dest.join("settings.yml"), &settings)?;

    if with_avatar {
        let avatar = src.join("avatar");
        if avatar.is_dir() {
            copy_dir_recursive(&avatar, &dest.join("avatar"))
                .map_err(|e| format!("复制立绘失败: {}", e))?;
        }
    } else {
        // 建空目录，作者想单独放几张覆盖用的立绘时有地方放
        let _ = std::fs::create_dir_all(dest.join("avatar"));
    }

    read_characters(&dir)
        .into_iter()
        .find(|c| c.folder == folder)
        .ok_or_else(|| "导入后读不回角色，请检查目录权限".to_string())
}

/// 重新扫描剧本目录，把新写/改名的剧本加载进引擎。
///
/// 引擎原本只在启动时扫一次，作者存完剧本必须重启整个应用才能试玩。
///
/// 刻意做成**增量 merge** 而不是整体替换 `script_manager`：
/// - `ScriptStatus` 里的 `current_chapter_key` / `current_event_process` / `vars` /
///   `running_client_id` 是运行进度，整体替换会把**所有**剧本的进度清零；
/// - `is_running` 是 `Arc<AtomicBool>`，调用方（`api/script.rs`、`api/adventure.rs`）
///   会先 clone 出来、放掉锁之后才 `store(true)`。整体替换会换掉这个 Arc，让
///   运行中的任务把状态写到一个已经被孤立的对象上，之后 `is_running` 永远是 false。
#[tauri::command]
pub async fn editor_rescan_scripts(app: AppHandle) -> Result<usize, String> {
    let state = app.state::<AppState>();
    let mut service = state.ai_service.lock().await;

    if service
        .script_manager
        .is_running
        .load(std::sync::atomic::Ordering::SeqCst)
    {
        return Err("有剧本正在运行，请先结束再重新扫描".to_string());
    }

    let data = service.data_dir.clone();
    let fresh = crate::ai_service::game_system::script_engine::ScriptManager::new(&data);

    let existing = &mut service.script_manager.all_scripts;

    // 磁盘上已经没有的剧本要摘掉（改名 / 删除）
    existing.retain(|name, _| fresh.all_scripts.contains_key(name));

    for (name, scanned) in fresh.all_scripts {
        match existing.get_mut(&name) {
            Some(old) => {
                // 配置字段来自磁盘，运行进度保留
                old.folder_key = scanned.folder_key;
                old.description = scanned.description;
                old.intro_chapter = scanned.intro_chapter;
                old.settings = scanned.settings;
                old.script_path = scanned.script_path;
                old.recommand_start = scanned.recommand_start;
                old.adventure = scanned.adventure;
            }
            None => {
                existing.insert(name, scanned);
            }
        }
    }

    let count = existing.len();
    tracing::info!("[ScriptEditor] 重新扫描完成，共 {} 个剧本", count);
    Ok(count)
}

// 这里原本有 editor_reorder_chapters（拖动章节改先后顺序）。已连同前端的拖拽
// 一起删除，理由是这个功能本身就站不住：
//
// 1. 章节先后是 chapter_end.next_chapter 串出来的，只有纯线性的一段才谈得上
//    「顺序」；一旦有分支，走向由条件决定，交换顺序没有意义 —— 这句话是我自己
//    在流程图上写给作者看的，那就不该同时提供一个假装能换顺序的入口。
// 2. 真正天天要调的是**章节内部的事件顺序**，那个已经改成拖拽（见前端
//    ChapterTimeline）。章节之间的接线改的是剧情结构，作者应该在
//    「章节结束」事件里显式指定下一章，那里看得见、可校验、可撤销。

/// 在编辑器里直接试玩，不必回主菜单。
///
/// 内部先 rescan（作者刚存的改动才能生效），然后用引擎的真实执行路径跑 ——
/// 语义与正式游玩完全一致，这是当初选「复用真引擎」而不是另写一套预览解释器的理由。
///
/// 与正式游玩的两点区别：
/// 1. `on_script_end` 传 `completed = false`，调试不会被记成通关；
/// 2. 不调用 `handle_adventure_completion`，不会解锁后续羁绊冒险、不发成就。
///
/// 因此这里刻意不用 `execute_script`，而是自己组合它内部那三个 `pub` 步骤。
/// `from_chapter` 为 `None` 时从开场章节开始；`use_llm` 为 `false` 时 AI 事件出占位。
#[tauri::command]
pub async fn editor_start_preview(
    app: AppHandle,
    key: String,
    from_chapter: Option<String>,
    use_llm: bool,
) -> Result<(), String> {
    // 先把磁盘状态同步进引擎
    editor_rescan_scripts(app.clone()).await?;

    let state = app.state::<AppState>();
    let dir = paths::resolve_script_dir(&key)?;
    let config_name = io::read_story_config(&dir)?
        .get("script_name")
        .and_then(|v| v.as_str())
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty())
        .unwrap_or_else(|| {
            dir.file_name()
                .map(|s| s.to_string_lossy().to_string())
                .unwrap_or_default()
        });

    let ai_service = state.ai_service.clone();
    let channels = state.script_channels.clone();
    let db = state.db.clone();
    let llm = crate::ai_service::llm::slot_snapshot(&state.chat.llm).await;

    let (mut script, game_status, cfg, is_running, data_dir) = {
        let service = ai_service.lock().await;
        if service
            .script_manager
            .is_running
            .load(std::sync::atomic::Ordering::SeqCst)
        {
            return Err("已经有剧本在运行，请先停止再试玩".to_string());
        }
        let script = service
            .script_manager
            .all_scripts
            .get(&config_name)
            .ok_or_else(|| {
                format!(
                    "引擎里找不到剧本「{}」。请先检查 story_config.yaml 的 script_name",
                    config_name
                )
            })?
            .clone();
        (
            script,
            service.game_status.clone(),
            service.config.clone(),
            service.script_manager.is_running.clone(),
            service.data_dir.clone(),
        )
    };

    // 从哪一章开始 —— run_script 以 script.intro_chapter 为起点
    if let Some(from) = from_chapter {
        let from = validate::chapter_id_of(&from).to_string();
        if !from.is_empty() {
            paths::resolve_chapter_file(&dir, &from, true)?;
            script.intro_chapter = from;
        }
    }

    // 把 MAIN 指到该指的人身上，并记住原值以便收尾时还原
    let restore_main = apply_preview_main_role(&db, &game_status, &script).await?;

    is_running.store(true, std::sync::atomic::Ordering::SeqCst);

    tokio::spawn(async move {
        let mut ctx = crate::ai_service::game_system::script_engine::events::ScriptContext {
            db: &db,
            data_dir: &data_dir,
            app: &app,
            game_status: game_status.clone(),
            config: &cfg,
            llm: llm.as_ref(),
            channels,
            dry_run_ai: !use_llm,
        };
        use crate::ai_service::game_system::script_engine::ScriptManager;

        let mut outcome = ScriptManager::init_script(&script, &mut ctx).await;
        if outcome.is_ok() {
            outcome = ScriptManager::run_script(&mut ctx).await;
        }
        if let Err(ref e) = outcome {
            tracing::error!("[ScriptEditor] 试玩执行失败: {:#}", e);
            crate::ai_service::message_system::events::emit_error(ctx.app, e);
        }
        // completed = false：试玩永远不记通关
        if let Err(e) = ScriptManager::on_script_end(&mut ctx, &is_running, false).await {
            tracing::error!("[ScriptEditor] 试玩收尾失败: {:#}", e);
        }

        // 还原主角。放在这里而不是 stop_preview 里：正常跑完、报错、被 stop 掐断
        // 三条路都会走到这，只在 stop 里还原会漏掉前两条。
        if let Some(prev) = restore_main {
            let mut gs = game_status.lock().await;
            gs.main_role_id = prev;
            gs.current_role_id = prev;
        }
        tracing::info!("[ScriptEditor] 试玩结束");
    });

    Ok(())
}

/// 试玩前把 `MAIN` 指到正确的角色上，返回原值供收尾还原（`None` 表示不用还原）。
///
/// 引擎里 `character: MAIN` 解析成 `game_status.main_role_id`，而这个字段是
/// **主菜单选角色**时设的（`init_game_status`）。正式玩羁绊冒险时你必然是从
/// 该角色的角色卡进去的，所以它天然是对的；编辑器里没有这一步，于是：
///
/// - 没选过角色 → `main_role_id` 是 `None`，第一个 `character: MAIN` 事件直接
///   报「MAIN 角色未设定」，剧本停在那里（表现就是立绘不出来、对话不往下走）；
/// - 选的是别的角色 → `MAIN` 解析成另一个人，试玩的是一出张冠李戴的戏。
///
/// 所以这里复刻正式路径的那条保证：羁绊剧本按 `bound_character_folder` 把 MAIN
/// 临时指过去，独立剧本沿用当前主角。两者都拿不到就直接报错让作者去选人，
/// 而不是让他对着一个卡住的画面猜。
async fn apply_preview_main_role(
    db: &DatabaseConnection,
    game_status: &Arc<Mutex<GameStatus>>,
    script: &ScriptStatus,
) -> Result<Option<Option<i32>>, String> {
    let bound = script.adventure.bound_character_folder.trim();

    if bound.is_empty() {
        // 独立剧本：没有绑定人，只能沿用当前主角
        let current = game_status.lock().await.main_role_id;
        return if current.is_some() {
            Ok(None)
        } else {
            Err("这个剧本没有绑定角色，而当前也还没有选定主角。\
                 请先回主菜单选一个角色（剧本里的 MAIN 就是他），再回来试玩。"
                .to_string())
        };
    }

    let role_id = find_main_role_by_folder(db, bound).await?.ok_or_else(|| {
        format!(
            "剧本绑定的角色「{}」不在角色库里。请确认 game_data/characters/ 下有这个目录，\
             或到剧本设置里把「绑定角色目录名」改成实际存在的角色。",
            bound
        )
    })?;

    let mut gs = game_status.lock().await;
    let prev = gs.main_role_id;
    if prev == Some(role_id) {
        return Ok(None); // 本来就是他，不用动也不用还原
    }
    // 先把角色载进 RoleManager，否则后面 get_role 拿不到立绘/显示名
    gs.get_role(db, role_id)
        .await
        .map_err(|e| format!("载入绑定角色失败: {}", e))?;
    gs.main_role_id = Some(role_id);
    gs.current_role_id = Some(role_id);
    Ok(Some(prev))
}

/// 按资源目录名找主角色。目录名就是 `game_data/characters/<目录>`。
async fn find_main_role_by_folder(
    db: &DatabaseConnection,
    folder: &str,
) -> Result<Option<i32>, String> {
    let roles = RoleRepo::get_all_main_roles(db)
        .await
        .map_err(|e| format!("查询角色库失败: {}", e))?;
    Ok(roles
        .into_iter()
        .find(|r| r.resource_folder.as_deref() == Some(folder))
        .map(|r| r.id))
}

/// 角色显示名，查不到就算了 —— 这只是给作者看的提示文案，不值得让整个命令失败。
async fn role_name_of(db: &DatabaseConnection, id: i32) -> Option<String> {
    RoleRepo::get_role_by_id(db, id)
        .await
        .ok()
        .flatten()
        .map(|r| r.name)
}

/// 试玩前的可行性检查，供编辑器在打开剧本时提前提示，而不是等作者点了才报错。
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PreviewReadiness {
    /// 能不能直接开跑
    pub ok: bool,
    /// 试玩时 `MAIN` 会是谁；`None` 表示定不下来
    pub main_role_name: Option<String>,
    /// 绑定角色目录名（独立剧本为空）
    pub bound_character_folder: String,
    /// `ok` 为 false 时给作者看的原因
    pub reason: Option<String>,
}

#[tauri::command]
pub async fn editor_preview_readiness(
    app: AppHandle,
    key: String,
) -> Result<PreviewReadiness, String> {
    let dir = paths::resolve_script_dir(&key)?;
    let cfg = io::read_story_config(&dir)?;
    let bound = cfg
        .get("adventure")
        .and_then(|a| a.get("bound_character_folder"))
        .and_then(|v| v.as_str())
        .unwrap_or("")
        .trim()
        .to_string();

    let state = app.state::<AppState>();
    let db = state.db.clone();
    let game_status = state.ai_service.lock().await.game_status.clone();

    if bound.is_empty() {
        let current = game_status.lock().await.main_role_id;
        return Ok(match current {
            Some(id) => PreviewReadiness {
                ok: true,
                main_role_name: role_name_of(&db, id).await,
                bound_character_folder: bound,
                reason: None,
            },
            None => PreviewReadiness {
                ok: false,
                main_role_name: None,
                bound_character_folder: bound,
                reason: Some(
                    "这个剧本没有绑定角色，当前也还没选定主角，试玩时 MAIN 会解析不到人。\
                     请先回主菜单选一个角色，或到剧本设置里把它设成某个角色的羁绊冒险。"
                        .to_string(),
                ),
            },
        });
    }

    match find_main_role_by_folder(&db, &bound).await? {
        Some(id) => Ok(PreviewReadiness {
            ok: true,
            main_role_name: role_name_of(&db, id).await,
            bound_character_folder: bound,
            reason: None,
        }),
        None => Ok(PreviewReadiness {
            ok: false,
            main_role_name: None,
            reason: Some(format!(
                "剧本绑定的角色「{}」不在角色库里，试玩时 MAIN 会解析不到人。\
                 请确认 game_data/characters/ 下有这个目录。",
                bound
            )),
            bound_character_folder: bound,
        }),
    }
}

/// 中止试玩。
///
/// 剧本大概率正阻塞在「等输入」或「等选择」上，所以先往对应通道塞一个值把它
/// 唤醒，再把 `is_running` 置 false。引擎会在当前事件跑完后走到章节末尾结束 ——
/// 不是立即掐断，而是让它自然收尾，避免留下半个状态。
#[tauri::command]
pub async fn editor_stop_preview(app: AppHandle) -> Result<(), String> {
    let state = app.state::<AppState>();

    {
        let mut ch = state.script_channels.lock().await;
        if let Some(tx) = ch.choice_tx.take() {
            let _ = tx.send(String::new());
        }
        if let Some(tx) = ch.input_tx.take() {
            let _ = tx.send(String::new());
        }
        ch.choice_allow_free = false;
    }

    let service = state.ai_service.lock().await;
    service
        .script_manager
        .is_running
        .store(false, std::sync::atomic::Ordering::SeqCst);
    Ok(())
}

/// 在系统文件管理器里打开剧本目录。
#[tauri::command]
pub fn editor_open_script_folder(key: String) -> Result<(), String> {
    let dir = paths::resolve_script_dir(&key)?;
    // open_folder 收的是 &str，不是 &Path
    crate::utils::system::open_folder(&dir.to_string_lossy())
}

#[cfg(test)]
mod tests {
    use super::allowed_extensions;

    #[test]
    fn asset_extensions_split_image_and_audio() {
        assert!(allowed_extensions("background").contains(&"png"));
        assert!(allowed_extensions("pic").contains(&"webp"));
        assert!(!allowed_extensions("background").contains(&"mp3"));
        for k in ["music", "sound", "ambient"] {
            assert!(allowed_extensions(k).contains(&"mp3"), "{}", k);
            assert!(!allowed_extensions(k).contains(&"png"), "{}", k);
        }
    }
}
