use std::fs;
use std::path::{Path, PathBuf};

use anyhow::{Context, Result};
use sea_orm::sea_query::Expr;
use sea_orm::{
    ActiveModelTrait, ColumnTrait, ConnectionTrait, DatabaseBackend, DatabaseConnection, EntityTrait,
    QueryFilter, QuerySelect, Set, Statement,
};
use tracing::warn;

use crate::ai_service::types::CharacterSettings;
use crate::db::entities::line;
use crate::db::entities::line_perception;
use crate::db::entities::role::{
    self, ActiveModel as RoleActiveModel, Model as RoleModel, RoleType,
};
use crate::db::entities::running_script;
use crate::db::entities::save;
use crate::db::managers::memory_repo::MemoryRepo;
use crate::db::managers::save_repo::SaveRepo;

pub struct RoleRepo;

/// 临时关闭 SQLite 外键约束。
/// 警告：调用方必须在 finally 路径中重新打开（建议用 `with_fk_disabled` 闭包形式）。
async fn disable_fk(db: &DatabaseConnection) -> Result<()> {
    db.execute(Statement::from_string(
        DatabaseBackend::Sqlite,
        "PRAGMA foreign_keys = OFF",
    ))
    .await
    .context("关闭外键约束失败")?;
    Ok(())
}

/// 重新打开 SQLite 外键约束。即使前面的操作已经成功，也要在 finally 路径调用。
async fn enable_fk(db: &DatabaseConnection) -> Result<()> {
    db.execute(Statement::from_string(
        DatabaseBackend::Sqlite,
        "PRAGMA foreign_keys = ON",
    ))
    .await
    .context("重新启用外键约束失败")?;
    Ok(())
}

/// RAII guard：在作用域结束时**尝试**重新打开外键约束。
/// 如果在 guard drop 时重新打开失败（例如连接已断），仅记 warn，不抛错。
/// 原因：guard 是在错误传播路径上 drop 的，原始错误已经更重要，不应被 PRAGMA 错误覆盖。
struct FkReEnableGuard<'a> {
    db: &'a DatabaseConnection,
    armed: bool,
}

impl<'a> FkReEnableGuard<'a> {
    fn new(db: &'a DatabaseConnection) -> Self {
        Self { db, armed: true }
    }
    /// 显式 disarm——主流程成功时调用，避免重复 enable。
    fn disarm(mut self) {
        self.armed = false;
    }
}

impl<'a> Drop for FkReEnableGuard<'a> {
    fn drop(&mut self) {
        if !self.armed {
            return;
        }
        // Drop 上下文无法 await；交给 tokio 异步任务兜底。
        let db = self.db.clone();
        tokio::spawn(async move {
            if let Err(e) = enable_fk(&db).await {
                warn!("FK guard drop 时重新启用外键约束失败: {}", e);
            }
        });
    }
}

