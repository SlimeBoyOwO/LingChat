use crate::ai_service::game_system::game_status::GameStatus;
use crate::ai_service::proactive_system::config::ProactiveConfig;
use crate::ai_service::proactive_system::types::{
    PerceptionResult, UserScheduleSettings, UserState,
};
use crate::ai_service::screen_analyzer::{ScreenAnalyzer, ScreenAnalyzerConfig};
use chrono::Local;
use rand::Rng;
use tokio::sync::Mutex;

pub struct StrategyDispatcher {
    screen_analyzer: Mutex<ScreenAnalyzer>,
}

impl StrategyDispatcher {
    pub fn new(app_handle: &tauri::AppHandle) -> Self {
        let config = ProactiveConfig::load(app_handle);
        let sa_config = ScreenAnalyzerConfig {
            vd_api_key: config.vd_api_key.clone(),
            vd_base_url: config.vd_base_url.clone(),
            vd_model: config.vd_model.clone(),
        };
        Self {
            screen_analyzer: Mutex::new(ScreenAnalyzer::new(sa_config)),
        }
    }

    /// 更新配置（同时同步 ScreenAnalyzer 的配置，同步执行无需 async）。
    pub fn update_config(&self, app_handle: &tauri::AppHandle) {
        let config = ProactiveConfig::load(app_handle);
        // try_lock: update_config 不涉及 async，用同步锁即可
        if let Ok(mut sa) = self.screen_analyzer.try_lock() {
            sa.update_config(ScreenAnalyzerConfig {
                vd_api_key: config.vd_api_key,
                vd_base_url: config.vd_base_url,
                vd_model: config.vd_model,
            });
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
    ) -> Option<String> {
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
                            return Some(format!(
                                "{{今天是特殊的一天：{}，{}。可以和{}聊聊哦}}",
                                day.title, desc, char_name
                            ));
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
                    return Some(prompt);
                }
                // 没有 Todo 时降级到 TOPIC
                if config.enable_topic_creator {
                    return Some(self.get_topic_prompt(game_status));
                }
                None
            }
            "SCREEN" => {
                if let Some(prompt) = self.get_screen_prompt(game_status).await {
                    return Some(prompt);
                }
                // SCREEN 抓取失败或接口失败时降级到 TOPIC
                if config.enable_topic_creator {
                    return Some(self.get_topic_prompt(game_status));
                }
                None
            }
            _ => Some(self.get_topic_prompt(game_status)),
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

    async fn get_screen_prompt(&self, game_status: &GameStatus) -> Option<String> {
        let analyze_prompt = "你是一个图像信息转述者，你将需要把你看到的画面描述给另一个AI让他理解用户的图片内容。用户开放了那个AI的自主窥屏功能，请获取桌面画面中的重点内容，用200字描述主体部分即可。如果你看到一个聊天窗口，有角色的立绘和对话框，不要描述这部分，只描述桌面上的其他内容。因为那部分是玩家与AI的聊天窗口。";

        let analysis = self
            .screen_analyzer
            .lock()
            .await
            .analyze_screen(analyze_prompt)
            .await?;

        let user_name = &game_status.player.user_name;
        let ai_name = game_status
            .current_role_id
            .and_then(|rid| game_status.role_manager.get_loaded(rid))
            .and_then(|role| role.display_name.clone())
            .unwrap_or_else(|| "你".to_string());

        Some(format!(
            "{{ {} 偷看了一眼 {} 的电脑桌面: {} }}",
            ai_name, user_name, analysis
        ))
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
