use std::collections::HashMap;
use std::fs;
use std::path::Path;

use anyhow::{Context, Result};
use sea_orm::{ActiveModelTrait, ColumnTrait, DatabaseConnection, EntityTrait, QueryFilter, Set};
use serde::Deserialize;

use crate::ai_service::game_system::player_identity::{IdentityStore, PlayerIdentity};
use crate::db::entities::role::{self, RoleType};
use crate::db::managers::role_repo::RoleRepo;
use crate::utils::yaml_file::resolve_settings_file;

// 手动创建与目录扫描共享锁，避免扫描读到新目录后重复入库。
pub(crate) static ROLE_SYNC_LOCK: tokio::sync::Mutex<()> = tokio::sync::Mutex::const_new(());

// 官方角色入库顺序：全新数据库下依次得到 1、2、3、4 号，1 号即默认角色。
const OFFICIAL_ROLE_ORDER: &[&str] = &["诺一钦灵", "风雪", "DeepSeek", "松子"];

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

    let mut folders = Vec::new();
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
        folders.push((folder_name, entry.path()));
    }
    // 新库按此顺序自增分配 ID，官方角色优先占前几号，其余按文件夹名排序保证跨平台一致。
    folders.sort_by_key(|(name, _)| {
        let rank = OFFICIAL_ROLE_ORDER
            .iter()
            .position(|f| f == name)
            .unwrap_or(OFFICIAL_ROLE_ORDER.len());
        (rank, name.clone())
    });

    for (folder_name, folder_path) in folders {
        let settings_path = resolve_settings_file(&folder_path);
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

/// 把身份卡投影成 `role_type='player'` 的角色行（文件是真源，本表是索引）。
///
/// 只清理玩家类型且卡片已不存在的行；AI/NPC 角色不受影响。
pub async fn sync_player_roles(db: &DatabaseConnection, data_dir: &Path) -> Result<usize> {
    let cards = IdentityStore::new(data_dir).list();
    let mut written = 0usize;

    for card in &cards {
        let existing = RoleRepo::get_role_by_identity_id(db, &card.id).await?;
        let name = card.display_name().to_string();
        match existing {
            None => {
                role::ActiveModel {
                    name: Set(name.clone()),
                    role_type: Set(RoleType::Player),
                    identity_id: Set(Some(card.id.clone())),
                    identity_relations_seeded: Set(0),
                    ..Default::default()
                }
                .insert(db)
                .await?;
                written += 1;
                tracing::info!("Inserted player role: {} ({})", name, card.id);
            },
            Some(model) => {
                if model.role_type != RoleType::Player
                    || model.name != name
                    || model.resource_folder.is_some()
                {
                    let id = model.id;
                    let mut active: role::ActiveModel = model.into();
                    active.name = Set(name.clone());
                    active.role_type = Set(RoleType::Player);
                    active.resource_folder = Set(None);
                    active.update(db).await?;
                    tracing::info!("Updated player role #{}: {} ({})", id, name, card.id);
                }
            },
        }
    }

    // 全量对齐只针对玩家角色；删除前会解绑存档、改写台词、清理关系。
    let players = role::Entity::find()
        .filter(role::Column::RoleType.eq(RoleType::Player))
        .all(db)
        .await?;
    let mut removed = 0usize;
    for model in players {
        let identity_id = model.identity_id.clone().unwrap_or_default();
        if !cards.iter().any(|card| card.id == identity_id) {
            RoleRepo::delete_player_role(db, model.id).await?;
            removed += 1;
        }
    }

    tracing::info!(
        "Synced player roles: {} cards, {} written, {} removed",
        cards.len(),
        written,
        removed
    );
    Ok(written)
}

/// 为单张身份卡确保玩家角色行存在，返回角色 id（运行时迁移建卡后立即用）。
pub async fn ensure_player_role(db: &DatabaseConnection, card: &PlayerIdentity) -> Result<i32> {
    if let Some(role) = RoleRepo::get_role_by_identity_id(db, &card.id).await? {
        return Ok(role.id);
    }
    let inserted = role::ActiveModel {
        name: Set(card.display_name().to_string()),
        role_type: Set(RoleType::Player),
        identity_id: Set(Some(card.id.clone())),
        identity_relations_seeded: Set(0),
        ..Default::default()
    }
    .insert(db)
    .await?;
    Ok(inserted.id)
}

/// 身份卡里的 relations 只导入一次（`identity_relations_seeded` 标记）：
/// `ai:<文件夹>` / `me:<身份id>` 映射成目标角色 id，写入「玩家角色 → 目标」的认知行。
pub async fn import_player_identity_relations(
    db: &DatabaseConnection,
    data_dir: &Path,
) -> Result<usize> {
    let cards = IdentityStore::new(data_dir).list();
    let mut by_folder: HashMap<String, i32> = HashMap::new();
    for model in role::Entity::find()
        .filter(role::Column::RoleType.is_in([RoleType::Main, RoleType::Npc]))
        .all(db)
        .await?
    {
        if let Some(folder) = model.resource_folder {
            by_folder.insert(folder, model.id);
        }
    }
    let players = role::Entity::find()
        .filter(role::Column::RoleType.eq(RoleType::Player))
        .all(db)
        .await?;
    let by_identity: HashMap<String, i32> = players
        .iter()
        .filter_map(|model| model.identity_id.clone().map(|id| (id, model.id)))
        .collect();

    let mut imported = 0usize;
    for player in players {
        if player.identity_relations_seeded != 0 {
            continue;
        }
        let identity_id = player.identity_id.clone().unwrap_or_default();
        if let Some(card) = cards.iter().find(|card| card.id == identity_id) {
            for (key, text) in &card.relations {
                let Some(target_role_id) = resolve_relation_target(key, &by_folder, &by_identity)
                else {
                    continue;
                };
                if target_role_id == player.id {
                    continue;
                }
                if RoleRepo::ensure_relation(db, player.id, target_role_id, text).await? {
                    imported += 1;
                }
            }
        }
        let mut active: role::ActiveModel = player.into();
        active.identity_relations_seeded = Set(1);
        active.update(db).await?;
    }

    if imported > 0 {
        tracing::info!("Imported {imported} identity relations into role_relation");
    }
    Ok(imported)
}

fn resolve_relation_target(
    key: &str,
    by_folder: &HashMap<String, i32>,
    by_identity: &HashMap<String, i32>,
) -> Option<i32> {
    if let Some(folder) = key.strip_prefix("ai:") {
        return by_folder.get(folder.trim()).copied();
    }
    if let Some(identity_id) = key.strip_prefix("me:") {
        return by_identity.get(identity_id.trim()).copied();
    }
    None
}
