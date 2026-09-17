use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Arc;

use anyhow::Result;
use sea_orm::DatabaseConnection;
use tokio::sync::Mutex;

use crate::ai_service::config::AIServiceConfig;
use crate::ai_service::game_system::game_status::GameStatus;
use crate::ai_service::game_system::persistent_memory_system::MemorySectionLimits;
use crate::ai_service::game_system::role_manager::GameRoleManager;
use crate::ai_service::game_system::script_engine::ScriptManager;
use crate::ai_service::llm::LlmSlot;
use crate::ai_service::tts::local::LocalTtsRuntime;
use crate::ai_service::types::{CharacterSettings, GameLine, LineAttributeExt, LineBase};
use crate::config::tts::TtsConfig;
use crate::db::entities::line::LineAttribute;
use crate::ai_service::game_system::player_identity::{
    IdentityStore, RelationEndpoint, build_player_block, resolve_relation, role_relations,
};
use crate::utils::prompt::{PromptOptions, sys_prompt_builder_with_player};

/// AI 服务：承载 `GameStatus` 与会话级配置。
pub struct AIService {
    pub db: DatabaseConnection,
    pub data_dir: PathBuf,
    pub game_status: Arc<Mutex<GameStatus>>,
    pub config: AIServiceConfig,

    pub init_character_id: Option<i32>, // 注释：这个是用于标记游戏状态初始化角色的
    pub prompt_options: Option<PromptOptions>, // 记录角色提示词构成方式选项

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
        memory_inject_continue_user: bool,
    ) -> Self {
        // Initialize the event handler registry before any script is run
        crate::ai_service::game_system::script_engine::init_event_registry();

        let role_manager = GameRoleManager::new(
            data_dir.clone(),
            db.clone(),
            llm,
            tts_config,
            local_tts,
            use_persistent_memory,
            memory_update_interval,
            memory_recent_window,
            memory_limits,
            memory_inject_continue_user,
        );
        let game_status = Arc::new(Mutex::new(GameStatus::new(role_manager)));
        let script_manager = ScriptManager::new(&data_dir);
        Self {
            db,
            data_dir,
            game_status,
            config: AIServiceConfig::default(),
            init_character_id: None,
            prompt_options: None,
            script_manager,
        }
    }

    pub async fn set_clothes_overrides(&mut self, overrides: HashMap<i32, String>) {
        let mut gs = self.game_status.lock().await;
        gs.role_manager.set_clothes_overrides(overrides);
    }

    pub async fn init_game_intro_character(
        &mut self,
        character_id: Option<i32>,
        prompt_options: PromptOptions,
    ) -> Result<()> {
        self.init_character_id = character_id;
        self.prompt_options = Some(prompt_options);

        let default_prompt =
            "你的信息被设置错误了，请你在接下来的对话中提示用户检查配置信息".to_string();

        let cid = match character_id {
            Some(v) => v,
            None => {
                tracing::error!("初始化游戏主角失败，未指定角色ID。");
                return Ok(());
            },
        };

        tracing::info!("正在初始化的角色id是: {:?}", cid);

        let mut gs = self.game_status.lock().await;

        let settings = gs
            .role_manager
            .get_role(&self.db, cid)
            .await?
            .settings
            .clone();

        // ── 「我」的身份 ────────────────────────────────────────────
        // 优先使用当前选中的身份卡；从未选择过时，按老规则（AI 角色卡上的
        // user_name / user_subtitle）合成一张并**落盘**，让「我」的名字
        // 从此不再随 AI 角色卡变化——这是本次改造的核心目的。
        let identity_store = IdentityStore::new(&self.data_dir);
        let identity = identity_store.current_or_synthesize(
            &settings.user_name,
            settings.user_subtitle.as_deref().unwrap_or_default(),
        );

        // 说话者（当前 AI 角色）眼中的「我」：关系按 (说话者, 目标) 解析。
        // 指向不存在对象的键会被静默跳过（resolve_relation 内部处理）。
        let speaker = RelationEndpoint::Ai(settings.character_folder.clone());
        let target = RelationEndpoint::Me(identity.id.clone());
        let speaker_relations = role_relations::load(&self.data_dir, &settings.character_folder);
        let relation = resolve_relation(
            &speaker,
            &target,
            &speaker_relations,
            &identity.relations,
            Some(identity.prompt.as_str()),
        );
        let player_block = build_player_block(
            &identity.name,
            &identity.subtitle,
            &identity.prompt,
            relation.as_ref(),
        );

        let ai_prompt = sys_prompt_builder_with_player(
            identity.display_name(),
            &settings.ai_name.clone(),
            &player_block,
            &settings.system_prompt.clone().unwrap_or(default_prompt),
            settings.system_prompt_example.clone().as_deref(),
            settings.system_prompt_example_old.clone().as_deref(),
            prompt_options,
        );

        // 玩家对象结构不变，只把「填值的来源」从 AI 角色卡换成身份卡：
        // 下游所有读 gs.player.user_name 的代码一行都不用改。
        gs.player.user_name = identity.display_name().to_string();
        gs.player.user_subtitle = identity.subtitle.clone();
        gs.player.user_prompt = identity.prompt.clone();
        gs.player.identity_id = if identity.id.trim().is_empty() {
            None
        } else {
            Some(identity.id.clone())
        };
        gs.player.relations = identity.relations.clone();

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

        let system_line = LineBase {
            content: ai_prompt.clone(),
            attribute: LineAttributeExt(LineAttribute::System),
            sender_role_id: Some(cid),
            display_name: Some(settings.ai_name.clone()),
            ..Default::default()
        };
        gs.add_line(&self.db, system_line).await?;

        Ok(())
    }

    pub async fn init_game_status(
        &mut self,
        cid: Option<i32>,
        prompt_options: PromptOptions,
    ) -> Result<()> {
        self.clear_game_status().await;
        self.init_game_intro_character(cid, prompt_options).await?;
        Ok(())
    }

    pub async fn reset_game_status(&mut self) -> Result<()> {
        self.clear_game_status().await;
        let prompt_options = match self.prompt_options {
            None => PromptOptions {
                output_sec_lang: true,
                no_emotion_limit: true,
            },
            Some(p) => p,
        };
        self.init_game_intro_character(self.init_character_id, prompt_options)
            .await?;
        Ok(())
    }

    async fn clear_game_status(&mut self) {
        let mut gs = self.game_status.lock().await;
        gs.role_manager.invalidate_memory_history();
        gs.role_manager.reset_roles();
        gs.line_list.clear();
        gs.onstage_role_ids.clear();
        gs.present_role_ids.clear();
        gs.entry_greeting_done = false;
        // 玩家对象承载「我的身份」，必须一起清空：
        // 否则「没指定角色 → 提前返回」这类路径会把上一条会话的身份残留下来，
        // 造成脏读（例如新会话里名字还是上一次的身份）。
        gs.player = Default::default();
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
            gs.role_manager.invalidate_memory_history();
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

    /// 辅助函数，用于快速获取人物的设定
    pub async fn get_role_settings_by_id(&self, role_id: i32) -> Result<CharacterSettings> {
        Ok(self
            .game_status
            .lock()
            .await
            .role_manager
            .get_role(&self.db, role_id)
            .await?
            .settings
            .clone())
    }
}

/// 在 Tauri managed state 中共享的句柄。
pub type SharedAIService = Arc<Mutex<AIService>>;
