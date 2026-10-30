//! 玩家身份迁移：把 A/B 两类老数据统一成一张「主玩家身份」，并给每张老卡补齐专属身份。
//!
//! A 类：没有任何身份数据，新建「玩家A（待填写）」并把老角色卡的玩家名/副标题/设定
//! 收进 alias 与双向关系。
//! B 类：已有身份卡，当前身份直接当主身份沿用（id 不变、内容不改、其他身份卡不动）。
//!
//! 两类共用：按 (user_name, user_subtitle) 去重，为每张老卡建立专属身份卡，并把角色文件夹
//! 记进身份卡的 `source_folders`（绑定只存在身份卡里，角色 yml 不出现身份 id）。
//! 「以下是我的设定」正文写进身份卡 prompt / `ai:<角色文件夹>` 关系；角色侧认知存数据库
//! `role_identity_relation`，并从 `relations.yml` 的历史 `me:<身份id>` 键迁入。
//! `player_identity` 索引表由 `sync_player_identities` 跟随文件更新。
//!
//! 迁移完成后写 `_primary_identity.json`；之后的启动只做增量补齐，不再新建主身份。

use std::collections::{HashMap, HashSet};
use std::fs;
use std::path::Path;

use anyhow::{Context, Result};
use sea_orm::{ColumnTrait, DatabaseConnection, EntityTrait, PaginatorTrait, QueryFilter};

use super::bootstrap::legacy_player_prompt;
use super::{IdentityStore, PlayerIdentity, RelationEndpoint, role_relations};
use crate::ai_service::types::CharacterSettings;
use crate::api::resolve_role_dir_in;
use crate::db::entities::role::{self, RoleType};
use crate::db::entities::{line, player_identity, save_identity};
use crate::db::managers::role_repo::RoleRepo;
use crate::utils::yaml_file::{read_yaml_as_json, resolve_settings_file, write_json_as_yaml};

const PRIMARY_NAME: &str = "玩家A（待填写）";
/// 老卡里这些名字只是占位，不算「真实旧名」，不建专属身份。
const DEFAULT_USER_NAMES: [&str; 2] = ["user_name未设定", "用户"];

/// 迁移结果，供启动日志与验收排查使用。
#[derive(Clone, Debug, Default)]
pub struct MigrationReport {
    /// `A` / `B` / `incremental`。
    pub mode: String,
    pub primary_identity_id: String,
    pub created_primary: bool,
    pub already_migrated: bool,
    pub roles_scanned: usize,
    pub roles_updated: usize,
    pub identities_created: usize,
    pub identities_bound: usize,
}

#[derive(Default)]
struct RoleSyncOutcome {
    scanned: usize,
    updated: usize,
    primary_changed: bool,
    identities_created: usize,
    identities_bound: usize,
}

#[derive(Default)]
struct RoleCardOutcome {
    cleaned: bool,
    primary_changed: bool,
    identity_created: bool,
    identity_bound: bool,
}

impl RoleCardOutcome {
    fn any(&self) -> bool {
        self.cleaned || self.primary_changed || self.identity_created || self.identity_bound
    }
}

/// 按 (name, subtitle) 与角色文件夹索引身份卡，用于同名老卡合并与绑定反查。
#[derive(Default)]
struct IdentityRegistry {
    primary_key: (String, String),
    by_key: HashMap<(String, String), PlayerIdentity>,
    by_folder: HashMap<String, String>,
    deleted_folders: HashSet<String>,
}

impl IdentityRegistry {
    fn new(
        primary: &PlayerIdentity,
        cards: Vec<PlayerIdentity>,
        deleted_folders: HashSet<String>,
    ) -> Self {
        let primary_key = identity_key(&primary.name, &primary.subtitle);
        let mut registry = Self {
            primary_key: primary_key.clone(),
            by_key: HashMap::new(),
            by_folder: HashMap::new(),
            deleted_folders,
        };
        for folder in &primary.source_folders {
            registry
                .by_folder
                .insert(folder.clone(), primary.id.clone());
        }
        for card in cards {
            registry.insert(card);
        }
        // 主身份的 key 优先指向主身份，避免被同名专属卡抢走。
        if !primary_key.0.is_empty() {
            registry.by_key.insert(primary_key, primary.clone());
        }
        registry
    }

