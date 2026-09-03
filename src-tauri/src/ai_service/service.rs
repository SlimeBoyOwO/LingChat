use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Arc;

use anyhow::Result;
use sea_orm::DatabaseConnection;
use tokio::sync::Mutex;

use crate::ai_service::config::AIServiceConfig;
use crate::ai_service::game_system::game_status::GameStatus;
use crate::ai_service::game_system::persistent_memory_system::MemorySectionLimits;
use crate::ai_service::game_system::player_profile_sync::rebuild_system_lines;
use crate::ai_service::game_system::role_manager::GameRoleManager;
use crate::ai_service::game_system::script_engine::ScriptManager;
use crate::ai_service::llm::LlmSlot;
use crate::ai_service::tts::local::LocalTtsRuntime;
use crate::ai_service::types::{CharacterSettings, GameLine, LineAttributeExt, LineBase};
use crate::config::tts::TtsConfig;
use crate::db::entities::line::LineAttribute;
use crate::utils::prompt::{PromptOptions, sys_prompt_builder};

/// AI 服务：承载 `GameStatus` 与会话级配置。
///
/// 本轮仅实现 Python 版 `AIService` 中与状态管理相关的部分：
/// import_settings / init_game_status / get_lines / load_lines /
/// reset_lines / clear_lines / set_active_save_id。
/// 消息生成（MessageGenerator）、主动对话（ProactiveSystem）、剧本引擎（ScriptManager）
/// 等子系统按计划稍后补。
pub struct AIService {
    pub db: DatabaseConnection,
    pub data_dir: PathBuf,
    pub game_status: Arc<Mutex<GameStatus>>,
    pub config: AIServiceConfig,

    // —— 从 CharacterSettings 导入的快照字段（Python 版的 self.ai_prompt 等） ——
    pub character_path: Option<String>,
    pub character_id: Option<i32>,
    pub ai_name: String,
    pub ai_subtitle: Option<String>,
    pub user_name: String,
    pub user_subtitle: Option<String>,
    /// 玩家设定块（简介/人格/示例，全局 player_profile 纯 DB 驱动）。
    /// 解耦玩家与 AI 后，把它注入系统提示词，让 AI 了解屏幕对面用户的身份与性格。
    pub player_prompt: String,
    pub ai_prompt: String,
    pub ai_prompt_example: Option<String>,
    pub ai_prompt_example_old: Option<String>,
    pub clothes_name: Option<String>,
    pub settings: Option<CharacterSettings>,

    /// Script/story mode engine: discovers and runs scripts.
    pub script_manager: ScriptManager,
}

impl AIService {
    pub async fn new(
        db: DatabaseConnection,
        data_dir: PathBuf,
        llm: LlmSlot,
        tts_config: TtsConfig,
        local_tts: Option<LocalTtsRuntime>,
        use_persistent_memory: bool,
        memory_update_interval: u32,
        memory_recent_window: u32,
        memory_limits: MemorySectionLimits,
    ) -> Self {
        // Initialize the event handler registry before any script is run
        crate::ai_service::game_system::script_engine::init_event_registry();

        let role_manager = GameRoleManager::new(
            data_dir.clone(),
            llm,
            tts_config,
            local_tts,
            use_persistent_memory,
            memory_update_interval,
            memory_recent_window,
            memory_limits,
        );
        let game_status = Arc::new(Mutex::new(GameStatus::new(role_manager)));
        let script_manager = ScriptManager::new(&data_dir);
        Self {
            db,
            data_dir,
            game_status,
            config: AIServiceConfig::default(),
            character_path: None,
            character_id: None,
            ai_name: String::new(),
            ai_subtitle: None,
            user_name: String::new(),
            user_subtitle: None,
            player_prompt: String::new(),
            ai_prompt: String::new(),
            ai_prompt_example: None,
            ai_prompt_example_old: None,
            clothes_name: None,
            settings: None,
            script_manager,
        }
    }

