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
    /// 可用作缩略图预览的立绘绝对路径：本地 avatar 优先，没有就回退到全局
    /// `game_data/characters/<folder>/avatar/`——与引擎运行时同一个查找顺序。
    /// 两处都没有时为 None，前端据此判断「立绘不会显示」（issue #9）。
    pub preview_image: Option<String>,
    /// 全局角色库里是否存在该角色的立绘。本地 avatar 为空但全局有时，引擎仍能
    /// 找到立绘，所以不该提示「立绘不会显示」；前端用它显示「立绘读自全局」徽标。
    pub global_avatar: bool,
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

/// 剧本内某类素材的子目录候选，与 `media.rs::subdir_candidates` 保持一致。
///
/// 抽成函数是因为有两个消费者（只要文件名的 `read_asset_index`、要路径的
/// `editor_list_asset_files`），各抄一份迟早会发散成「下拉里有、素材页里没有」。
fn asset_subdir_candidates(kind: &str) -> &'static [&'static str] {
    match kind {
        "background" => &["Backgrounds", "Pics", "Pictures", "Pic", "Picture"],
        "music" => &["Musics", "BGMs", "Music", "BGM"],
        "sound" => &["Sounds", "SoundEffects", "Sound", "SoundEffect"],
        "ambient" => &["Ambients", "AmbientSounds", "Environment", "Ambient"],
        "pic" => &["Pics", "Pictures", "Pic", "Picture"],
        _ => &[],
    }
}

fn read_asset_index(script_dir: &Path) -> AssetIndex {
    let one = |kind: &str| list_asset_dir(script_dir, asset_subdir_candidates(kind));
    AssetIndex {
        background: one("background"),
        music: one("music"),
        sound: one("sound"),
        ambient: one("ambient"),
        pic: one("pic"),
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

        // 立绘预览图与「全局有没有立绘」：本地 avatar 优先，没有再回退到全局
        // game_data/characters/<folder>/avatar/。引擎运行时也是这个顺序（issue #9）。
        let global_avatar_dir = crate::api::characters_dir().join(&folder).join("avatar");
        let global_avatar = first_avatar_image(&global_avatar_dir).is_some();
        let preview_image = first_avatar_image(&avatar)
            .or_else(|| first_avatar_image(&global_avatar_dir));

        out.push(ScriptCharacter {
            folder,
            role_key,
            ai_name,
            emotions,
            clothes,
            preview_image,
            global_avatar,
        });
    }
    out.sort_by(|a, b| a.folder.cmp(&b.folder));
    out
}

/// 取一个 avatar 目录里「最适合当缩略图」的图片绝对路径。
///
/// 优先 default / normal / idle 这类基准表情，其次按文件名升序，保证每次选到的
/// 是同一张（不会刷新一下就换脸）。目录不存在或没图返回 None。
fn first_avatar_image(dir: &Path) -> Option<String> {
    let entries = std::fs::read_dir(dir).ok()?;
    let mut imgs: Vec<(u8, String, std::path::PathBuf)> = Vec::new();
    for e in entries.flatten() {
        if !e.file_type().map(|t| t.is_file()).unwrap_or(false) {
            continue;
        }
        let path = e.path();
        let ext = path
            .extension()
            .and_then(|x| x.to_str())
            .map(|x| x.to_lowercase())
            .unwrap_or_default();
        if !matches!(ext.as_str(), "png" | "webp" | "jpg" | "jpeg" | "gif") {
            continue;
        }
        let stem = path
            .file_stem()
            .map(|s| s.to_string_lossy().to_string())
            .unwrap_or_default();
        // 基准表情优先级最高（0），其余统一 1，再按名字排序
        let prio = match stem.as_str() {
            "default" | "normal" | "idle" | "stand" => 0,
            _ => 1,
        };
        imgs.push((prio, stem, path));
    }
    imgs.sort_by(|a, b| a.0.cmp(&b.0).then_with(|| a.1.cmp(&b.1)));
    imgs.into_iter()
        .next()
        .map(|(_, _, p)| p.to_string_lossy().to_string())
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

    std::fs::remove_file(&file).map_err(|e| format!("删除章节失败: {}", e))?;
    Ok(())
}

