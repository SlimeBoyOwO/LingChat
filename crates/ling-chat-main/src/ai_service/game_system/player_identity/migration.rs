//! 玩家身份迁移：把 A/B 两类老数据统一成一张「主玩家身份」。
//!
//! A 类：没有任何身份数据，新建「玩家A（待填写）」并把老角色卡的玩家名/副标题/设定
//! 收进 alias 与双向关系。
//! B 类：已有身份卡，当前身份直接当主身份沿用（id 不变、内容不改、其他身份卡不动）。
//!
//! 两类共用：老卡 `user_name`/`user_subtitle` 收进 per-character alias，「以下是我的设定」
//! 正文写三处——角色侧关系 `me:<主身份>`、身份侧关系 `ai:<角色>`、主身份 prompt 聚合。
//! 不改数据库引用（save_identity / line.identity_id / affection / relations 键），不归档旧身份卡。
//!
//! 迁移完成后写 `_primary_identity.json`；之后的启动只做增量补齐，不再新建身份。

use std::collections::{HashMap, HashSet};
use std::fs;
use std::path::Path;

use anyhow::{Context, Result};
use sea_orm::{ColumnTrait, DatabaseConnection, EntityTrait, PaginatorTrait, QueryFilter};
use serde_json::{Map, Value};

use super::bootstrap::legacy_player_prompt;
use super::{IdentityStore, PlayerIdentity, RelationEndpoint, role_relations};
use crate::ai_service::types::{CharacterSettings, LegacyIdentity};
use crate::api::resolve_role_dir_in;
use crate::db::entities::role::RoleType;
use crate::db::entities::{line, player_identity, save_identity};
use crate::utils::yaml_file::{read_yaml_as_json, resolve_settings_file, write_json_as_yaml};

const PRIMARY_NAME: &str = "玩家A（待填写）";
/// 老卡里这些名字只是占位，不算「真实旧名」，不生成 alias/legacy。
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
}

#[derive(Default)]
struct RoleSyncOutcome {
    scanned: usize,
    updated: usize,
    identity_relations_changed: bool,
    prompt_changed: bool,
}

#[derive(Default)]
struct RoleCardOutcome {
    alias_changed: bool,
    legacy_changed: bool,
    role_relation_changed: bool,
    identity_relation_changed: bool,
    /// 该卡「以下是我的设定」正文（无则空串），供主身份 prompt 聚合。
    player_setting: String,
}

impl RoleCardOutcome {
    fn any(&self) -> bool {
        self.alias_changed
            || self.legacy_changed
            || self.role_relation_changed
            || self.identity_relation_changed
    }
}

/// 启动时调用：已完成迁移则只做增量补齐。
pub async fn run(data_dir: &Path, db: &DatabaseConnection) -> Result<MigrationReport> {
    let store = IdentityStore::new(data_dir);
    let now = chrono::Local::now().to_rfc3339();

    if let Some(marker) = store.primary_marker() {
        if let Some(primary) = store.find_by_id(&marker.primary_identity_id) {
            return run_incremental(data_dir, db, &store, primary, &now).await;
        }
        tracing::warn!(
            "主身份标记指向的身份卡不存在，重新执行迁移: {}",
            marker.primary_identity_id
        );
    }

    // 没有标记文件时一律按数据形态跑全量迁移：B 迁移可能只写了 is_primary 就崩溃，
    // 角色卡尚未补完，不能靠 is_primary 兜底跳过。
    if has_identity_data(&store, db).await {
        migrate_b(data_dir, db, &store, &now).await
    } else {
        migrate_a(data_dir, db, &store, &now).await
    }
}

