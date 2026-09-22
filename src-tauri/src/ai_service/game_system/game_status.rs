use std::collections::{HashMap, HashSet};

use anyhow::Result;
use chrono::{DateTime, Local};
use sea_orm::DatabaseConnection;
use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::ai_service::game_system::role_manager::{GameRoleManager, user_identity_settings};
use crate::ai_service::types::{
    CharacterSettings, GameLine, GameRole, LineAttributeExt, LineBase, PLAYER_ROLE_ID, Player,
    ScriptStatus,
};
use crate::db::entities::line::LineAttribute;
use crate::db::entities::role::RoleType;
use crate::db::managers::role_repo::RoleRepo;
use crate::utils::prompt::{PromptOptions, PromptRole, sys_prompt_builder_by_settings};

/// 存储所有运行时共享的游戏状态。
pub struct GameStatus {
    /// 玩家名/副标题/人设的**缓存视图**，默认只经 `refresh_possessed_cache` 维护。
    /// 真相源是 `role` 表 + `profile_json`：全局有大量 `player.user_name` 读取点
    /// （占位符替换、提示词构建、前端展示），缓存起来避免每个读取点各自查库。
    /// 例外仅两处、均不入库且可被下一次 refresh 冲掉：剧本 `script_settings` 临时覆盖、
    /// 编辑器试玩的快照备份与还原。
    pub player: Player,

    /// 当前被玩家附身的实体 role_id。默认 `PLAYER_ROLE_ID`（默认身份实体）。
    /// 附身是会话态而非实体属性，因此不写回 `role.role_type`；它决定玩家台词的
    /// `sender_role_id` 与被附身 AI 的生成休眠。
    pub possessed_role_id: i32,

    /// 玩家身份实体 id 缓存（`role_type=User` ∪ {0}）。
    ///
    /// 供同步签名的判据（God Agent 候选过滤、附身接管者选择）读取——这些调用点
    /// 拿不到 DB，故由初始化/清档/附身/读档/身份增删等写路径调用
    /// `refresh_human_role_ids` 维护。不参与存档快照，属于会话语境的派生缓存。
    pub human_role_ids: HashSet<i32>,

    /// 台词列表，用于记忆构建和历史记忆
    pub line_list: Vec<GameLine>,

    pub role_manager: GameRoleManager,
    /// 当前对话角色的 role_id；作为 LLM 传输入的对象，使用本角色的记忆
    pub current_role_id: Option<i32>,
    /// 舞台角色 role_id 列表：用于展示舞台上角色的信息（保持顺序）
    pub onstage_role_ids: Vec<i32>,
    /// 在场角色 role_id 集合：只有在场的角色才能感知到台词
    pub present_role_ids: HashSet<i32>,
    /// 游戏主角的 role_id（剧本模式冒险的主角）
    pub main_role_id: Option<i32>,

    pub background: String,
    pub present_pic: String,
    pub background_music: String,
    pub background_effect: String,

    /// 当前用户选择的场景 ID（对应 scenes.json 中的场景）
    pub current_scene_id: Option<String>,
    /// 上一次 process_message 处理时的场景 ID，用于检测场景切换
    pub last_processed_scene_id: Option<String>,

    pub global_variables: HashMap<String, Value>,
    pub completed_scripts: HashSet<String>,
    pub last_dialog_time: Option<DateTime<Local>>,

    pub script_status: Option<ScriptStatus>,

    /// 当前激活的存档 ID（用于 MemoryBank 持久化/载入/自动压缩）
    pub active_save_id: Option<i32>,

    /// 试玩会话代号。每次试玩「进来备份 / 走时还原」都会递增；
    /// 消息生成管线在写入台词前比对捕获值与当前值，不一致即视为已过期
    /// （试玩任务被中止后，游离的流式任务可能仍在写）——直接丢弃，保证
    /// 试玩内容不会漏进已还原的自由对话会话。自由对话本身不递增，恒等比对，
    /// 行为不受影响。
    pub preview_generation: u64,

    /// 本会话是否已触发过入场问候（内存标记，重启/清档重置）。
    /// `notify_player_entry` 靠它去重，避免重复生成问候台词；与"玩家是否在游戏里"无关。
    pub entry_greeting_done: bool,

