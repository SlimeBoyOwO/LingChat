pub mod activity_monitor;
pub mod config;
pub mod interest_manager;
pub mod schedule_manager;
pub mod strategy_dispatcher;
pub mod types;
pub mod visual_monitor;

use sea_orm::DatabaseConnection;
use std::sync::Arc;
use std::time::Duration;
use tauri::AppHandle;
use tokio::sync::{Mutex, RwLock};
use tokio::task::JoinHandle;

use crate::ai_service::message_system::events;
use crate::ai_service::message_system::generator::{GeneratorDeps, MessageGenerator};
use crate::ai_service::service::SharedAIService;
use crate::ai_service::types::{LineAttributeExt, LineBase};
use crate::db::entities::line::LineAttribute;
use crate::utils::prompt::PromptRole;
use crate::ChatComponents;

use activity_monitor::UserActivityMonitor;
use config::ProactiveConfig;
use interest_manager::InterestManager;
use schedule_manager::ScheduleManager;
use strategy_dispatcher::StrategyDispatcher;
use types::UserScheduleSettings;
use visual_monitor::VisualMonitor;

pub struct ProactiveSystem {
    app: AppHandle,
    db: DatabaseConnection,
    ai_service: SharedAIService,
    chat: ChatComponents,
    generation_lock: Arc<Mutex<()>>,

    config: ProactiveConfig,
    settings: Arc<RwLock<UserScheduleSettings>>,
    interest_manager: InterestManager,
    activity_monitor: UserActivityMonitor,
    visual_monitor: VisualMonitor,
    schedule_manager: ScheduleManager,
    strategy_dispatcher: StrategyDispatcher,

    loop_handle: Option<JoinHandle<()>>,
    is_running: bool,
}

impl ProactiveSystem {
    pub fn new(
        app: AppHandle,
        db: DatabaseConnection,
        ai_service: SharedAIService,
        chat: ChatComponents,
        generation_lock: Arc<Mutex<()>>,
    ) -> Self {
        let config = ProactiveConfig::load(&app);
        let interest_manager = InterestManager::new(config.max_proactive_times);
        let activity_monitor = UserActivityMonitor::new();
        let visual_monitor = VisualMonitor::new();
        let schedule_manager = ScheduleManager::new();
        let strategy_dispatcher = StrategyDispatcher::new(&app);

        let system = Self {
            app,
            db,
            ai_service,
            chat,
            generation_lock,
            config,
            settings: Arc::new(RwLock::new(UserScheduleSettings::default())),
            interest_manager,
            activity_monitor,
            visual_monitor,
            schedule_manager,
            strategy_dispatcher,
            loop_handle: None,
            is_running: false,
        };

        system
    }

    /// 启动主动对话的后台轮询 Loop。
    pub async fn start(system_arc: Arc<Mutex<Self>>) {
        let mut sys = system_arc.lock().await;
        if sys.is_running {
            return;
        }
        sys.is_running = true;

        // 首次加载日程设置
        sys.load_schedule_settings().await;

        let sys_clone = system_arc.clone();
        let handle = tokio::spawn(async move {
            tracing::info!("[ProactiveSystem] Loop task started.");

            // Loop runs every 30 seconds
            let mut interval = tokio::time::interval(Duration::from_secs(30));
            loop {
                interval.tick().await;

                // Grab locks safely to avoid blocking startup or chat interaction
                let (enabled, is_script_active) = {
                    let sys = sys_clone.lock().await;
                    if !sys.is_running {
                        break;
                    }

                    let svc = sys.ai_service.lock().await;
                    let is_script_active = svc.game_status.lock().await.script_status.is_some();
                    (sys.config.enable_proactive_system, is_script_active)
                };

                if !enabled {
                    // tracing::info!("[ProactiveSystem] Disabled via settings, skipping...");
                    continue;
                }

                if is_script_active {
                    // tracing::info!("[ProactiveSystem] Script is currently running, bypassing proactive talk to avoid collision.");
                    continue;
                }

                // Run main proactive check cycle
                if let Err(e) = Self::run_cycle(sys_clone.clone()).await {
                    tracing::error!("[ProactiveSystem] Error running cycle: {:?}", e);
                }
            }
            tracing::info!("[ProactiveSystem] Loop task stopped.");
        });

        sys.loop_handle = Some(handle);
    }

