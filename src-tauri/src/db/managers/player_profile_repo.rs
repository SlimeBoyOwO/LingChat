use anyhow::{Context, Result, anyhow, bail};
use sea_orm::sea_query::Expr;
use sea_orm::*;
use serde::{Deserialize, Serialize};

use crate::ai_service::types::CharacterSettings;
use crate::db::entities::player_profile;
use crate::db::entities::player_profile::{
    ActiveModel as PlayerProfileActiveModel, Model as PlayerProfileRow,
};
use crate::db::managers::role_repo::RoleRepo;
use crate::init::static_copy::get_data_dir;

/// 玩家人设卡的**内容**（纯 DB 存储，一行 = 一张人设卡）。
///
/// 字段与前端 `PlayerProfile` 接口对齐，并复用 `CharacterSettings` 的部分字段语义，
/// 让 AI 更完整地了解屏幕对面的真实用户。`system_prompt`（=玩家的 `user_prompt`）
/// 与 `system_prompt_example` 语义与角色卡一致。
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct PlayerProfileData {
    #[serde(default = "default_user_name")]
    pub user_name: String,
    #[serde(default)]
    pub user_subtitle: Option<String>,
    /// 人格设定（原 YAML 键 `system_prompt`）。
    #[serde(default)]
    pub user_prompt: Option<String>,
    /// 简介 / 一句话人设（类似角色卡的 `info`）。
    #[serde(default)]
    pub info: Option<String>,
    /// 说话风格示例（类似角色卡的 `system_prompt_example`）。
    #[serde(default)]
    pub system_prompt_example: Option<String>,
}

fn default_user_name() -> String {
    "玩家".to_string()
}

/// 归一化可选文本字段：trim 后为空串则存 NULL（与旧文件版「空键不写盘」语义一致）。
fn normalize_optional(value: Option<String>) -> Option<String> {
    value
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty())
}

/// 归一化旧角色卡中的文本字段：过滤空串、纯空白和 serde 缺省值。
fn normalize_legacy_text(raw: &str) -> Option<&str> {
    let trimmed = raw.trim();
    if trimmed.is_empty() || trimmed == "user_name未设定" {
        return None;
    }
    Some(trimmed)
}

impl Default for PlayerProfileData {
    fn default() -> Self {
        Self {
            user_name: default_user_name(),
            user_subtitle: None,
            user_prompt: None,
            info: None,
            system_prompt_example: None,
        }
    }
}

impl PlayerProfileData {
    /// 把玩家档案的「设定块」合并成一段文本，注入系统提示词。
    ///
    /// 组合顺序：简介（info）→ 人格设定（user_prompt）→ 说话风格示例（system_prompt_example）。
    /// 与角色卡的 `info` / `system_prompt` / `system_prompt_example` 语义一致，
    /// 让 AI 更完整地了解屏幕对面的真实用户。空字段自动跳过。
    pub fn to_prompt_fragment(&self) -> String {
        let mut parts: Vec<String> = Vec::new();
        if let Some(s) = self.info.as_deref().filter(|s| !s.is_empty()) {
            parts.push(format!("【简介】{}", s));
        }
        if let Some(s) = self.user_prompt.as_deref().filter(|s| !s.is_empty()) {
            parts.push(format!("【人格设定】{}", s));
        }
        if let Some(s) = self.system_prompt_example.as_deref().filter(|s| !s.is_empty()) {
            parts.push(format!("【说话风格示例】\n{}", s));
        }
        parts.join("\n")
    }
}

/// 玩家人设卡摘要（前端人设列表展示用）。
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct PersonaSummary {
    /// 人设卡 id（沿用原目录名语义：可读、稳定，可用于剧本 `persona_id`）。
    pub card_id: String,
    /// 显示名（人设卡里的 `user_name`）。
    pub user_name: String,
    /// 是否为当前激活人设。
    pub active: bool,
}

/// 玩家档案仓库：**纯 DB 多卡**（`player_profile` 表，一行一张人设卡）。
///
/// 存储与 AI 角色卡解耦：玩家身份（名字/副标题/简介/人格设定/说话示例）全部落在
/// SQLite 里。当前激活人设由 `is_active` 标记（部分唯一索引保证至多一行），是
/// 「谁是当前玩家」的唯一权威。头像功能已按 Owner 决策移除。
pub struct PlayerProfileRepo;