    /// 场景感知开关（关闭后切换场景不再触发旁白）
    pub scene_awareness_enabled: bool,
}

impl GameStatus {
    pub fn new(role_manager: GameRoleManager) -> Self {
        Self {
            player: Player::default(),
            possessed_role_id: PLAYER_ROLE_ID,
            human_role_ids: HashSet::new(),
            line_list: Vec::new(),
            role_manager,
            current_role_id: None,
            onstage_role_ids: Vec::new(),
            present_role_ids: HashSet::new(),
            main_role_id: None,
            background: String::new(),
            present_pic: String::new(),
            background_music: String::new(),
            background_effect: String::new(),
            current_scene_id: None,
            last_processed_scene_id: None,
            global_variables: HashMap::new(),
            completed_scripts: HashSet::new(),
            last_dialog_time: None,
            script_status: None,
            active_save_id: None,
            preview_generation: 0,
            entry_greeting_done: false,
            scene_awareness_enabled: true,
        }
    }

    pub async fn get_role<'a>(
        &'a mut self,
        db: &DatabaseConnection,
        role_id: i32,
    ) -> Result<&'a mut GameRole> {
        self.role_manager.get_role(db, role_id).await
    }

    /// 追加台词，记录当前在场者为感知列表，并刷新相关角色的记忆。
    pub async fn add_line(&mut self, db: &DatabaseConnection, line: LineBase) -> Result<()> {
        let perceived: Vec<i32> = self.present_role_ids.iter().copied().collect();
        let game_line = GameLine::from_base(line, perceived);
        self.line_list.push(game_line);
        self.refresh_memories(db).await?;
        Ok(())
    }

    pub async fn refresh_memories(&mut self, db: &DatabaseConnection) -> Result<()> {
        self.role_manager
            .sync_memories(db, &self.line_list, None)
            .await
    }

    /// 所有"是不是当前扮演者"的判据都应走这里，避免各处直接拿 `possessed_role_id`
    /// 做相等比较而散落口径。
    pub fn is_possessed(&self, role_id: i32) -> bool {
        self.possessed_role_id == role_id
    }

    /// 刷新玩家身份实体 id 缓存（`role_type=User` ∪ {0}）。
    ///
    /// 身份集合只在新建/删除身份时变化，但本缓存要供同步判据使用，故在
    /// 初始化/清档/附身/读档/身份增删等写路径后统一刷新。
    pub async fn refresh_human_role_ids(&mut self, db: &DatabaseConnection) -> Result<()> {
        self.human_role_ids = RoleRepo::get_user_role_ids(db).await?;
        Ok(())
    }

    /// 刷新玩家缓存视图：把当前被附身实体的名字/副标题/人设写回 `player`。
    ///
    /// 读不到实体（如刚被删除）时保留原值并告警，不阻断对话链路——缓存允许短暂滞后，
    /// 下一轮附身/读档会再次校正。
    pub async fn refresh_possessed_cache(&mut self, db: &DatabaseConnection) -> Result<()> {
        let role_id = self.possessed_role_id;
        let Some(role) = RoleRepo::get_role_by_id(db, role_id).await? else {
            tracing::warn!("附身实体不存在，玩家缓存保持原值: role_id={}", role_id);
            return Ok(());
        };
        let profile = RoleRepo::get_role_profile(db, role_id).await?;

        // 显示名/副标题口径与前端身份切换器一致：AI 角色以 settings.yml 的 ai_name 为准
        // （role.name 只是入库时的名字，改 ai_name 后不再同步）；User 身份则以 role.name 为准。
        let (name, subtitle) = if role.role_type == RoleType::User {
            (role.name.clone(), profile.subtitle.clone())
        } else {
            let loaded_settings = self
                .role_manager
                .get_loaded(role_id)
                .map(|loaded| loaded.settings.clone());
            let settings = match loaded_settings {
                Some(settings) => Some(settings),
                None => {
                    RoleRepo::get_role_settings_by_id(db, &crate::api::data_dir(), role_id).await?
                },
            };
            match settings {
                Some(settings) => (
                    settings.ai_name.clone(),
                    settings.ai_subtitle.clone().unwrap_or_default(),
                ),
                // settings.yml 缺失时退回 role.name，避免玩家名被占位默认值污染
                None => (role.name.clone(), profile.subtitle.clone()),
            }
        };

        self.player.user_name = if name.trim().is_empty() {
            "玩家".to_string()
        } else {
            name
        };
        self.player.user_subtitle = subtitle;
        Ok(())
    }

    // ============ 全局变量便捷方法 ============

    pub fn set_variable(&mut self, key: impl Into<String>, value: Value) {
        self.global_variables.insert(key.into(), value);
    }

    pub fn get_variable(&self, key: &str) -> Option<&Value> {
        self.global_variables.get(key)
    }

    /// 非系统消息数量（用于羁绊冒险解锁条件检测）
    pub fn chat_message_count(&self) -> usize {
        self.line_list
            .iter()
            .filter(|l| !matches!(l.attribute(), LineAttribute::System))
            .count()
    }

    // ============ 舞台管理 ============

    pub fn onstage_role(&mut self, role_id: i32) {
        if !self.onstage_role_ids.contains(&role_id) {
            self.onstage_role_ids.push(role_id);
        }
        self.present_role_ids.insert(role_id);
    }

    pub fn offstage_role(&mut self, role_id: i32) {
        self.onstage_role_ids.retain(|id| *id != role_id);
        self.present_role_ids.remove(&role_id);
    }

    pub async fn add_character_clothes_change_line(
        &mut self,
        db: &DatabaseConnection,
        role_id: i32,
        clothes_name: &str,
    ) -> Result<()> {
        let role = self
            .role_manager
            .get_loaded_mut(role_id)
            .ok_or_else(|| anyhow::anyhow!("角色 {} 未加载", role_id))?;

        role.current_clothes = clothes_name.to_string();

        let ai_name = role.settings.ai_name.clone();
        let clothes_prompt = role
            .settings
            .clothes
            .as_ref()
            .and_then(|list| {
                list.iter().find_map(|item| {
                    if item.get("name").map(|s| s.as_str()) == Some(clothes_name) {
                        item.get("prompt").cloned()
                    } else {
                        None
                    }
                })
            })
            .unwrap_or_default();

        let prompt = format!(
            "{}换上了新服装：{}，{}",
            ai_name, clothes_name, clothes_prompt
        );

        self.add_line(
            db,
            LineBase {
                content: PromptRole::Narrator.build_prompt(&prompt),
                attribute: LineAttributeExt(LineAttribute::User),
                display_name: Some("旁白".to_string()),
                ..Default::default()
            },
        )
        .await
        .map_err(|e| anyhow::anyhow!("添加换装台词失败: {}", e))?;

        Ok(())
    }

    /// 切换角色服装并生成旁白台词。
    /// 若已是目标服装则跳过。返回是否实际切换。
    pub async fn on_character_change_clothes(
        &mut self,
        db: &DatabaseConnection,
        role_id: i32,
        clothes_name: &str,
    ) -> Result<bool> {
        let role = self
            .role_manager
            .get_loaded_mut(role_id)
            .ok_or_else(|| anyhow::anyhow!("角色 {} 未加载", role_id))?;

        if role.current_clothes == clothes_name {
            return Ok(false);
        }

        self.add_character_clothes_change_line(db, role_id, clothes_name)
            .await?;

        Ok(true)
    }

    pub fn reactivate_all_voice_makers(&self) {
        self.role_manager.reactivate_all_voice_makers();
    }

    // ============ SYSTEM 人设行 ============

    /// 按最新附身/人设信息重建全部 SYSTEM 人设行，并为在场实体补建缺失的人设行。
    ///
    /// 为什么必须重建：统一实体后"玩家名"由 `sys_prompt_builder` 的 framing 前缀嵌进
    /// 每条 AI 人设，附身切换/身份改名后不同步，模型会继续以为在跟旧名字的人说话。
    /// 做法是按行 sender 重新调用构建器，**保留行 id 只替换 content**——严禁字符串替换，
    /// 否则既可能漏改格式提示里的名字，也可能误伤正文中恰好同名的词。
    ///
    /// 补建的意义：附身一个此前没有 SYSTEM 行的实体后，"在场即有完整人设"必须由这里
    /// 保证——否则 `sync_memories` 找不到该角色的人设行，模型感知不到它。
    ///
    /// AI 角色优先取内存中已加载的最新 settings，未加载才回盘；User 实体用合成 settings。
    pub async fn rebuild_system_prompts(
        &mut self,
        db: &DatabaseConnection,
        options: PromptOptions,
    ) -> Result<()> {
        let data_dir = crate::api::data_dir();
        let player_name = self.player.user_name.clone();

        // a) 重写已存在的 SYSTEM 行：content 与 display_name 同步为实体当前值
        let system_indices: Vec<usize> = self
            .line_list
            .iter()
            .enumerate()
            .filter(|(_, line)| matches!(line.attribute(), LineAttribute::System))
            .map(|(index, _)| index)
            .collect();

        for index in system_indices {
            let Some(role_id) = self.line_list[index].base.sender_role_id else {
                continue;
            };
            let Some((settings, display_name)) =
                self.resolve_role_persona(db, &data_dir, role_id).await?
            else {
                continue;
            };
            self.line_list[index].base.content =
                sys_prompt_builder_by_settings(&settings, options, &player_name);
            // 署名跟随实体当前显示名，修 ai_name 改名后 SYSTEM 行仍挂着旧名的问题
            self.line_list[index].base.display_name = Some(display_name);
        }

        // b) 补建：在场且已加载（或可加载）的实体若无 SYSTEM 行则插入一条
        let present_ids: Vec<i32> = self.present_role_ids.iter().copied().collect();
        for role_id in present_ids {
            let has_system = self.line_list.iter().any(|line| {
                matches!(line.attribute(), LineAttribute::System)
                    && line.base.sender_role_id == Some(role_id)
            });
            if has_system {
                continue;
            }
            let Some((settings, display_name)) =
                self.resolve_role_persona(db, &data_dir, role_id).await?
            else {
                continue;
            };
            let line = LineBase {
                content: sys_prompt_builder_by_settings(&settings, options, &player_name),
                attribute: LineAttributeExt(LineAttribute::System),
                sender_role_id: Some(role_id),
                display_name: Some(display_name),
                ..Default::default()
            };
            // 感知者记录与 add_line 一致：当前在场者都感知到这条人设行
            let perceived: Vec<i32> = self.present_role_ids.iter().copied().collect();
            let insert_at = system_line_insert_pos(&self.line_list, role_id);
            self.line_list
                .insert(insert_at, GameLine::from_base(line, perceived));
        }

        self.refresh_memories(db).await?;
        Ok(())
    }

    /// 解析实体的运行时人设与显示名，供 SYSTEM 行重建使用。
    ///
    /// User 实体没有 settings.yml，用 `role` + `profile_json` 合成 settings；
    /// AI 角色优先取内存中已加载的最新 settings，未加载才回盘。返回 `None`
    /// 表示实体不存在或人设不可用（如 settings.yml 缺失）。
    async fn resolve_role_persona(
        &self,
        db: &DatabaseConnection,
        data_dir: &std::path::Path,
        role_id: i32,
    ) -> Result<Option<(CharacterSettings, String)>> {
        let Some(role) = RoleRepo::get_role_by_id(db, role_id).await? else {
            return Ok(None);
        };
        // 先取出内存 settings 并立即释放对 role_manager 的借用，避免跨越磁盘查询的 await
        let loaded_settings = self
            .role_manager
            .get_loaded(role_id)
            .map(|loaded| loaded.settings.clone());
        let settings = if role.role_type == RoleType::User {
            let profile = RoleRepo::get_role_profile(db, role_id).await?;
            user_identity_settings(&role, &profile)
        } else if let Some(settings) = loaded_settings {
            settings
        } else {
            match RoleRepo::get_role_settings_by_id(db, data_dir, role_id).await? {
                Some(settings) => settings,
                None => return Ok(None),
            }
        };
        let display_name = settings.ai_name.clone();
        Ok(Some((settings, display_name)))
    }

    // ============ 存档状态快照 ============

    /// 将当前 GameStatus 中需要持久化的字段导出为可序列化的快照
    pub fn to_snapshot(&self) -> GameStatusSnapshot {
        GameStatusSnapshot {
            present_role_ids: self.present_role_ids.iter().copied().collect(),
            current_role_id: self.current_role_id,
            possessed_role_id: self.possessed_role_id,
            background: self.background.clone(),
            background_music: self.background_music.clone(),
            background_effect: self.background_effect.clone(),
            current_scene_id: self.current_scene_id.clone(),
            global_variables: self.global_variables.clone(),
            completed_scripts: self.completed_scripts.iter().cloned().collect(),
            last_dialog_time: self.last_dialog_time.map(|dt| dt.to_rfc3339()),
            scene_awareness_enabled: self.scene_awareness_enabled,
        }
    }

    /// 从快照恢复场景状态。
    ///
    /// 尾部会按快照里的被附身实体刷新玩家缓存——`player` 是缓存视图，
    /// 读档后必须与 `possessed_role_id` 同源，否则界面仍显示上一个存档的玩家名。
    pub async fn apply_snapshot(&mut self, snapshot: &GameStatusSnapshot, db: &DatabaseConnection) {
        self.background = snapshot.background.clone();
        self.background_music = snapshot.background_music.clone();
        self.background_effect = snapshot.background_effect.clone();
        self.current_scene_id = snapshot.current_scene_id.clone();
        self.global_variables = snapshot.global_variables.clone();
        self.completed_scripts = snapshot.completed_scripts.iter().cloned().collect();
        self.last_dialog_time = snapshot.last_dialog_time.as_ref().and_then(|s| {
            chrono::DateTime::parse_from_rfc3339(s)
                .ok()
                .map(|dt| dt.with_timezone(&Local))
        });
        self.current_role_id = snapshot.current_role_id;
        self.present_role_ids = snapshot.present_role_ids.iter().copied().collect();
        self.onstage_role_ids = snapshot.present_role_ids.clone();
        self.scene_awareness_enabled = snapshot.scene_awareness_enabled;
        self.possessed_role_id = snapshot.possessed_role_id;
        if let Err(e) = self.refresh_possessed_cache(db).await {
            tracing::warn!("应用快照后刷新附身缓存失败: {e}");
        }
        // 玩家身份集合不随快照变化，但读档会跨越一次会话边界，顺手校正缓存
        if let Err(e) = self.refresh_human_role_ids(db).await {
            tracing::warn!("应用快照后刷新玩家身份缓存失败: {e}");
        }
    }
}

