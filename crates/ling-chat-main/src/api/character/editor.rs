//! 角色编辑会话：资源只写入独立副本，底部保存统一提交，取消销毁副本。
use crate::ai_service::types::CharacterSettings;
use crate::utils::yaml_file::{read_yaml_as_json, resolve_settings_file, write_json_as_yaml};
use serde::Serialize;
use sha2::{Digest, Sha256};
use std::{
    collections::{BTreeSet, HashMap},
    fs,
    path::{Path, PathBuf},
    sync::{LazyLock, Mutex},
};
use tauri::{AppHandle, Emitter, Manager};

struct EditSession {
    role_id: i32,
    original: PathBuf,
    draft: tempfile::TempDir,
    settings_version: Vec<u8>,
    resource_versions: HashMap<String, Vec<u8>>,
    changes: Vec<(String, String)>,
    resources_changed: BTreeSet<String>,
}
static SESSIONS: LazyLock<Mutex<HashMap<String, EditSession>>> = LazyLock::new(Default::default);

const COMMITTED_MARKER: &str = ".committed";

fn remove_editor_directory(path: &Path, base: &Path) -> Result<(), String> {
    if path.is_symlink() {
        return Err("编辑器临时目录不能是符号链接".into());
    }
    crate::utils::path::validate_path_in_base(path, base)?;
    if path.canonicalize().map_err(|e| e.to_string())?
        == base.canonicalize().map_err(|e| e.to_string())?
    {
        return Err("不能删除角色或临时数据根目录".into());
    }
    fs::remove_dir_all(path).map_err(|e| e.to_string())
}

fn remove_empty_parent(path: &Path) {
    if let Err(error) = fs::remove_dir(path) {
        if !matches!(
            error.kind(),
            std::io::ErrorKind::NotFound | std::io::ErrorKind::DirectoryNotEmpty
        ) {
            tracing::warn!(path = %path.display(), "清理编辑器空目录失败: {error}");
        }
    }
}

fn close_session(session: EditSession) {
    let parent = session.draft.path().parent().unwrap().to_path_buf();
    if let Err(error) = session.draft.close() {
        tracing::warn!(path = %parent.display(), "清理角色编辑草稿失败: {error}");
    }
    if parent
        .file_name()
        .is_some_and(|name| name == ".editor-drafts")
    {
        remove_empty_parent(&parent);
    }
}

fn cleanup_committed_backup(backup: &Path, original: &Path) {
    // 标记失败或清理失败都不改变已成功的保存结果。
    if let Err(error) = fs::write(backup.join(COMMITTED_MARKER), b"") {
        tracing::warn!("标记角色备份已提交失败: {error}");
    }
    if let Err(error) = remove_editor_directory(backup, original) {
        tracing::warn!("清理已提交的角色备份失败: {error}");
    }
    remove_empty_parent(&original.join(".editor-trash"));
}

fn cleanup_role_editor_data(root: &Path) -> Result<(), String> {
    let root = root.canonicalize().map_err(|e| e.to_string())?;
    for (name, prefix) in [
        (".editor-drafts", "edit-"),
        (".editor-trash", "edit-backup-"),
    ] {
        let parent = root.join(name);
        if !parent.exists() {
            continue;
        }
        if parent.is_symlink() {
            return Err("编辑器临时目录不能是符号链接".into());
        }
        crate::utils::path::validate_path_in_base(&parent, &root)?;
        for entry in fs::read_dir(&parent).map_err(|e| e.to_string())? {
            let entry = entry.map_err(|e| e.to_string())?;
            if !entry.file_name().to_string_lossy().starts_with(prefix) {
                continue;
            }
            let path = entry.path();
            if !entry.file_type().map_err(|e| e.to_string())?.is_dir() {
                continue;
            }
            if name == ".editor-trash" && !path.join(COMMITTED_MARKER).is_file() {
                // 中断的提交可能把正式资源移到了这里，不能当作垃圾删除。
                tracing::warn!(path = %path.display(), "保留状态不明的角色回滚备份，请检查资源后再处理");
                continue;
            }
            if let Err(error) = remove_editor_directory(&path, &parent) {
                tracing::warn!(path = %path.display(), "清理角色编辑残留失败: {error}");
            }
        }
        remove_empty_parent(&parent);
    }
    Ok(())
}