    fn is_primary(&self, key: &(String, String)) -> bool {
        key == &self.primary_key
    }

    fn get(&self, key: &(String, String)) -> Option<&PlayerIdentity> {
        self.by_key.get(key)
    }

    fn insert(&mut self, card: PlayerIdentity) {
        for folder in &card.source_folders {
            self.by_folder.insert(folder.clone(), card.id.clone());
        }
        let key = identity_key(&card.name, &card.subtitle);
        if key.0.is_empty() {
            return;
        }
        self.by_key.insert(key, card);
    }

    fn bind_folder(&mut self, folder: &str, identity_id: &str) {
        self.by_folder
            .insert(folder.to_string(), identity_id.to_string());
    }

    fn bound_identity(&self, folder: &str) -> Option<&str> {
        self.by_folder.get(folder).map(String::as_str)
    }

    fn is_deleted(&self, folder: &str) -> bool {
        self.deleted_folders.contains(folder)
    }
}

/// 启动时调用：已完成迁移则只做增量补齐。
pub async fn run(data_dir: &Path, db: &DatabaseConnection) -> Result<MigrationReport> {
    let store = IdentityStore::new(data_dir);

    if let Some(marker) = store.primary_marker() {
        if let Some(primary) = store.find_by_id(&marker.primary_identity_id) {
            return run_incremental(data_dir, db, &store, primary).await;
        }
        tracing::warn!(
            "主身份标记指向的身份卡不存在，重新执行迁移: {}",
            marker.primary_identity_id
        );
    }

    // 没有标记文件时一律按数据形态跑全量迁移：B 迁移可能只写了 is_primary 就崩溃，
    // 角色卡尚未补完，不能靠 is_primary 兜底跳过。
    if has_identity_data(&store, db).await {
        migrate_b(data_dir, db, &store).await
    } else {
        migrate_a(data_dir, db, &store).await
    }
}

async fn run_incremental(
    data_dir: &Path,
    db: &DatabaseConnection,
    store: &IdentityStore,
    mut primary: PlayerIdentity,
) -> Result<MigrationReport> {
    let mut registry =
        IdentityRegistry::new(&primary, store.list(), store.deleted_source_folders());
    let outcome = sync_roles(data_dir, store, db, &mut primary, &mut registry).await?;
    if outcome.primary_changed {
        primary.is_primary = true;
        store.save(&primary)?;
    }
    store.set_current_id(&primary.id)?;
    // 导入老卡等增量场景同样要刷新身份索引表。
    crate::db::role_sync::sync_player_identities(db, data_dir).await?;

    let report = MigrationReport {
        mode: "incremental".to_string(),
        primary_identity_id: primary.id.clone(),
        roles_scanned: outcome.scanned,
        roles_updated: outcome.updated,
        identities_created: outcome.identities_created,
        identities_bound: outcome.identities_bound,
        already_migrated: true,
        ..Default::default()
    };
    log_report(&report);
    Ok(report)
}

async fn migrate_a(
    data_dir: &Path,
    db: &DatabaseConnection,
    store: &IdentityStore,
) -> Result<MigrationReport> {
    let mut primary = store.set_primary(&default_primary_card())?;
    let mut registry =
        IdentityRegistry::new(&primary, store.list(), store.deleted_source_folders());
    let outcome = sync_roles(data_dir, store, db, &mut primary, &mut registry).await?;
    if outcome.primary_changed {
        primary.is_primary = true;
        store.save(&primary)?;
    }
    store.set_current_id(&primary.id)?;
    store.write_primary_marker(&primary.id, "A")?;
    crate::db::role_sync::sync_player_identities(db, data_dir).await?;

    let report = MigrationReport {
        mode: "A".to_string(),
        primary_identity_id: primary.id.clone(),
        created_primary: true,
        roles_scanned: outcome.scanned,
        roles_updated: outcome.updated,
        identities_created: outcome.identities_created,
        identities_bound: outcome.identities_bound,
        ..Default::default()
    };
    log_report(&report);
    Ok(report)
}