/// 为缺失 SYSTEM 行的实体挑选插入位置。
///
/// 优先紧随该角色首条台词之前，让记忆构建按"人设在前、台词在后"切分；
/// 尚无任何台词时插到已有最后一条 SYSTEM 行之后，保持人设行成组。
fn system_line_insert_pos(lines: &[GameLine], role_id: i32) -> usize {
    if let Some(index) = lines
        .iter()
        .position(|line| line.base.sender_role_id == Some(role_id))
    {
        return index;
    }
    lines
        .iter()
        .rposition(|line| matches!(line.attribute(), LineAttribute::System))
        .map(|index| index + 1)
        .unwrap_or(lines.len())
}

/// `GameStatus` 中需要持久化到 `save.status` JSON 的字段。
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub struct GameStatusSnapshot {
    pub present_role_ids: Vec<i32>,
    pub current_role_id: Option<i32>,
    /// 当前被附身实体。旧存档无此字段 → `#[serde(default)]` 得 0，
    /// 即回落到默认身份实体，与统一实体前的语义一致。
    #[serde(default)]
    pub possessed_role_id: i32,
    #[serde(default)]
    pub background: String,
    #[serde(default = "default_background_music")]
    pub background_music: String,
    #[serde(default = "default_background_effect")]
    pub background_effect: String,
    #[serde(default)]
    pub current_scene_id: Option<String>,
    #[serde(default)]
    pub global_variables: HashMap<String, Value>,
    #[serde(default)]
    pub completed_scripts: Vec<String>,
    pub last_dialog_time: Option<String>,
    #[serde(default = "default_true")]
    pub scene_awareness_enabled: bool,
}

fn default_true() -> bool {
    true
}

fn default_background_music() -> String {
    "none".into()
}
fn default_background_effect() -> String {
    "none".into()
}