/// 仅在启动、尚未创建编辑会话时调用；按角色路径覆盖普通、剧本及插件角色。
pub async fn cleanup_stale_editor_data(db: &sea_orm::DatabaseConnection) {
    use sea_orm::EntityTrait;
    let _guard = super::costumes::RESOURCE_LOCK.lock().await;
    let roles = match crate::db::entities::role::Entity::find().all(db).await {
        Ok(roles) => roles,
        Err(error) => {
            tracing::warn!("读取角色编辑残留目录失败: {error}");
            return;
        },
    };
    for role in roles {
        let Some(folder) = role.resource_folder.as_deref() else {
            continue;
        };
        let Ok(root) =
            crate::api::resolve_role_dir(&role.role_type, role.script_key.as_deref(), folder)
        else {
            continue;
        };
        if root.is_dir() {
            if let Err(error) = cleanup_role_editor_data(&root) {
                tracing::warn!(role_id = role.id, "清理角色编辑残留失败: {error}");
            }
        }
    }
}

fn copy_tree(source: &Path, target: &Path) -> Result<(), String> {
    if !source.exists() {
        return Ok(());
    }
    if source.is_symlink() {
        return Err("角色资源不能包含符号链接".into());
    }
    if source.is_dir() {
        fs::create_dir_all(target).map_err(|e| e.to_string())?;
        for entry in fs::read_dir(source).map_err(|e| e.to_string())? {
            let entry = entry.map_err(|e| e.to_string())?;
            copy_tree(&entry.path(), &target.join(entry.file_name()))?;
        }
    } else {
        fs::copy(source, target).map_err(|e| e.to_string())?;
    }
    Ok(())
}

// 素材只比对目录项、大小和修改时间，不读取图片/模型正文。
fn fingerprint(path: &Path) -> Result<Vec<u8>, String> {
    fn stamp(path: &Path, digest: &mut Sha256) -> Result<(), String> {
        if path.is_symlink() {
            return Err("角色资源不能包含符号链接".into());
        }
        if !path.exists() {
            digest.update(b"missing");
            return Ok(());
        }
        let metadata = fs::metadata(path).map_err(|e| e.to_string())?;
        if metadata.is_dir() {
            digest.update(b"directory");
            let mut entries = fs::read_dir(path)
                .map_err(|e| e.to_string())?
                .map(|e| e.map(|e| e.path()))
                .collect::<Result<Vec<_>, _>>()
                .map_err(|e| e.to_string())?;
            entries.sort();
            digest.update((entries.len() as u64).to_le_bytes());
            for entry in entries {
                let name = entry.file_name().unwrap().to_string_lossy();
                digest.update((name.len() as u64).to_le_bytes());
                digest.update(name.as_bytes());
                stamp(&entry, digest)?;
            }
        } else {
            digest.update(b"file");
            digest.update(metadata.len().to_le_bytes());
            let modified = metadata
                .modified()
                .map_err(|e| e.to_string())?
                .duration_since(std::time::UNIX_EPOCH)
                .map_err(|e| e.to_string())?;
            digest.update(modified.as_nanos().to_le_bytes());
        }
        Ok(())
    }
    let mut digest = Sha256::new();
    stamp(path, &mut digest)?;
    Ok(digest.finalize().to_vec())
}

fn settings_version(root: &Path) -> Result<Vec<u8>, String> {
    let path = resolve_settings_file(root);
    let mut digest = Sha256::new();
    digest.update(path.file_name().unwrap().to_string_lossy().as_bytes());
    digest.update(fs::read(path).map_err(|e| e.to_string())?);
    Ok(digest.finalize().to_vec())
}

pub(super) fn resource_root(
    role_id: i32,
    edit_id: Option<&str>,
    resource: &str,
) -> Result<Option<PathBuf>, String> {
    let Some(id) = edit_id else {
        return Ok(None);
    };
    let sessions = SESSIONS.lock().map_err(|e| e.to_string())?;
    let session = sessions
        .get(id)
        .filter(|s| s.role_id == role_id)
        .ok_or("角色编辑草稿已失效")?;
    Ok(Some(if session.resource_versions.contains_key(resource) {
        session.draft.path().to_path_buf()
    } else {
        session.original.clone()
    }))
}

