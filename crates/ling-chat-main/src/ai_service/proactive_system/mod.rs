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
use tokio::sync::{Mutex, OwnedMutexGuard, RwLock};
use tokio::task::JoinHandle;

use crate::ChatComponents;
use crate::ai_service::message_system::events;
use crate::ai_service::message_system::generator::{
    GeneratorDeps, GeneratorSource, MessageGenerator,
};
use crate::ai_service::service::SharedAIService;
use crate::ai_service::tools::registry::ToolRegistry;
use crate::ai_service::types::{LineAttributeExt, LineBase};
use crate::db::entities::line::LineAttribute;
use crate::utils::prompt::PromptRole;

use activity_monitor::UserActivityMonitor;
use config::ProactiveConfig;
use delivery_evaluator::DeliveryEvaluator;
use interest_manager::InterestManager;
use schedule_manager::ScheduleManager;
use strategy_dispatcher::StrategyDispatcher;
use types::{IntentType, PendingIntent, ProactivePrompt, UserScheduleSettings};
use visual_monitor::VisualMonitor;

pub struct ProactiveSystem {
    app: AppHandle,
    db: DatabaseConnection,
    ai_service: SharedAIService,
    chat: ChatComponents,
    tool_registry: Arc<ToolRegistry>,
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
        tool_registry: Arc<ToolRegistry>,
        generation_lock: Arc<Mutex<()>>,
    ) -> Self {
        let config = ProactiveConfig::load(&app);
        let interest_manager =
            InterestManager::new(config.max_proactive_times, config.expected_interval_minutes);
        let activity_monitor = UserActivityMonitor::new();
        let visual_monitor = VisualMonitor::new();
        let schedule_manager = ScheduleManager::new();
        let strategy_dispatcher = StrategyDispatcher::new(&app);

        Self {
            app,
            db,
            ai_service,
            chat,
            tool_registry,
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
        }
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

            // 周期与兴趣增长的期望换算共用同一个常量，改这里两边一起变
            let mut interval = tokio::time::interval(Duration::from_secs_f64(
                interest_manager::LOOP_INTERVAL_SECS,
            ));
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

                if is_script_active {
                    // tracing::info!("[ProactiveSystem] Script is currently running, bypassing proactive talk to avoid collision.");
                    continue;
                }

                // 到点提醒（日程闹钟 / 待办截止）有意跑在总开关判断之前：
                // 用户关掉主动对话后，依然应当收到自己设过的提醒。
                match Self::run_reminder_check(sys_clone.clone()).await {
                    Ok(true) => continue,
                    Ok(false) => {},
                    Err(e) => {
                        tracing::error!("[ProactiveSystem] Error running reminder check: {:?}", e)
                    },
                }

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
    pub async fn reload(&mut self) {
        self.config = ProactiveConfig::load(&self.app);
        self.strategy_dispatcher.update_config(&self.app);
        self.interest_manager.update_from_config(
            self.config.max_proactive_times,
            self.config.expected_interval_minutes,
        );
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
                    },
                    Err(e) => {
                        tracing::error!(
                            "[ProactiveSystem] Failed to parse schedules.json: {:?}",
                            e
                        );
                    },
                },
                Err(e) => {
                    tracing::error!("[ProactiveSystem] Failed to read schedules.json: {:?}", e);
                },
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
    /// 调用者需确保 prompt 已完整生成（含截图分析结果）。
    async fn deliver(
        &mut self,
        prompt: String,
        transient_image: Option<String>,
    ) -> anyhow::Result<()> {
        // 先克隆 Arc 再上锁：guard 因此不借用 self，后面才能把 &mut self 交给 prepare
        let lock = self.generation_lock.clone();
        let _lock = lock.lock().await;
        let generator = self.prepare_delivery(Some(prompt), transient_image).await?;
        let _ = generator.process_message(None).await;
        Ok(())
    }

    /// 外部（插件）请求一次主动对话：判定通过就排进后台，本调用立刻返回。
    pub async fn request_deliver(
        system: Arc<Mutex<Self>>,
        raw: String,
    ) -> Result<(), &'static str> {
        // 先做不受生成锁影响的那几道判定：这些原因当场就能回给插件，不必等。
        {
            let Ok(this) = system.try_lock() else {
                return Err("system_busy");
            };
            if !this.config.enable_proactive_system {
                return Err("proactive_disabled");
            }
            let perception = this.activity_monitor.get_user_status();
            if !DeliveryEvaluator::can_deliver(IntentType::Topic, &perception, this.can_deliver) {
                return Err("not_deliverable");
            }
        }
        tauri::async_runtime::spawn(async move {
            if let Err(reason) = deliver_when_idle(system, raw).await {
                tracing::info!("[ProactiveSystem] 插件请求的主动对话最终没能投放：{reason}");
            }
        });
        Ok(())
    }

    /// 投放的前半段：点亮思考指示、取好依赖、把提示行写进台词表。
    async fn prepare_delivery(
        &mut self,
        prompt: Option<String>,
        transient_image: Option<String>,
    ) -> anyhow::Result<MessageGenerator> {
        tracing::info!("[ProactiveSystem] Delivering proactive dialogue...");
        events::emit_thinking(&self.app, true);

        let generator = {
            let (game_status, preview_generation) = {
                let svc = self.ai_service.lock().await;
                let gs = svc.game_status.clone();
                let gen = gs.lock().await.preview_generation;
                (gs, gen)
            };
            let deps = GeneratorDeps {
                source: GeneratorSource::Proactive,
                app: self.app.clone(),
                db: self.db.clone(),
                game_status,
                processor: self.chat.processor.clone(),
                translator: self.chat.translator.clone(),
                llm: match crate::ai_service::llm::slot_snapshot(&self.chat.llm).await {
                    Some(llm) => llm,
                    None => {
                        events::emit_thinking(&self.app, false);
                        anyhow::bail!("LLM is not configured");
                    },
                },
                tool_registry: self.tool_registry.clone(),
                concurrency: 1,
                god_agent: None,
                suppress_thinking: false,
                // 捕获当前试玩代号（自由对话恒等，行为不变）
                generation: preview_generation,
                is_preview: false,
                // 屏幕感知原生识图时，把截图当轮直发给对话模型（不写记忆，省上下文/缓存）
                transient_image,
            };
            MessageGenerator::new(deps)
        };

        if let Some(prompt) = prompt {
            let svc = self.ai_service.lock().await;
            let mut gs = svc.game_status.lock().await;
            let written = gs
                .add_line(
                    &self.db,
                    LineBase {
                        attribute: LineAttributeExt(LineAttribute::User),
                        content: prompt,
                        sender_role_id: None,
                        display_name: Some(PromptRole::System.display_name().to_string()),
                        ..Default::default()
                    },
                )
                .await;
            if let Err(e) = written {
                events::emit_thinking(&self.app, false);
                return Err(e);
            }
        }

        Ok(generator)
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
                self.deliver(intent.prompt, intent.transient_image).await?;
                return Ok(true);
            }
        }

        Ok(false)
    }

    // ============================================================
    // run_reminder_check（到点提醒，独立于主动对话总开关）
    // ============================================================

    /// 检查日程闹钟与待办截止提醒，命中即立即投放。
    ///
    /// 与 run_cycle 分开的原因：它有意绕开主动对话总开关，也绕开 `can_deliver`
    /// 投放闸门——到点提醒晚到就失去意义，进「小本本」暂存（Alarm TTL 30 分钟）
    /// 等于没提醒。剧本进行中由调用方负责跳过。
    ///
    /// 返回 true 表示本轮已投放，调用方应跳过 run_cycle，避免同一轮再闲聊一句。
    async fn run_reminder_check(system_arc: Arc<Mutex<Self>>) -> anyhow::Result<bool> {
        let mut sys = system_arc.lock().await;

        let settings_snap = {
            let snap = sys.settings.read().await;
            snap.clone()
        };

        let (user_name, role_name) = {
            let svc = sys.ai_service.lock().await;
            let gs = svc.game_status.lock().await;
            let role_name = gs
                .current_role_id
                .and_then(|rid| gs.role_manager.get_loaded(rid))
                .and_then(|role| role.display_name.clone());
            (gs.player.user_name.clone(), role_name)
        };

        // ─── 日程闹钟 ───
        if sys.config.enable_schedule_reminder
            && (sys.config.enable_proactive_system || sys.config.force_schedule_reminder)
        {
            if let Some(raw_prompt) = sys
                .schedule_manager
                .check_schedule_reminder(&user_name, &settings_snap)
            {
                tracing::info!("[ProactiveSystem] Schedule alarm delivering immediately");
                let formatted = PromptRole::System.build_prompt(&raw_prompt);
                sys.deliver(formatted, None).await?;
                return Ok(true);
            }
        }

        // ─── 待办截止提醒 ───
        if sys.config.enable_proactive_system || sys.config.force_todo_reminder {
            if let Some(reminder) = sys.schedule_manager.check_todo_reminder(&settings_snap) {
                if sys.config.todo_reminder_style == "toast" {
                    events::emit_proactive_remind(
                        &sys.app,
                        role_name.unwrap_or_else(|| "LingChat".to_string()),
                        reminder.todo_text,
                        reminder.minutes_before,
                    );
                } else {
                    let raw_prompt = format!(
                        "{{你想起来 {} 有件事快到期了：'{}'，还有 {} 分钟就到截止时间。提醒他一下吧？}}",
                        user_name, reminder.todo_text, reminder.minutes_before
                    );
                    let formatted = PromptRole::System.build_prompt(&raw_prompt);
                    sys.deliver(formatted, None).await?;
                }
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

        // ─── 2. 感知 ───
        let perception = sys.activity_monitor.get_user_status();

        // ─── 3. 兴趣累积 ───
        sys.interest_manager.update_interest();
        sys.interest_manager
            .set_status_mod(perception.interest_modifier);

        // ─── 4. 触发检查 ───
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
                .get_proactive_prompt(&sys.app, &gs, &settings_snap, &perception, &sys.config)
                .await
        };
        let Some(ProactivePrompt {
            raw_prompt,
            intent_type,
            transient_image,
        }) = prompt_result
        else {
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
            sys.deliver(formatted, transient_image).await?;
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
                transient_image,
                triggered_at: std::time::Instant::now(),
            });
            events::emit_thinking(&sys.app, false);
        }

        Ok(())
    }
}