    /// 从 `CharacterSettings` 导入快照字段，并初始化 player 信息。
    ///
    /// `prompt_options` 控制对话格式提示（日语开关、情绪放开）。
    /// 调用方（通常是 Tauri command）从 AppConfig 读取后传入。
    /// 钦灵：TODO，这一段代码是老函数，新版已经不是单一人物，而是多个人物，这部分代码之后需要清理。
    ///
    /// 解耦玩家与 AI：玩家身份不再从 CharacterSettings 中读取，
    /// 而是从全局 player_profile（纯 DB）加载（fallback 到 CharacterSettings 以兼容旧数据）。
    pub async fn import_settings(
        &mut self,
        settings: CharacterSettings,
        prompt_options: PromptOptions,
    ) {
        let default_prompt =
            "你的信息被设置错误了，请你在接下来的对话中提示用户检查配置信息".to_string();

        self.character_path = settings.resource_path.clone();
        self.character_id = settings.character_id;
        self.ai_name = settings.ai_name.clone();
        self.ai_subtitle = settings.ai_subtitle.clone();
        let base_prompt = settings.system_prompt.clone().unwrap_or(default_prompt);
        self.ai_prompt_example = settings.system_prompt_example.clone();
        self.ai_prompt_example_old = settings.system_prompt_example_old.clone();
        self.clothes_name = settings.clothes_name.clone(); // TODO: 这个是冗余的，之后可以去掉

        // 玩家身份从全局 player_profile（纯 DB）加载（解耦：不再从角色 settings.yml 读取）
        // 读取整个档案，并把「设定块」（简介/人格/示例）合并注入系统提示词。
        // 表为空时，自动从旧角色卡迁移 user_name/user_subtitle 播种默认人设卡。
        let (user_name, user_subtitle, player_prompt) = {
            match crate::db::managers::player_profile_repo::PlayerProfileRepo::ensure_profile(
                &self.db,
                Some(&settings),
            )
            .await
            {
                Ok(profile) => {
                    let uname = profile.user_name.clone();
                    let usub = profile.user_subtitle.clone().unwrap_or_default();
                    let uprompt = profile.to_prompt_fragment();
                    (uname, usub, uprompt)
                }
                Err(e) => {
                    tracing::warn!("读取玩家档案失败，回退到角色设置: {e}");
                    // 回退到旧行为：从 CharacterSettings 读（兼容老版本数据）
                    (
                        settings.user_name.clone(),
                        settings.user_subtitle.clone().unwrap_or_default(),
                        String::new(),
                    )
                }
            }
        };

        self.user_name = user_name.clone();
        self.user_subtitle = if user_subtitle.is_empty() { None } else { Some(user_subtitle.clone()) };
        // 玩家设定块（简介/人格/示例）也存到 AIService，供后续注入系统提示词
        self.player_prompt = player_prompt.clone();

        self.ai_prompt = sys_prompt_builder(
            &self.user_name,
            &self.ai_name,
            &base_prompt,
            self.ai_prompt_example.as_deref(),
            self.ai_prompt_example_old.as_deref(),
            prompt_options,
            &player_prompt,
        );

        {
            let mut gs = self.game_status.lock().await;
            gs.player.user_name = self.user_name.clone();
            gs.player.user_subtitle = self.user_subtitle.clone().unwrap_or_default();
            gs.player.user_prompt = player_prompt;
        }

        self.settings = Some(settings);
    }

    /// 初始化 `GameStatus`：清空台词列表，写入首条 system 人设，
    /// 并把导入的角色设为主角 + 上台。
    /// 注入角色服装覆盖（session store → GameRoleManager）。
    /// 必须在 `init_game_status()` 之前调用。
    pub async fn set_clothes_overrides(&mut self, overrides: HashMap<i32, String>) {
        let mut gs = self.game_status.lock().await;
        gs.role_manager.set_clothes_overrides(overrides);
    }

