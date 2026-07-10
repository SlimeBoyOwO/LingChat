use crate::ai_service::proactive_system::config::ProactiveConfig;
use crate::ai_service::proactive_system::proactive_history::ProactiveDeduplicator;
use crate::ai_service::proactive_system::types::{
    DispatchOutcome, IntentType, PerceptionResult, ProactiveCandidate, ProactiveContext,
    UserScheduleSettings, UserState,
};
use crate::ai_service::screen_analyzer::{
    build_screen_analyzer_config, ScreenAnalyzer, ScreenContext,
};
use chrono::Local;
use rand::Rng;
use std::time::{Duration, Instant};
use tokio::sync::Mutex;

const SCREEN_ATTEMPT_COOLDOWN: Duration = Duration::from_secs(60);

pub struct StrategyDispatcher {
    screen_analyzer: Mutex<ScreenAnalyzer>,
    deduplicator: Mutex<ProactiveDeduplicator>,
    last_screen_attempt: Mutex<Option<Instant>>,
    last_important_day_date: Mutex<Option<String>>,
}

impl StrategyDispatcher {
    pub fn new(app_handle: &tauri::AppHandle) -> Self {
        let config = ProactiveConfig::load(app_handle);
        let sa_config = build_screen_analyzer_config(app_handle, &config);
        Self {
            screen_analyzer: Mutex::new(ScreenAnalyzer::new(sa_config)),
            deduplicator: Mutex::new(ProactiveDeduplicator::default()),
            last_screen_attempt: Mutex::new(None),
            last_important_day_date: Mutex::new(None),
        }
    }

    /// 更新配置，同时可靠地同步 ScreenAnalyzer。不能因视觉请求正在占锁而静默丢失更新。
    pub async fn update_config(&self, app_handle: &tauri::AppHandle) {
        let config = ProactiveConfig::load(app_handle);
        self.screen_analyzer
            .lock()
            .await
            .update_config(build_screen_analyzer_config(app_handle, &config));
    }

    async fn is_duplicate(&self, prompt: &str) -> bool {
        let dedup = self.deduplicator.lock().await;
        let (dup, score) = dedup.is_duplicate(prompt);
        if dup {
            tracing::info!(
                "[StrategyDispatcher] Duplicate proactive prompt detected (score={:.2}), skipping.",
                score
            );
            true
        } else {
            false
        }
    }

    /// 只有真正生成出非空回复并成功投递后才提交去重历史。
    pub async fn record_delivered(&self, prompt: &str, intent_type: IntentType) {
        self.deduplicator.lock().await.record(prompt.to_string());
        if intent_type == IntentType::ImportantDay {
            *self.last_important_day_date.lock().await =
                Some(Local::now().format("%Y-%m-%d").to_string());
        }
    }

    async fn reserve_screen_attempt(&self) -> bool {
        let mut last = self.last_screen_attempt.lock().await;
        let now = Instant::now();
        if last.is_some_and(|at| now.duration_since(at) < SCREEN_ATTEMPT_COOLDOWN) {
            return false;
        }
        *last = Some(now);
        true
    }

    async fn candidate_if_new(
        &self,
        prompt: String,
        intent_type: IntentType,
    ) -> Option<ProactiveCandidate> {
        if self.is_duplicate(&prompt).await {
            None
        } else {
            Some(ProactiveCandidate {
                prompt,
                intent_type,
            })
        }
    }

