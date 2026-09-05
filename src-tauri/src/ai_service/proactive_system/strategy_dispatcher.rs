use crate::ai_service::game_system::game_status::GameStatus;
use crate::ai_service::llm::provider_config::resolve_chat_provider;
use crate::ai_service::proactive_system::config::ProactiveConfig;
use crate::ai_service::proactive_system::types::{
    IntentType, PerceptionResult, ProactivePrompt, UserScheduleSettings, UserState,
};
use crate::ai_service::screen_analyzer::{
    NativeImageCompress, ScreenAnalyzer, ScreenAnalyzerConfig, capture_screen_as_jpeg,
    image_bytes_to_native_data_url,
};
use chrono::Local;
use rand::Rng;
use tokio::sync::Mutex;

pub struct StrategyDispatcher {
    screen_analyzer: Mutex<ScreenAnalyzer>,
}

impl StrategyDispatcher {
    pub fn new(app_handle: &tauri::AppHandle) -> Self {
        Self {
            screen_analyzer: Mutex::new(ScreenAnalyzer::new(ScreenAnalyzerConfig::resolve(
                app_handle,
            ))),
        }
    }

    /// 更新配置（同时同步 ScreenAnalyzer 的配置，同步执行无需 async）。
    pub fn update_config(&self, app_handle: &tauri::AppHandle) {
        // try_lock: update_config 不涉及 async，用同步锁即可
        if let Ok(mut sa) = self.screen_analyzer.try_lock() {
            sa.update_config(ScreenAnalyzerConfig::resolve(app_handle));
        }
    }

    /// 生成主动对话的 Prompt。
    /// 优先顺序: ImportantDay (每天仅一次) > Todo > Screen Observation > Topic
    pub async fn get_proactive_prompt(
        &self,
        app: &tauri::AppHandle,
        game_status: &GameStatus,
        settings: &UserScheduleSettings,
        perception: &PerceptionResult,
        config: &ProactiveConfig,
    ) -> Option<ProactivePrompt> {
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
                            return Some(ProactivePrompt {
                                raw_prompt: format!(
                                    "{{今天是特殊的一天：{}，{}。可以和{}聊聊哦}}",
                                    day.title, desc, char_name
                                ),
                                intent_type: IntentType::ImportantDay,
                                transient_image: None,
                            });
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
            selected_mode,
            todo_w,
            topic_w,
            screen_w
        );

        match selected_mode {
            "TODO" => {
                if let Some(prompt) = self.get_todo_prompt(game_status, settings) {
                    return Some(ProactivePrompt {
                        raw_prompt: prompt,
                        intent_type: IntentType::Todo,
                        transient_image: None,
                    });
                }
                // 没有 Todo 时降级到 TOPIC
                if config.enable_topic_creator {
                    return Some(ProactivePrompt {
                        raw_prompt: self.get_topic_prompt(game_status),
                        intent_type: IntentType::Topic,
                        transient_image: None,
                    });
                }
                None
            },
            "SCREEN" => {
                if let Some(pp) = self.get_screen_prompt(app, game_status).await {
                    return Some(pp);
                }
                // SCREEN 抓取失败或接口失败时降级到 TOPIC
                if config.enable_topic_creator {
                    return Some(ProactivePrompt {
                        raw_prompt: self.get_topic_prompt(game_status),
                        intent_type: IntentType::Topic,
                        transient_image: None,
                    });
                }
                None
            },
            _ => Some(ProactivePrompt {
                raw_prompt: self.get_topic_prompt(game_status),
                intent_type: IntentType::Topic,
                transient_image: None,
            }),
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
        app: &tauri::AppHandle,
        game_status: &GameStatus,
    ) -> Option<ProactivePrompt> {
        let analyze_prompt = "你是一个图像信息转述者，你将需要把你看到的画面描述给另一个AI让他理解用户的图片内容。用户开放了那个AI的自主窥屏功能，请获取桌面画面中的重点内容，用200字描述主体部分即可。如果你看到一个聊天窗口，有角色的立绘和对话框，不要描述这部分，只描述桌面上的其他内容。因为那部分是玩家与AI的聊天窗口。";

        let user_name = &game_status.player.user_name;
        let ai_name = game_status
            .current_role_id
            .and_then(|rid| game_status.role_manager.get_loaded(rid))
            .and_then(|role| role.display_name.clone())
            .unwrap_or_else(|| "你".to_string());

        // 对话模型是否启用原生多模态识图：是则直接把桌面截图当轮直发（不转述），
        // 否则回退到「旁白转述」路径（VLM 先把画面转成文本）。
        let native_vision = resolve_chat_provider(app)
            .map(|p| p.support_vision && p.is_genai_multimodal_capable())
            .unwrap_or(false);
        let native_compress = resolve_chat_provider(app)
            .map(|p| p.native_image_compress())
            .unwrap_or(NativeImageCompress::default());

        // ─── 原生识图路径：截屏 → 压缩/原图 → 当轮直发图片 ───
        if native_vision {
            let jpeg_bytes = capture_screen_as_jpeg()?;
            let transient_image = image_bytes_to_native_data_url(&jpeg_bytes, native_compress)?;
            let raw_prompt = format!("{{ {} 偷看了一眼 {} 的电脑桌面，请你结合桌面截图内容与 {} 自然地聊两句。 }}", ai_name, user_name, user_name);
            tracing::info!(
                "[StrategyDispatcher] 主动偷看走原生识图: 截图当轮直发对话模型（不写记忆）。"
            );
            return Some(ProactivePrompt {
                raw_prompt,
                intent_type: IntentType::Screen,
                transient_image: Some(transient_image),
            });
        }

        // ─── 旁白转述路径（未启用原生识图）：VLM 先把截图转成文本描述 ───
        let analysis = self
            .screen_analyzer
            .lock()
            .await
            .analyze_screen(analyze_prompt)
            .await?;

        Some(ProactivePrompt {
            raw_prompt: format!(
                "{{ {} 偷看了一眼 {} 的电脑桌面: {} }}",
                ai_name, user_name, analysis
            ),
            intent_type: IntentType::Screen,
            transient_image: None,
        })
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
