pub mod activity_monitor;
pub mod config;
pub mod delivery_evaluator;
pub mod interest_manager;
pub mod proactive_history;
pub mod schedule_manager;
pub mod strategy_dispatcher;
pub mod types;
pub mod visual_monitor;

use sea_orm::DatabaseConnection;
use std::sync::Arc;
use std::time::{Duration, Instant};
use tauri::AppHandle;
use tokio::sync::{Mutex, OwnedMutexGuard, RwLock};
use tokio::task::JoinHandle;

use crate::ai_service::llm::LlmClient;
use crate::ai_service::message_system::events;
use crate::ai_service::message_system::generator::{GeneratorDeps, MessageGenerator};
use crate::ai_service::message_system::processor::MessageProcessor;
use crate::ai_service::service::SharedAIService;
use crate::ai_service::translator::Translator;
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
use types::{IntentType, PendingIntent, PerceptionResult, ProactiveContext, UserScheduleSettings};
use visual_monitor::VisualMonitor;

const MAX_PENDING_INTENTS: usize = 5;
const MAX_SAFE_DELIVERY_ATTEMPTS: u8 = 3;
const DELIVERY_FAILURE_BACKOFF: Duration = Duration::from_secs(60);

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
    strategy_dispatcher: Arc<StrategyDispatcher>,

    loop_handle: Option<JoinHandle<()>>,
    is_running: bool,

    /// 前端上报的“当前是否适合投放主动对话”。
    /// 条件：用户在聊天界面(/chat 或 /pet) 且 设置面板未打开 且 输入框为空。
    can_deliver: bool,
    /// 每次用户主动发言递增；让在途视觉/闲聊结果可被识别为过期。
    interaction_epoch: u64,
    /// 模型/消费者失败后的全局退避，避免每轮重新污染上下文。
    delivery_backoff_until: Option<Instant>,
    /// 暂存的主动对话意图（"小本本"）。每轮 cycle 开头尝试投放。
    pending_intents: Vec<PendingIntent>,
}

/// 可以脱离 `ProactiveSystem` 大锁执行投递的只读依赖快照。
#[derive(Clone)]
struct DeliveryContext {
    app: AppHandle,
    db: DatabaseConnection,
    ai_service: SharedAIService,
    generation_lock: Arc<Mutex<()>>,
    processor: Arc<MessageProcessor>,
    translator: Arc<Translator>,
    llm: Option<Arc<LlmClient>>,
}

/// 确保主动生成 future 被取消或任务被 abort 时也会恢复前端思考状态。
struct ProactiveThinkingGuard {
    app: AppHandle,
}

impl ProactiveThinkingGuard {
    fn new(app: &AppHandle) -> Self {
        events::emit_thinking(app, true, true);
        events::emit_proactive_thinking(app, true);
        Self { app: app.clone() }
    }
}

impl Drop for ProactiveThinkingGuard {
    fn drop(&mut self) {
        events::emit_proactive_thinking(&self.app, false);
        events::emit_thinking(&self.app, false, true);
    }
}

impl DeliveryContext {
    fn try_generation_guard(&self) -> Option<OwnedMutexGuard<()>> {
        self.generation_lock.clone().try_lock_owned().ok()
    }

    async fn rollback_prompt_line(
        &self,
        game_status: &Arc<Mutex<crate::ai_service::game_system::game_status::GameStatus>>,
        prompt_index: usize,
        expected_prompt: &str,
    ) {
        let mut gs = game_status.lock().await;
        let is_our_prompt = gs.line_list.get(prompt_index).is_some_and(|line| {
            matches!(line.attribute(), LineAttribute::User)
                && line.base.sender_role_id.is_none()
                && line.base.content == expected_prompt
        });
        if !is_our_prompt {
            return;
        }

        gs.line_list.remove(prompt_index);
        if let Err(error) = gs.refresh_memories(&self.db).await {
            tracing::error!(
                "[ProactiveSystem] Failed to refresh memories after prompt rollback: {error:#}"
            );
        }
    }