    /// 生成主动对话的 Prompt。
    /// 优先顺序: ImportantDay (每天仅一次) > Todo > Screen Observation > Topic
    pub async fn get_proactive_prompt(
        &self,
        context: &ProactiveContext,
        settings: &UserScheduleSettings,
        perception: &PerceptionResult,
        config: &ProactiveConfig,
        screen_eligible: bool,
    ) -> DispatchOutcome {
        let mut outcome = DispatchOutcome::default();
        let now = Local::now();
        let today_str = now.format("%m-%d").to_string();
        let today_full = now.format("%Y-%m-%d").to_string();

        // 1. 检查 ImportantDay。重复候选只跳过本策略，不能阻断后续策略。
        let important_day_already_delivered =
            self.last_important_day_date.lock().await.as_deref() == Some(today_full.as_str());
        if config.enable_important_day_reminder && !important_day_already_delivered {
            if let Some(important_days) = &settings.important_days {
                for day in important_days {
                    if day.date.ends_with(&today_str) {
                        let desc = day.desc.as_deref().unwrap_or("");
                        let prompt = format!(
                            "{{今天是特殊的一天：{}，{}。可以和{}聊聊哦}}",
                            day.title, desc, context.ai_name
                        );
                        if let Some(candidate) = self
                            .candidate_if_new(prompt, IntentType::ImportantDay)
                            .await
                        {
                            tracing::info!(
                                "[StrategyDispatcher] Triggered important day reminder: {}",
                                day.title
                            );
                            outcome.candidate = Some(candidate);
                            return outcome;
                        }
                    }
                }
            }
        }

        // 如果开启视觉理解优先模式，优先尝试 SCREEN，失败再按原逻辑随机
        if config.enable_visual_perception
            && config.visual_perception_priority
            && screen_eligible
            && self.reserve_screen_attempt().await
        {
            tracing::info!(
                "[StrategyDispatcher] Visual perception priority enabled, trying SCREEN first."
            );
            outcome.screen_attempted = true;
            if let Some((prompt, intent)) = self.get_screen_prompt(context).await {
                if let Some(candidate) = self.candidate_if_new(prompt, intent).await {
                    outcome.candidate = Some(candidate);
                    return outcome;
                }
            }
            tracing::info!(
                "[StrategyDispatcher] SCREEN priority failed, falling back to weighted random."
            );
        }

        // 2. 随机模式选择，根据启用状态动态构建候选列表
        let mut modes = Vec::new();
        let mut weights = Vec::new();

        // 获取权重（如果配置文件有设置，否则基于 UserState 动态决定）
        let todo_default = if perception.state == UserState::WORK {
            60.0
        } else {
            10.0
        };
        let topic_default = if perception.state == UserState::IDLE {
            80.0
        } else {
            60.0
        };
        let screen_default = if perception.state == UserState::GAME {
            60.0
        } else {
            30.0
        };
        let todo_w = normalized_weight(config.todo_weight, todo_default);
        let topic_w = normalized_weight(config.topic_weight, topic_default);
        // 有尚未消费的画面变化时提高 SCREEN 权重，但仍保留其他策略的机会。
        let screen_w = normalized_weight(config.screen_weight, screen_default)
            * if screen_eligible { 2.0 } else { 1.0 };

        if config.enable_todo_perception && todo_w > 0.0 {
            modes.push("TODO");
            weights.push(todo_w);
        }
        if config.enable_topic_creator && topic_w > 0.0 {
            modes.push("TOPIC");
            weights.push(topic_w);
        }
        // priority 已经尝试过 SCREEN 时，本轮不能再次把 SCREEN 放回随机池。
        if config.enable_visual_perception
            && screen_eligible
            && !outcome.screen_attempted
            && screen_w > 0.0
        {
            modes.push("SCREEN");
            weights.push(screen_w);
        }

        if modes.is_empty() {
            return outcome;
        }

        // 轮盘赌/加权随机选择
        let selected_mode = {
            let mut rng = rand::thread_rng();
            let total_weight: f64 = weights.iter().sum();
            if !total_weight.is_finite() || total_weight <= 0.0 {
                tracing::warn!(
                    "[StrategyDispatcher] Invalid total strategy weight: {total_weight}"
                );
                return outcome;
            }
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
                if let Some(prompt) = self.get_todo_prompt(context, settings) {
                    if let Some(candidate) = self.candidate_if_new(prompt, IntentType::Todo).await {
                        outcome.candidate = Some(candidate);
                        return outcome;
                    }
                }
                // 没有 Todo 或候选重复时降级到 TOPIC。
                if config.enable_topic_creator {
                    let prompt = self.get_topic_prompt(context);
                    outcome.candidate = self.candidate_if_new(prompt, IntentType::Topic).await;
                }
                outcome
            }
            "SCREEN" => {
                if self.reserve_screen_attempt().await {
                    outcome.screen_attempted = true;
                    if let Some((prompt, intent)) = self.get_screen_prompt(context).await {
                        if let Some(candidate) = self.candidate_if_new(prompt, intent).await {
                            outcome.candidate = Some(candidate);
                            return outcome;
                        }
                    }
                }
                // SCREEN 抓取失败、PASS、重复或仍在冷却时降级到 TOPIC。
                if config.enable_topic_creator {
                    let prompt = self.get_topic_prompt(context);
                    outcome.candidate = self.candidate_if_new(prompt, IntentType::Topic).await;
                }
                outcome
            }
            _ => {
                let prompt = self.get_topic_prompt(context);
                outcome.candidate = self.candidate_if_new(prompt, IntentType::Topic).await;
                outcome
            }
        }
    }

    fn get_todo_prompt(
        &self,
        context: &ProactiveContext,
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
        Some(format!(
            "{{你想起来{}有一个未完成的任务：'{} '。提醒一下吧？}}",
            context.user_name, selected.text
        ))
    }

    pub(crate) async fn get_screen_prompt_for_test(
        &self,
        proactive_context: &ProactiveContext,
    ) -> Option<(String, IntentType)> {
        let screen_context = ScreenContext {
            ai_name: Some(proactive_context.ai_name.clone()),
            user_name: Some(proactive_context.user_name.clone()),
            recent_chat_summary: None,
        };

        // 测试专用：强制评论屏幕，不允许 [PASS]。
        let test_prompt = "你刚刚看了一眼主人的电脑屏幕。请用一句简短自然的话（不超过30字）评论屏幕上最有趣或最新的内容，像不经意间看到的那样。不要解释，直接说出这句话。";

        let reply = self
            .screen_analyzer
            .lock()
            .await
            .analyze_screen(test_prompt, Some(&screen_context))
            .await?;

        let cleaned = clean_screen_reply(&reply)?;

        Some((format!("{{ {}}}", cleaned), IntentType::Screen))
    }

    pub(crate) async fn get_screen_prompt(
        &self,
        proactive_context: &ProactiveContext,
    ) -> Option<(String, IntentType)> {
        let screen_context = ScreenContext {
            ai_name: Some(proactive_context.ai_name.clone()),
            user_name: Some(proactive_context.user_name.clone()),
            recent_chat_summary: None,
        };

        let reply = self
            .screen_analyzer
            .lock()
            .await
            .analyze_screen_for_proactive(Some(&screen_context))
            .await?;

        let Some(cleaned) = clean_screen_reply(&reply) else {
            tracing::info!("[StrategyDispatcher] Screen proactive returned [PASS], falling back.");
            return None;
        };

        Some((format!("{{ {}}}", cleaned), IntentType::Screen))
    }

    fn get_topic_prompt(&self, context: &ProactiveContext) -> String {
        // 加入随机变体，避免 deduplicator 把每次 TOPIC 都判为重复
        let templates = [
            format!("{{ {} 想找主人说说话}}", context.ai_name),
            format!("{{ {} 突然想到一件事}}", context.ai_name),
            format!("{{ {} 有话想对主人说}}", context.ai_name),
            format!("{{ {} 想继续陪主人聊天}}", context.ai_name),
            format!("{{ {} 看着主人，忍不住想开口}}", context.ai_name),
        ];

        let mut rng = rand::thread_rng();
        let idx = rng.gen_range(0..templates.len());
        templates[idx].clone()
    }
}