    /// 停止主动对话系统。
    pub async fn stop(&mut self) {
        tracing::info!("[ProactiveSystem] Stopping...");
        self.is_running = false;
        if let Some(handle) = self.loop_handle.take() {
            handle.abort();
        }
    }

    /// 重新载入环境配置和日程设置。
    pub async fn reload(&mut self) {
        self.config = ProactiveConfig::load(&self.app);
        self.strategy_dispatcher.update_config(&self.app);
        self.interest_manager
            .update_from_config(self.config.max_proactive_times);
        self.load_schedule_settings().await;
    }

    /// 重新载入日程设置文件 schedules.json。
    pub async fn load_schedule_settings(&mut self) {
        let schedules_path = crate::api::data_dir()
            .join("game_data")
            .join("schedules.json");

        if schedules_path.exists() {
            match std::fs::read_to_string(&schedules_path) {
                Ok(content) => match serde_json::from_str::<UserScheduleSettings>(&content) {
                    Ok(parsed) => {
                        let mut settings_lock = self.settings.write().await;
                        *settings_lock = parsed;
                    }
                    Err(e) => {
                        tracing::error!(
                            "[ProactiveSystem] Failed to parse schedules.json: {:?}",
                            e
                        );
                    }
                },
                Err(e) => {
                    tracing::error!("[ProactiveSystem] Failed to read schedules.json: {:?}", e);
                }
            }
        } else {
            tracing::warn!(
                "[ProactiveSystem] schedules.json not found at {:?}",
                schedules_path
            );
        }
    }

    /// 当用户主动发送消息时触发的回调，用于恢复好感度/兴趣阈值。
    pub async fn on_user_message_received(&mut self) {
        tracing::info!("[ProactiveSystem] User message received! Restoring engagement cap.");
        self.interest_manager.restore_max_interest_cap();
    }

