pub mod activity_monitor;
pub mod config;
pub mod delivery_evaluator;
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
use delivery_evaluator::DeliveryEvaluator;
use interest_manager::InterestManager;
use schedule_manager::ScheduleManager;
use strategy_dispatcher::StrategyDispatcher;
use types::{IntentType, PendingIntent, UserScheduleSettings};
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
    _visual_monitor: VisualMonitor,
    schedule_manager: ScheduleManager,
    strategy_dispatcher: StrategyDispatcher,

    loop_handle: Option<JoinHandle<()>>,
    is_running: bool,

    /// 前端上报的“当前是否适合投放主动对话”。
    /// 条件：用户在聊天界面(/chat 或 /pet) 且 设置面板未打开 且 输入框为空。
    can_deliver: bool,
    /// 暂存的主动对话意图（"小本本"）。每轮 cycle 开头尝试投放。
    pending_intents: Vec<PendingIntent>,
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
            _visual_monitor: visual_monitor,
            schedule_manager,
            strategy_dispatcher,
            loop_handle: None,
            is_running: false,
            can_deliver: false,
            pending_intents: Vec::new(),
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

    /// 前端通知后端当前是否具备投放条件。
    /// 前端仅在最终布尔值翻转时调用（不会反复上报）。
    pub fn set_can_deliver(&mut self, val: bool) {
        if self.can_deliver == val {
            return;
        }
        tracing::info!(
            "[ProactiveSystem] can_deliver changed: {} -> {}",
            self.can_deliver,
            val
        );
        self.can_deliver = val;
    }

    // ============================================================
    // 核心投放方法
    // ============================================================

    /// 统一的主动对话投放入口。持有锁不放，直到 LLM 流式生成完毕。
    /// 已侵入 game_status 的副作用（add_line），调用者需确保：
    /// - generation_lock 未被持有
    /// - prompt 已完整生成（含截图分析结果）
    async fn deliver(&mut self, prompt: String) -> anyhow::Result<()> {
        tracing::info!("[ProactiveSystem] Delivering proactive dialogue...");

        let _lock = self.generation_lock.lock().await;
        events::emit_thinking(&self.app, true);

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
                suppress_thinking: false,
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
                    content: prompt,
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

    /// 遍历暂存队列，尝试投放一个合适的意图。
    /// 返回 `true` 表示本轮已投放（调用方应 return）。
    async fn try_flush_pending_intent(&mut self) -> anyhow::Result<bool> {
        if self.pending_intents.is_empty() {
            return Ok(false);
        }

        if self.generation_lock.try_lock().is_err() {
            tracing::debug!("[ProactiveSystem] gen_lock busy, skip flush");
            return Ok(false);
        }

        let perception = self.activity_monitor.get_user_status();
        let now = std::time::Instant::now();

        // 按优先级降序：Alarm(4) > ImportantDay(3) > Todo(2) > Screen(1) > Topic(0)
        self.pending_intents
            .sort_by_key(|i| std::cmp::Reverse(i.intent_type));

        // 第一步：清理过期意图
        let before = self.pending_intents.len();
        self.pending_intents.retain(|intent| {
            let elapsed = now.duration_since(intent.triggered_at).as_secs();
            if elapsed > intent.intent_type.ttl_secs() {
                tracing::info!(
                    "[ProactiveSystem] Intent {:?} expired after {}s, discarding",
                    intent.intent_type,
                    elapsed
                );
                false
            } else {
                true
            }
        });
        if before != self.pending_intents.len() {
            tracing::info!(
                "[ProactiveSystem] Expired discard: {} -> {}",
                before,
                self.pending_intents.len()
            );
        }

        // 第二步：找第一个可投放的（索引有效，因为不在遍历中收集）
        for (i, intent) in self.pending_intents.iter().enumerate() {
            if DeliveryEvaluator::can_deliver(intent.intent_type, &perception, self.can_deliver) {
                let intent = self.pending_intents.remove(i);
                let waited = now.duration_since(intent.triggered_at);
                tracing::info!(
                    "[ProactiveSystem] Flushing deferred {:?} intent (waited {:.0}s, {} remaining in queue)",
                    intent.intent_type,
                    waited.as_secs(),
                    self.pending_intents.len()
                );
                self.deliver(intent.prompt).await?;
                return Ok(true);
            }
        }

        Ok(false)
    }

    // ============================================================
    // run_cycle（核心流程）
    // ============================================================

    /// 执行单次主动对话检查周期。
    async fn run_cycle(system_arc: Arc<Mutex<Self>>) -> anyhow::Result<()> {
        let mut sys = system_arc.lock().await;

        tracing::info!(
            "[ProactiveSystem] Cycle start. Interest: {:.2}/{:.2}, count today: {}/{}, can_deliver={}, pending={}",
            sys.interest_manager.interest,
            sys.interest_manager.max_interest_cap,
            sys.interest_manager.proactive_times,
            sys.interest_manager.max_proactive_count,
            sys.can_deliver,
            sys.pending_intents.len(),
        );

        // ─── 0. 优先清暂存队列 ───
        if sys.try_flush_pending_intent().await? {
            return Ok(());
        }

        // ─── 1. 快照 ───
        let settings_snap = {
            let snap = sys.settings.read().await;
            snap.clone()
        };

        // ─── 2. 感知（提前到这里，供 Alarm 的 evaluate 使用）───
        let perception = sys.activity_monitor.get_user_status();

        // ─── 3. 日程警报（跳过兴趣累积，但走 evaluate 闸门）───
        if sys.config.enable_schedule_reminder {
            let user_name = {
                let svc = sys.ai_service.lock().await;
                let gs = svc.game_status.lock().await;
                gs.player.user_name.clone()
            };
            if let Some(raw_prompt) = sys
                .schedule_manager
                .check_schedule_reminder(&user_name, &settings_snap)
            {
                let formatted = PromptRole::System.build_prompt(&raw_prompt);
                if DeliveryEvaluator::can_deliver(IntentType::Alarm, &perception, sys.can_deliver) {
                    tracing::info!(
                        "[ProactiveSystem] Alarm triggered, evaluate passed, delivering"
                    );
                    sys.deliver(formatted).await?;
                } else {
                    tracing::info!("[ProactiveSystem] Alarm triggered, evaluate failed, stashing");
                    sys.pending_intents.push(PendingIntent {
                        prompt: formatted,
                        intent_type: IntentType::Alarm,
                        triggered_at: std::time::Instant::now(),
                    });
                }
                return Ok(());
            }
        }

        // ─── 4. 兴趣累积 ───
        sys.interest_manager.update_interest();
        sys.interest_manager
            .set_status_mod(perception.interest_modifier);

        // ─── 5. 触发检查 ───
        if !sys.interest_manager.should_trigger_talk() {
            return Ok(());
        }

        tracing::info!(
            "[ProactiveSystem] Trigger fired! Interest={:.2}",
            sys.interest_manager.interest
        );

        // gen_lock 快检
        if sys.generation_lock.try_lock().is_err() {
            tracing::debug!("[ProactiveSystem] gen_lock busy, aborting this trigger");
            return Ok(());
        }

        // ─── 生成 prompt（SCREEN 调用视觉模型可能耗时，先禁用前端输入）───
        events::emit_thinking(&sys.app, true);
        let prompt_result = {
            let svc = sys.ai_service.lock().await;
            let gs = svc.game_status.lock().await;
            sys.strategy_dispatcher
                .get_proactive_prompt(&gs, &settings_snap, &perception, &sys.config)
                .await
        };
        let Some((raw_prompt, intent_type)) = prompt_result else {
            events::emit_thinking(&sys.app, false);
            return Ok(());
        };

        // 兴趣立刻消耗（"已经想说话了"）
        sys.interest_manager.reset_interest();

        // ─── 投放闸门 ───
        if DeliveryEvaluator::can_deliver(intent_type, &perception, sys.can_deliver) {
            tracing::info!(
                "[ProactiveSystem] Evaluate passed for {:?}, delivering immediately",
                intent_type
            );
            let formatted = PromptRole::System.build_prompt(&raw_prompt);
            sys.deliver(formatted).await?;
        } else {
            let formatted = PromptRole::System.build_prompt(&raw_prompt);
            tracing::info!(
                "[ProactiveSystem] Evaluate failed for {:?}, stashing (queue size: {} -> {})",
                intent_type,
                sys.pending_intents.len(),
                sys.pending_intents.len() + 1,
            );
            sys.pending_intents.push(PendingIntent {
                prompt: formatted,
                intent_type,
                triggered_at: std::time::Instant::now(),
            });
            events::emit_thinking(&sys.app, false);
        }

        Ok(())
    }
}
