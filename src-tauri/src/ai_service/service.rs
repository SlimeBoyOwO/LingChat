use std::path::PathBuf;
use std::sync::Arc;

use anyhow::Result;
use sea_orm::DatabaseConnection;
use tokio::sync::Mutex;

use crate::ai_service::config::AIServiceConfig;
use crate::ai_service::game_system::game_status::GameStatus;
use crate::ai_service::game_system::role_manager::GameRoleManager;
use crate::ai_service::game_system::script_engine::ScriptManager;
use crate::ai_service::llm::LlmClient;
use crate::ai_service::prompt::{sys_prompt_builder, PromptOptions};
use crate::ai_service::types::{CharacterSettings, GameLine, LineBase, LineAttributeExt};
use crate::config::tts::TtsConfig;
use crate::db::entities::line::LineAttribute;

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
        llm: Option<Arc<LlmClient>>,
        tts_config: TtsConfig,
    ) -> Self {
        // Initialize the event handler registry before any script is run
        crate::ai_service::game_system::script_engine::init_event_registry();

        let role_manager = GameRoleManager::new(data_dir.clone(), llm, tts_config);
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
        self.user_name = settings.user_name.clone();
        self.user_subtitle = settings.user_subtitle.clone();
        let base_prompt = settings.system_prompt.clone().unwrap_or(default_prompt);
        self.ai_prompt_example = settings.system_prompt_example.clone();
        self.ai_prompt_example_old = settings.system_prompt_example_old.clone();
        self.clothes_name = settings.clothes_name.clone();

        self.ai_prompt = sys_prompt_builder(
            &self.user_name,
            &self.ai_name,
            &base_prompt,
            self.ai_prompt_example.as_deref(),
            self.ai_prompt_example_old.as_deref(),
            prompt_options,
        );

        {
            let mut gs = self.game_status.lock().await;
            gs.player.user_name = self.user_name.clone();
            gs.player.user_subtitle = self.user_subtitle.clone().unwrap_or_default();
        }

        self.settings = Some(settings);
    }

    /// 初始化 `GameStatus`：清空台词列表，写入首条 system 人设，
    /// 并把导入的角色设为主角 + 上台。
    pub async fn init_game_status(&mut self) -> Result<()> {
        let mut gs = self.game_status.lock().await;
        gs.role_manager.reset_roles();
        gs.line_list.clear();
        gs.onstage_role_ids.clear();
        gs.present_role_ids.clear();

        let system_line = LineBase {
            content: self.ai_prompt.clone(),
            attribute: LineAttributeExt(LineAttribute::System),
            sender_role_id: self.character_id,
            display_name: Some(self.ai_name.clone()),
            ..Default::default()
        };
        gs.add_line(&self.db, system_line).await?;

        if let Some(cid) = self.character_id {
            let _ = gs.get_role(&self.db, cid).await?;
            gs.current_role_id = Some(cid);
            gs.onstage_role(cid);
            gs.main_role_id = Some(cid);
        } else {
            log::error!("初始化游戏主角失败，未指定角色ID。");
        }
        Ok(())
    }

    pub async fn set_active_save_id(&mut self, save_id: Option<i32>) {
        self.game_status.lock().await.active_save_id = save_id;
    }

    /// 载入存档台词并恢复 MemoryBank。
    pub async fn load_lines(
        &mut self,
        lines: Vec<GameLine>,
        main_role_id: i32,
        save_id: Option<i32>,
    ) -> Result<()> {
        {
            let mut gs = self.game_status.lock().await;
            gs.line_list = lines;
            if let Some(sid) = save_id {
                gs.active_save_id = Some(sid);
            }
            gs.refresh_memories(&self.db).await?;
            let _ = gs.get_role(&self.db, main_role_id).await?;
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
        self.game_status
            .lock()
            .await
            .role_manager
            .load_memory_banks_from_db(&self.db, save_id, None)
            .await
    }

    /// 轻量清理：只清空台词 + 主角短期记忆，NPC 记忆保留。
    pub async fn clear_lines(&mut self) -> Result<()> {
        let mut gs = self.game_status.lock().await;
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
        log::info!("对话历史已清除（仅主角记忆）");
        Ok(())
    }

    pub async fn reset_lines(&mut self) -> Result<()> {
        self.init_game_status().await
    }
}

/// 在 Tauri managed state 中共享的句柄。
pub type SharedAIService = Arc<Mutex<AIService>>;
