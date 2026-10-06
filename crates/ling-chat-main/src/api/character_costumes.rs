use serde::Serialize;
use tauri::{AppHandle, Emitter, Manager};
use tauri_plugin_store::StoreExt;

use super::character_avatars::{EMOTIONS, avatar_dir, emotion_files, validate_segment};
use crate::{AppState, ai_service::types::CharacterSettings, config};

// 与差分增删共享锁，防止上传图片时服装目录被改名。
pub(super) static RESOURCE_LOCK: tokio::sync::Mutex<()> = tokio::sync::Mutex::const_new(());

#[derive(Serialize)]
pub struct CostumeSummary {
    name: String,
    preview: Option<String>,
    present: usize,
    total: usize,
    missing: Vec<String>,
}

#[tauri::command]
pub async fn list_character_costumes(
    app: AppHandle,
    role_id: i32,
) -> Result<Vec<CostumeSummary>, String> {
    let _guard = RESOURCE_LOCK.lock().await;
    let settings = super::character::get_role_settings(app.clone(), role_id).await?;
    let root = avatar_dir(&app, role_id, "default").await?;
    let mut names = std::collections::BTreeSet::new();
    names.insert("default".to_string());
    for item in settings.clothes.unwrap_or_default() {
        if let Some(name) = item.get("name").filter(|n| !n.is_empty()) {
            names.insert(name.clone());
        }
    }
    if root.exists() {
        for entry in std::fs::read_dir(&root).map_err(|e| e.to_string())? {
            let entry = entry.map_err(|e| e.to_string())?;
            if entry.file_type().map_err(|e| e.to_string())?.is_dir() {
                names.insert(entry.file_name().to_string_lossy().into_owned());
            }
        }
    }
    let mut result = Vec::new();
    for name in names {
        let dir = avatar_dir(&app, role_id, &name).await?;
        let preview = emotion_files(&dir, "正常")?.into_iter().next();
        let mut missing = Vec::new();
        for emotion in EMOTIONS.iter().filter(|name| **name != "头像") {
            if emotion_files(&dir, emotion)?.is_empty()
                && !(*emotion == "平静" && preview.is_some())
            {
                missing.push((*emotion).to_string());
            }
        }
        let total = EMOTIONS.len() - 1;
        result.push(CostumeSummary {
            name,
            preview: preview.map(|p| p.to_string_lossy().into_owned()),
            present: total - missing.len(),
            total,
            missing,
        });
    }
    Ok(result)
}

fn remap_settings(
    settings: &mut CharacterSettings,
    old: &str,
    new: Option<&str>,
) -> Result<(), String> {
    if let Some(new) = new {
        if settings.clothes.as_ref().is_some_and(|items| {
            items
                .iter()
                .any(|i| i.get("name").is_some_and(|n| n == new))
        }) {
            return Err("目标服装名已存在".into());
        }
    }
    if let Some(items) = &mut settings.clothes {
        if let Some(new) = new {
            let mut found = false;
            for item in items.iter_mut() {
                if item.get("name").is_some_and(|name| name == old) {
                    item.insert("name".into(), new.into());
                    found = true;
                }
            }
            if !found {
                items.push(std::collections::HashMap::from([
                    ("name".into(), new.into()),
                    ("prompt".into(), String::new()),
                ]));
            }
        } else {
            items.retain(|item| !item.get("name").is_some_and(|name| name == old));
        }
    } else if let Some(new) = new {
        settings.clothes = Some(vec![std::collections::HashMap::from([
            ("name".into(), new.into()),
            ("prompt".into(), String::new()),
        ])]);
    }
    if old.is_empty() {
        return Ok(());
    }
    if settings.clothes_name.as_deref() == Some(old) {
        settings.clothes_name = Some(new.unwrap_or("default").into());
    }
    if let Some(live2d) = &mut settings.live2d {
        if new.is_some_and(|n| live2d.clothes_variants.contains_key(n)) {
            return Err("目标服装已有 Live2D 映射".into());
        }
        if let Some(value) = live2d.clothes_variants.remove(old) {
            if let Some(new) = new {
                live2d.clothes_variants.insert(new.into(), value);
            }
        }
    }
    if let Some(parts) = &mut settings.body_part {
        if new.is_some_and(|n| parts.contains_key(n)) {
            return Err("目标服装已有触摸区域".into());
        }
        if let Some(value) = parts.remove(old) {
            if let Some(new) = new {
                parts.insert(new.into(), value);
            }
        }
        // 旧触摸配置在每个部位内部记录 clothesName，不改变其多边形坐标。
        let mut remove = Vec::new();
        for (key, value) in parts.iter_mut() {
            if value.get("clothesName").and_then(|v| v.as_str()) == Some(old) {
                if let Some(new) = new {
                    value["clothesName"] = new.into();
                } else {
                    remove.push(key.clone());
                }
            }
        }
        for key in remove {
            parts.remove(&key);
        }
    }
    Ok(())
}

#[derive(Clone, Serialize)]
pub struct CostumeChange {
    role_id: i32,
    old_name: String,
    new_name: String,
}