pub(in crate::api) fn file_root(
    role_id: i32,
    edit_id: Option<&str>,
    relative: &str,
) -> Result<Option<PathBuf>, String> {
    let Some(id) = edit_id else {
        return Ok(None);
    };
    let sessions = SESSIONS.lock().map_err(|e| e.to_string())?;
    let session = sessions
        .get(id)
        .filter(|s| s.role_id == role_id)
        .ok_or("角色编辑草稿已失效")?;
    // 后续调用仍会校验文件位于所选根目录内。
    Ok(Some(if session.draft.path().join(relative).exists() {
        session.draft.path().to_path_buf()
    } else {
        session.original.clone()
    }))
}

// 调用者持有 RESOURCE_LOCK。只在真正修改素材时创建资源副本，耗时复制放入阻塞线程池。
pub(in crate::api) async fn prepare_resource(
    role_id: i32,
    edit_id: &str,
    resource: &str,
) -> Result<(), String> {
    if !["avatar", "live2d"].contains(&resource) {
        return Err("无效资源类型".into());
    }
    let (original, draft) = {
        let sessions = SESSIONS.lock().map_err(|e| e.to_string())?;
        let session = sessions
            .get(edit_id)
            .filter(|s| s.role_id == role_id)
            .ok_or("草稿已失效")?;
        if session.resource_versions.contains_key(resource) {
            return Ok(());
        }
        (session.original.clone(), session.draft.path().to_path_buf())
    };
    let resource = resource.to_string();
    let key = resource.clone();
    let expected = tokio::task::spawn_blocking(move || {
        let expected = fingerprint(&original.join(&resource))?;
        let target = draft.join(&resource);
        if target.exists() {
            crate::utils::path::validate_path_in_base(&target, &draft)?;
            fs::remove_dir_all(&target).map_err(|e| e.to_string())?;
        }
        if resource == "live2d" {
            // 导入时才复制模型及相邻纹理，兼容历史资源目录；不覆盖已修改的头像草稿。
            for entry in fs::read_dir(&original).map_err(|e| e.to_string())? {
                let entry = entry.map_err(|e| e.to_string())?;
                let name = entry.file_name();
                let text = name.to_string_lossy();
                if text.starts_with(".editor-")
                    || text.starts_with(".live2d-staging-")
                    || text == ".git"
                    || text.starts_with("settings.yml")
                    || text.starts_with("settings_local.yml")
                    || (text == "avatar" && draft.join(&name).exists())
                {
                    continue;
                }
                copy_tree(&entry.path(), &draft.join(name))?;
            }
        } else {
            copy_tree(&original.join(&resource), &draft.join(&resource))?;
        }
        if fingerprint(&original.join(&resource))? != expected {
            return Err("准备草稿时资源已被其他操作修改，请重试".to_string());
        }
        Ok(expected)
    })
    .await
    .map_err(|e| e.to_string())??;
    let mut sessions = SESSIONS.lock().map_err(|e| e.to_string())?;
    let session = sessions
        .get_mut(edit_id)
        .filter(|s| s.role_id == role_id)
        .ok_or("草稿已失效")?;
    session.resource_versions.insert(key, expected);
    Ok(())
}

pub(in crate::api) fn draft_root(
    role_id: i32,
    edit_id: Option<&str>,
) -> Result<Option<PathBuf>, String> {
    let Some(id) = edit_id else {
        return Ok(None);
    };
    let sessions = SESSIONS.lock().map_err(|e| e.to_string())?;
    let session = sessions
        .get(id)
        .filter(|s| s.role_id == role_id)
        .ok_or("角色编辑草稿已失效，请重新打开配置")?;
    Ok(Some(session.draft.path().to_path_buf()))
}

pub(in crate::api) async fn settings(
    app: AppHandle,
    role_id: i32,
    edit_id: Option<&str>,
) -> Result<CharacterSettings, String> {
    if let Some(root) = draft_root(role_id, edit_id)? {
        serde_json::from_value(read_yaml_as_json(&resolve_settings_file(&root))?)
            .map_err(|e| e.to_string())
    } else {
        super::get_role_settings(app, role_id).await
    }
}