/// 删除剧本内一个角色（整个 `characters/<folder>/` 目录）。
///
/// 删除后校验器会把它当成不存在，被引用的角色会变成一条 `character.unknown` 诊断，
/// 提示作者剧本里还有地方在引用它。
#[tauri::command]
pub fn editor_delete_character(key: String, folder: String) -> Result<(), String> {
    let dir = paths::resolve_script_dir(&key)?;
    let safe = paths::sanitize_folder_name(&folder)?;
    let target = dir.join("characters").join(&safe);
    if !target.exists() {
        return Err(format!("角色目录不存在: characters/{}", safe));
    }

    std::fs::remove_dir_all(&target).map_err(|e| format!("删除角色失败: {}", e))?;
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

    std::fs::remove_dir_all(&dir).map_err(|e| format!("删除剧本失败: {}", e))?;
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
/// 注意 background 与 pic 在全局层共享同一个目录 —— 这是 `MediaType::fallback_dir()`
/// 的既有行为。**音效（sound）例外**：它没有独立的全局目录，此前会 fallback 到
/// 全局音乐目录，但音效本就该是剧本私有素材，全局列出只会让作者误以为能跨剧本
/// 复用，所以这里直接返回空（issue #6）。环境音（ambient）仍保留全局列。
#[tauri::command]
pub fn editor_list_global_assets() -> Result<AssetIndex, String> {
    let one = |kind: &str| -> Vec<String> {
        // 全局没有音效目录：跳过，不读音乐目录
        if kind == "sound" {
            return Vec::new();
        }
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

/// 一个素材文件的详细信息，供素材页做预览与删除。
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AssetFile {
    pub name: String,
    /// 绝对路径。前端用 `convertFileSrc` 转成 asset URL 就能直接 `<img>` / `<audio>`
    pub path: String,
    pub size: u64,
}

#[derive(Debug, Clone, Default, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AssetFileIndex {
    pub background: Vec<AssetFile>,
    pub music: Vec<AssetFile>,
    pub sound: Vec<AssetFile>,
    pub ambient: Vec<AssetFile>,
    pub pic: Vec<AssetFile>,
}

/// 列素材，带路径和大小。
///
/// 与 `editor_read_script` / `editor_list_global_assets` 只给文件名的版本并存：
/// 那两个喂的是属性面板的下拉框，只需要名字；素材页要显示缩略图、放音频、
/// 报体积，就得有绝对路径。一次调用把五类全给出来，免得每个文件一次 IPC。
#[tauri::command]
pub fn editor_list_asset_files(key: String, scope: AssetScope) -> Result<AssetFileIndex, String> {
    let script_dir = match scope {
        AssetScope::Script => Some(paths::resolve_script_dir(&key)?),
        AssetScope::Global => None,
    };

    let one = |kind: &str| -> Vec<AssetFile> {
        // 全局没有音效目录（与 editor_list_global_assets 一致），剧本内仍正常扫
        if script_dir.is_none() && kind == "sound" {
            return Vec::new();
        }
        let allowed = allowed_extensions(kind);
        let dirs: Vec<PathBuf> = match &script_dir {
            // 剧本内可能落在 media.rs 的任意一个候选子目录，全都扫
            Some(root) => asset_subdir_candidates(kind)
                .iter()
                .map(|s| root.join("Assets").join(s))
                .collect(),
            None => asset_dirs(kind).map(|(_, d)| vec![d]).unwrap_or_default(),
        };

        let mut out: Vec<AssetFile> = Vec::new();
        let mut seen: HashSet<String> = HashSet::new();
        for dir in dirs {
            let Ok(entries) = std::fs::read_dir(&dir) else {
                continue;
            };
            for e in entries.flatten() {
                let Ok(meta) = e.metadata() else { continue };
                if !meta.is_file() {
                    continue;
                }
                let name = e.file_name().to_string_lossy().to_string();
                if name.starts_with('.') || !seen.insert(name.clone()) {
                    continue;
                }
                let ext = Path::new(&name)
                    .extension()
                    .map(|x| x.to_string_lossy().to_lowercase())
                    .unwrap_or_default();
                if !allowed.contains(&ext.as_str()) {
                    continue;
                }
                out.push(AssetFile {
                    path: e.path().to_string_lossy().to_string(),
                    size: meta.len(),
                    name,
                });
            }
        }
        out.sort_by(|a, b| a.name.cmp(&b.name));
        out
    };

    Ok(AssetFileIndex {
        background: one("background"),
        music: one("music"),
        sound: one("sound"),
        ambient: one("ambient"),
        pic: one("pic"),
    })
}

/// 删除一个素材文件。
///
/// 删掉之后剧本里的引用会变成校验器的一条 `asset.missing` 诊断。
#[tauri::command]
pub fn editor_delete_asset(
    key: String,
    kind: String,
    scope: AssetScope,
    name: String,
) -> Result<(), String> {
    let name = paths::sanitize_file_name(&name)?;

    let files = editor_list_asset_files(key, scope)?;
    let list = match kind.as_str() {
        "background" => files.background,
        "music" => files.music,
        "sound" => files.sound,
        "ambient" => files.ambient,
        "pic" => files.pic,
        other => return Err(format!("未知的素材类别: {}", other)),
    };
    let target = list
        .into_iter()
        .find(|f| f.name == name)
        .ok_or_else(|| format!("找不到素材「{}」", name))?;
    let path = PathBuf::from(&target.path);

    std::fs::remove_file(&path).map_err(|e| format!("删除素材失败: {}", e))?;
    Ok(())
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

    // 刚创建的角色 avatar 目录是空的；全局同名角色若有立绘，仍按引擎回退顺序标出来
    let global_avatar_dir = crate::api::characters_dir().join(&folder).join("avatar");
    let global_avatar = first_avatar_image(&global_avatar_dir).is_some();

    Ok(ScriptCharacter {
        folder: folder.clone(),
        role_key: folder,
        ai_name: name,
        emotions: Vec::new(),
        clothes: Vec::new(),
        preview_image: None,
        global_avatar,
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
/// `from_chapter` 为 `None` 时从开场章节开始。试玩会真调 LLM；LLM 未配置时遇到 AI 事件即终止剧本。
#[tauri::command]
pub async fn editor_start_preview(
    app: AppHandle,
    key: String,
    from_chapter: Option<String>,
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

        // 防御同名 script_name 导致的串读：HashMap 用 script_name 作 key，
        // 多个剧本的 story_config.yaml 写同一个名字时会互相覆盖。
        // 这里把 hashmap 返回的 script_path 与实际目录比对，不匹配说明读到了别的剧本，
        // 直接拒绝启动，并把双方路径打出来让作者看到。
        // 双方都 canonicalize 后再比——Windows 上 canonicalize 会给路径加 \\?\ 前缀，
        // 而 script.script_path 存在 HashMap 里时不带这个前缀，简单字符串比对会误报。
        let resolved_dir = dir
            .canonicalize()
            .unwrap_or_else(|_| dir.clone());
        let resolved_script = script
            .script_path
            .canonicalize()
            .unwrap_or_else(|_| script.script_path.clone());
        if resolved_script != resolved_dir {
            return Err(format!(
                "剧本路径不匹配：请求 {} 但引擎返回的是 {}。可能原因：多个剧本的 story_config.yaml 里 script_name 重名，后扫的覆盖了前面的。",
                resolved_dir.display(),
                script.script_path.display(),
            ));
        }

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

    // 备份整个会话状态，并按「刚进游戏」的样子把试玩场次搭好
    let session = PreviewSession::begin(&db, &data_dir, &game_status, &script).await?;

    // 快照托管给 AppState：试玩任务自然结束时还原一次，editor_stop_preview 兜底
    // 再 take 一次（为空即跳过）。这样无论「跑完 / 报错 / 被中止」哪条路，
    // 共享 GameStatus 都能回到试玩前，不会污染玩家自由对话的上下文。
    {
        let state = app.state::<AppState>();
        *state.pending_preview_restore.lock().await = Some(session);
        // 清掉上一轮可能残留的已结束句柄
        let _ = state.preview_task.lock().await.take();
    }

    is_running.store(true, std::sync::atomic::Ordering::SeqCst);

    let app_for_handle = app.clone();
    let handle = tokio::spawn(async move {
        let mut ctx = crate::ai_service::game_system::script_engine::events::ScriptContext {
            db: &db,
            data_dir: &data_dir,
            app: &app,
            game_status: game_status.clone(),
            config: &cfg,
            llm: llm.as_ref(),
            channels,
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

        // 把会话状态整个还原回试玩之前。幂等（Option::take）：被 editor_stop_preview
        // 先 take 走时这里拿到 None 直接跳过。
        apply_pending_restore(&app).await;
        tracing::info!("[ScriptEditor] 试玩结束，会话状态已还原");
    });
    *app_for_handle
        .state::<AppState>()
        .preview_task
        .lock()
        .await = Some(handle);

    Ok(())
}

/// 一次试玩对**共享会话状态**的全部改动，以及把它们撤回去的能力。
///
/// 这是这一轮最重要的一处修正。此前试玩直接在 `GameStatus` 上跑，而那是
/// 玩家正在用的**同一个**会话对象，于是两个方向都出问题：
///
/// **往里看**——试玩场次是残缺的。正式游玩前必然走过 `init_game_status()`，
/// 它做三件事：清空台词表、写入主角的人设 SYSTEM 台词、把主角 `onstage`。
/// 编辑器是独立路由，这三件一件都没做，后果是：立绘不出来（没人在台上），
/// 日志刷「role_id=N 没有找到 SYSTEM 属性的台词，可能人设丢失」（没有人设台词），
/// 而且 AI 对话是在没有人设的上下文里生成的。
///
/// **往外看**——试玩会往真实会话里漏东西。剧本跑出来的每一句台词都进了玩家的
/// `line_list`，`〔试玩已关闭 LLM〕` 这类占位也一样；背景、音乐、在场角色、
/// `script_status` 全都留在原地。退出编辑器回自由对话，看到的就是试玩的残留。
///
/// 所以现在的做法是：**进来时整体备份、按新会话搭好场子、走的时候整体还原**。
/// 试玩期间引擎爱怎么改怎么改，出去之后玩家的会话一个字节都没变。
pub struct PreviewSession {
    /// 试玩开始时台词表的长度。引擎只往后追加，截回这个长度即可
    line_len: usize,
    /// `to_snapshot()` 覆盖的场景状态：背景 / 音乐 / 特效 / 在场角色 / 全局变量 …
    scene: crate::ai_service::game_system::game_status::GameStatusSnapshot,
    /// 快照没覆盖的三个字段
    main_role_id: Option<i32>,
    current_role_id: Option<i32>,
    script_status: Option<Box<ScriptStatus>>,
    /// 玩家名。begin 会按绑定角色卡覆盖它，必须单独存还原（scene 快照不含 player）
    user_name: String,
}

impl PreviewSession {
    async fn begin(
        db: &DatabaseConnection,
        data_dir: &Path,
        game_status: &Arc<Mutex<GameStatus>>,
        script: &ScriptStatus,
    ) -> Result<Self, String> {
        // 先确定 MAIN 是谁 —— 定不下来就别开场，免得作者对着不动的画面猜
        let main_id = resolve_preview_main_role(db, game_status, script).await?;

        let mut gs = game_status.lock().await;
        let saved = PreviewSession {
            line_len: gs.line_list.len(),
            scene: gs.to_snapshot(),
            main_role_id: gs.main_role_id,
            current_role_id: gs.current_role_id,
            script_status: gs.script_status.clone().map(Box::new),
            user_name: gs.player.user_name.clone(),
        };

        // ---- 按「刚进游戏」的样子搭场次，对齐 init_game_status 的三件事 ----
        gs.get_role(db, main_id)
            .await
            .map_err(|e| format!("载入主角失败: {}", e))?;
        gs.main_role_id = Some(main_id);
        gs.current_role_id = Some(main_id);
        // 清空在场角色——试玩期间只该有主角一个人在台上。不做这步的话，自由对话
        // 里在场的人物会残留在试玩中，影响立绘站位和引擎上下文（issue #19）。
        gs.present_role_ids.clear();
        gs.onstage_role_ids.clear();
        gs.onstage_role(main_id); // 不做这步立绘不会出现
        // 玩家名（绑定角色卡里的 settings.user_name）。缺了它 %player% 替换为空、
        // 前端玩家气泡也会显示空名（issue #8）。读不到就保持原值，不阻断试玩。
        let uname = user_name_of(db, main_id).await;
        if !uname.is_empty() {
            gs.player.user_name = uname;
        }

        // 人设 SYSTEM 台词。缺了它 role_manager 会警告「人设丢失」，
        // 而且 AI 对话会在没有人设的上下文里生成。
        drop(gs);
        if let Some(prompt) = build_main_role_prompt(db, data_dir, main_id).await {
            let line = crate::ai_service::types::LineBase {
                content: prompt.text,
                attribute: crate::ai_service::types::LineAttributeExt(
                    crate::db::entities::line::LineAttribute::System,
                ),
                sender_role_id: Some(main_id),
                display_name: Some(prompt.name),
                ..Default::default()
            };
            let mut gs = game_status.lock().await;
            if let Err(e) = gs.add_line(db, line).await {
                tracing::warn!("[ScriptEditor] 写入试玩人设台词失败: {}", e);
            }
        } else {
            tracing::warn!("[ScriptEditor] 主角 {} 读不到人设，试玩将缺少人设上下文", main_id);
        }

        Ok(saved)
    }

    /// 尽力还原，任何一步失败都只记日志 —— 收尾阶段再抛错没有接收方，
    /// 而且半途放弃只会让残留更多。
    async fn restore(self, db: &DatabaseConnection, game_status: &Arc<Mutex<GameStatus>>) {
        let mut gs = game_status.lock().await;
        gs.line_list.truncate(self.line_len);
        gs.apply_snapshot(&self.scene);
        gs.main_role_id = self.main_role_id;
        gs.current_role_id = self.current_role_id;
        gs.script_status = self.script_status.map(|b| *b);
        gs.player.user_name = self.user_name;
        // 台词表变短了，角色记忆要按新的列表重建，否则里面还留着试玩的内容
        if let Err(e) = gs.refresh_memories(db).await {
            tracing::warn!("[ScriptEditor] 还原后刷新记忆失败: {}", e);
        }
    }
}

/// 取出托管在 AppState 里的试玩快照并还原（幂等）。
///
/// 试玩任务自然结束时调一次，`editor_stop_preview` 兜底再调一次：先到者拿走
/// `Option` 执行还原，后到者拿到 `None` 直接返回，不会重复还原。
async fn apply_pending_restore(app: &AppHandle) {
    // 注意：app.state() 返回的 State 是借用，必须先用 let 绑定延长生命周期，
    // 否则它作为临时值在本语句结束就被释放，MutexGuard 的借用会悬空（E0716）
    let session = {
        let state = app.state::<AppState>();
        let mut slot = state.pending_preview_restore.lock().await;
        match slot.take() {
            Some(s) => s,
            None => return,
        }
    };
    let state = app.state::<AppState>();
    let db = state.db.clone();
    let game_status = state.ai_service.lock().await.game_status.clone();
    session.restore(&db, &game_status).await;
}

/// 试玩时 `MAIN` 应该解析成谁。
///
/// 引擎里 `character: MAIN` 走 `game_status.main_role_id`，而这个字段是
/// **主菜单选角色**时设的。正式玩羁绊冒险你必然从该角色的角色卡进去，所以它
/// 天然正确；编辑器没有这一步，于是羁绊剧本按 `bound_character_folder` 找人，
/// 独立剧本沿用当前主角。两者都拿不到就直接报错。
async fn resolve_preview_main_role(
    db: &DatabaseConnection,
    game_status: &Arc<Mutex<GameStatus>>,
    script: &ScriptStatus,
) -> Result<i32, String> {
    let bound = script.adventure.bound_character_folder.trim();

    if bound.is_empty() {
        return game_status.lock().await.main_role_id.ok_or_else(|| {
            "这个剧本没有绑定角色，而当前也还没有选定主角。\
             请先回主菜单选一个角色（剧本里的 MAIN 就是他），再回来试玩。"
                .to_string()
        });
    }

    find_main_role_by_folder(db, bound).await?.ok_or_else(|| {
        format!(
            "剧本绑定的角色「{}」不在角色库里。请确认 game_data/characters/ 下有这个目录，\
             或到剧本设置里把「绑定角色目录名」改成实际存在的角色。",
            bound
        )
    })
}

struct MainRolePrompt {
    text: String,
    name: String,
}

/// 按 `init_game_status` 的同一套做法给主角构建人设提示词。
///
/// 只有在角色设定**完全读不到**时才返回 `None`。system_prompt 为空时不返回
/// None——而是走 `sys_prompt_builder_by_settings` 自带的占位回退（与正式游玩
/// `import_settings` 一致），保证 MAIN 始终有一条 SYSTEM 台词；否则 role_manager
/// 会刷「role_id=N 没有找到 SYSTEM 属性的台词，可能人设丢失」（issue #1）。
async fn build_main_role_prompt(
    db: &DatabaseConnection,
    data_dir: &Path,
    role_id: i32,
) -> Option<MainRolePrompt> {
    use crate::utils::prompt::{sys_prompt_builder_by_settings, PromptOptions};

    let settings = RoleRepo::get_role_settings_by_id(db, data_dir, role_id)
        .await
        .ok()
        .flatten()?;
    if settings
        .system_prompt
        .as_deref()
        .unwrap_or("")
        .trim()
        .is_empty()
    {
        // 与正式游玩一致：空人设用占位提示词兜底，AI 对话至少有人设占位，
        // 不至于在没人设的上下文里生成。作者应在角色卡里补上人设。
        tracing::warn!(
            "[ScriptEditor] 主角 {} 未填写人设，已用占位提示词兜底（AI 对话可能不符预期）",
            role_id
        );
    }
    // 用 by_settings 版本而不是自己拼参数：它会一并带上 settings.user_name，
    // 与正式游玩走的是同一条构建路径
    Some(MainRolePrompt {
        text: sys_prompt_builder_by_settings(
            &settings,
            PromptOptions {
                output_sec_lang: true,
                no_emotion_limit: true,
            },
        ),
        name: settings.ai_name,
    })
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

/// 角色卡里写的玩家名（settings.user_name）。查不到或为空返回空串 ——
/// 试玩用它显示玩家身份、替换 %player%，缺了只是显示空，不该阻断试玩（issue #8）。
async fn user_name_of(db: &DatabaseConnection, id: i32) -> String {
    RoleRepo::get_role_settings_by_id(db, &data_dir(), id)
        .await
        .ok()
        .flatten()
        .and_then(|s| {
            let n = s.user_name.trim().to_string();
            if n.is_empty() { None } else { Some(n) }
        })
        .unwrap_or_default()
}

/// 试玩前的可行性检查，供编辑器在打开剧本时提前提示，而不是等作者点了才报错。
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PreviewReadiness {
    /// 能不能直接开跑
    pub ok: bool,
    /// 试玩时 `MAIN` 会是谁；`None` 表示定不下来
    pub main_role_name: Option<String>,
    /// MAIN 对应的 role_id；前端据此载入角色立绘/名字、设 mainRoleId（issue #8）
    pub main_role_id: Option<i32>,
    /// 绑定角色卡里写的玩家名（settings.user_name）；前端用它显示玩家身份、
    /// 后端用它替换 %player%。空字符串表示该角色卡没写玩家名
    pub user_name: String,
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
                main_role_id: Some(id),
                user_name: user_name_of(&db, id).await,
                bound_character_folder: bound,
                reason: None,
            },
            None => PreviewReadiness {
                ok: false,
                main_role_name: None,
                main_role_id: None,
                user_name: String::new(),
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
            main_role_id: Some(id),
            user_name: user_name_of(&db, id).await,
            bound_character_folder: bound,
            reason: None,
        }),
        None => Ok(PreviewReadiness {
            ok: false,
            main_role_name: None,
            main_role_id: None,
            user_name: String::new(),
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

    state
        .ai_service
        .lock()
        .await
        .script_manager
        .is_running
        .store(false, std::sync::atomic::Ordering::SeqCst);

    // 等试玩任务真正收尾——它在结束时会把共享 GameStatus 还原回试玩前。
    // 超时（多半卡在长 AI 请求上）就 abort 并兜底截断，避免退出后自由对话
    // 还读到试玩留下的台词/在场角色（issue #5）。
    if let Some(mut h) = state.preview_task.lock().await.take() {
        let done = tokio::select! {
            r = &mut h => {
                let _ = r;
                true
            }
            _ = tokio::time::sleep(std::time::Duration::from_secs(4)) => false
        };
        if !done {
            h.abort();
            tracing::warn!(
                "[ScriptEditor] 试玩任务 4s 未收尾（可能在跑长 AI 请求），已中止并兜底还原"
            );
        }
    }
    // 幂等兜底：任务已自行还原则 take 为空跳过；超时被中止则现在补上截断
    apply_pending_restore(&app).await;
    Ok(())
}

/// 离开编辑器时把游戏会话整体重置回「刚进游戏」的样子。
///
/// 编辑器内的试玩通过「快照 + 还原」隔离，但多次进出后还原链路存在
/// 竞态窗口（如试玩任务被超时中止时，晚到的 AI 响应可能把剧本台词写回
/// 已还原的 line_list）。与其继续在还原上打补丁，不如在离开编辑器这个
/// 明确边界上直接全量重置：清空 line_list / 在场角色 / player_entered，
/// 主角色重新上台。代价是编辑前后台往返会丢掉自由对话的历史 —— 这是
/// 有意取舍，换取零残留的确定性。
#[tauri::command]
pub async fn editor_reset_game_status(app: AppHandle) -> Result<(), String> {
    let state = app.state::<AppState>();
    let mut service = state.ai_service.lock().await;
    service
        .init_game_status()
        .await
        .map_err(|e| format!("重置游戏状态失败: {}", e))
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