    /// 调用方必须先取得 generation guard，并在取得后重新验证投放条件。
    async fn deliver(
        &self,
        prompt: String,
        _generation_guard: OwnedMutexGuard<()>,
    ) -> anyhow::Result<String> {
        let llm = self
            .llm
            .clone()
            .ok_or_else(|| anyhow::anyhow!("LLM is not configured"))?;

        let game_status = {
            let svc = self.ai_service.lock().await;
            svc.game_status.clone()
        };
        let generator = MessageGenerator::new(GeneratorDeps {
            app: self.app.clone(),
            db: self.db.clone(),
            game_status: game_status.clone(),
            processor: self.processor.clone(),
            translator: self.translator.clone(),
            llm,
            concurrency: 1,
            god_agent: None,
            // 由本方法统一维护 thinking 状态，确保所有错误路径都能复位。
            suppress_thinking: true,
            is_proactive: true,
        });

        let _thinking_guard = ProactiveThinkingGuard::new(&self.app);

        let expected_prompt = prompt.clone();
        let (prompt_index, add_result) = {
            let mut gs = game_status.lock().await;
            let prompt_index = gs.line_list.len();
            let result = gs
                .add_line(
                    &self.db,
                    LineBase {
                        attribute: LineAttributeExt(LineAttribute::User),
                        content: prompt,
                        sender_role_id: None,
                        display_name: None,
                        ..Default::default()
                    },
                )
                .await;
            (prompt_index, result)
        };
        if let Err(error) = add_result {
            self.rollback_prompt_line(&game_status, prompt_index, &expected_prompt)
                .await;
            return Err(error);
        }

        let generation_result = generator.process_message(None).await;
        let assistant_count = {
            let gs = game_status.lock().await;
            gs.line_list
                .iter()
                .skip(prompt_index + 1)
                .filter(|line| matches!(line.attribute(), LineAttribute::Assistant))
                .count()
        };

        match generation_result {
            Ok(output) if assistant_count > 0 => Ok(output),
            Ok(_) => {
                self.rollback_prompt_line(&game_status, prompt_index, &expected_prompt)
                    .await;
                anyhow::bail!("proactive generation produced no persisted assistant response")
            }
            Err(error) if assistant_count > 0 => {
                tracing::warn!(
                    "[ProactiveSystem] Generator ended with an error after persisting {assistant_count} response line(s): {error:#}"
                );
                Ok(String::new())
            }
            Err(error) => {
                self.rollback_prompt_line(&game_status, prompt_index, &expected_prompt)
                    .await;
                Err(error)
            }
        }
    }
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
        let interest_manager = InterestManager::new(
            config.max_proactive_times,
            config.interest_trigger_threshold,
            config.interest_decay_step,
        );
        let activity_monitor = UserActivityMonitor::new();
        let visual_monitor = VisualMonitor::new();
        let schedule_manager = ScheduleManager::new();
        let strategy_dispatcher = Arc::new(StrategyDispatcher::new(&app));

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
            can_deliver: false,
            interaction_epoch: 0,
            delivery_backoff_until: None,
            pending_intents: Vec::new(),
        };

        system
    }

    fn delivery_context(&self) -> DeliveryContext {
        DeliveryContext {
            app: self.app.clone(),
            db: self.db.clone(),
            ai_service: self.ai_service.clone(),
            generation_lock: self.generation_lock.clone(),
            processor: self.chat.processor.clone(),
            translator: self.chat.translator.clone(),
            llm: self.chat.llm.clone(),
        }
    }

    /// 只复制策略需要的少量状态，绝不把 GameStatus guard 带入 VLM 网络请求。
    async fn read_runtime_context(ai_service: &SharedAIService) -> (ProactiveContext, bool) {
        let game_status = {
            let svc = ai_service.lock().await;
            svc.game_status.clone()
        };
        let gs = game_status.lock().await;
        let ai_name = gs
            .current_role_id
            .and_then(|rid| gs.role_manager.get_loaded(rid))
            .and_then(|role| role.display_name.clone())
            .unwrap_or_else(|| "你".to_string());
        (
            ProactiveContext {
                user_name: gs.player.user_name.clone(),
                ai_name,
            },
            gs.script_status.is_some(),
        )
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

            loop {
                // 每次循环从配置读取最新轮询间隔，保存后无需重启即可生效
                let interval_secs = {
                    let sys = sys_clone.lock().await;
                    if !sys.is_running {
                        break;
                    }
                    sys.config.proactive_interval_secs.max(1)
                };

                tokio::time::sleep(Duration::from_secs(interval_secs)).await;

                let enabled = {
                    let sys = sys_clone.lock().await;
                    if !sys.is_running {
                        break;
                    }
                    sys.config.enable_proactive_system
                };

                if !enabled {
                    // tracing::info!("[ProactiveSystem] Disabled via settings, skipping...");
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
    pub async fn reload(system_arc: Arc<Mutex<Self>>) {
        // ScreenAnalyzer 可能被一次较慢的 VLM 请求占用。复制依赖后先释放系统锁，
        // 避免配置重载期间阻塞 can_deliver 上报和用户消息回调。
        let (app, strategy) = {
            let sys = system_arc.lock().await;
            (sys.app.clone(), sys.strategy_dispatcher.clone())
        };
        let config = ProactiveConfig::load(&app);
        {
            let mut sys = system_arc.lock().await;
            sys.config = config.clone();
            sys.delivery_backoff_until = None;
            if !config.enable_proactive_system {
                sys.pending_intents.clear();
            } else {
                sys.pending_intents.retain(|intent| {
                    Self::intent_enabled(&config, intent.intent_type)
                        && (intent.intent_type == IntentType::Alarm
                            || config.max_proactive_times > 0)
                });
            }
            sys.interest_manager.update_from_config(
                config.max_proactive_times,
                config.interest_trigger_threshold,
                config.interest_decay_step,
            );
        }

        // 这一步可能等待正在进行的 VLM，但已不再持有 ProactiveSystem 锁。
        strategy.update_config(&app).await;
        system_arc.lock().await.load_schedule_settings().await;
    }

    /// 手动触发一次基于屏幕的主动搭话测试，直接截屏 → VLM → 投递到前端。
    /// 跳过兴趣累积、投放闸门和反重复检测，用于在设置页快速验证视觉主动回复链路。
    pub async fn test_screen_proactive(
        system_arc: Arc<Mutex<Self>>,
    ) -> Result<Option<String>, String> {
        let (ai_service, strategy, delivery) = {
            let sys = system_arc.lock().await;
            (
                sys.ai_service.clone(),
                sys.strategy_dispatcher.clone(),
                sys.delivery_context(),
            )
        };

        tracing::info!("[ProactiveSystem] Test proactive message triggered.");

        let prompt_result = async {
            let (context, script_active) = Self::read_runtime_context(&ai_service).await;
            if script_active {
                return Err("剧情正在运行，暂不执行主动消息测试".to_string());
            }
            let (raw_prompt, _intent_type) = strategy
                .get_screen_prompt_for_test(&context)
                .await
                .ok_or_else(|| {
                    "屏幕分析未返回有效内容（API Key 为空、网络超时或模型不支持视觉）".to_string()
                })?;

            tracing::info!(
                "[ProactiveSystem] Test screen prompt generated: {}",
                raw_prompt
            );

            let formatted = crate::utils::prompt::PromptRole::System.build_prompt(&raw_prompt);
            let (_, script_active) = Self::read_runtime_context(&ai_service).await;
            if script_active {
                return Err("视觉分析期间剧情已开始，本次测试已取消".to_string());
            }
            let guard = delivery
                .try_generation_guard()
                .ok_or_else(|| "当前有其他回复正在生成，请稍后再试".to_string())?;
            delivery
                .deliver(formatted, guard)
                .await
                .map_err(|e| format!("投递主动消息失败: {}", e))?;

            Ok::<String, String>(raw_prompt)
        }
        .await;

        if let Err(ref e) = prompt_result {
            tracing::error!("[ProactiveSystem] Test proactive message failed: {}", e);
        }

        prompt_result.map(Some)
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
        self.interaction_epoch = self.interaction_epoch.wrapping_add(1);
        self.interest_manager.restore_max_interest_cap();
        // 用户已经主动开启了一轮新对话，旧的闲聊/屏幕观察已失去时效。
        self.pending_intents
            .retain(|intent| !matches!(intent.intent_type, IntentType::Screen | IntentType::Topic));
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

    fn stash_intent(&mut self, intent: PendingIntent) {
        if matches!(intent.intent_type, IntentType::Screen | IntentType::Topic)
            && intent.interaction_epoch != self.interaction_epoch
        {
            tracing::debug!(
                "[ProactiveSystem] Dropping stale {:?} intent from epoch {} (current {})",
                intent.intent_type,
                intent.interaction_epoch,
                self.interaction_epoch
            );
            return;
        }

        if intent.intent_type == IntentType::Alarm {
            // 同一分钟可能有多个日程；闹钟逐条保留，只过滤完全相同的重复项。
            if self.pending_intents.iter().any(|queued| {
                queued.intent_type == IntentType::Alarm && queued.prompt == intent.prompt
            }) {
                return;
            }
        } else if let Some(existing) = self
            .pending_intents
            .iter_mut()
            .find(|queued| queued.intent_type == intent.intent_type)
        {
            if intent.triggered_at >= existing.triggered_at {
                tracing::debug!(
                    "[ProactiveSystem] Replacing queued {:?} intent with newer content",
                    intent.intent_type
                );
                *existing = intent;
            }
            return;
        }

        if self.pending_intents.len() >= MAX_PENDING_INTENTS {
            self.pending_intents
                .sort_by_key(|queued| (queued.intent_type, queued.triggered_at));
            self.pending_intents.remove(0);
        }
        self.pending_intents.push(intent);
    }

    fn take_deliverable_pending(&mut self, perception: &PerceptionResult) -> Option<PendingIntent> {
        let now = Instant::now();
        let interaction_epoch = self.interaction_epoch;
        let delivery_backoff_active = self.delivery_backoff_until.is_some_and(|until| now < until);
        self.pending_intents.retain(|intent| {
            now.duration_since(intent.triggered_at).as_secs() <= intent.intent_type.ttl_secs()
                && (!matches!(intent.intent_type, IntentType::Screen | IntentType::Topic)
                    || intent.interaction_epoch == interaction_epoch)
        });
        self.pending_intents
            .sort_by_key(|intent| std::cmp::Reverse(intent.intent_type));
        let index = self.pending_intents.iter().position(|intent| {
            (!delivery_backoff_active
                || (intent.intent_type == IntentType::Alarm && intent.delivery_attempts == 0))
                && DeliveryEvaluator::can_deliver(intent.intent_type, perception, self.can_deliver)
        })?;
        Some(self.pending_intents.remove(index))
    }

    fn intent_enabled(config: &ProactiveConfig, intent_type: IntentType) -> bool {
        match intent_type {
            IntentType::Alarm => config.enable_schedule_reminder,
            IntentType::ImportantDay => config.enable_important_day_reminder,
            IntentType::Todo => config.enable_todo_perception,
            IntentType::Screen => config.enable_visual_perception,
            IntentType::Topic => config.enable_topic_creator,
        }
    }

    async fn attempt_delivery(
        system_arc: Arc<Mutex<Self>>,
        delivery: DeliveryContext,
        strategy: Arc<StrategyDispatcher>,
        mut intent: PendingIntent,
    ) -> anyhow::Result<bool> {
        let Some(guard) = delivery.try_generation_guard() else {
            tracing::debug!("[ProactiveSystem] generation lock busy; deferring intent");
            system_arc.lock().await.stash_intent(intent);
            return Ok(false);
        };

        let (_, script_active) = Self::read_runtime_context(&delivery.ai_service).await;
        let should_defer = {
            let sys = system_arc.lock().await;
            if !sys.is_running || !sys.config.enable_proactive_system {
                return Ok(false);
            }
            if !Self::intent_enabled(&sys.config, intent.intent_type) {
                tracing::debug!(
                    "[ProactiveSystem] Dropping disabled {:?} intent",
                    intent.intent_type
                );
                return Ok(false);
            }
            if matches!(intent.intent_type, IntentType::Screen | IntentType::Topic)
                && intent.interaction_epoch != sys.interaction_epoch
            {
                tracing::debug!(
                    "[ProactiveSystem] Dropping stale {:?} result from epoch {} (current {})",
                    intent.intent_type,
                    intent.interaction_epoch,
                    sys.interaction_epoch
                );
                return Ok(false);
            }
            if intent.intent_type != IntentType::Alarm
                && sys.interest_manager.proactive_times >= sys.interest_manager.max_proactive_count
            {
                return Ok(false);
            }
            let current = sys.activity_monitor.get_user_status();
            script_active
                || !DeliveryEvaluator::can_deliver(intent.intent_type, &current, sys.can_deliver)
        };
        if should_defer {
            system_arc.lock().await.stash_intent(intent);
            return Ok(false);
        }

        tracing::info!(
            "[ProactiveSystem] Delivering {:?} proactive intent",
            intent.intent_type
        );
        match delivery.deliver(intent.prompt.clone(), guard).await {
            Ok(_) => {
                if let Some(raw_prompt) = intent.raw_prompt.as_deref() {
                    strategy
                        .record_delivered(raw_prompt, intent.intent_type)
                        .await;
                }
                let mut sys = system_arc.lock().await;
                sys.delivery_backoff_until = None;
                // 闹钟不占用“用户回复前最多主动搭话次数”，即时与延迟投递保持一致。
                if intent.intent_type != IntentType::Alarm {
                    sys.interest_manager.reset_interest();
                }
                Ok(true)
            }
            Err(error) => {
                // deliver 已确认没有持久化 assistant，并回滚了本次 prompt，才会进入这里；
                // 因此可以在全局退避后做有限次幂等重试，避免日程提醒永久丢失。
                intent.delivery_attempts = intent.delivery_attempts.saturating_add(1);
                let should_retry = intent.delivery_attempts < MAX_SAFE_DELIVERY_ATTEMPTS
                    && intent.triggered_at.elapsed().as_secs() <= intent.intent_type.ttl_secs();
                let mut sys = system_arc.lock().await;
                sys.delivery_backoff_until = Some(Instant::now() + DELIVERY_FAILURE_BACKOFF);
                let cooled = (sys.interest_manager.trigger_threshold * 0.5)
                    .min(sys.interest_manager.max_interest_cap);
                sys.interest_manager.interest = cooled;
                if should_retry {
                    sys.stash_intent(intent);
                }
                Err(error.context("proactive intent delivery failed"))
            }
        }
    }

    // ============================================================
    // run_cycle（核心流程）
    // ============================================================

    /// 执行单次主动对话检查周期。
    async fn run_cycle(system_arc: Arc<Mutex<Self>>) -> anyhow::Result<()> {
        let (config, settings, strategy, delivery, interaction_epoch) = {
            let mut sys = system_arc.lock().await;
            if !sys.is_running || !sys.config.enable_proactive_system {
                return Ok(());
            }
            sys.interest_manager.check_daily_reset();
            tracing::info!(
                "[ProactiveSystem] Cycle start. Interest: {:.2}/{:.2}, count: {}/{}, can_deliver={}, pending={}",
                sys.interest_manager.interest,
                sys.interest_manager.max_interest_cap,
                sys.interest_manager.proactive_times,
                sys.interest_manager.max_proactive_count,
                sys.can_deliver,
                sys.pending_intents.len(),
            );
            (
                sys.config.clone(),
                sys.settings.clone(),
                sys.strategy_dispatcher.clone(),
                sys.delivery_context(),
                sys.interaction_epoch,
            )
        };

        let settings_snap = settings.read().await.clone();
        let (context, script_active) = Self::read_runtime_context(&delivery.ai_service).await;
        if script_active {
            return Ok(());
        }

        let (perception, pending) = {
            let mut sys = system_arc.lock().await;
            let perception = sys.activity_monitor.get_user_status();
            let pending = sys.take_deliverable_pending(&perception);
            (perception, pending)
        };
        if let Some(intent) = pending {
            let _ = Self::attempt_delivery(
                system_arc.clone(),
                delivery.clone(),
                strategy.clone(),
                intent,
            )
            .await?;
            return Ok(());
        }

        // 日程 prompt 生成很便宜，可以直接进入统一投递/暂存流程。
        if config.enable_schedule_reminder {
            let alarm = {
                let mut sys = system_arc.lock().await;
                sys.schedule_manager
                    .check_schedule_reminder(&context.user_name, &settings_snap)
            };
            if let Some(raw_prompt) = alarm {
                let intent = PendingIntent::new(
                    PromptRole::System.build_prompt(&raw_prompt),
                    None,
                    IntentType::Alarm,
                    interaction_epoch,
                );
                let _ = Self::attempt_delivery(
                    system_arc.clone(),
                    delivery.clone(),
                    strategy.clone(),
                    intent,
                )
                .await?;
                return Ok(());
            }
        }

        if system_arc
            .lock()
            .await
            .delivery_backoff_until
            .is_some_and(|until| Instant::now() < until)
        {
            tracing::debug!("[ProactiveSystem] Delivery failure backoff active; skipping strategy");
            return Ok(());
        }

        let (triggered, can_deliver, screen_eligible) = {
            let mut sys = system_arc.lock().await;
            let visual_pending =
                config.enable_visual_perception && sys.visual_monitor.check_visual_change();
            let modifier = perception
                .interest_modifier
                .saturating_add(if visual_pending { 20 } else { 0 });
            sys.interest_manager.update_interest(modifier);
            let triggered = sys.interest_manager.should_trigger_talk();
            let can_deliver = sys.can_deliver;
            let screen_eligible = visual_pending
                && DeliveryEvaluator::can_deliver(IntentType::Screen, &perception, can_deliver);
            (triggered, can_deliver, screen_eligible)
        };

        if !triggered || !can_deliver {
            return Ok(());
        }
        // 用户回复正在生成时，不要先白跑一次 VLM。
        if delivery.generation_lock.try_lock().is_err() {
            return Ok(());
        }

        // 这里不持有 ProactiveSystem / AIService / GameStatus 锁；VLM 再慢也不会阻塞 UI 状态上报。
        let outcome = strategy
            .get_proactive_prompt(
                &context,
                &settings_snap,
                &perception,
                &config,
                screen_eligible,
            )
            .await;

        if outcome.screen_attempted {
            system_arc.lock().await.visual_monitor.mark_analyzed();
        }

        let Some(candidate) = outcome.candidate else {
            // PASS、重复或模型失败后退回阈值以下，避免十秒后立刻再次触发昂贵策略。
            let mut sys = system_arc.lock().await;
            let cooled = (sys.interest_manager.trigger_threshold * 0.5)
                .min(sys.interest_manager.max_interest_cap);
            sys.interest_manager.interest = cooled;
            return Ok(());
        };

        let intent = PendingIntent::new(
            PromptRole::System.build_prompt(&candidate.prompt),
            Some(candidate.prompt),
            candidate.intent_type,
            interaction_epoch,
        );
        let _ = Self::attempt_delivery(system_arc, delivery, strategy, intent).await?;
        Ok(())
    }
}
