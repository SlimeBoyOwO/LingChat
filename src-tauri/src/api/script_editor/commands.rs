//! 剧本编辑器的 Tauri 命令。
//!
//! 这一层之前完全不存在 —— `api/script.rs` 只有 5 个只读命令，剧本从前端视角
//! 是只读的，而 `fs` 插件的 scope 也覆盖不到剧本目录。所有写入都必须走这里。
//!
//! 命名统一 `editor_` 前缀，避免与既有的 `list_scripts` 等混淆。

use std::collections::HashMap;
use std::path::Path;

use serde::{Deserialize, Serialize};
use serde_json::{Map, Value as JsonValue};
use tauri::{AppHandle, Manager};

use crate::api::{data_dir, game_data_dir};
use crate::AppState;

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

fn read_package(key: &str, loaded_names: &HashMap<String, ()>) -> Result<ScriptPackage, String> {
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
        loaded_by_engine: loaded_names.contains_key(&script_name),
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
async fn loaded_script_names(app: &AppHandle) -> HashMap<String, ()> {
    let state = app.state::<AppState>();
    let service = state.ai_service.lock().await;
    service
        .script_manager
        .all_scripts
        .keys()
        .map(|k| (k.clone(), ()))
        .collect()
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

#[tauri::command]
pub fn editor_rename_chapter(key: String, from: String, to: String) -> Result<(), String> {
    let dir = paths::resolve_script_dir(&key)?;
    for seg in to.split('/') {
        paths::sanitize_folder_name(seg)?;
    }
    let src = paths::resolve_chapter_file(&dir, &from, true)?;
    let dest = paths::resolve_chapter_file(&dir, &to, false)?;
    if dest.exists() {
        return Err(format!("目标章节已存在: '{}'", to));
    }
    std::fs::rename(&src, &dest).map_err(|e| format!("重命名失败: {}", e))?;
    Ok(())
}

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

    std::fs::rename(&dir, &dest).map_err(|e| {
        format!(
            "移动剧本到回收目录失败: {}。剧本仍在原处，未做任何删除",
            e
        )
    })?;
    Ok(())
}

/// 上传素材到剧本自带的 Assets 子目录。
///
/// `kind` 取 background / music / sound / ambient / pic，与 schema 的 assetKind 一致。
#[tauri::command]
pub fn editor_upload_asset(
    key: String,
    kind: String,
    file_name: String,
    data: Vec<u8>,
) -> Result<String, String> {
    let dir = paths::resolve_script_dir(&key)?;
    let name = paths::sanitize_folder_name(&file_name)?;

    // 只放进 media.rs 候选列表的**第一个**目录，保证引擎一定能找到
    let subdir = match kind.as_str() {
        "background" => "Backgrounds",
        "music" => "Musics",
        "sound" => "Sounds",
        "ambient" => "Ambients",
        "pic" => "Pics",
        other => return Err(format!("未知素材类别: {}", other)),
    };

    let ext = Path::new(&name)
        .extension()
        .map(|e| e.to_string_lossy().to_lowercase())
        .unwrap_or_default();
    let allowed: &[&str] = match kind.as_str() {
        "background" | "pic" => &["png", "jpg", "jpeg", "webp", "bmp", "gif"],
        _ => &["mp3", "wav", "ogg", "flac", "m4a"],
    };
    if !allowed.contains(&ext.as_str()) {
        return Err(format!(
            "不支持的文件类型 .{}；{} 支持: {}",
            ext,
            kind,
            allowed.join(" / ")
        ));
    }

    const MAX_BYTES: usize = 64 * 1024 * 1024;
    if data.len() > MAX_BYTES {
        return Err(format!("文件过大（上限 {} MB）", MAX_BYTES / 1024 / 1024));
    }

    let target_dir = dir.join("Assets").join(subdir);
    std::fs::create_dir_all(&target_dir).map_err(|e| format!("无法创建素材目录: {}", e))?;
    let target = target_dir.join(&name);
    if target.exists() {
        return Err(format!("素材「{}」已存在", name));
    }
    io::atomic_write(&target, &data)?;
    Ok(name)
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

/// 重新扫描剧本目录，把新写/改名的剧本加载进引擎。
///
/// 引擎原本只在启动时扫一次，作者存完剧本必须重启整个应用才能试玩。
#[tauri::command]
pub async fn editor_rescan_scripts(app: AppHandle) -> Result<usize, String> {
    let state = app.state::<AppState>();
    let mut service = state.ai_service.lock().await;

    if service.script_manager.is_running.load(std::sync::atomic::Ordering::SeqCst) {
        return Err("有剧本正在运行，请先结束再重新扫描".to_string());
    }

    let data = service.data_dir.clone();
    service.script_manager =
        crate::ai_service::game_system::script_engine::ScriptManager::new(&data);
    let count = service.script_manager.all_scripts.len();
    tracing::info!("[ScriptEditor] 重新扫描完成，共 {} 个剧本", count);
    Ok(count)
}

/// 在系统文件管理器里打开剧本目录。
#[tauri::command]
pub fn editor_open_script_folder(key: String) -> Result<(), String> {
    let dir = paths::resolve_script_dir(&key)?;
    // open_folder 收的是 &str，不是 &Path
    crate::utils::system::open_folder(&dir.to_string_lossy())
}