impl PlayerProfileRepo {
    /// 默认人设 id（空表播种 / 旧玩家数据迁移落点）。
    const DEFAULT_PERSONA: &'static str = "default";

    // ============ 查询 ============

    /// 读取当前激活人设 id。
    pub async fn active_persona_id(db: &DatabaseConnection) -> Result<String> {
        let row = player_profile::Entity::find()
            .filter(player_profile::Column::IsActive.eq(true))
            .one(db)
            .await
            .context("查询激活玩家人设失败")?;
        row.map(|r| r.card_id)
            .ok_or_else(|| anyhow!("尚无激活玩家人设（应先行调用 ensure_profile 播种）"))
    }

    /// 读取当前激活人设 id；表中还没有激活行时返回 None（不报错，供列表等只读场景）。
    async fn active_persona_id_option(db: &DatabaseConnection) -> Result<Option<String>> {
        let row = player_profile::Entity::find()
            .filter(player_profile::Column::IsActive.eq(true))
            .one(db)
            .await
            .context("查询激活玩家人设失败")?;
        Ok(row.map(|r| r.card_id))
    }

    /// 列出所有玩家人设卡摘要（按 card_id 排序）。
    pub async fn list_personas(db: &DatabaseConnection) -> Result<Vec<PersonaSummary>> {
        let rows = player_profile::Entity::find()
            .order_by_asc(player_profile::Column::CardId)
            .all(db)
            .await
            .context("查询玩家人设列表失败")?;
        let active = Self::active_persona_id_option(db).await?;
        Ok(rows
            .into_iter()
            .map(|row| PersonaSummary {
                card_id: row.card_id.clone(),
                user_name: row.user_name.clone(),
                active: Some(row.card_id.as_str()) == active.as_deref(),
            })
            .collect())
    }

    /// 读取指定人设卡。
    pub async fn get_persona(
        db: &DatabaseConnection,
        card_id: &str,
    ) -> Result<Option<PlayerProfileData>> {
        if is_invalid_card_id(card_id) {
            return Ok(None);
        }
        let row = player_profile::Entity::find()
            .filter(player_profile::Column::CardId.eq(card_id.to_string()))
            .one(db)
            .await
            .context("查询玩家人设失败")?;
        Ok(row.as_ref().map(Self::profile_from_row))
    }

    /// 读取全局玩家档案（当前激活人设）。
    ///
    /// 实际逻辑见 [`Self::ensure_profile`]，此处等价于 `ensure_profile(db, None)`。
    pub async fn get_profile(db: &DatabaseConnection) -> Result<PlayerProfileData> {
        Self::ensure_profile(db, None).await
    }

    // ============ 激活 / 保证 ============

    /// 设置当前激活人设。校验该人设卡存在。
    pub async fn set_active_persona(db: &DatabaseConnection, card_id: &str) -> Result<()> {
        if is_invalid_card_id(card_id) {
            bail!("非法的人设卡 id: {card_id}");
        }
        if player_profile::Entity::find()
            .filter(player_profile::Column::CardId.eq(card_id.to_string()))
            .one(db)
            .await
            .context("查询人设卡失败")?
            .is_none()
        {
            bail!("人设卡不存在: {card_id}");
        }

        // 事务内先清空全部激活位、再点亮目标卡，避免出现瞬时双激活；
        // 迁移里的部分唯一索引是第二道保险。
        let tx = db.begin().await.context("开启事务失败")?;
        player_profile::Entity::update_many()
            .col_expr(player_profile::Column::IsActive, Expr::value(false))
            .exec(&tx)
            .await
            .context("清除原激活人设失败")?;
        player_profile::Entity::update_many()
            .col_expr(player_profile::Column::IsActive, Expr::value(true))
            .filter(player_profile::Column::CardId.eq(card_id.to_string()))
            .exec(&tx)
            .await
            .context("激活目标人设失败")?;
        tx.commit().await.context("提交切换激活人设失败")?;
        Ok(())
    }

