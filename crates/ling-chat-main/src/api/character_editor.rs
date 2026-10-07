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
    fingerprint: Vec<u8>,
    changes: Vec<(String, String)>,
    resources_changed: BTreeSet<String>,
}
static SESSIONS: LazyLock<Mutex<HashMap<String, EditSession>>> = LazyLock::new(Default::default);

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

fn fingerprint(root: &Path) -> Result<Vec<u8>, String> {
    fn hash(path: &Path, digest: &mut Sha256) -> Result<(), String> {
        if path.is_symlink() {
            return Err("角色资源不能包含符号链接".into());
        }
        if !path.exists() {
            digest.update(b"missing");
            return Ok(());
        }
        if path.is_dir() {
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
                hash(&entry, digest)?;
            }
        } else {
            digest.update(b"file");
            digest.update(
                fs::metadata(path)
                    .map_err(|e| e.to_string())?
                    .len()
                    .to_le_bytes(),
            );
            let mut file = fs::File::open(path).map_err(|e| e.to_string())?;
            use std::io::Read;
            let mut buffer = [0u8; 65536];
            loop {
                let count = file.read(&mut buffer).map_err(|e| e.to_string())?;
                if count == 0 {
                    break;
                }
                digest.update(&buffer[..count]);
            }
        }
        Ok(())
    }
    let mut digest = Sha256::new();
    for path in [
        resolve_settings_file(root),
        root.join("avatar"),
        root.join("live2d"),
    ] {
        hash(&path, &mut digest)?;
    }
    Ok(digest.finalize().to_vec())
}