async fn migrate_b(
    data_dir: &Path,
    db: &DatabaseConnection,
    store: &IdentityStore,
) -> Result<MigrationReport> {
    // 新方案不改数据库引用、不归档旧身份卡：当前身份直接当主身份沿用。
    let cards = store.list();
    let mut primary = store
        .current()
        .or_else(|| cards.first().cloned())
        .unwrap_or_else(default_primary_card);
    let created_primary = !cards.iter().any(|card| card.id == primary.id);
    // 新建的默认卡先落盘拿 id；已有卡只在内存里改 is_primary，关系同步完成后统一落盘，
    // 避免中途崩溃被下次启动误判成已迁移。
    if primary.id.trim().is_empty() {
        let mut card = primary.clone();
        card.is_primary = false;
        primary = store.save(&card)?;
    }
    primary.is_primary = true;

    let mut registry =
        IdentityRegistry::new(&primary, store.list(), store.deleted_source_folders());
    let outcome = sync_roles(data_dir, store, db, &mut primary, &mut registry).await?;
    primary = store.save(&primary)?;
    store.set_current_id(&primary.id)?;
    store.write_primary_marker(&primary.id, "B")?;
    crate::db::role_sync::sync_player_identities(db, data_dir).await?;

    let report = MigrationReport {
        mode: "B".to_string(),
        primary_identity_id: primary.id.clone(),
        created_primary,
        roles_scanned: outcome.scanned,
        roles_updated: outcome.updated,
        identities_created: outcome.identities_created,
        identities_bound: outcome.identities_bound,
        ..Default::default()
    };
    log_report(&report);
    Ok(report)
}

fn log_report(report: &MigrationReport) {
    let mode = report.mode.as_str();
    let primary_id = report.primary_identity_id.as_str();
    let created = report.created_primary;
    let migrated = report.already_migrated;
    let updated = report.roles_updated;
    let scanned = report.roles_scanned;
    let identities_created = report.identities_created;
    let identities_bound = report.identities_bound;
    tracing::info!(
        "玩家身份迁移完成: mode={mode} primary={primary_id} created={created} migrated={migrated} roles={updated}/{scanned} identities={identities_created}/{identities_bound}"
    );
}

fn default_primary_card() -> PlayerIdentity {
    PlayerIdentity {
        id: String::new(),
        name: PRIMARY_NAME.to_string(),
        subtitle: PRIMARY_NAME.to_string(),
        prompt: String::new(),
        avatar: None,
        relations: HashMap::new(),
        source_folders: Vec::new(),
        is_primary: true,
    }
}

async fn has_identity_data(store: &IdentityStore, db: &DatabaseConnection) -> bool {
    if !store.list().is_empty() {
        return true;
    }
    if save_identity::Entity::find().count(db).await.unwrap_or(0) > 0 {
        return true;
    }
    if line::Entity::find()
        .filter(line::Column::IdentityId.is_not_null())
        .filter(line::Column::IdentityId.ne(""))
        .count(db)
        .await
        .unwrap_or(0)
        > 0
    {
        return true;
    }
    player_identity::Entity::find().count(db).await.unwrap_or(0) > 0
}

/// 把正文合并进身份卡 prompt：空段或已包含时返回 false。
fn append_prompt_body(prompt: &mut String, body: &str) -> bool {
    let body = body.trim();
    if body.is_empty() || prompt.contains(body) {
        return false;
    }
    let existing = prompt.trim();
    *prompt = if existing.is_empty() {
        body.to_string()
    } else {
        format!("{existing}\n\n{body}")
    };
    true
}

/// 身份去重键：名字 + 副标题，去首尾空白。
fn identity_key(name: &str, subtitle: &str) -> (String, String) {
    (name.trim().to_string(), subtitle.trim().to_string())
}

