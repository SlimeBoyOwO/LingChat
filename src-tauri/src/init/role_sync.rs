use std::fs;
use std::path::Path;

use anyhow::{Context, Result};
use sea_orm::{ActiveModelTrait, ColumnTrait, DatabaseConnection, EntityTrait, QueryFilter, Set};
use serde::Deserialize;

use crate::db::entities::role::{self, RoleType};

#[derive(Debug, Deserialize)]
struct CharacterSettings {
    title: Option<String>,
}

pub async fn sync_roles_from_folder(db: &DatabaseConnection, data_dir: &Path) -> Result<Vec<i32>> {
    let characters_dir = data_dir.join("game_data").join("characters");
    if !characters_dir.exists() {
        return Ok(vec![]);
    }

    let mut created_ids = Vec::new();

    for entry in fs::read_dir(&characters_dir)
        .with_context(|| format!("Failed to read {:?}", characters_dir))?
    {
        let entry = entry?;
        if !entry.file_type()?.is_dir() {
            continue;
        }
        let folder_name = entry.file_name().to_string_lossy().to_string();
        // 跳过保留名与隐藏目录:
        // - "avatar" 是资源子目录, 非角色
        // - 以 "." 开头的是隐藏目录 (包括 .import_staging_* 临时解压目录, 避免被误注册为角色)
        if folder_name == "avatar" || folder_name.starts_with('.') {
            continue;
        }

        let settings_path = entry.path().join("settings.yml");
        if !settings_path.exists() {
            continue;
        }

        let title = match load_title(&settings_path) {
            Ok(t) => t.unwrap_or_else(|| folder_name.clone()),
            Err(e) => {
                tracing::warn!("Failed to load {:?}: {}", settings_path, e);
                continue;
            }
        };

        let existing = role::Entity::find()
            .filter(role::Column::ResourceFolder.eq(folder_name.clone()))
            .filter(role::Column::RoleType.eq(RoleType::Main))
            .one(db)
            .await?;

        match existing {
            None => {
                let new_role = role::ActiveModel {
                    name: Set(title),
                    resource_folder: Set(Some(folder_name.clone())),
                    role_type: Set(RoleType::Main),
                    ..Default::default()
                };
                let inserted = new_role.insert(db).await?;
                tracing::info!("Created role: {} ({})", inserted.name, folder_name);
                created_ids.push(inserted.id);
            }
            Some(model) => {
                if model.name != title {
                    let id = model.id;
                    let mut active: role::ActiveModel = model.into();
                    active.name = Set(title);
                    active.update(db).await?;
                    tracing::info!("Updated role #{} name for {}", id, folder_name);
                }
            }
        }
    }

    Ok(created_ids)
}

fn load_title(path: &Path) -> Result<Option<String>> {
    let content = fs::read_to_string(path)?;
    let settings: CharacterSettings = serde_yaml::from_str(&content)?;
    Ok(settings.title.filter(|s| !s.is_empty()))
}