/// 已经写进台词表、只差跑生成的一次投放。
///
/// 生成锁随它一起移交（`_guard`），所以「谁在生成」这条不变量仍然成立：轮询那侧
/// 动手前会 `try_lock`，拿不到就当这轮没轮上。
struct PreparedDelivery {
    _guard: OwnedMutexGuard<()>,
    generator: MessageGenerator,
}

/// 插件请求的主动对话，在生成占用中时最多等这么久。
///
/// 等的是「这一轮回复说完」——动作收尾这类外部事件常常正好落在这段时间里。
const PLUGIN_WAIT_FOR_GENERATION: Duration = Duration::from_secs(45);

/// 等到生成空闲，再投放一次插件请求的主动对话。
///
/// 锁序与别处相反（先拿生成锁、再碰系统锁），所以系统锁只用 `try_lock`：拿不到就
/// 当场放弃，否则会和「持系统锁等生成锁」的轮询线程对锁。投不投得成最后都只记日志，
/// 调用方早已拿到「已受理」。
async fn deliver_when_idle(
    system: Arc<Mutex<ProactiveSystem>>,
    raw: String,
) -> Result<(), &'static str> {
    let guard = {
        let Ok(this) = system.try_lock() else {
            return Err("system_busy");
        };
        let lock = this.generation_lock.clone();
        drop(this);
        match tokio::time::timeout(PLUGIN_WAIT_FOR_GENERATION, lock.lock_owned()).await {
            Ok(guard) => guard,
            Err(_) => return Err("generation_busy"),
        }
    };

    let prepared = {
        let Ok(mut this) = system.try_lock() else {
            return Err("system_busy");
        };
        // 等待期间状态可能变了（用户关了主动对话、切到不方便被打断的状态），重判一次
        if !this.config.enable_proactive_system {
            return Err("proactive_disabled");
        }
        let perception = this.activity_monitor.get_user_status();
        if !DeliveryEvaluator::can_deliver(IntentType::Topic, &perception, this.can_deliver) {
            return Err("not_deliverable");
        }
        // 提示语由插件给（那是策略，改它不该重编宿主），信封由宿主包——
        // 与 add_prompt_line 同一个原则：插件伪造不出旁白的形状。空提示语表示
        // 只请宿主开一次口、不写提示行：要说的事插件已经自己记成旁白了。
        let prompt = (!raw.is_empty()).then(|| PromptRole::System.build_prompt(&raw));
        let generator = match this.prepare_delivery(prompt, None).await {
            Ok(generator) => generator,
            Err(e) => {
                tracing::warn!("[ProactiveSystem] 插件请求的主动对话没能起头：{e}");
                return Err("llm_unavailable");
            },
        };
        PreparedDelivery {
            _guard: guard,
            generator,
        }
    };
    prepared.run().await;
    Ok(())
}

impl PreparedDelivery {
    /// 把这一轮说完。调用方是后台任务，没人接错误，失败只记日志。
    async fn run(self) {
        let _ = self.generator.process_message(None).await;
    }
}