async fn sync_roles(
    data_dir: &Path,
    store: &IdentityStore,
    db: &DatabaseConnection,
    primary: &mut PlayerIdentity,
    registry: &mut IdentityRegistry,
) -> Result<RoleSyncOutcome> {
    let mut outcome = RoleSyncOutcome::default();
    let role_ids = main_role_ids_by_folder(db).await?;
    for folder in main_character_folders(data_dir) {
        outcome.scanned += 1;
        let role_id = role_ids.get(&folder).copied();
        match sync_role_card(data_dir, store, db, &folder, role_id, primary, registry).await {
            Ok(card) => {
                if card.primary_changed {
                    outcome.primary_changed = true;
                }
                if card.any() {
                    outcome.updated += 1;
                }
                if card.identity_created {
                    outcome.identities_created += 1;
                }
                if card.identity_bound {
                    outcome.identities_bound += 1;
                }
            },
            Err(e) => tracing::warn!("同步角色卡 {folder} 的玩家身份失败: {e}"),
        }
    }
    Ok(outcome)
}

async fn main_role_ids_by_folder(db: &DatabaseConnection) -> Result<HashMap<String, i32>> {
    let roles = role::Entity::find()
        .filter(role::Column::RoleType.eq(RoleType::Main))
        .all(db)
        .await?;
    Ok(roles
        .into_iter()
        .filter_map(|row| row.resource_folder.map(|folder| (folder, row.id)))
        .collect())
}