#[tauri::command]
pub async fn manage_character_costume(
    app: AppHandle,
    role_id: i32,
    action: String,
    old_name: String,
    new_name: String,
    settings: serde_json::Value,
) -> Result<CharacterSettings, String> {
    let _guard = RESOURCE_LOCK.lock().await;
    let mut settings: CharacterSettings =
        serde_json::from_value(settings).map_err(|e| e.to_string())?;
    if !["create", "rename", "remove"].contains(&action.as_str()) {
        return Err("不支持的服装操作".into());
    }
    if action == "create" && !old_name.is_empty() {
        return Err("新增服装不应指定旧名称".into());
    }
    if action != "create" {
        validate_segment(&old_name)?;
    }
    if action != "remove" {
        validate_segment(&new_name)?;
    }
    if ["default", "默认"].contains(&old_name.as_str())
        || ["default", "默认"].contains(&new_name.as_str())
    {
        return Err("默认服装不能被重命名或移除".into());
    }
    let root = avatar_dir(&app, role_id, "default").await?;
    let source = if action == "create" {
        None
    } else {
        Some(avatar_dir(&app, role_id, &old_name).await?)
    };
    let target = if action == "remove" {
        None
    } else {
        Some(avatar_dir(&app, role_id, &new_name).await?)
    };
    if target.as_ref().is_some_and(|p| p.exists()) {
        return Err("目标服装目录已存在".into());
    }
    remap_settings(
        &mut settings,
        &old_name,
        if action == "remove" {
            None
        } else {
            Some(&new_name)
        },
    )?;
    std::fs::create_dir_all(&root).map_err(|e| e.to_string())?;
    // 移除资源先移入角色目录的回收区，不递归删除用户素材。
    let trash = if action == "remove" && source.as_ref().is_some_and(|p| p.exists()) {
        let parent = root.parent().ok_or("角色目录不可用")?.join(".editor-trash");
        std::fs::create_dir_all(&parent).map_err(|e| e.to_string())?;
        if !parent
            .canonicalize()
            .map_err(|e| e.to_string())?
            .starts_with(root.parent().ok_or("角色目录不可用")?)
        {
            return Err("回收区路径超出角色目录".into());
        }
        Some(
            tempfile::Builder::new()
                .prefix("costume-")
                .tempdir_in(parent)
                .map_err(|e| e.to_string())?
                .keep()
                .join(&old_name),
        )
    } else {
        None
    };
    let destination = target.as_ref().or(trash.as_ref());
    let moved = source.as_ref().is_some_and(|p| p.exists());
    if moved {
        std::fs::rename(
            source.as_ref().unwrap(),
            destination.ok_or("目标目录不可用")?,
        )
        .map_err(|e| e.to_string())?;
    } else if let Some(target) = &target {
        std::fs::create_dir(target).map_err(|e| e.to_string())?;
    }
    let saved = super::character::update_role_settings(
        app.clone(),
        role_id,
        serde_json::to_value(&settings).map_err(|e| e.to_string())?,
    )
    .await;
    if let Err(error) = saved {
        if moved {
            std::fs::rename(destination.unwrap(), source.as_ref().unwrap())
                .map_err(|e| format!("{error}; 恢复服装目录失败: {e}"))?;
        } else if let Some(target) = &target {
            std::fs::remove_dir(target).map_err(|e| format!("{error}; 恢复服装目录失败: {e}"))?;
        }
        return Err(error);
    }
    let replacement = if action == "remove" {
        "default"
    } else {
        &new_name
    };
    {
        let state = app.state::<AppState>();
        let service = state.ai_service.lock().await;
        service
            .game_status
            .lock()
            .await
            .role_manager
            .remap_character_costume(role_id, &old_name, replacement);
    }
    if let Ok(store) = app.store(config::STORE_FILE) {
        let key = config::session::last_clothes_key(role_id);
        if store
            .get(&key)
            .and_then(|v| v.as_str().map(str::to_owned))
            .as_deref()
            == Some(&old_name)
        {
            store.set(key, serde_json::Value::String(replacement.into()));
            if let Err(error) = store.save() {
                tracing::warn!("服装会话保存失败: {error}");
            }
        }
    }
    app.emit(
        "character:costume-renamed",
        CostumeChange {
            role_id,
            old_name,
            new_name: replacement.into(),
        },
    )
    .map_err(|e| e.to_string())?;
    app.emit("role:list-updated", ())
        .map_err(|e| e.to_string())?;
    super::character::get_role_settings(app, role_id).await
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rename_preserves_prompts_and_remaps_touch_and_models() {
        let mut settings: CharacterSettings = serde_json::from_value(serde_json::json!({
            "clothes": [{"name":"冬装","prompt":"暖和的外套"}], "clothes_name":"冬装",
            "body_part":{"冬装":{"head":{"polygons":[[[0,0],[1,0],[0,1]]]}},"ear":{"clothesName":"冬装","X":[1,2,3]}},
            "live2d":{"version":1,"default_variant":"winter","variants":{},"clothes_variants":{"冬装":"winter"}}
        })).unwrap();
        remap_settings(&mut settings, "冬装", Some("外套")).unwrap();
        assert_eq!(
            settings.clothes.as_ref().unwrap()[0]["prompt"],
            "暖和的外套"
        );
        assert_eq!(settings.clothes_name.as_deref(), Some("外套"));
        assert_eq!(
            settings.live2d.as_ref().unwrap().clothes_variants["外套"],
            "winter"
        );
        assert!(settings.body_part.as_ref().unwrap().contains_key("外套"));
        assert_eq!(
            settings.body_part.as_ref().unwrap()["ear"]["clothesName"],
            "外套"
        );
        remap_settings(&mut settings, "外套", None).unwrap();
        assert_eq!(settings.clothes_name.as_deref(), Some("default"));
        assert!(settings.clothes.unwrap().is_empty());
        assert!(settings.live2d.unwrap().clothes_variants.is_empty());
        assert!(settings.body_part.unwrap().is_empty());
    }
}
