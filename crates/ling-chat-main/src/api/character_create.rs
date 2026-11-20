//! 创建最小有效主角色，后续配置和素材由现有角色编辑会话完成。
use std::{fs, path::Path};

use sea_orm::{ActiveModelTrait, ColumnTrait, DatabaseConnection, EntityTrait, QueryFilter, Set};
use serde::Serialize;
use tauri::{AppHandle, Manager};

use crate::{
    AppState,
    ai_service::types::CharacterSettings,
    db::entities::role::{self, RoleType},
    utils::{path::validate_directory_name, yaml_file::write_json_as_yaml},
};

#[derive(Serialize)]
pub struct CreatedCharacter {
    character_id: i32,
    title: String,
    resource_folder: String,
}

async fn create_in(
    db: &DatabaseConnection,
    base: &Path,
    name: &str,
    folder: &str,
) -> Result<CreatedCharacter, String> {
    let _guard = crate::db::role_sync::ROLE_SYNC_LOCK.lock().await;
    let name = name.trim();
    if name.is_empty() || name.chars().count() > 100 || name.chars().any(char::is_control) {
        return Err("角色名称需为 1–100 个字符，且不能包含控制字符".into());
    }
    let folder = validate_directory_name(folder).map_err(|e| e.replace("分类名", "资源目录名"))?;
    if folder.starts_with('.') || folder.chars().count() > 80 {
        return Err("资源目录名不能以点开头，且不能超过 80 个字符".into());
    }
    let roles = role::Entity::find()
        .filter(role::Column::RoleType.eq(RoleType::Main))
        .all(db)
        .await
        .map_err(|e| format!("检查角色目录失败: {e}"))?;
    if roles.iter().any(|r| {
        r.resource_folder
            .as_deref()
            .is_some_and(|f| f.to_lowercase() == folder.to_lowercase())
    }) {
        return Err("资源目录已被角色使用，请换一个目录名".into());
    }
    fs::create_dir_all(base).map_err(|e| format!("创建角色根目录失败: {e}"))?;
    let base = base.canonicalize().map_err(|e| e.to_string())?;
    for entry in fs::read_dir(&base).map_err(|e| e.to_string())? {
        let entry = entry.map_err(|e| e.to_string())?;
        if entry.file_name().to_string_lossy().to_lowercase() == folder.to_lowercase() {
            return Err("资源目录已存在，请换一个目录名".into());
        }
    }
    let target = base.join(&folder);
    // create_dir 独占目录；只对本次确实创建的目录执行失败清理。
    fs::create_dir(&target).map_err(|e| format!("创建角色目录失败: {e}"))?;
    let result = async {
        fs::create_dir(target.join("avatar")).map_err(|e| e.to_string())?;
        let settings = CharacterSettings {
            ai_name: name.into(),
            user_name: "用户".into(),
            title: Some(name.into()),
            system_prompt: Some(format!(
                "你的名字是{name}，请以这个名字与用户自然交流。用户尚未填写详细人设，不要编造固定背景或关系；根据用户提供的信息逐步了解彼此。"
            )),
            ..Default::default()
        };
        write_json_as_yaml(
            &target.join("settings.yml"),
            &serde_json::to_value(settings).map_err(|e| e.to_string())?,
        )?;
        // 入库为最后一步：失败的单次 INSERT 不会留下角色记录。
        let inserted = role::ActiveModel {
            name: Set(name.into()),
            role_type: Set(RoleType::Main),
            resource_folder: Set(Some(folder.clone())),
            ..Default::default()
        }
        .insert(db)
        .await
        .map_err(|e| format!("角色入库失败: {e}"))?;
        Ok(CreatedCharacter {
            character_id: inserted.id,
            title: name.into(),
            resource_folder: folder,
        })
    }
    .await;
    if let Err(error) = &result {
        crate::utils::path::validate_path_in_base(&target, &base)?;
        fs::remove_dir_all(&target).map_err(|e| format!("{error}; 清理新建角色目录失败: {e}"))?;
    }
    result
}

#[tauri::command]
pub async fn create_character(
    app: AppHandle,
    name: String,
    resource_folder: String,
) -> Result<CreatedCharacter, String> {
    let _guard = super::character::costumes::RESOURCE_LOCK.lock().await;
    let state = app.state::<AppState>();
    create_in(&state.db, &super::characters_dir(), &name, &resource_folder).await
}
