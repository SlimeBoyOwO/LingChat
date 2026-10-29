use std::fs;
use std::path::Path;

use anyhow::{Context, Result};
use sea_orm::{ActiveModelTrait, ColumnTrait, DatabaseConnection, EntityTrait, QueryFilter, Set};
use serde::Deserialize;

use crate::ai_service::game_system::player_identity::IdentityStore;
use crate::db::entities::player_identity;
use crate::db::entities::role::{self, RoleType};
use crate::utils::yaml_file::resolve_settings_file;

// 手动创建与目录扫描共享锁，避免扫描读到新目录后重复入库。
pub(crate) static ROLE_SYNC_LOCK: tokio::sync::Mutex<()> = tokio::sync::Mutex::const_new(());

#[derive(Debug, Deserialize)]
struct CharacterSettings {
    title: Option<String>,
}

/// 一条待入库的插件角色。`encoded_folder` = `plugin:<id>/<folder>`。
pub struct PluginRoleInput {
    pub encoded_folder: String,
    pub title: String,
}

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
                tracing::info!(
                    "Created plugin role: {} ({})",
                    inserted.name,
                    input.encoded_folder
                );
                created_ids.push(inserted.id);
            },
            Some(model) => {
                if model.name != input.title {
                    let mut active: role::ActiveModel = model.into();
                    active.name = Set(input.title.clone());
                    active.update(db).await?;
                }
            },
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
            // 复用 delete_main_role 的完整级联，裸 delete_by_id 会因 FK 残留行而失败。
            crate::db::managers::role_repo::RoleRepo::delete_main_role(db, model.id).await?;
            tracing::info!("Removed stale plugin role #{} ({})", model.id, folder);
        }
    }

    Ok(created_ids)
}

pub async fn sync_roles_from_folder(db: &DatabaseConnection, data_dir: &Path) -> Result<Vec<i32>> {
    let _guard = ROLE_SYNC_LOCK.lock().await;
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
        if folder_name == "avatar" || folder_name.starts_with('.') {
            continue;
        }

        let settings_path = resolve_settings_file(&entry.path());
        if !settings_path.exists() {
            continue;
        }

        let title = match load_title(&settings_path) {
            Ok(t) => t.unwrap_or_else(|| folder_name.clone()),
            Err(e) => {
                tracing::warn!("Failed to load {:?}: {}", settings_path, e);
                continue;
            },
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
            },
            Some(model) => {
                if model.name != title {
                    let id = model.id;
                    let mut active: role::ActiveModel = model.into();
                    active.name = Set(title);
                    active.update(db).await?;
                    tracing::info!("Updated role #{} name for {}", id, folder_name);
                }
            },
        }
    }

    Ok(created_ids)
}

fn load_title(path: &Path) -> Result<Option<String>> {
    let content = fs::read_to_string(path)?;
    let settings: CharacterSettings = serde_yaml::from_str(&content)?;
    Ok(settings.title.filter(|s| !s.is_empty()))
}

/// 同步身份卡到 `player_identity` 索引表（文件是真源，本表是索引）。
pub async fn sync_player_identities(db: &DatabaseConnection, data_dir: &Path) -> Result<usize> {
    let cards = IdentityStore::new(data_dir).list();
    let mut inserted = 0usize;

    for card in &cards {
        let existing = player_identity::Entity::find_by_id(card.id.clone())
            .one(db)
            .await?;
        match existing {
            None => {
                let active = player_identity::ActiveModel {
                    identity_id: Set(card.id.clone()),
                    name: Set(card.name.clone()),
                    subtitle: Set(card.subtitle.clone()),
                    updated_at: Set(now_string()),
                };
                active.insert(db).await?;
                inserted += 1;
                tracing::info!("Inserted player identity: {} ({})", card.name, card.id);
            },
            Some(model) => {
                if model.name != card.name || model.subtitle != card.subtitle {
                    let mut active: player_identity::ActiveModel = model.into();
                    active.name = Set(card.name.clone());
                    active.subtitle = Set(card.subtitle.clone());
                    active.updated_at = Set(now_string());
                    active.update(db).await?;
                    tracing::info!("Updated player identity: {} ({})", card.name, card.id);
                }
            },
        }
    }

    // 全量对齐：表中存在而文件中已不存在的行删除。
    let all = player_identity::Entity::find().all(db).await?;
    let mut removed = 0usize;
    for model in all {
        if !cards.iter().any(|c| c.id == model.identity_id) {
            let active: player_identity::ActiveModel = model.into();
            active.delete(db).await?;
            removed += 1;
        }
    }

    tracing::info!(
        "Synced player identities: {} cards, {} inserted, {} removed",
        cards.len(),
        inserted,
        removed
    );
    Ok(inserted)
}

fn now_string() -> String {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs().to_string())
        .unwrap_or_else(|_| "0".to_string())
}