    pub async fn init_game_status(&mut self) -> Result<()> {
        let mut gs = self.game_status.lock().await;
        gs.role_manager.invalidate_memory_history();
        gs.role_manager.reset_roles();
        gs.line_list.clear();
        gs.onstage_role_ids.clear();
        gs.present_role_ids.clear();
        gs.player_entered = false;

        let system_line = LineBase {
            content: self.ai_prompt.clone(),
            attribute: LineAttributeExt(LineAttribute::System),
            sender_role_id: self.character_id,
            display_name: Some(self.ai_name.clone()),
            ..Default::default()
        };
        gs.add_line(&self.db, system_line).await?;

        if let Some(cid) = self.character_id {
            // 此处是初始角色被注册的地方
            let _ = gs.get_role(&self.db, cid).await?;
            gs.current_role_id = Some(cid);
            gs.onstage_role(cid);
            gs.main_role_id = Some(cid);

            // 若恢复的服装不是默认服装，生成换装旁白
            let clothes = gs
                .role_manager
                .get_loaded(cid)
                .map(|r| r.current_clothes.clone())
                .unwrap_or_default();
            tracing::info!("外部获取的当前服装是: {:?}", clothes);
            if clothes != "default" && !clothes.is_empty() {
                // 不是你个傻逼 AI 角色服装已经换过了你再他妈比较那台词表能变吗我草你的？，已修复
                let _ = gs
                    .add_character_clothes_change_line(&self.db, cid, &clothes)
                    .await;
            }
        } else {
            tracing::error!("初始化游戏主角失败，未指定角色ID。");
        }
        Ok(())
    }

    pub async fn set_active_save_id(&mut self, save_id: Option<i32>) {
        self.game_status.lock().await.active_save_id = save_id;
    }

    /// 载入存档台词并恢复 MemoryBank。
    ///
    /// `prompt_options` 用于在记忆刷新前按当前玩家档案重建 System 人设行，
    /// 保证旧档里的旧名字/旧设定不会先进入角色记忆。
    pub async fn load_lines(
        &mut self,
        lines: Vec<GameLine>,
        main_role_id: i32,
        save_id: Option<i32>,
        prompt_options: PromptOptions,
    ) -> Result<()> {
        {
            let mut gs = self.game_status.lock().await;
            gs.line_list = lines;
            if let Some(sid) = save_id {
                gs.active_save_id = Some(sid);
            }
            // 先加载主角设置，再整体重建旧档里的 System 行；旧 System 行不会被
            // 后续 sync_memories 写进角色记忆。
            let _ = gs.get_role(&self.db, main_role_id).await?;
            rebuild_system_lines(&self.db, &self.data_dir, &mut gs, prompt_options).await?;
            gs.role_manager.invalidate_memory_history();
            gs.refresh_memories(&self.db).await?;
            gs.current_role_id = Some(main_role_id);
            gs.main_role_id = Some(main_role_id);
        }
        Ok(())
    }

    /// 将当前所有已加载角色的 `GameMemoryBank` 持久化到 DB。
    /// 委托给 `GameRoleManager` 以确保后台压缩结果先同步再写入。
    pub async fn persist_memory_banks(&mut self, save_id: i32) -> Result<()> {
        self.game_status
            .lock()
            .await
            .role_manager
            .persist_memory_banks_to_db(&self.db, save_id, None)
            .await
    }

    /// 从 DB 恢复所有 MemoryBank 到对应已加载角色，并惰性创建压缩系统。
    pub async fn restore_memory_banks(&mut self, save_id: i32) -> Result<()> {
        // 玩家名用于永久记忆压缩时格式化旧 User 行；先取快照再二次加锁，
        // 避免 tokio::Mutex 不可重入。
        let player_name = {
            let gs = self.game_status.lock().await;
            gs.player.user_name.clone()
        };
        self.game_status
            .lock()
            .await
            .role_manager
            .load_memory_banks_from_db(&self.db, save_id, None, &player_name)
            .await
    }

    /// 轻量清理：只清空台词 + 主角短期记忆，NPC 记忆保留。
    pub async fn clear_lines(&mut self) -> Result<()> {
        let mut gs = self.game_status.lock().await;
        gs.role_manager.invalidate_memory_history();
        gs.line_list.clear();

        let system_line = LineBase {
            content: self.ai_prompt.clone(),
            attribute: LineAttributeExt(LineAttribute::System),
            sender_role_id: self.character_id,
            display_name: Some(self.ai_name.clone()),
            ..Default::default()
        };
        gs.add_line(&self.db, system_line).await?;

        if let Some(mid) = gs.main_role_id {
            gs.role_manager.clear_role_memory(mid);
        }
        tracing::info!("对话历史已清除（仅主角记忆）");
        Ok(())
    }

    pub async fn reset_lines(&mut self) -> Result<()> {
        self.init_game_status().await
    }
}

/// 在 Tauri managed state 中共享的句柄。
pub type SharedAIService = Arc<Mutex<AIService>>;
