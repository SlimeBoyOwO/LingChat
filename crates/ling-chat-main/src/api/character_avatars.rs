//! 按角色 ID 管理静态差分，所有文件操作限制在该角色的 avatar 目录。
use std::path::{Component, Path, PathBuf};

use serde::Serialize;
use tauri::{AppHandle, Emitter, Manager};

use crate::{AppState, db::managers::role_repo::RoleRepo};

const EXTENSIONS: &[&str] = &["png", "jpg", "jpeg", "webp", "bmp", "gif"];
pub(super) const EMOTIONS: &[&str] = &[
    "头像", "正常", "平静", "厌恶", "高兴", "担心", "生气", "紧张", "害怕", "害羞", "慌张", "认真",
    "无奈", "兴奋", "疑惑", "伤心", "心动", "调皮", "羞耻", "自信", "惊讶",
];

pub(super) fn validate_segment(value: &str) -> Result<(), String> {
    if value.is_empty()
        || value.contains(['/', '\\', ':'])
        || value.ends_with(['.', ' '])
        || !matches!(
            Path::new(value).components().next(),
            Some(Component::Normal(_))
        )
    {
        return Err("无效的资源名称".into());
    }
    Ok(())
}

pub(super) async fn avatar_dir(
    app: &AppHandle,
    role_id: i32,
    clothes: &str,
) -> Result<PathBuf, String> {
    avatar_dir_for_edit(app, role_id, clothes, None).await
}

pub(super) async fn avatar_dir_for_edit(
    app: &AppHandle,
    role_id: i32,
    clothes: &str,
    edit_id: Option<&str>,
) -> Result<PathBuf, String> {
    let root =
        if let Some(root) = super::character_editor::resource_root(role_id, edit_id, "avatar")? {
            root
        } else {
            let state = app.state::<AppState>();
            let role = RoleRepo::get_role_by_id(&state.db, role_id)
                .await
                .map_err(|e| e.to_string())?
                .ok_or("角色不存在")?;
            let folder = role.resource_folder.as_deref().ok_or("角色资源不存在")?;
            super::resolve_role_dir(&role.role_type, role.script_key.as_deref(), folder)?
        };
    let root = root.canonicalize().map_err(|e| e.to_string())?;
    let mut dir = root.join("avatar");
    if !clothes.is_empty() && clothes != "default" {
        validate_segment(clothes)?;
        dir.push(clothes);
    }
    // 逐层检查已有目录，拒绝指向角色目录之外的符号链接。
    let mut parent = dir.as_path();
    while !parent.exists() {
        parent = parent.parent().ok_or("资源路径不可用")?;
    }
    if !parent
        .canonicalize()
        .map_err(|e| e.to_string())?
        .starts_with(&root)
    {
        return Err("资源路径超出角色目录".into());
    }
    Ok(dir)
}

pub(super) fn avatar_files(
    dir: &Path,
) -> Result<std::collections::BTreeMap<String, Vec<PathBuf>>, String> {
    let mut files: std::collections::BTreeMap<String, Vec<PathBuf>> = Default::default();
    if !dir.exists() {
        return Ok(files);
    }
    for entry in std::fs::read_dir(dir).map_err(|e| e.to_string())? {
        let entry = entry.map_err(|e| e.to_string())?;
        let path = entry.path();
        let emotion = path.file_stem().and_then(|s| s.to_str()).unwrap_or("");
        if !EMOTIONS.contains(&emotion)
            || !EXTENSIONS.contains(
                &path
                    .extension()
                    .and_then(|s| s.to_str())
                    .unwrap_or("")
                    .to_lowercase()
                    .as_str(),
            )
        {
            continue;
        }
        let kind = entry.file_type().map_err(|e| e.to_string())?;
        if kind.is_symlink() {
            return Err("不支持符号链接图片".into());
        }
        if kind.is_file() {
            files.entry(emotion.into()).or_default().push(path);
        }
    }
    for paths in files.values_mut() {
        paths.sort();
    }
    Ok(files)
}

pub(super) fn emotion_files(dir: &Path, emotion: &str) -> Result<Vec<PathBuf>, String> {
    Ok(avatar_files(dir)?.remove(emotion).unwrap_or_default())
}

#[derive(Serialize)]
pub struct AvatarSlot {
    emotion: String,
    path: Option<String>,
    fallback: bool,
}