    /// 确保玩家档案存在并返回当前激活人设的可用档案。
    ///
    /// - 表为空（全新安装 / 无任何存档）时，从旧 AI 角色卡迁移
    ///   `user_name/user_subtitle` 播种默认卡 `default`（幂等：已有任何行则跳过）。
    /// - 若激活位缺失或异常（旧库脏数据），把第一张卡修复为激活。
    pub async fn ensure_profile(
        db: &DatabaseConnection,
        fallback: Option<&CharacterSettings>,
    ) -> Result<PlayerProfileData> {
        // 1. 空表播种：这是「旧角色卡字段 → 玩家默认人设」的一次性数据搬运，
        //    幂等（只在表为空时执行），失败不阻断（下次启动会重试）。
        let count = player_profile::Entity::find()
            .count(db)
            .await
            .context("统计玩家人设失败")?;
        if count == 0 {
            let (user_name, user_subtitle) = Self::legacy_profile_fields(db, fallback).await;
            let row = PlayerProfileActiveModel {
                card_id: Set(Self::DEFAULT_PERSONA.to_string()),
                user_name: Set(user_name.clone()),
                user_subtitle: Set(user_subtitle.clone()),
                user_prompt: Set(None),
                info: Set(None),
                system_prompt_example: Set(None),
                is_active: Set(true),
            };
            if let Err(e) = row.insert(db).await {
                tracing::warn!("播种默认玩家人设失败，本次会话继续用内存默认值，下次启动会重试: {e}");
                return Ok(PlayerProfileData {
                    user_name,
                    user_subtitle,
                    ..Default::default()
                });
            }
            tracing::info!("已播种默认玩家人设（card_id=default），来源：旧 AI 角色卡字段");
            return Ok(PlayerProfileData {
                user_name,
                user_subtitle,
                ..Default::default()
            });
        }

        // 2. 修复激活位：无激活行则激活第一张卡（按 card_id 升序，默认卡优先）。
        if Self::active_persona_id_option(db).await?.is_none() {
            if let Some(row) = player_profile::Entity::find()
                .order_by_asc(player_profile::Column::CardId)
                .one(db)
                .await
                .context("查询玩家人设失败")?
            {
                let mut am: PlayerProfileActiveModel = row.into();
                am.is_active = Set(true);
                am.update(db).await.context("修复激活人设失败")?;
            }
        }

        // 3. 返回激活人设。
        let row = player_profile::Entity::find()
            .filter(player_profile::Column::IsActive.eq(true))
            .one(db)
            .await
            .context("查询激活玩家人设失败")?
            .ok_or_else(|| anyhow!("读取激活玩家人设失败：表中没有任何人设卡"))?;
        Ok(Self::profile_from_row(&row))
    }

    // ============ 写操作 ============

    /// 保存玩家档案到**当前激活人设**。
    pub async fn save_profile(db: &DatabaseConnection, profile: &PlayerProfileData) -> Result<()> {
        let active = Self::active_persona_id(db).await?;
        Self::save_persona(db, &active, profile).await
    }

    /// 保存玩家档案到指定人设卡（更新已存在的行）。
    pub async fn save_persona(
        db: &DatabaseConnection,
        card_id: &str,
        profile: &PlayerProfileData,
    ) -> Result<()> {
        if is_invalid_card_id(card_id) {
            bail!("非法的人设卡 id: {card_id}");
        }
        let Some(row) = player_profile::Entity::find()
            .filter(player_profile::Column::CardId.eq(card_id.to_string()))
            .one(db)
            .await
            .context("查询人设卡失败")?
        else {
            bail!("人设卡不存在: {card_id}");
        };

        let mut am: PlayerProfileActiveModel = row.into();
        am.user_name = Set(profile.user_name.trim().to_string());
        am.user_subtitle = Set(normalize_optional(profile.user_subtitle.clone()));
        am.user_prompt = Set(normalize_optional(profile.user_prompt.clone()));
        am.info = Set(normalize_optional(profile.info.clone()));
        am.system_prompt_example = Set(normalize_optional(profile.system_prompt_example.clone()));
        am.update(db).await.context("保存玩家档案失败")?;
        Ok(())
    }

