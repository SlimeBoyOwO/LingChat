use std::fs;
use std::path::{Path, PathBuf};

use anyhow::{Context, Result};
use sea_orm::{ColumnTrait, DatabaseConnection, EntityTrait, QueryFilter};

use crate::ai_service::types::CharacterSettings;
use crate::db::entities::role::{self, Model as RoleModel, RoleType};

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

    pub async fn get_all_main_roles(db: &DatabaseConnection) -> Result<Vec<RoleModel>> {
        Ok(role::Entity::find()
            .filter(role::Column::RoleType.eq(RoleType::Main))
            .all(db)
            .await?)
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
            RoleType::System => {
                return Ok(None);
            }
        };

        let yaml = path.join("settings.yml");
        if !yaml.exists() {
            log::warn!("角色设置文件不存在: {:?}", path);
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
