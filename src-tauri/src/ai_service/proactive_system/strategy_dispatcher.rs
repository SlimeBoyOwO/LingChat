use crate::ai_service::game_system::game_status::GameStatus;
use crate::ai_service::proactive_system::config::ProactiveConfig;
use crate::ai_service::proactive_system::types::{
    IntentType, PerceptionResult, UserScheduleSettings, UserState,
};
use crate::ai_service::proactive_system::proactive_history::ProactiveDeduplicator;
use crate::ai_service::screen_analyzer::{ScreenAnalyzer, ScreenContext, build_screen_analyzer_config};
use chrono::Local;
use rand::Rng;
use tokio::sync::Mutex;

pub struct StrategyDispatcher {
    screen_analyzer: Mutex<ScreenAnalyzer>,
    deduplicator: Mutex<ProactiveDeduplicator>,
}

impl StrategyDispatcher {
    pub fn new(app_handle: &tauri::AppHandle) -> Self {
        let config = ProactiveConfig::load(app_handle);
        let sa_config = build_screen_analyzer_config(app_handle, &config);
        Self {
            screen_analyzer: Mutex::new(ScreenAnalyzer::new(sa_config)),
            deduplicator: Mutex::new(ProactiveDeduplicator::default()),
        }
    }

    /// 更新配置（同时同步 ScreenAnalyzer 的配置，同步执行无需 async）。
    pub fn update_config(&self, app_handle: &tauri::AppHandle) {
        let config = ProactiveConfig::load(app_handle);
        // try_lock: update_config 不涉及 async，用同步锁即可
        if let Ok(mut sa) = self.screen_analyzer.try_lock() {
            sa.update_config(build_screen_analyzer_config(app_handle, &config));
        }
    }

    async fn filter_duplicate(&self, prompt: String) -> Option<String> {
        let mut dedup = self.deduplicator.lock().await;
        let (dup, score) = dedup.check_and_record(&prompt);
        if dup {
            tracing::info!(
                "[StrategyDispatcher] Duplicate proactive prompt detected (score={:.2}), skipping.",
                score
            );
            None
        } else {
            Some(prompt)
        }
    }
    /// 生成主动对话的 Prompt。
    /// 优先顺序: ImportantDay (每天仅一次) > Todo > Screen Observation > Topic
    pub async fn get_proactive_prompt(
        &self,
        game_status: &GameStatus,
        settings: &UserScheduleSettings,
        perception: &PerceptionResult,
        config: &ProactiveConfig,
    ) -> Option<(String, IntentType)> {
        let now = Local::now();
        let today_str = now.format("%m-%d").to_string();

        // 1. 检查 ImportantDay (如果是今天且今天未触发过)
        if config.enable_important_day_reminder {
            if let Some(important_days) = &settings.important_days {
                let last_talk_date = game_status
                    .last_dialog_time
                    .map(|dt| dt.format("%m-%d").to_string())
                    .unwrap_or_default();

                if last_talk_date != today_str {
                    for day in important_days {
                        if day.date.ends_with(&today_str) {
                            let desc = day.desc.as_deref().unwrap_or("");
                            let char_name = game_status
                                .current_role_id
                                .and_then(|rid| game_status.role_manager.get_loaded(rid))
                                .and_then(|role| role.display_name.clone())
                                .unwrap_or_else(|| "小灵".to_string());

                            tracing::info!(
                                "[StrategyDispatcher] Triggered important day reminder: {}",
                                day.title
                            );
                            let prompt = format!(
                                "{{今天是特殊的一天：{}，{}。可以和{}聊聊哦}}",
                                day.title, desc, char_name
                            );
                            let prompt = self.filter_duplicate(prompt).await?;
                            return Some((prompt, IntentType::ImportantDay));
                        }
                    }
                }
            }
        }

        // 2. 随机模式选择，根据启用状态动态构建候选列表
        let mut modes = Vec::new();
        let mut weights = Vec::new();

        // 获取权重（如果配置文件有设置，否则基于 UserState 动态决定）
        let mut todo_w = config.todo_weight;
        let mut topic_w = config.topic_weight;
        let mut screen_w = config.screen_weight;

        if todo_w <= 0.0 {
            todo_w = if perception.state == UserState::WORK {
                60.0
            } else {
                10.0
            };
        }
        if topic_w <= 0.0 {
            topic_w = if perception.state == UserState::IDLE {
                80.0
            } else {
                60.0
            };
        }
        if screen_w <= 0.0 {
            screen_w = if perception.state == UserState::GAME {
                60.0
            } else {
                30.0
            };
        }

        if config.enable_todo_perception {
            modes.push("TODO");
            weights.push(todo_w);
        }
        if config.enable_topic_creator {
            modes.push("TOPIC");
            weights.push(topic_w);
        }
        if config.enable_visual_perception {
            modes.push("SCREEN");
            weights.push(screen_w);
        }

        if modes.is_empty() {
            return None;
        }

        // 轮盘赌/加权随机选择
        let selected_mode = {
            let mut rng = rand::thread_rng();
            let total_weight: f64 = weights.iter().sum();
            let mut roll = rng.gen_range(0.0..total_weight);
            let mut selected = modes[0];
            for (i, &w) in weights.iter().enumerate() {
                roll -= w;
                if roll <= 0.0 {
                    selected = modes[i];
                    break;
                }
            }
            selected
        };

        tracing::info!(
            "[StrategyDispatcher] Selected proactive mode: {} (weights: TODO={:.1}, TOPIC={:.1}, SCREEN={:.1})",
            selected_mode, todo_w, topic_w, screen_w
        );

        match selected_mode {
            "TODO" => {
                if let Some(prompt) = self.get_todo_prompt(game_status, settings) {
                    let prompt = self.filter_duplicate(prompt).await?;
                    return Some((prompt, IntentType::Todo));
                }
                // 没有 Todo 时降级到 TOPIC
                if config.enable_topic_creator {
                    let prompt = self.get_topic_prompt(game_status);
                    let prompt = self.filter_duplicate(prompt).await?;
                    return Some((prompt, IntentType::Topic));
                }
                None
            }
            "SCREEN" => {
                if let Some((prompt, intent)) = self.get_screen_prompt(game_status).await {
                    let prompt = self.filter_duplicate(prompt).await?;
                    return Some((prompt, intent));
                }
                // SCREEN 抓取失败或接口失败时降级到 TOPIC
                if config.enable_topic_creator {
                    let prompt = self.get_topic_prompt(game_status);
                    let prompt = self.filter_duplicate(prompt).await?;
                    return Some((prompt, IntentType::Topic));
                }
                None
            }
            _ => {
                let prompt = self.get_topic_prompt(game_status);
                let prompt = self.filter_duplicate(prompt).await?;
                Some((prompt, IntentType::Topic))
            }
        }
    }

