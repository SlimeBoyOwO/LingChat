use std::collections::HashSet;
use std::fs;
use std::path::{Path, PathBuf};

use anyhow::{Context, Result};
use sea_orm::sea_query::Expr;
use sea_orm::{
    ActiveModelTrait, ColumnTrait, ConnectionTrait, DatabaseBackend, DatabaseConnection,
    EntityTrait, QueryFilter, QueryOrder, QuerySelect, Set, Statement,
};
use tracing::warn;

use crate::ai_service::types::{CharacterSettings, PLAYER_ROLE_ID, RoleProfile};
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
    pub async fn delete_main_role(db: &DatabaseConnection, role_id: i32) -> Result<bool> {
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
    pub async fn get_all_tool_role_names(db: &DatabaseConnection) -> Result<Vec<(String, String)>> {
        let roles = role::Entity::find()
            .filter(role::Column::RoleType.is_in([RoleType::Main, RoleType::Npc]))
            .all(db)
            .await?;
        let data_dir = crate::api::data_dir();
        let mut names = Vec::with_capacity(roles.len());

        for role in roles {
            let Some(settings) = Self::get_role_settings_by_id(db, &data_dir, role.id).await?
            else {
                tracing::warn!("跳过缺少角色设置的工具权限初始化: role_id={}", role.id);
                continue;
            };
            names.push((role.name, settings.ai_name));
        }

        Ok(names)
    }

    /// 确保 role 表中存在 id=0 的 User 角色（代表人类玩家）。
    /// 已存在时只校正类型，绝不回写 name。
    /// 幂等操作，每次启动调用。
    pub async fn ensure_user_role(db: &DatabaseConnection) -> Result<()> {
        if let Some(existing) = role::Entity::find_by_id(0).one(db).await? {
            // 为什么只改类型：统一实体后 id=0 的 name 会被"玩家名搬家"改写为真实玩家名，
            // 若在此处按旧逻辑强制回写 "User"，会抹掉搬家结果并让每次启动重复搬运（甚至丢名）。
            if existing.role_type != RoleType::User {
                let mut active: role::ActiveModel = existing.into();
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
            RoleType::Main => crate::api::resolve_character_dir_in(data_dir, &folder),
            RoleType::Npc => {
                let Some(script_key) = role.script_key.clone() else {
                    return Ok(None);
                };
                base.join("scripts")
                    .join(&script_key)
                    .join("characters")
                    .join(&folder)
            },
            RoleType::System | RoleType::User => {
                return Ok(None);
            },
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

    // ==============================================================
    // 统一实体：实体人设（profile_json）与玩家身份（role_type=User）
    // ==============================================================

    // 本节方法属于统一实体的数据层公面，运行时消费方在 api/identity.rs 与
    // game_system/possession.rs。

    /// 从 `profile_json` 原文解析人设。
    /// NULL/空白/坏 JSON 一律静默回退默认值——该列是可空增量列，旧库升级后全为 NULL，
    /// "尚未设置"不是故障；一条脏数据也不该阻断聊天启动链路，故只告警不报错。
    fn parse_profile_json(role_id: i32, raw: Option<&str>) -> RoleProfile {
        let Some(raw) = raw.filter(|s| !s.trim().is_empty()) else {
            return RoleProfile::default();
        };
        serde_json::from_str(raw).unwrap_or_else(|e| {
            warn!("角色人设 JSON 损坏，回退默认值: role_id={}, err={}", role_id, e);
            RoleProfile::default()
        })
    }

    /// 读取实体人设（`role.profile_json`），缺失/损坏时返回默认人设。
    pub async fn get_role_profile(db: &DatabaseConnection, role_id: i32) -> Result<RoleProfile> {
        let Some(role) = Self::get_role_by_id(db, role_id).await? else {
            return Ok(RoleProfile::default());
        };
        Ok(Self::parse_profile_json(role_id, role.profile_json.as_deref()))
    }

    /// 列出全部玩家身份（role_type=User，按 id 升序），并附带解析好的人设。
    /// 排序固定升序：id=0 是默认身份，前端与运行时都需要稳定顺序。
    pub async fn list_player_identities(
        db: &DatabaseConnection,
    ) -> Result<Vec<(RoleModel, RoleProfile)>> {
        let roles = role::Entity::find()
            .filter(role::Column::RoleType.eq(RoleType::User))
            .order_by_asc(role::Column::Id)
            .all(db)
            .await?;
        Ok(roles
            .into_iter()
            .map(|role| {
                let profile = Self::parse_profile_json(role.id, role.profile_json.as_deref());
                (role, profile)
            })
            .collect())
    }

    /// 全部玩家身份实体的 id 集合（`role_type=User`），恒含默认身份 0。
    ///
    /// 供"回溯定位"等按身份而非按当前附身判断玩家消息的链路使用：恒含 0 是历史兼容，
    /// id=0 永存且旧存档台词 `sender_role_id=0` 必须继续被认作玩家消息。
    pub async fn get_user_role_ids(db: &DatabaseConnection) -> Result<HashSet<i32>> {
        let ids: Vec<i32> = role::Entity::find()
            .select_only()
            .column(role::Column::Id)
            .filter(role::Column::RoleType.eq(RoleType::User))
            .into_tuple()
            .all(db)
            .await?;
        let mut set: HashSet<i32> = ids.into_iter().collect();
        set.insert(PLAYER_ROLE_ID);
        Ok(set)
    }

    /// 新建玩家身份（role_type=User），返回新行 id。
    /// script/resource 键显式置 NULL：玩家身份不绑定剧本，也不能带人物资源目录，
    /// 否则会被 `role_sync` 当作可同步的剧本角色处理。
    pub async fn create_player_identity(
        db: &DatabaseConnection,
        name: &str,
        profile: &RoleProfile,
    ) -> Result<i32> {
        let json = serde_json::to_string(profile).context("序列化角色人设失败")?;
        let active = RoleActiveModel {
            name: Set(name.to_string()),
            role_type: Set(RoleType::User),
            script_key: Set(Option::<String>::None),
            script_role_key: Set(Option::<String>::None),
            resource_folder: Set(Option::<String>::None),
            profile_json: Set(Some(json)),
            ..Default::default()
        };
        let inserted = active.insert(db).await?;
        tracing::info!("创建玩家身份: id={}, name={}", inserted.id, name);
        Ok(inserted.id)
    }

    /// 改名保护与删除保护是两级：`delete_player_identity` 拒绝系统保护 id（id=0 永存，
    /// 不可删），但 id=0 作为**最常用的默认身份必须能改名/改人设**，故这里只校验
    /// `role_type=User`——id=1 等 Main/Npc/System 行自然被类型检查挡下。
    pub async fn update_player_identity(
        db: &DatabaseConnection,
        role_id: i32,
        name: &str,
        profile: &RoleProfile,
    ) -> Result<()> {
        let Some(role) = Self::get_role_by_id(db, role_id).await? else {
            anyhow::bail!("玩家身份不存在: id={}", role_id);
        };
        if role.role_type != RoleType::User {
            anyhow::bail!("目标角色不是玩家身份: id={}", role_id);
        }
        let json = serde_json::to_string(profile).context("序列化角色人设失败")?;
        let mut active: RoleActiveModel = role.into();
        active.name = Set(name.to_string());
        active.profile_json = Set(Some(json));
        active.update(db).await?;
        Ok(())
    }

    /// 删除玩家身份，返回是否实际删除了行。
    /// 系统保护 id 或非 User 行一律"拒绝并返回 false"而非报错：
    /// 调用方（前端列表）对不可删项做静默处理即可，无需把它当异常流程分支。
    pub async fn delete_player_identity(db: &DatabaseConnection, role_id: i32) -> Result<bool> {
        if Self::is_system_protected_role(role_id) {
            warn!("拒绝删除系统保护的玩家身份: id={}", role_id);
            return Ok(false);
        }
        let Some(role) = Self::get_role_by_id(db, role_id).await? else {
            return Ok(false);
        };
        if role.role_type != RoleType::User {
            warn!("拒绝删除非玩家身份的角色: id={}", role_id);
            return Ok(false);
        }

        // 为什么必须手工清理引用：玩家身份行会成为 line.sender_role_id（附身后玩家以该实体 id 发言），
        // 也会被 line_perception / memory_bank 引用；SQLite 开着外键约束，直接删 role 行必然失败。
        // 无需像 delete_main_role 那样关闭 FK——玩家身份不参与 save.main_role_id 与存档级联，不存在循环引用。

        // 1. 台词归属回落到默认身份 id=0，而非置 NULL：保留"这条是玩家说的"语义。
        //    与 delete_main_role 的置 NULL 不同——那是 AI 角色删除后台词确实失去归属。
        line::Entity::update_many()
            .col_expr(line::Column::SenderRoleId, Expr::value(Some(0i32)))
            .filter(line::Column::SenderRoleId.eq(role_id))
            .exec(db)
            .await?;

        // 2. 感知记录：NOT NULL 外键，不删会变孤儿行并阻塞角色删除
        line_perception::Entity::delete_many()
            .filter(line_perception::Column::RoleId.eq(role_id))
            .exec(db)
            .await?;

        // 3. 记忆行：玩家实体参与记忆后，会按 role_id 落 memory_bank
        MemoryRepo::delete_all_memories_by_role_id(db, role_id).await?;

        // save.main_role_id 固定指向 AI 主角色（存档以 AI 主角为轴），不会指向 User 行，故无需处理。

        let result = role::Entity::delete_by_id(role_id).exec(db).await?;
        Ok(result.rows_affected > 0)
    }

    /// 判定玩家名是否为"尚未搬家"的占位值。
    fn is_placeholder_player_name(name: &str) -> bool {
        let name = name.trim();
        name.is_empty() || name == "User" || name == "user_name未设定"
    }

    /// 首次启动把散落在主角色 settings.yml 的玩家名搬到 id=0 玩家实体（幂等，只读不改写文件）。
    ///
    /// 为什么需要搬家：统一实体后玩家名的唯一真相源是 role 行，而旧版本把它写在每个 AI 的
    /// settings.yml 里；本期只做单向读取，settings.yml 的旧字段保留只读（后续版本再废弃）。
    /// 为什么以 name 是否占位来判断"搬过"：这是唯一能区分"从未搬过"与"用户就叫 User"的廉价信号，
    /// 且读不到/占位时保持原样，下次启动再试，天然幂等、不阻断启动。
    pub async fn ensure_default_player_identity(
        db: &DatabaseConnection,
        data_dir: &Path,
    ) -> Result<()> {
        // 复用既有路径确保 id=0 存在（该函数已不再回写 name，不会覆盖搬家结果）
        Self::ensure_user_role(db).await?;
        let Some(user) = Self::get_role_by_id(db, 0).await? else {
            // ensure_user_role 已保证存在，这里兜底避免 unwrap 崩掉启动
            return Ok(());
        };
        if !Self::is_placeholder_player_name(&user.name) {
            return Ok(());
        }

        // dev 基线把 user_name/user_subtitle 写在主角色（id=1）的 settings.yml
        let Some(settings) = Self::get_role_settings_by_id(db, data_dir, 1).await? else {
            tracing::info!("默认玩家身份未搬家：主角色 (id=1) 暂无可用 settings.yml");
            return Ok(());
        };
        let migrated_name = settings.user_name.trim();
        if Self::is_placeholder_player_name(migrated_name) {
            tracing::info!(
                "默认玩家身份未搬家：主角色 user_name 仍是占位值 {:?}",
                settings.user_name
            );
            return Ok(());
        }

        // 只覆盖 subtitle：profile_json 里其它字段可能已由用户设置过，不能整份丢弃
        let mut profile = Self::parse_profile_json(0, user.profile_json.as_deref());
        if let Some(subtitle) = settings.user_subtitle.as_deref() {
            profile.subtitle = subtitle.to_string();
        }
        let json = serde_json::to_string(&profile).context("序列化角色人设失败")?;
        let mut active: RoleActiveModel = user.into();
        active.name = Set(migrated_name.to_string());
        active.profile_json = Set(Some(json));
        active.update(db).await?;
        tracing::info!("默认玩家身份已从主角色设置搬家: name={}", migrated_name);
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db::entities::memory_bank;
    use crate::migration::Migrator;
    use sea_orm::Database;
    use sea_orm_migration::MigratorTrait;
    use std::sync::atomic::{AtomicU64, Ordering};

    /// 每个测试一份独立的内存 SQLite，并跑完整迁移，保证 role 表结构与生产一致（含 profile_json）。
    /// 用 `cache=shared` 而非裸 `sqlite::memory:`：连接池内多连接必须看到同一份库，
    /// 否则迁移建的表和后续查询会落在不同库上。
    async fn test_db() -> DatabaseConnection {
        static SEQ: AtomicU64 = AtomicU64::new(0);
        let seq = SEQ.fetch_add(1, Ordering::Relaxed);
        let url = format!("sqlite:file:role_repo_test_{seq}?mode=memory&cache=shared");
        let db = Database::connect(&url).await.expect("连接内存 SQLite 失败");
        Migrator::up(&db, None).await.expect("内存库迁移失败");
        db
    }

    /// 插入一行角色，用于构造主角色/玩家身份等前置数据。
    async fn insert_role(
        db: &DatabaseConnection,
        id: i32,
        name: &str,
        role_type: RoleType,
        resource_folder: Option<&str>,
    ) -> RoleModel {
        RoleActiveModel {
            id: Set(id),
            name: Set(name.to_string()),
            role_type: Set(role_type),
            resource_folder: Set(resource_folder.map(|s| s.to_string())),
            ..Default::default()
        }
        .insert(db)
        .await
        .expect("插入角色失败")
    }

    /// 插入一行存档：line / memory_bank 的 save_id 是非空外键，测试台词与记忆前必须先有 save 行。
    async fn insert_save(db: &DatabaseConnection, id: i32, main_role_id: Option<i32>) {
        let now = chrono::Utc::now().naive_utc();
        save::ActiveModel {
            id: Set(id),
            title: Set("测试存档".into()),
            status: Set("{}".into()),
            create_date: Set(now),
            update_date: Set(now),
            main_role_id: Set(main_role_id),
            ..Default::default()
        }
        .insert(db)
        .await
        .expect("插入存档失败");
    }

    /// 写一份主角色 settings.yml（搬家只读它，不写它）。
    fn write_main_settings(data_dir: &Path, user_name: &str, user_subtitle: &str) {
        let folder = data_dir.join("game_data").join("characters").join("hero");
        std::fs::create_dir_all(&folder).expect("创建角色目录失败");
        let yaml =
            format!("ai_name: Hero\nuser_name: {user_name}\nuser_subtitle: {user_subtitle}\n");
        std::fs::write(folder.join("settings.yml"), yaml).expect("写 settings.yml 失败");
    }

    #[tokio::test]
    async fn role_profile_missing_or_broken_json_falls_back_to_default() {
        let db = test_db().await;
        insert_role(&db, 1, "Hero", RoleType::Main, Some("hero")).await;

        // NULL：旧库升级后的初始状态
        assert_eq!(
            RoleRepo::get_role_profile(&db, 1).await.unwrap(),
            RoleProfile::default()
        );
        // 角色不存在同样兜底，不报错
        assert_eq!(
            RoleRepo::get_role_profile(&db, 999).await.unwrap(),
            RoleProfile::default()
        );

        // 坏 JSON
        role::Entity::update_many()
            .col_expr(
                role::Column::ProfileJson,
                Expr::value(Some("{ 这不是 JSON".to_string())),
            )
            .filter(role::Column::Id.eq(1))
            .exec(&db)
            .await
            .unwrap();
        assert_eq!(
            RoleRepo::get_role_profile(&db, 1).await.unwrap(),
            RoleProfile::default()
        );

        // 空白字符串
        role::Entity::update_many()
            .col_expr(
                role::Column::ProfileJson,
                Expr::value(Some("   ".to_string())),
            )
            .filter(role::Column::Id.eq(1))
            .exec(&db)
            .await
            .unwrap();
        assert_eq!(
            RoleRepo::get_role_profile(&db, 1).await.unwrap(),
            RoleProfile::default()
        );

        // 合法 JSON：缺失字段靠 serde(default) 补齐
        role::Entity::update_many()
            .col_expr(
                role::Column::ProfileJson,
                Expr::value(Some(r#"{"subtitle":"称号"}"#.to_string())),
            )
            .filter(role::Column::Id.eq(1))
            .exec(&db)
            .await
            .unwrap();
        let parsed = RoleRepo::get_role_profile(&db, 1).await.unwrap();
        assert_eq!(parsed.subtitle, "称号");
        assert!(parsed.location_id.is_none());
    }

    #[tokio::test]
    async fn player_identity_crud_roundtrip() {
        let db = test_db().await;
        let profile = RoleProfile {
            subtitle: "小名".into(),
            info: "温柔的人".into(),
            ..Default::default()
        };

        let id = RoleRepo::create_player_identity(&db, "小明", &profile).await.unwrap();
        assert!(id > 0);

        let listed = RoleRepo::list_player_identities(&db).await.unwrap();
        assert_eq!(listed.len(), 1);
        assert_eq!(listed[0].0.id, id);
        assert_eq!(listed[0].0.role_type, RoleType::User);
        assert!(listed[0].0.script_key.is_none());
        assert!(listed[0].0.resource_folder.is_none());
        assert_eq!(listed[0].1, profile);

        let mut updated = profile.clone();
        updated.subtitle = "新称号".into();
        RoleRepo::update_player_identity(&db, id, "小红", &updated).await.unwrap();
        let role = RoleRepo::get_role_by_id(&db, id).await.unwrap().unwrap();
        assert_eq!(role.name, "小红");
        assert_eq!(RoleRepo::get_role_profile(&db, id).await.unwrap(), updated);

        assert!(RoleRepo::delete_player_identity(&db, id).await.unwrap());
        assert!(RoleRepo::get_role_by_id(&db, id).await.unwrap().is_none());
        // 重复删除返回 false 而非报错
        assert!(!RoleRepo::delete_player_identity(&db, id).await.unwrap());
    }

    #[tokio::test]
    async fn update_protection_is_two_tiered_and_delete_blocks_default_identity() {
        let db = test_db().await;
        RoleRepo::ensure_user_role(&db).await.unwrap();
        insert_role(&db, 1, "Hero", RoleType::Main, Some("hero")).await;

        // 改名保护低于删除保护：id=0 是系统保护行（不可删），但作为默认身份必须可改名/改人设
        let profile = RoleProfile {
            subtitle: "默认称号".into(),
            info: "默认介绍".into(),
            ..Default::default()
        };
        RoleRepo::update_player_identity(&db, 0, "阿宅", &profile).await.unwrap();
        let user = RoleRepo::get_role_by_id(&db, 0).await.unwrap().unwrap();
        assert_eq!(user.name, "阿宅");
        assert_eq!(user.role_type, RoleType::User);
        assert_eq!(RoleRepo::get_role_profile(&db, 0).await.unwrap(), profile);

        // 删除保护保持不变：id=0 永存
        assert!(!RoleRepo::delete_player_identity(&db, 0).await.unwrap());

        // AI 角色行不是玩家身份：update 被类型检查拒绝、delete 拒绝
        assert!(
            RoleRepo::update_player_identity(&db, 1, "x", &RoleProfile::default())
                .await
                .is_err()
        );
        assert!(!RoleRepo::delete_player_identity(&db, 1).await.unwrap());
        assert!(RoleRepo::get_role_by_id(&db, 1).await.unwrap().is_some());
    }

    #[tokio::test]
    async fn deleting_player_identity_cleans_up_related_rows() {
        let db = test_db().await;
        RoleRepo::ensure_user_role(&db).await.unwrap();
        insert_role(&db, 1, "Hero", RoleType::Main, Some("hero")).await;
        insert_save(&db, 20, Some(1)).await;

        let id = RoleRepo::create_player_identity(&db, "小明", &RoleProfile::default())
            .await
            .unwrap();

        // 构造被引用的三类行：台词（sender=身份 id）、感知、记忆
        let line_row = line::ActiveModel {
            content: Set("你好".into()),
            attribute: Set(line::LineAttribute::User),
            sender_role_id: Set(Some(id)),
            save_id: Set(20),
            ..Default::default()
        }
        .insert(&db)
        .await
        .unwrap();
        line_perception::ActiveModel {
            line_id: Set(line_row.id),
            role_id: Set(id),
        }
        .insert(&db)
        .await
        .unwrap();
        memory_bank::ActiveModel {
            info: Set("一条记忆".into()),
            save_id: Set(20),
            role_id: Set(Some(id)),
            ..Default::default()
        }
        .insert(&db)
        .await
        .unwrap();

        assert!(RoleRepo::delete_player_identity(&db, id).await.unwrap());
        assert!(RoleRepo::get_role_by_id(&db, id).await.unwrap().is_none());

        // 台词保留，归属回落默认身份 0（而非失去归属）
        let kept = line::Entity::find_by_id(line_row.id)
            .one(&db)
            .await
            .unwrap()
            .unwrap();
        assert_eq!(kept.sender_role_id, Some(0));
        // 感知与记忆行连根清除，不留外键孤儿
        assert!(line_perception::Entity::find().all(&db).await.unwrap().is_empty());
        assert!(memory_bank::Entity::find().all(&db).await.unwrap().is_empty());

        // 默认身份 id=0 依旧不可删
        assert!(!RoleRepo::delete_player_identity(&db, 0).await.unwrap());
        assert!(RoleRepo::get_role_by_id(&db, 0).await.unwrap().is_some());
    }

    #[tokio::test]
    async fn default_player_identity_migration_moves_name_and_is_idempotent() {
        let db = test_db().await;
        insert_role(&db, 1, "Hero", RoleType::Main, Some("hero")).await;
        let tmp = tempfile::tempdir().unwrap();
        write_main_settings(tmp.path(), "小明", "小名的称号");

        RoleRepo::ensure_default_player_identity(&db, tmp.path()).await.unwrap();
        let user = RoleRepo::get_role_by_id(&db, 0).await.unwrap().unwrap();
        assert_eq!(user.name, "小明");
        assert_eq!(
            RoleRepo::get_role_profile(&db, 0).await.unwrap().subtitle,
            "小名的称号"
        );

        // 第二次调用：即使 settings.yml 变了也不该再改写（id=0 已非占位值）
        write_main_settings(tmp.path(), "应被忽略", "应被忽略");
        RoleRepo::ensure_default_player_identity(&db, tmp.path()).await.unwrap();
        let user = RoleRepo::get_role_by_id(&db, 0).await.unwrap().unwrap();
        assert_eq!(user.name, "小明");
        assert_eq!(
            RoleRepo::get_role_profile(&db, 0).await.unwrap().subtitle,
            "小名的称号"
        );
    }

    #[tokio::test]
    async fn default_player_identity_skips_placeholder_name() {
        let db = test_db().await;
        insert_role(&db, 1, "Hero", RoleType::Main, Some("hero")).await;
        let tmp = tempfile::tempdir().unwrap();
        write_main_settings(tmp.path(), "user_name未设定", "");

        RoleRepo::ensure_default_player_identity(&db, tmp.path()).await.unwrap();
        let user = RoleRepo::get_role_by_id(&db, 0).await.unwrap().unwrap();
        assert_eq!(user.name, "User");
        assert!(user.profile_json.is_none());
    }
}