    /// 新建一张人设卡并**直接激活**（前端创建流程无需再二次切换）。
    ///
    /// 事务内先熄灭全部激活位再插入新卡（新卡即激活），避免部分唯一索引冲突；
    /// 若 `card_id` 已存在，插入失败会整体回滚，不会留下半激活的脏状态。
    pub async fn create_persona_active(
        db: &DatabaseConnection,
        card_id: &str,
        profile: &PlayerProfileData,
    ) -> Result<()> {
        if is_invalid_card_id(card_id) {
            bail!("非法的人设卡 id: {card_id}");
        }

        let tx = db.begin().await.context("开启事务失败")?;
        player_profile::Entity::update_many()
            .col_expr(player_profile::Column::IsActive, Expr::value(false))
            .exec(&tx)
            .await
            .context("清除原激活人设失败")?;
        let row = PlayerProfileActiveModel {
            card_id: Set(card_id.to_string()),
            user_name: Set(profile.user_name.trim().to_string()),
            user_subtitle: Set(normalize_optional(profile.user_subtitle.clone())),
            user_prompt: Set(normalize_optional(profile.user_prompt.clone())),
            info: Set(normalize_optional(profile.info.clone())),
            system_prompt_example: Set(normalize_optional(profile.system_prompt_example.clone())),
            is_active: Set(true),
        };
        row.insert(&tx)
            .await
            .inspect_err(|e| {
                // 完整错误链写入日志：toast 只显示最外层，底层 DbErr 需靠日志排查。
                tracing::error!("创建人设卡 DB 插入失败（card_id={card_id}）: {e:?}");
            })
            .context("创建人设卡失败")?;
        tx.commit().await.context("提交创建人设卡失败")?;
        Ok(())
    }

    /// 删除一张人设卡。禁止删除当前激活人设，避免玩家身份悬空。
    pub async fn delete_persona(db: &DatabaseConnection, card_id: &str) -> Result<()> {
        if is_invalid_card_id(card_id) {
            bail!("非法的人设卡 id: {card_id}");
        }
        let active = Self::active_persona_id_option(db).await?;
        if active.as_deref() == Some(card_id) {
            bail!("不能删除当前激活人设");
        }
        // 与旧文件版行为对齐：目标本就不存在时静默成功。
        player_profile::Entity::delete_many()
            .filter(player_profile::Column::CardId.eq(card_id.to_string()))
            .exec(db)
            .await
            .context("删除人设卡失败")?;
        Ok(())
    }

    // ============ 内部工具 ============

    fn profile_from_row(row: &PlayerProfileRow) -> PlayerProfileData {
        PlayerProfileData {
            user_name: row.user_name.clone(),
            user_subtitle: row.user_subtitle.clone(),
            user_prompt: row.user_prompt.clone(),
            info: row.info.clone(),
            system_prompt_example: row.system_prompt_example.clone(),
        }
    }

    /// 从旧 AI 角色卡提取可迁移的玩家字段（fallback 优先，其次扫 DB 主角色）。
    async fn legacy_profile_fields(
        db: &DatabaseConnection,
        fallback: Option<&CharacterSettings>,
    ) -> (String, Option<String>) {
        if let Some(pair) = fallback.and_then(Self::extract_legacy_profile_fields) {
            return pair;
        }
        match RoleRepo::get_all_main_roles(db).await {
            Ok(roles) => {
                for role in roles {
                    match RoleRepo::get_role_settings_by_id(db, get_data_dir(), role.id).await {
                        Ok(Some(settings)) => {
                            if let Some(pair) = Self::extract_legacy_profile_fields(&settings) {
                                return pair;
                            }
                        }
                        Ok(None) => {}
                        Err(e) => {
                            tracing::warn!("读取主角色设置以迁移玩家档案失败: role_id={}, {e}", role.id);
                        }
                    }
                }
            }
            Err(e) => tracing::warn!("查询主角色列表以迁移玩家档案失败: {e}"),
        }
        (PlayerProfileData::default().user_name, None)
    }

    /// 从旧角色卡中提取可迁移的玩家字段。
    fn extract_legacy_profile_fields(
        settings: &CharacterSettings,
    ) -> Option<(String, Option<String>)> {
        let user_name = normalize_legacy_text(&settings.user_name)?;
        let user_subtitle = settings
            .user_subtitle
            .as_deref()
            .and_then(normalize_legacy_text)
            .map(|s| s.to_string());
        Some((user_name.to_string(), user_subtitle))
    }
}

/// 判断人设卡 id 是否非法（沿用原目录名安全约束：防路径穿越/控制字符）。
fn is_invalid_card_id(name: &str) -> bool {
    name.is_empty()
        || name == "."
        || name == ".."
        || name.contains('/')
        || name.contains('\\')
        || name.contains('\0')
        || name.chars().count() > 128
}