    fn get_todo_prompt(
        &self,
        game_status: &GameStatus,
        settings: &UserScheduleSettings,
    ) -> Option<String> {
        let todo_groups = settings.todo_groups.as_ref()?;
        let mut candidates = Vec::new();

        for group in todo_groups.values() {
            for todo in &group.todos {
                if !todo.completed && todo.priority >= 1 {
                    candidates.push(todo);
                }
            }
        }

        if candidates.is_empty() {
            return None;
        }

        let mut rng = rand::thread_rng();
        let idx = rng.gen_range(0..candidates.len());
        let selected = candidates[idx];
        let user_name = &game_status.player.user_name;

        Some(format!(
            "{{你想起来{}有一个未完成的任务：'{} '。提醒一下吧？}}",
            user_name, selected.text
        ))
    }

    async fn get_screen_prompt(
        &self,
        game_status: &GameStatus,
    ) -> Option<(String, IntentType)> {
        let user_name = game_status.player.user_name.clone();
        let ai_name = game_status
            .current_role_id
            .and_then(|rid| game_status.role_manager.get_loaded(rid))
            .and_then(|role| role.display_name.clone())
            .unwrap_or_else(|| "你".to_string());

        let context = ScreenContext {
            ai_name: Some(ai_name.clone()),
            user_name: Some(user_name.clone()),
            recent_chat_summary: None,
        };

        let reply = self
            .screen_analyzer
            .lock()
            .await
            .analyze_screen_for_proactive(Some(&context))
            .await?;

        let reply = reply.trim();
        if reply.is_empty() || reply == "[PASS]" {
            tracing::info!("[StrategyDispatcher] Screen proactive returned [PASS], falling back.");
            return None;
        }

        // 清理模型可能附带的前缀（如 "AI:" 或引号）
        let cleaned = reply
            .trim_start_matches("[PASS]")
            .trim_start_matches(":")
            .trim()
            .to_string();

        if cleaned.is_empty() {
            return None;
        }

        Some((format!("{{ {}}}", cleaned), IntentType::Screen))
    }

    fn get_topic_prompt(&self, game_status: &GameStatus) -> String {
        let ai_name = game_status
            .current_role_id
            .and_then(|rid| game_status.role_manager.get_loaded(rid))
            .and_then(|role| role.display_name.clone())
            .unwrap_or_else(|| "你".to_string());

        format!("{{ {} 想继续说话了}}", ai_name)
    }
}