pub(super) fn draft_root(role_id: i32, edit_id: Option<&str>) -> Result<Option<PathBuf>, String> {
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

pub(super) async fn settings(
    app: AppHandle,
    role_id: i32,
    edit_id: Option<&str>,
) -> Result<CharacterSettings, String> {
    if let Some(root) = draft_root(role_id, edit_id)? {
        serde_json::from_value(read_yaml_as_json(&resolve_settings_file(&root))?)
            .map_err(|e| e.to_string())
    } else {
        super::character::get_role_settings(app, role_id).await
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

pub(super) fn mark_resources_changed(
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
    let _guard = super::character_costumes::RESOURCE_LOCK.lock().await;
    let original = super::character_avatars::avatar_dir(&app, role_id, "default")
        .await?
        .parent()
        .ok_or("角色目录不存在")?
        .to_path_buf();
    let expected = fingerprint(&original)?;
    let settings = super::character::get_role_settings(app.clone(), role_id).await?;
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
    // 兼容模型不在 live2d/ 内、纹理在相邻目录的历史资源结构。
    for entry in fs::read_dir(&original).map_err(|e| e.to_string())? {
        let entry = entry.map_err(|e| e.to_string())?;
        let name = entry.file_name();
        let text = name.to_string_lossy();
        if text.starts_with(".editor-")
            || text.starts_with(".live2d-staging-")
            || text == ".git"
            || text.starts_with("settings.yml")
            || text.starts_with("settings_local.yml")
        {
            continue;
        }
        copy_tree(&entry.path(), &draft.path().join(name))?;
    }
    write_json_as_yaml(
        &draft.path().join("settings.yml"),
        &serde_json::to_value(&settings).map_err(|e| e.to_string())?,
    )?;
    if fingerprint(&original)? != expected {
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
            fingerprint: expected,
            changes: Vec::new(),
            resources_changed: BTreeSet::new(),
        },
    );
    Ok(EditorDraft { edit_id, settings })
}

#[tauri::command]
pub async fn discard_character_edit(role_id: i32, edit_id: String) -> Result<(), String> {
    let _guard = super::character_costumes::RESOURCE_LOCK.lock().await;
    let mut sessions = SESSIONS.lock().map_err(|e| e.to_string())?;
    if sessions.get(&edit_id).is_some_and(|s| s.role_id != role_id) {
        return Err("草稿不属于此角色".into());
    }
    sessions.remove(&edit_id);
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
    let _guard = super::character_costumes::RESOURCE_LOCK.lock().await;
    let validated: CharacterSettings =
        serde_json::from_value(settings.clone()).map_err(|e| e.to_string())?;
    let (original, draft, expected, changes, resources_changed) = {
        let sessions = SESSIONS.lock().map_err(|e| e.to_string())?;
        let s = sessions
            .get(&edit_id)
            .filter(|s| s.role_id == role_id)
            .ok_or("草稿已失效")?;
        (
            s.original.clone(),
            s.draft.path().to_path_buf(),
            s.fingerprint.clone(),
            s.changes.clone(),
            s.resources_changed.clone(),
        )
    };
    if fingerprint(&original)? != expected {
        return Err("角色配置或资源已被其他操作修改，请保留当前内容并重新打开配置后再保存".into());
    }
    // 只有资源修改才替换目录，普通配置保存不复制旧模型到回收区。
    let names: Vec<&str> = resources_changed.iter().map(String::as_str).collect();
    // 保留整个旧资源版本，也涵盖草稿中移除的服装。
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
        .tempdir_in(trash)
        .map_err(|e| e.to_string())?
        .keep();
    move_resources(&original, &backup, &names)?;
    if let Err(error) = move_resources(&draft, &original, &names) {
        move_resources(&backup, &original, &names)?;
        return Err(error);
    }
    if let Err(error) = super::character::update_role_settings(
        app.clone(),
        role_id,
        serde_json::to_value(validated).map_err(|e| e.to_string())?,
    )
    .await
    {
        move_resources(&original, &draft, &names)?;
        move_resources(&backup, &original, &names)?;
        return Err(error);
    }
    for (old, new) in changes {
        super::character_costumes::notify_costume_change(&app, role_id, old, new).await;
    }
    if resources_changed.is_empty() {
        let _ = fs::remove_dir(&backup);
    }
    SESSIONS.lock().map_err(|e| e.to_string())?.remove(&edit_id);
    let _ = app.emit("character:avatars-updated", role_id);
    let _ = app.emit("role:list-updated", ());
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn draft_copy_and_cancel_leave_original_unchanged() {
        let root = tempfile::tempdir().unwrap();
        fs::create_dir(root.path().join("avatar")).unwrap();
        fs::write(root.path().join("avatar/正常.png"), b"old").unwrap();
        fs::write(root.path().join("settings.yml"), b"ai_name: original").unwrap();
        let before = fingerprint(root.path()).unwrap();
        let draft = tempfile::tempdir().unwrap();
        copy_tree(&root.path().join("avatar"), &draft.path().join("avatar")).unwrap();
        fs::write(draft.path().join("avatar/正常.png"), b"draft").unwrap();
        drop(draft);
        assert_eq!(fingerprint(root.path()).unwrap(), before);
        fs::write(root.path().join("avatar/正常.png"), b"external").unwrap();
        assert_ne!(fingerprint(root.path()).unwrap(), before);
    }
    #[test]
    fn failed_resource_move_restores_prior_moves() {
        let source = tempfile::tempdir().unwrap();
        let target = tempfile::tempdir().unwrap();
        fs::create_dir(source.path().join("avatar")).unwrap();
        fs::create_dir(source.path().join("live2d")).unwrap();
        fs::write(target.path().join("live2d"), b"conflict").unwrap();
        assert!(move_resources(source.path(), target.path(), &["avatar", "live2d"]).is_err());
        assert!(source.path().join("avatar").is_dir());
        assert!(source.path().join("live2d").is_dir());
        assert!(!target.path().join("avatar").exists());
    }

    #[test]
    fn replacing_resources_can_restore_original_and_keep_draft_for_retry() {
        let original = tempfile::tempdir().unwrap();
        let draft = tempfile::tempdir().unwrap();
        let backup = tempfile::tempdir().unwrap();
        for root in [&original, &draft] {
            fs::create_dir(root.path().join("avatar")).unwrap();
            fs::create_dir(root.path().join("live2d")).unwrap();
        }
        fs::write(original.path().join("avatar/正常.png"), b"original").unwrap();
        fs::write(draft.path().join("avatar/正常.png"), b"edited").unwrap();
        fs::write(draft.path().join("live2d/model.model3.json"), b"imported").unwrap();
        let names = ["avatar", "live2d"];
        move_resources(original.path(), backup.path(), &names).unwrap();
        move_resources(draft.path(), original.path(), &names).unwrap();
        assert_eq!(
            fs::read(original.path().join("avatar/正常.png")).unwrap(),
            b"edited"
        );
        // Simulate settings persistence failure after publishing resource directories.
        move_resources(original.path(), draft.path(), &names).unwrap();
        move_resources(backup.path(), original.path(), &names).unwrap();
        assert_eq!(
            fs::read(original.path().join("avatar/正常.png")).unwrap(),
            b"original"
        );
        assert_eq!(
            fs::read(draft.path().join("avatar/正常.png")).unwrap(),
            b"edited"
        );
        assert!(!original.path().join("live2d/model.model3.json").exists());
        assert!(draft.path().join("live2d/model.model3.json").exists());
    }

    #[tokio::test]
    async fn sessions_reject_other_roles_and_discard_only_their_own_files() {
        let original = tempfile::tempdir().unwrap();
        fs::write(original.path().join("settings.yml"), b"ai_name: original").unwrap();
        let draft = tempfile::tempdir().unwrap();
        let path = draft.path().to_path_buf();
        let id = uuid::Uuid::new_v4().to_string();
        SESSIONS.lock().unwrap().insert(
            id.clone(),
            EditSession {
                role_id: 42,
                original: original.path().to_path_buf(),
                draft,
                fingerprint: fingerprint(original.path()).unwrap(),
                changes: Vec::new(),
                resources_changed: BTreeSet::new(),
            },
        );
        assert!(draft_root(43, Some(&id)).is_err());
        assert!(discard_character_edit(43, id.clone()).await.is_err());
        assert!(path.exists());
        discard_character_edit(42, id.clone()).await.unwrap();
        assert!(!path.exists());
        assert!(draft_root(42, Some(&id)).is_err());
        assert_eq!(
            fs::read(original.path().join("settings.yml")).unwrap(),
            b"ai_name: original"
        );
        discard_character_edit(42, id).await.unwrap();
    }
}
