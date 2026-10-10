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
    let _guard = super::character_costumes::RESOURCE_LOCK.lock().await;
    let state = app.state::<AppState>();
    create_in(&state.db, &super::characters_dir(), &name, &resource_folder).await
}

#[cfg(test)]
mod tests {
    use super::*;
    use sea_orm::{ConnectionTrait, Database, DatabaseBackend, Schema, Statement};

    async fn database() -> DatabaseConnection {
        let db = Database::connect("sqlite::memory:").await.unwrap();
        let schema = Schema::new(DatabaseBackend::Sqlite);
        db.execute(
            db.get_database_backend()
                .build(&schema.create_table_from_entity(role::Entity)),
        )
        .await
        .unwrap();
        db
    }

    #[tokio::test]
    async fn creates_empty_character_and_survives_rescan() {
        let db = database().await;
        let dir = tempfile::tempdir().unwrap();
        let base = dir.path().join("game_data/characters");
        let created = create_in(&db, &base, " 测试角色 ", "test-role")
            .await
            .unwrap();
        let settings: CharacterSettings =
            serde_yaml::from_str(&fs::read_to_string(base.join("test-role/settings.yml")).unwrap())
                .unwrap();
        assert_eq!(settings.ai_name, "测试角色");
        assert_eq!(settings.user_name, "用户");
        assert_eq!(
            fs::read_dir(base.join("test-role/avatar")).unwrap().count(),
            0
        );
        crate::db::role_sync::sync_roles_from_folder(&db, dir.path())
            .await
            .unwrap();
        let rows = role::Entity::find().all(&db).await.unwrap();
        assert_eq!(rows.len(), 1);
        assert_eq!(rows[0].id, created.character_id);
        assert_eq!(rows[0].role_type, RoleType::Main);
    }

    #[tokio::test]
    async fn rejects_invalid_names_and_conflicts_without_overwriting() {
        let db = database().await;
        let dir = tempfile::tempdir().unwrap();
        for folder in [
            "",
            "../escape",
            "a/b",
            "a\\b",
            "CON",
            "NUL.txt",
            "bad:",
            "trailing.",
            ".hidden",
        ] {
            assert!(
                create_in(&db, dir.path(), "角色", folder).await.is_err(),
                "{folder}"
            );
        }
        assert!(create_in(&db, dir.path(), " ", "valid").await.is_err());
        create_in(&db, dir.path(), "角色", "Unique").await.unwrap();
        let path = dir.path().join("Unique/settings.yml");
        let original = fs::read(&path).unwrap();
        assert!(
            create_in(&db, dir.path(), "另一个角色", "unique")
                .await
                .is_err()
        );
        assert_eq!(fs::read(path).unwrap(), original);
        fs::create_dir(dir.path().join("existing")).unwrap();
        assert!(
            create_in(&db, dir.path(), "角色", "existing")
                .await
                .is_err()
        );
        assert_eq!(role::Entity::find().all(&db).await.unwrap().len(), 1);
    }

    #[tokio::test]
    async fn concurrent_creation_and_rescan_do_not_duplicate_roles() {
        let db = database().await;
        let dir = tempfile::tempdir().unwrap();
        let base = dir.path().join("game_data/characters");
        let (created, scanned) = tokio::join!(
            create_in(&db, &base, "并发测试", "concurrent-role"),
            crate::db::role_sync::sync_roles_from_folder(&db, dir.path()),
        );
        let id = created.unwrap().character_id;
        assert!(scanned.unwrap().is_empty());
        let rows = role::Entity::find().all(&db).await.unwrap();
        assert_eq!(rows.len(), 1);
        assert_eq!(rows[0].id, id);
    }

    #[tokio::test]
    async fn database_failure_removes_only_new_directory() {
        let db = database().await;
        db.execute(Statement::from_string(DatabaseBackend::Sqlite,
            "CREATE TRIGGER reject_role BEFORE INSERT ON role BEGIN SELECT RAISE(ABORT, 'test failure'); END"))
            .await.unwrap();
        let dir = tempfile::tempdir().unwrap();
        fs::create_dir(dir.path().join("keep")).unwrap();
        fs::write(dir.path().join("keep/marker"), "keep").unwrap();
        assert!(
            create_in(&db, dir.path(), "角色", "new-role")
                .await
                .is_err()
        );
        assert!(!dir.path().join("new-role").exists());
        assert_eq!(
            fs::read_to_string(dir.path().join("keep/marker")).unwrap(),
            "keep"
        );
        assert!(role::Entity::find().all(&db).await.unwrap().is_empty());
    }
}