pub(super) fn write_draft(
    role_id: i32,
    edit_id: &str,
    settings: &CharacterSettings,
) -> Result<(), String> {
    let root = draft_root(role_id, Some(edit_id))?.ok_or("草稿不存在")?;
    write_json_as_yaml(
        &resolve_settings_file(&root),
        &serde_json::to_value(settings).map_err(|e| e.to_string())?,
    )
}

pub(super) fn record_costume(
    role_id: i32,
    edit_id: &str,
    old: String,
    new: String,
) -> Result<(), String> {
    let mut sessions = SESSIONS.lock().map_err(|e| e.to_string())?;
    let session = sessions
        .get_mut(edit_id)
        .filter(|s| s.role_id == role_id)
        .ok_or("草稿不存在")?;
    session.changes.push((old, new));
    session.resources_changed.insert("avatar".into());
    Ok(())
}

pub(in crate::api) fn mark_resources_changed(
    role_id: i32,
    edit_id: &str,
    resource: &str,
) -> Result<(), String> {
    let mut sessions = SESSIONS.lock().map_err(|e| e.to_string())?;
    let session = sessions
        .get_mut(edit_id)
        .filter(|s| s.role_id == role_id)
        .ok_or("草稿不存在")?;
    session.resources_changed.insert(resource.into());
    Ok(())
}

#[derive(Serialize)]
pub struct EditorDraft {
    edit_id: String,
    settings: CharacterSettings,
}

#[tauri::command]
pub async fn begin_character_edit(app: AppHandle, role_id: i32) -> Result<EditorDraft, String> {
    let _guard = super::costumes::RESOURCE_LOCK.lock().await;
    let original = super::avatars::avatar_dir(&app, role_id, "default")
        .await?
        .parent()
        .ok_or("角色目录不存在")?
        .to_path_buf();
    let expected = settings_version(&original)?;
    let settings = super::get_role_settings(app.clone(), role_id).await?;
    let parent = original.join(".editor-drafts");
    fs::create_dir_all(&parent).map_err(|e| e.to_string())?;
    if !parent
        .canonicalize()
        .map_err(|e| e.to_string())?
        .starts_with(&original)
    {
        return Err("草稿目录越界".into());
    }
    let draft = tempfile::Builder::new()
        .prefix("edit-")
        .tempdir_in(parent)
        .map_err(|e| e.to_string())?;
    write_json_as_yaml(
        &draft.path().join("settings.yml"),
        &serde_json::to_value(&settings).map_err(|e| e.to_string())?,
    )?;
    if settings_version(&original)? != expected {
        return Err("创建草稿期间角色已被其他操作修改，请重试".into());
    }
    app.asset_protocol_scope()
        .allow_directory(draft.path(), true)
        .map_err(|e| e.to_string())?;
    let edit_id = uuid::Uuid::new_v4().to_string();
    SESSIONS.lock().map_err(|e| e.to_string())?.insert(
        edit_id.clone(),
        EditSession {
            role_id,
            original,
            draft,
            settings_version: expected,
            resource_versions: HashMap::new(),
            changes: Vec::new(),
            resources_changed: BTreeSet::new(),
        },
    );
    Ok(EditorDraft { edit_id, settings })
}

#[tauri::command]
pub async fn discard_character_edit(role_id: i32, edit_id: String) -> Result<(), String> {
    let _guard = super::costumes::RESOURCE_LOCK.lock().await;
    let mut sessions = SESSIONS.lock().map_err(|e| e.to_string())?;
    if sessions.get(&edit_id).is_some_and(|s| s.role_id != role_id) {
        return Err("草稿不属于此角色".into());
    }
    if let Some(session) = sessions.remove(&edit_id) {
        drop(sessions);
        close_session(session);
    }
    Ok(())
}

