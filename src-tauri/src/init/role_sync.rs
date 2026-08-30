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

/// 一条待入库的插件角色。`encoded_folder` = `plugin:<id>/<folder>`。
pub struct PluginRoleInput {
    pub encoded_folder: String,
    pub title: String,
}

/// 同步插件角色到 DB（决策 5：插件人物可完整对话，入 role 表，role_type=Main）。
///
/// 全量对齐：把 `inputs`（当前所有「启用 + 未隐藏 + 无冲突」的插件角色）逐条 upsert；
/// 再删除所有 `resource_folder` 以 `plugin:` 开头、但不在 `inputs` 里的 Main 行
/// （插件被禁用 / 隐藏 / 删除 / 与游戏冲突时清理）。游戏自有角色不受影响。
pub async fn sync_plugin_roles(
    db: &DatabaseConnection,
    inputs: &[PluginRoleInput],
) -> Result<Vec<i32>> {
    let mut created_ids = Vec::new();
    let mut desired: std::collections::HashSet<String> = std::collections::HashSet::new();

    for input in inputs {
        desired.insert(input.encoded_folder.clone());
        let existing = role::Entity::find()
            .filter(role::Column::ResourceFolder.eq(input.encoded_folder.clone()))
            .filter(role::Column::RoleType.eq(RoleType::Main))
            .one(db)
            .await?;
        match existing {
            None => {
                let new_role = role::ActiveModel {
                    name: Set(input.title.clone()),
                    resource_folder: Set(Some(input.encoded_folder.clone())),
                    role_type: Set(RoleType::Main),
                    ..Default::default()
                };
                let inserted = new_role.insert(db).await?;
                tracing::info!("Created plugin role: {} ({})", inserted.name, input.encoded_folder);
                created_ids.push(inserted.id);
            }
            Some(model) => {
                if model.name != input.title {
                    let mut active: role::ActiveModel = model.into();
                    active.name = Set(input.title.clone());
                    active.update(db).await?;
                }
            }
        }
    }

    // 清理不再有效的插件角色行（禁用 / 隐藏 / 冲突 / 删除）
    let all_plugin = role::Entity::find()
        .filter(role::Column::RoleType.eq(RoleType::Main))
        .all(db)
        .await?
        .into_iter()
        .filter(|r| {
            r.resource_folder
                .as_deref()
                .unwrap_or_default()
                .starts_with("plugin:")
        })
        .collect::<Vec<_>>();
    for model in all_plugin {
        let folder = model.resource_folder.clone().unwrap_or_default();
        if !desired.contains(&folder) {
            // 复用 delete_main_role 的完整级联：删该角色全部存档（含 line/line_perception
            // 级联）、清记忆、解绑其他存档引用，并临时关闭 FK 防御。直接裸 delete_by_id
            // 会因「聊过天」产生的 line_perception/save 等 FK 行而失败，残留行永远清不掉。
            crate::db::managers::role_repo::RoleRepo::delete_main_role(db, model.id).await?;
            tracing::info!("Removed stale plugin role #{} ({})", model.id, folder);
        }
    }

    Ok(created_ids)
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
