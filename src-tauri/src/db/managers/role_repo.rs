use std::fs;
use std::path::{Path, PathBuf};

use anyhow::{Context, Result};
use sea_orm::{ActiveModelTrait, ColumnTrait, DatabaseConnection, EntityTrait, QueryFilter, Set};

use crate::ai_service::types::CharacterSettings;
use crate::db::entities::role::{self, ActiveModel as RoleActiveModel, Model as RoleModel, RoleType};

pub struct RoleRepo;

impl RoleRepo {
    pub async fn get_role_by_id(db: &DatabaseConnection, role_id: i32) -> Result<Option<RoleModel>> {
        Ok(role::Entity::find_by_id(role_id).one(db).await?)
    }

    pub async fn get_role_by_script_keys(
        db: &DatabaseConnection,
        script_key: &str,
        script_role_key: &str,
    ) -> Result<Option<RoleModel>> {
        Ok(role::Entity::find()
            .filter(role::Column::ScriptKey.eq(script_key))
            .filter(role::Column::ScriptRoleKey.eq(script_role_key))
            .one(db)
            .await?)
    }

    pub async fn get_script_roles(
        db: &DatabaseConnection,
        script_key: &str,
    ) -> Result<Vec<RoleModel>> {
        Ok(role::Entity::find()
            .filter(role::Column::ScriptKey.eq(script_key))
            .all(db)
            .await?)
    }

    /// Find an existing role by script keys, or create a new one.
    pub async fn find_or_create_role(
        db: &DatabaseConnection,
        name: &str,
        role_type: RoleType,
        script_key: Option<&str>,
        script_role_key: Option<&str>,
    ) -> Result<i32> {
        // Try to find existing
        if let (Some(sk), Some(srk)) = (script_key, script_role_key) {
            if let Some(existing) = Self::get_role_by_script_keys(db, sk, srk).await? {
                return Ok(existing.id);
            }
        }

        // Create new
        let active = RoleActiveModel {
            name: Set(name.to_string()),
            role_type: Set(role_type),
            script_key: Set(script_key.map(|s| s.to_string())),
            script_role_key: Set(script_role_key.map(|s| s.to_string())),
            ..Default::default()
        };
        let inserted = active.insert(db).await?;
        Ok(inserted.id)
    }

    pub async fn get_all_main_roles(db: &DatabaseConnection) -> Result<Vec<RoleModel>> {
        Ok(role::Entity::find()
            .filter(role::Column::RoleType.eq(RoleType::Main))
            .all(db)
            .await?)
    }

    /// 确保 role 表中存在 id=0 的 User 角色（代表人类玩家）。
    /// 若已有 id=0 的行但名称/类型不匹配，则更新为正确值。
    /// 幂等操作，每次启动调用。
    pub async fn ensure_user_role(db: &DatabaseConnection) -> Result<()> {
        if let Some(existing) = role::Entity::find_by_id(0).one(db).await? {
            if existing.name != "User" || existing.role_type != RoleType::User {
                let mut active: role::ActiveModel = existing.into();
                active.name = Set("User".to_string());
                active.role_type = Set(RoleType::User);
                active.update(db).await?;
            }
            return Ok(());
        }

        let active = role::ActiveModel {
            id: Set(0),
            name: Set("User".to_string()),
            role_type: Set(RoleType::User),
            ..Default::default()
        };
        active.insert(db).await?;
        tracing::info!("Created user role with id=0");
        Ok(())
    }

    /// 读取某个角色的 settings.yml（MAIN 在 characters/下；NPC 在 scripts/{key}/characters/下）
    pub async fn get_role_settings_by_id(
        db: &DatabaseConnection,
        data_dir: &Path,
        role_id: i32,
    ) -> Result<Option<CharacterSettings>> {
        let Some(role) = Self::get_role_by_id(db, role_id).await? else {
            return Ok(None);
        };
        let Some(folder) = role.resource_folder.clone() else {
            return Ok(None);
        };

        let base = data_dir.join("game_data");
        let path: PathBuf = match role.role_type {
            RoleType::Main => base.join("characters").join(&folder),
            RoleType::Npc => {
                let Some(script_key) = role.script_key.clone() else {
                    return Ok(None);
                };
                base.join("scripts")
                    .join(&script_key)
                    .join("characters")
                    .join(&folder)
            }
            RoleType::System | RoleType::User => {
                return Ok(None);
            }
        };

        let yaml = path.join("settings.yml");
        if !yaml.exists() {
            tracing::warn!("角色设置文件不存在: {:?}", path);
            return Ok(None);
        }

        let content = fs::read_to_string(&yaml)
            .with_context(|| format!("Failed to read {:?}", yaml))?;
        let mut settings: CharacterSettings = serde_yaml::from_str(&content)
            .with_context(|| format!("Failed to parse {:?}", yaml))?;
        settings.character_id = Some(role_id);
        settings.character_folder = folder;
        settings.resource_path = Some(path.to_string_lossy().into_owned());
        Ok(Some(settings))
    }
}