// 资源移动可回退；失败时草稿仍可重试，正式资源不留半提交状态。
fn move_resources(source: &Path, target: &Path, names: &[&str]) -> Result<Vec<String>, String> {
    let mut moved: Vec<String> = Vec::new();
    for name in names {
        if !source.join(name).exists() {
            continue;
        }
        let result = if target.join(name).exists() {
            Err(std::io::Error::new(
                std::io::ErrorKind::AlreadyExists,
                "目标资源目录已存在",
            ))
        } else {
            fs::rename(source.join(name), target.join(name))
        };
        if let Err(error) = result {
            for name in moved.iter().rev() {
                fs::rename(target.join(name), source.join(name))
                    .map_err(|e| format!("{error}; 恢复资源失败: {e}"))?;
            }
            return Err(error.to_string());
        }
        moved.push((*name).into());
    }
    Ok(moved)
}

#[tauri::command]
pub async fn commit_character_edit(
    app: AppHandle,
    role_id: i32,
    edit_id: String,
    settings: serde_json::Value,
) -> Result<(), String> {
    let _guard = super::costumes::RESOURCE_LOCK.lock().await;
    let validated: CharacterSettings =
        serde_json::from_value(settings.clone()).map_err(|e| e.to_string())?;
    let (original, draft, expected, changes, resources_changed, resource_versions) = {
        let sessions = SESSIONS.lock().map_err(|e| e.to_string())?;
        let s = sessions
            .get(&edit_id)
            .filter(|s| s.role_id == role_id)
            .ok_or("草稿已失效")?;
        (
            s.original.clone(),
            s.draft.path().to_path_buf(),
            s.settings_version.clone(),
            s.changes.clone(),
            s.resources_changed.clone(),
            s.resource_versions.clone(),
        )
    };
    if settings_version(&original)? != expected {
        return Err("角色配置或资源已被其他操作修改，请保留当前内容并重新打开配置后再保存".into());
    }
    for resource in &resources_changed {
        if resource_versions.get(resource) != Some(&fingerprint(&original.join(resource))?) {
            return Err("待保存的角色资源已被其他操作修改，请保留当前内容并重新打开配置".into());
        }
    }
    // 只有资源修改才替换目录，普通配置保存不复制旧模型到回收区。
    let names: Vec<&str> = resources_changed.iter().map(String::as_str).collect();
    let backup = if names.is_empty() {
        None
    } else {
        let trash = original.join(".editor-trash");
        fs::create_dir_all(&trash).map_err(|e| e.to_string())?;
        if !trash
            .canonicalize()
            .map_err(|e| e.to_string())?
            .starts_with(&original)
        {
            return Err("回收区目录越界".into());
        }
        let backup = tempfile::Builder::new()
            .prefix("edit-backup-")
            .tempdir_in(&trash)
            .map_err(|e| e.to_string())?
            .keep();
        if let Err(error) = move_resources(&original, &backup, &names) {
            // 只删空目录；若回滚失败留下素材，保留备份。
            remove_empty_parent(&backup);
            remove_empty_parent(&trash);
            return Err(error);
        }
        if let Err(error) = move_resources(&draft, &original, &names) {
            move_resources(&backup, &original, &names)?;
            if let Err(cleanup_error) = remove_editor_directory(&backup, &original) {
                tracing::warn!("清理已回滚的角色备份失败: {cleanup_error}");
            }
            remove_empty_parent(&trash);
            return Err(error);
        }
        Some(backup)
    };
    if let Err(error) = super::update_role_settings(
        app.clone(),
        role_id,
        serde_json::to_value(validated).map_err(|e| e.to_string())?,
    )
    .await
    {
        if let Some(backup) = &backup {
            move_resources(&original, &draft, &names)?;
            move_resources(backup, &original, &names)?;
            if let Err(cleanup_error) = remove_editor_directory(backup, &original) {
                tracing::warn!("清理已回滚的角色备份失败: {cleanup_error}");
            }
            remove_empty_parent(&original.join(".editor-trash"));
        }
        return Err(error);
    }
    if let Some(backup) = &backup {
        // 仅在正式配置成功保存后标记；清理失败不改变保存结果，启动时再清理。
        cleanup_committed_backup(backup, &original);
    }
    for (old, new) in changes {
        super::costumes::notify_costume_change(&app, role_id, old, new).await;
    }
    let session = SESSIONS.lock().map_err(|e| e.to_string())?.remove(&edit_id);
    if let Some(session) = session {
        close_session(session);
    }
    let _ = app.emit("character:avatars-updated", role_id);
    let _ = app.emit("role:list-updated", ());
    Ok(())
}