fn normalized_weight(configured: f64, fallback: f64) -> f64 {
    if configured.is_finite() && configured >= 0.0 {
        configured
    } else if fallback.is_finite() && fallback >= 0.0 {
        fallback
    } else {
        0.0
    }
}

fn clean_screen_reply(reply: &str) -> Option<String> {
    let mut cleaned = reply.trim().trim_matches(['"', '\'', '“', '”']).trim();
    for prefix in ["AI:", "Ai:", "ai:", "AI：", "Ai：", "ai："] {
        if let Some(rest) = cleaned.strip_prefix(prefix) {
            cleaned = rest.trim();
            break;
        }
    }
    cleaned = cleaned.trim_matches(['"', '\'', '“', '”']).trim();
    if cleaned.is_empty() || cleaned.to_ascii_uppercase().starts_with("[PASS]") {
        None
    } else {
        Some(cleaned.to_string())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn invalid_weights_fall_back_to_positive_values() {
        assert_eq!(normalized_weight(f64::NAN, 30.0), 30.0);
        assert_eq!(normalized_weight(f64::INFINITY, 30.0), 30.0);
        assert_eq!(normalized_weight(-10.0, 30.0), 30.0);
        assert_eq!(normalized_weight(0.0, 30.0), 0.0);
        assert_eq!(normalized_weight(f64::NAN, 0.0), 0.0);
    }

    #[test]
    fn pass_variants_are_never_delivered() {
        assert!(clean_screen_reply("[PASS]").is_none());
        assert!(clean_screen_reply(" [pass]：画面没有变化").is_none());
        assert!(clean_screen_reply("[PASS].").is_none());
        assert!(clean_screen_reply("\"[PASS]\"").is_none());
        assert!(clean_screen_reply("AI: [PASS]").is_none());
        assert!(clean_screen_reply("“AI： [PASS]”").is_none());
        assert_eq!(
            clean_screen_reply("AI：挺有意思的").as_deref(),
            Some("挺有意思的")
        );
    }
}