#[tauri::command]
pub async fn list_character_avatars(
    app: AppHandle,
    role_id: i32,
    clothes: String,
    edit_id: Option<String>,
) -> Result<Vec<AvatarSlot>, String> {
    let dir = avatar_dir_for_edit(&app, role_id, &clothes, edit_id.as_deref()).await?;
    let files = avatar_files(&dir)?;
    EMOTIONS
        .iter()
        .map(|emotion| {
            let mut path = files.get(*emotion).and_then(|paths| paths.first()).cloned();
            let fallback = path.is_none() && *emotion == "平静";
            if fallback {
                path = files.get("正常").and_then(|paths| paths.first()).cloned();
            }
            Ok(AvatarSlot {
                emotion: (*emotion).into(),
                path: path.map(|p| p.to_string_lossy().into_owned()),
                fallback: fallback && files.contains_key("正常"),
            })
        })
        .collect()
}

#[tauri::command]
pub async fn write_character_avatar(
    app: AppHandle,
    role_id: i32,
    clothes: String,
    emotion: String,
    bytes: Vec<u8>,
    edit_id: Option<String>,
) -> Result<(), String> {
    let _guard = super::character_costumes::RESOURCE_LOCK.lock().await;
    if !EMOTIONS.contains(&emotion.as_str()) {
        return Err("不支持的情绪".into());
    }
    if bytes.len() > 20 * 1024 * 1024 {
        return Err("图片不能超过 20 MB".into());
    }
    // 解码后统一存为 PNG，避免扩展名伪装，并限制解压后的内存占用。
    let mut reader = image::ImageReader::new(std::io::Cursor::new(bytes))
        .with_guessed_format()
        .map_err(|e| e.to_string())?;
    let mut limits = image::Limits::default();
    limits.max_image_width = Some(8192);
    limits.max_image_height = Some(8192);
    limits.max_alloc = Some(256 * 1024 * 1024);
    reader.limits(limits);
    let image = reader.decode().map_err(|e| format!("图片无法读取: {e}"))?;
    if let Some(id) = edit_id.as_deref() {
        super::character_editor::prepare_resource(role_id, id, "avatar").await?;
    }
    let dir = avatar_dir_for_edit(&app, role_id, &clothes, edit_id.as_deref()).await?;
    let old = emotion_files(&dir, &emotion)?;
    std::fs::create_dir_all(&dir).map_err(|e| e.to_string())?;
    let mut temporary = tempfile::NamedTempFile::new_in(&dir).map_err(|e| e.to_string())?;
    image
        .write_to(&mut temporary, image::ImageFormat::Png)
        .map_err(|e| e.to_string())?;
    let target = dir.join(format!("{emotion}.png"));
    temporary.persist(&target).map_err(|e| e.to_string())?;
    for file in old {
        if file != target
            && !(cfg!(windows)
                && file
                    .to_string_lossy()
                    .eq_ignore_ascii_case(&target.to_string_lossy()))
        {
            std::fs::remove_file(file).map_err(|e| e.to_string())?;
        }
    }
    if let Some(id) = edit_id.as_deref() {
        super::character_editor::mark_resources_changed(role_id, id, "avatar")?;
    } else {
        app.emit("character:avatars-updated", role_id)
            .map_err(|e| e.to_string())?;
    }
    Ok(())
}

#[tauri::command]
pub async fn delete_character_avatar(
    app: AppHandle,
    role_id: i32,
    clothes: String,
    emotion: String,
    edit_id: Option<String>,
) -> Result<(), String> {
    let _guard = super::character_costumes::RESOURCE_LOCK.lock().await;
    if !EMOTIONS.contains(&emotion.as_str()) {
        return Err("不支持的情绪".into());
    }
    if let Some(id) = edit_id.as_deref() {
        super::character_editor::prepare_resource(role_id, id, "avatar").await?;
    }
    let dir = avatar_dir_for_edit(&app, role_id, &clothes, edit_id.as_deref()).await?;
    for file in emotion_files(&dir, &emotion)? {
        std::fs::remove_file(file).map_err(|e| e.to_string())?;
    }
    if let Some(id) = edit_id.as_deref() {
        super::character_editor::mark_resources_changed(role_id, id, "avatar")?;
    } else {
        app.emit("character:avatars-updated", role_id)
            .map_err(|e| e.to_string())?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rejects_path_traversal_and_windows_aliases() {
        for name in [
            "",
            ".",
            "..",
            "../outside",
            "a/b",
            "a\\b",
            "C:foo",
            "dress.",
            "dress ",
        ] {
            assert!(validate_segment(name).is_err(), "{name}");
        }
        assert!(validate_segment("冬装").is_ok());
    }

    #[test]
    fn finds_all_extensions_without_matching_other_emotions() {
        let dir = tempfile::tempdir().unwrap();
        for name in ["正常.png", "正常.webp", "高兴.png"] {
            std::fs::write(dir.path().join(name), b"test").unwrap();
        }
        assert_eq!(emotion_files(dir.path(), "正常").unwrap().len(), 2);
        assert!(emotion_files(dir.path(), "平静").unwrap().is_empty());
    }
}