impl RoleRepo {
    pub async fn get_role_by_id(
        db: &DatabaseConnection,
        role_id: i32,
    ) -> Result<Option<RoleModel>> {
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

    #[allow(dead_code)]
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
        resource_folder: Option<&str>,
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
            resource_folder: Set(resource_folder.map(|s| s.to_string())),
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

    /// 系统保护的角色 ID 集合，禁止删除。
    /// - 0: User 角色（玩家本体）
    /// - 1: 默认 main 角色（启动兜底）
    /// - 2: 预留系统角色位
    pub const SYSTEM_PROTECTED_ROLE_IDS: &'static [i32] = &[0, 1, 2];

    /// 检查给定角色 ID 是否为系统保护角色。
    pub fn is_system_protected_role(role_id: i32) -> bool {
        Self::SYSTEM_PROTECTED_ROLE_IDS.contains(&role_id)
    }

/// 级联删除一个 main 角色及其全部关联数据。///
/// 顺序：
/// 1. 临时关闭外键约束（PRAGMA foreign_keys = OFF）
/// 2. 找出所有 main_role_id = role_id 的存档
/// 3. 清理这些存档的 running_script（FK running_script.save_id → save.id 会阻止 save 删除）
/// 4. 逐个删除存档（级联清 line/line_perception）
/// 5. 清该角色所有 memory_bank 行（跨存档兜底）
/// 6. 防御性清 line_perception.role_id（其他存档里残留的感知记录，NOT NULL FK 不能置 NULL）
/// 7. 清空 line.sender_role_id 指向此角色的所有台词 FK 引用（其他存档的台词可能引用此角色作 sender）
/// 8. 防御性解绑 save.main_role_id（其他存档引用此角色的情况）
/// 9. delete role by id
/// 10. 重新打开外键约束（即使中途错误，也由 FkReEnableGuard 兜底）
///
/// 返回是否实际删除了行。
///
/// **为什么需要关闭 FK？**
/// 角色的 FK 引用散布在 save/line/line_perception/memory_bank/running_script 等多个表，
/// 任何一处漏清理都会触发 SQLite FK 约束失败。关闭 FK 让我们不再"打地鼠"，即使将来 schema
/// 增加新引用也不会破坏删除流程。手工清理步骤保留是为了不留下孤儿行（line_perception
/// 等 NOT NULL 字段不删会留垃圾，line.sender_role_id 保留归属更有用——所以这部分仍然置 NULL）。
///
/// 风险分析：删除角色是低频用户主动操作；即使 PRAGMA 重启用失败，进程重启后 SQLite 会
/// 重新应用外键约束（PRAGMA 是 connection-level 的），不留持久影响。
pub async fn delete_main_role(
    db: &DatabaseConnection,
    role_id: i32,
) -> Result<bool> {
    // 1. 关闭 FK 约束
    disable_fk(db).await?;
    // guard 保证即使中间 panic / 早返回，外键也会在最后被重新启用
    let guard = FkReEnableGuard::new(db);

    // 2. 找出引用此角色的所有存档
    let saves_to_delete: Vec<i32> = save::Entity::find()
        .select_only()
        .column(save::Column::Id)
        .filter(save::Column::MainRoleId.eq(role_id))
        .into_tuple()
        .all(db)
        .await?;

    // 3. 清理这些存档的 running_script
    if !saves_to_delete.is_empty() {
        running_script::Entity::delete_many()
            .filter(running_script::Column::SaveId.is_in(saves_to_delete.clone()))
            .exec(db)
            .await?;
    }

    // 4. 逐个删除存档（级联清 line/line_perception）
    for save_id in &saves_to_delete {
        SaveRepo::delete_save(db, *save_id).await?;
    }

    // 5. 清该角色全部 memory_bank
    MemoryRepo::delete_all_memories_by_role_id(db, role_id).await?;

    // 6. 防御性清 line_perception（NOT NULL FK，必须删）
    line_perception::Entity::delete_many()
        .filter(line_perception::Column::RoleId.eq(role_id))
        .exec(db)
        .await?;

    // 7. 清空 line.sender_role_id 指向此角色的引用（保留对话内容，仅失归属）
    line::Entity::update_many()
        .col_expr(line::Column::SenderRoleId, Expr::value(Option::<i32>::None))
        .filter(line::Column::SenderRoleId.eq(role_id))
        .exec(db)
        .await?;

    // 8. 防御性解绑其他存档的 main_role_id
    save::Entity::update_many()
        .col_expr(save::Column::MainRoleId, Expr::value(Option::<i32>::None))
        .filter(save::Column::MainRoleId.eq(role_id))
        .exec(db)
        .await?;

    // 9. 删 role 本身
    let result = role::Entity::delete_by_id(role_id).exec(db).await?;
    let rows_affected = result.rows_affected > 0;

    // 10. 成功路径：显式重新启用 FK 并 disarm guard（避免重复 enable）
    enable_fk(db).await?;
    guard.disarm();

    Ok(rows_affected)
}

    /// 获取可调用工具的角色名称，返回 `(数据库名称, settings.yml 中的运行时名称)`。
    /// User 和 System 没有角色 settings，不能作为工具调用主体。
    pub async fn get_all_tool_role_names(
        db: &DatabaseConnection,
    ) -> Result<Vec<(String, String)>> {
        let roles = role::Entity::find()
            .filter(role::Column::RoleType.is_in([RoleType::Main, RoleType::Npc]))
            .all(db)
            .await?;
        let data_dir = crate::api::data_dir();
        let mut names = Vec::with_capacity(roles.len());

        for role in roles {
            let Some(settings) = Self::get_role_settings_by_id(db, &data_dir, role.id).await? else {
                tracing::warn!("跳过缺少角色设置的工具权限初始化: role_id={}", role.id);
                continue;
            };
            names.push((role.name, settings.ai_name));
        }

        Ok(names)
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

        let content =
            fs::read_to_string(&yaml).with_context(|| format!("Failed to read {:?}", yaml))?;
        let mut settings: CharacterSettings = serde_yaml::from_str(&content)
            .with_context(|| format!("Failed to parse {:?}", yaml))?;
        settings.character_id = Some(role_id);
        settings.character_folder = folder;
        settings.resource_path = Some(path.to_string_lossy().into_owned());
        Ok(Some(settings))
    }
}