async fn run_incremental(
    data_dir: &Path,
    db: &DatabaseConnection,
    store: &IdentityStore,
    mut primary: PlayerIdentity,
    now: &str,
) -> Result<MigrationReport> {
    let outcome = sync_roles(data_dir, &mut primary, now)?;
    if outcome.identity_relations_changed || outcome.prompt_changed {
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
    now: &str,
) -> Result<MigrationReport> {
    let mut primary = store.set_primary(&default_primary_card())?;
    let outcome = sync_roles(data_dir, &mut primary, now)?;
    if outcome.identity_relations_changed || outcome.prompt_changed {
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
        ..Default::default()
    };
    log_report(&report);
    Ok(report)
}

async fn migrate_b(
    data_dir: &Path,
    db: &DatabaseConnection,
    store: &IdentityStore,
    now: &str,
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

    let outcome = sync_roles(data_dir, &mut primary, now)?;
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
        ..Default::default()
    };
    log_report(&report);
    Ok(report)
}

fn log_report(report: &MigrationReport) {
    let mode = report.mode.as_str();
    let primary_id = report.primary_identity_id.as_str();
    tracing::info!(
        "玩家身份迁移完成: mode={mode} primary={primary_id} created={} migrated={} roles={}/{}",
        report.created_primary,
        report.already_migrated,
        report.roles_updated,
        report.roles_scanned
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

/// 把所有老卡「以下是我的设定」正文合并进主身份 prompt：
/// 去掉空段、相同正文去重、已有 prompt 已包含的正文不重复追加，用空行拼接。
/// 返回 prompt 是否有变化。
fn merge_prompt_bodies(prompt: &mut String, bodies: &[String]) -> bool {
    let mut seen: HashSet<&str> = HashSet::new();
    let mut additions: Vec<&str> = Vec::new();
    for body in bodies {
        let body = body.trim();
        if body.is_empty() || !seen.insert(body) {
            continue;
        }
        if prompt.contains(body) {
            continue;
        }
        additions.push(body);
    }
    if additions.is_empty() {
        return false;
    }
    let existing = prompt.trim();
    *prompt = if existing.is_empty() {
        additions.join("\n\n")
    } else {
        format!("{existing}\n\n{}", additions.join("\n\n"))
    };
    true
}

fn sync_roles(
    data_dir: &Path,
    primary: &mut PlayerIdentity,
    migrated_at: &str,
) -> Result<RoleSyncOutcome> {
    let mut outcome = RoleSyncOutcome::default();
    let mut bodies: Vec<String> = Vec::new();
    for folder in main_character_folders(data_dir) {
        outcome.scanned += 1;
        match sync_role_card(data_dir, &folder, primary, migrated_at) {
            Ok(card) => {
                if card.identity_relation_changed {
                    outcome.identity_relations_changed = true;
                }
                if !card.player_setting.trim().is_empty() {
                    bodies.push(card.player_setting);
                }
                if card.any() {
                    outcome.updated += 1;
                }
            },
            Err(e) => tracing::warn!("同步角色卡 {folder} 的玩家身份失败: {e}"),
        }
    }
    if merge_prompt_bodies(&mut primary.prompt, &bodies) {
        outcome.prompt_changed = true;
    }
    Ok(outcome)
}

fn sync_role_card(
    data_dir: &Path,
    folder: &str,
    primary: &mut PlayerIdentity,
    migrated_at: &str,
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

    let Some(obj) = value.as_object_mut() else {
        tracing::warn!(
            "角色设定 {:?} 顶层不是对象，跳过玩家身份同步",
            settings_path
        );
        return Ok(outcome);
    };

    // 只动 user_aliases / legacy_identity，其他字段原样写回。
    let mut aliases = obj
        .get("user_aliases")
        .and_then(Value::as_object)
        .cloned()
        .unwrap_or_default();
    match aliases.get_mut(&primary.id) {
        Some(Value::Object(entry)) => {
            let name_empty = entry
                .get("name")
                .and_then(Value::as_str)
                .map(|text| text.trim().is_empty())
                .unwrap_or(true);
            if name_empty {
                if let Some(name) = legacy_name.as_deref() {
                    entry.insert("name".to_string(), Value::String(name.to_string()));
                    outcome.alias_changed = true;
                }
            }
            let subtitle_empty = entry
                .get("subtitle")
                .and_then(Value::as_str)
                .map(|text| text.trim().is_empty())
                .unwrap_or(true);
            if subtitle_empty && !legacy_subtitle.is_empty() {
                entry.insert(
                    "subtitle".to_string(),
                    Value::String(legacy_subtitle.clone()),
                );
                outcome.alias_changed = true;
            }
        },
        Some(slot) => {
            if let Some(name) = legacy_name.as_deref() {
                *slot = alias_value(name, &legacy_subtitle);
                outcome.alias_changed = true;
            }
        },
        None => {
            if let Some(name) = legacy_name.as_deref() {
                aliases.insert(primary.id.clone(), alias_value(name, &legacy_subtitle));
                outcome.alias_changed = true;
            }
        },
    }
    if outcome.alias_changed {
        obj.insert("user_aliases".to_string(), Value::Object(aliases));
    }

    // 只有真实旧名才写迁移记录；默认名的卡（含 B 类里新建的卡）不算老卡。
    let has_legacy = obj.get("legacy_identity").is_some_and(Value::is_object);
    if !has_legacy {
        if let Some(name) = legacy_name.as_deref() {
            let record = LegacyIdentity {
                migrated_at: migrated_at.to_string(),
                primary_identity_id: primary.id.clone(),
                user_name: name.to_string(),
                user_subtitle: legacy_subtitle.clone(),
            };
            obj.insert(
                "legacy_identity".to_string(),
                serde_json::to_value(&record).context("序列化 legacy_identity 失败")?,
            );
            outcome.legacy_changed = true;
        }
    }

    if outcome.alias_changed || outcome.legacy_changed {
        write_json_as_yaml(&settings_path, &value).map_err(|e| anyhow::anyhow!(e))?;
    }

    // 「以下是我的设定」正文同时写入角色侧 me:<主身份> 与身份侧 ai:<角色文件夹>。
    let body = legacy_player_prompt(&settings);
    outcome.player_setting = body.clone();
    if body.is_empty() {
        return Ok(outcome);
    }
    let me_key = RelationEndpoint::Me(primary.id.clone()).key();
    let mut relations = role_relations::load(data_dir, folder);
    if relations
        .get(&me_key)
        .map(|text| text.trim().is_empty())
        .unwrap_or(true)
    {
        relations.insert(me_key, body.clone());
        match role_relations::save(data_dir, folder, &relations) {
            Ok(()) => outcome.role_relation_changed = true,
            Err(e) => tracing::warn!("写入角色侧关系失败 {folder}: {e}"),
        }
    }

    let ai_key = RelationEndpoint::Ai(folder.to_string()).key();
    if primary
        .relations
        .get(&ai_key)
        .map(|text| text.trim().is_empty())
        .unwrap_or(true)
    {
        primary.relations.insert(ai_key, body);
        outcome.identity_relation_changed = true;
    }

    Ok(outcome)
}

fn alias_value(name: &str, subtitle: &str) -> Value {
    let mut map = Map::new();
    map.insert("name".to_string(), Value::String(name.to_string()));
    map.insert("subtitle".to_string(), Value::String(subtitle.to_string()));
    map.insert("initial_affection".to_string(), Value::Null);
    Value::Object(map)
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn merge_prompt_bodies_dedupes_and_appends() {
        let mut prompt = String::new();
        let bodies = vec![
            "我是莱姆。".to_string(),
            "".to_string(),
            "我是莱姆。".to_string(),
            "  ".to_string(),
            "喜欢喝咖啡。".to_string(),
        ];
        assert!(merge_prompt_bodies(&mut prompt, &bodies));
        assert_eq!(prompt, "我是莱姆。\n\n喜欢喝咖啡。");
        // 已包含的正文不重复追加，幂等
        assert!(!merge_prompt_bodies(&mut prompt, &bodies));
        assert_eq!(prompt, "我是莱姆。\n\n喜欢喝咖啡。");
    }

    #[test]
    fn merge_prompt_bodies_keeps_existing_prompt() {
        let mut prompt = "已有设定。".to_string();
        let bodies = vec!["已有设定。".to_string(), "新设定。".to_string()];
        assert!(merge_prompt_bodies(&mut prompt, &bodies));
        assert_eq!(prompt, "已有设定。\n\n新设定。");
    }
}