    /// 执行单次主动对话检查周期。
    async fn run_cycle(system_arc: Arc<Mutex<Self>>) -> anyhow::Result<()> {
        let mut sys = system_arc.lock().await;

        tracing::info!(
            "[ProactiveSystem] Cycle start. Interest: {:.2}/{:.2}, count today: {}/{}",
            sys.interest_manager.interest,
            sys.interest_manager.max_interest_cap,
            sys.interest_manager.proactive_times,
            sys.interest_manager.max_proactive_count
        );

        // 1. 获取日程设置快照以供后续分析使用
        let settings_snap = {
            let snap = sys.settings.read().await;
            snap.clone()
        };

        // 2. 检查日程强提醒 (Alarm)
        if sys.config.enable_schedule_reminder {
            let reminder_prompt = {
                let user_name = {
                    let svc = sys.ai_service.lock().await;
                    let gs = svc.game_status.lock().await;
                    gs.player.user_name.clone()
                };
                sys.schedule_manager
                    .check_schedule_reminder(&user_name, &settings_snap)
            };

            if let Some(prompt) = reminder_prompt {
                // 日程时间到了直接强制触发对话
                sys.trigger_proactive_dialogue(prompt).await?;
                return Ok(());
            }
        }

        // 3. 执行键盘鼠标与视觉变更感知
        let perception = sys.activity_monitor.get_user_status();

        // 视觉变化监测 TODO: 视觉模块将会在后面优化为更智能的模块，目前先不启用
        // if sys.config.enable_visual_perception {
        //     let change = sys.visual_monitor.check_visual_change();
        //     perception.visual_change_detected = change;
        // }

        // 4. 更新好感度/兴趣度
        sys.interest_manager.update_interest();
        sys.interest_manager
            .set_status_mod(perception.interest_modifier);

        // 5. 检查是否满足主动出击触发阈值
        if sys.interest_manager.should_trigger_talk() {
            tracing::info!(
                "[ProactiveSystem] Trigger fired! Interest: {:.2}, preparing proactive dialogue...",
                sys.interest_manager.interest
            );

            // 确保没有正在进行的 LLM 对话
            let lock_clone = sys.generation_lock.clone();
            if lock_clone.try_lock().is_err() {
                tracing::debug!(
                    "[ProactiveSystem] Chat generation is locked. Postponing proactive talk..."
                );
                return Ok(());
            }

            // 获取锁，准备触发主动对话
            let _lock = lock_clone.lock().await;

            // 发送思考状态，防止前端输入
            events::emit_thinking(&sys.app, true);

            let prompt_opt = {
                let svc = sys.ai_service.lock().await;
                let gs = svc.game_status.lock().await;
                sys.strategy_dispatcher
                    .get_proactive_prompt(&gs, &settings_snap, &perception, &sys.config)
                    .await
            };

            if let Some(prompt) = prompt_opt {
                sys.interest_manager.reset_interest();

                // 必须在持有 generation_lock 期间进行消息处理，防止与用户回复混淆
                let generator = {
                    let game_status = {
                        let svc = sys.ai_service.lock().await;
                        svc.game_status.clone()
                    };
                    let deps = GeneratorDeps {
                        app: sys.app.clone(),
                        db: sys.db.clone(),
                        game_status,
                        processor: sys.chat.processor.clone(),
                        translator: sys.chat.translator.clone(),
                        llm: sys
                            .chat
                            .llm
                            .clone()
                            .ok_or_else(|| anyhow::anyhow!("LLM is not configured"))?,
                        concurrency: 1,
                        god_agent: None,
                    };
                    MessageGenerator::new(deps)
                };

                // 往 game_status 追加系统级的主动 prompt 作为隐形触发台词
                {
                    let svc = sys.ai_service.lock().await;
                    let mut gs = svc.game_status.lock().await;
                    gs.add_line(
                        &sys.db,
                        LineBase {
                            attribute: LineAttributeExt(LineAttribute::User),
                            content: PromptRole::System.build_prompt(&prompt),
                            sender_role_id: None,
                            display_name: None,
                            ..Default::default()
                        },
                    )
                    .await?;
                }

                // 派发流式生成任务并等待其处理完毕 (包括 TTS 音频分段输出)
                let _ = generator.process_message(None).await;
            } else {
                events::emit_thinking(&sys.app, false);
            }
        }

        Ok(())
    }

    /// 强行触发日程警报对话 (忽略兴趣好感阈值)
    async fn trigger_proactive_dialogue(&mut self, prompt: String) -> anyhow::Result<()> {
        let _lock = self.generation_lock.lock().await;

        let generator = {
            let game_status = {
                let svc = self.ai_service.lock().await;
                svc.game_status.clone()
            };
            let deps = GeneratorDeps {
                app: self.app.clone(),
                db: self.db.clone(),
                game_status,
                processor: self.chat.processor.clone(),
                translator: self.chat.translator.clone(),
                llm: self
                    .chat
                    .llm
                    .clone()
                    .ok_or_else(|| anyhow::anyhow!("LLM is not configured"))?,
                concurrency: 1,
                god_agent: None,
            };
            MessageGenerator::new(deps)
        };

        {
            let svc = self.ai_service.lock().await;
            let mut gs = svc.game_status.lock().await;
            gs.add_line(
                &self.db,
                LineBase {
                    attribute: LineAttributeExt(LineAttribute::User),
                    content: PromptRole::System.build_prompt(&prompt),
                    sender_role_id: None,
                    display_name: None,
                    ..Default::default()
                },
            )
            .await?;
        }

        let _ = generator.process_message(None).await;
        Ok(())
    }
}