async fn sync_role_card(
    data_dir: &Path,
    store: &IdentityStore,
    db: &DatabaseConnection,
    folder: &str,
    role_id: Option<i32>,
    primary: &mut PlayerIdentity,
    registry: &mut IdentityRegistry,
) -> Result<RoleCardOutcome> {
    let mut outcome = RoleCardOutcome::default();
    let role_dir = resolve_role_dir_in(data_dir, &RoleType::Main, None, folder)
        .map_err(|e| anyhow::anyhow!(e))?;
    let settings_path = resolve_settings_file(&role_dir);
    if !settings_path.is_file() {
        return Ok(outcome);
    }

    let mut value = read_yaml_as_json(&settings_path).map_err(|e| anyhow::anyhow!(e))?;
    let settings: CharacterSettings = serde_json::from_value(value.clone())
        .with_context(|| format!("解析角色设定失败: {:?}", settings_path))?;
    let legacy_name = real_legacy_name(&settings).map(str::to_string);
    let legacy_subtitle = settings
        .user_subtitle
        .as_deref()
        .unwrap_or_default()
        .trim()
        .to_string();
    let body = legacy_player_prompt(&settings);

    let Some(obj) = value.as_object_mut() else {
        tracing::warn!(
            "角色设定 {:?} 顶层不是对象，跳过玩家身份同步",
            settings_path
        );
        return Ok(outcome);
    };

    // 清理上一版迁移留在角色卡里的数据库键值：关系改存数据库，绑定改存身份卡。
    if obj.remove("user_aliases").is_some() {
        outcome.cleaned = true;
    }
    if obj.remove("legacy_identity").is_some() {
        outcome.cleaned = true;
    }

    // 把 relations.yml 里历史 `me:<身份id>` 键搬进数据库，再从文件里摘掉。
    if let Some(role_id) = role_id {
        let raw_relations = role_relations::load_raw(data_dir, folder);
        let mut kept = HashMap::new();
        let mut moved = false;
        for (key, text) in raw_relations {
            if let Some(identity_id) = key.strip_prefix("me:") {
                moved = true;
                if store.find_by_id(identity_id.trim()).is_some() {
                    RoleRepo::ensure_identity_relation(db, role_id, identity_id, &text).await?;
                }
            } else {
                kept.insert(key, text);
            }
        }
        if moved {
            role_relations::save(data_dir, folder, &kept)
                .with_context(|| format!("清理角色侧关系失败 {folder}"))?;
            outcome.cleaned = true;
        }
    }

    let mut identity_id = registry.bound_identity(folder).map(str::to_string);
    if let Some(bound) = identity_id.as_deref() {
        tracing::debug!("角色卡 {folder} 已绑定身份 {bound}");
    } else if registry.is_deleted(folder) {
        tracing::info!("角色卡 {folder} 的专属身份已被删除，按用户意图不再重建");
    } else if let Some(name) = legacy_name {
        let key = identity_key(&name, &legacy_subtitle);
        let bound_id = if registry.is_primary(&key) {
            // 与主身份同名：复用主身份，不碰主身份 prompt。
            if !primary.source_folders.iter().any(|item| item == folder) {
                primary.source_folders.push(folder.to_string());
                outcome.primary_changed = true;
            }
            let ai_key = RelationEndpoint::Ai(folder.to_string()).key();
            if !body.trim().is_empty()
                && primary
                    .relations
                    .get(&ai_key)
                    .map(|text| text.trim().is_empty())
                    .unwrap_or(true)
            {
                primary.relations.insert(ai_key, body.clone());
                outcome.primary_changed = true;
            }
            registry.bind_folder(folder, &primary.id);
            primary.id.clone()
        } else if let Some(existing) = registry.get(&key).cloned() {
            let mut card = existing;
            let mut changed = false;
            if !card.source_folders.iter().any(|item| item == folder) {
                card.source_folders.push(folder.to_string());
                changed = true;
            }
            let ai_key = RelationEndpoint::Ai(folder.to_string()).key();
            if !body.trim().is_empty()
                && card
                    .relations
                    .get(&ai_key)
                    .map(|text| text.trim().is_empty())
                    .unwrap_or(true)
            {
                card.relations.insert(ai_key, body.clone());
                changed = true;
            }
            if append_prompt_body(&mut card.prompt, &body) {
                changed = true;
            }
            if changed {
                card = store
                    .save(&card)
                    .with_context(|| format!("更新专属玩家身份失败 {folder}"))?;
                registry.insert(card.clone());
            }
            card.id.clone()
        } else {
            let mut relations = HashMap::new();
            if !body.trim().is_empty() {
                relations.insert(RelationEndpoint::Ai(folder.to_string()).key(), body.clone());
            }
            let card = PlayerIdentity {
                id: String::new(),
                name: name.clone(),
                subtitle: legacy_subtitle.clone(),
                prompt: body.clone(),
                avatar: None,
                relations,
                source_folders: vec![folder.to_string()],
                is_primary: false,
            };
            let saved = store
                .save(&card)
                .with_context(|| format!("创建专属玩家身份失败 {folder}"))?;
            if store.find_by_id(&saved.id).is_none() {
                anyhow::bail!("专属玩家身份保存后回读失败: {}", saved.id);
            }
            registry.insert(saved.clone());
            outcome.identity_created = true;
            saved.id
        };

        // 该角色对这张身份的认知存数据库；已有非空文本不覆盖。
        if let Some(role_id) = role_id {
            RoleRepo::ensure_identity_relation(db, role_id, &bound_id, &body).await?;
        }
        identity_id = Some(bound_id);
        outcome.identity_bound = true;
    }

    // 角色卡 yml 的初始好感度只用于初始化数据库里的身份覆写，已有值时不覆盖。
    if let (Some(role_id), Some(identity_id), Some(value)) =
        (role_id, identity_id.as_deref(), settings.initial_affection)
    {
        RoleRepo::ensure_identity_affection(db, role_id, identity_id, value).await?;
    }

    if outcome.cleaned {
        write_json_as_yaml(&settings_path, &value).map_err(|e| anyhow::anyhow!(e))?;
    }

    Ok(outcome)
}

fn real_legacy_name(settings: &CharacterSettings) -> Option<&str> {
    let name = settings.user_name.trim();
    if name.is_empty() || DEFAULT_USER_NAMES.contains(&name) {
        None
    } else {
        Some(name)
    }
}

fn main_character_folders(data_dir: &Path) -> Vec<String> {
    let characters_dir = data_dir.join("game_data").join("characters");
    let Ok(entries) = fs::read_dir(&characters_dir) else {
        return Vec::new();
    };

    let mut folders = Vec::new();
    for entry in entries.flatten() {
        let path = entry.path();
        if !path.is_dir() {
            continue;
        }
        let folder = entry.file_name().to_string_lossy().to_string();
        if folder.starts_with('.') || folder == "avatar" {
            continue;
        }
        if resolve_settings_file(&path).is_file() {
            folders.push(folder);
        }
    }
    folders.sort();
    folders
}
