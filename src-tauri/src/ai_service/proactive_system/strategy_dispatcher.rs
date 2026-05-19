use chrono::Local;
use rand::Rng;
use reqwest::Client;
use serde_json::Value;
use crate::ai_service::proactive_system::types::{
    UserScheduleSettings, PerceptionResult, UserState
};
use crate::ai_service::proactive_system::config::ProactiveConfig;
use crate::ai_service::game_system::game_status::GameStatus;

pub struct StrategyDispatcher;

impl StrategyDispatcher {
    pub fn new() -> Self {
        Self
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
                            
                            log::info!("[StrategyDispatcher] Triggered important day reminder: {}", day.title);
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
            todo_w = if perception.state == UserState::WORK { 60.0 } else { 10.0 };
        }
        if topic_w <= 0.0 {
            topic_w = if perception.state == UserState::IDLE { 80.0 } else { 60.0 };
        }
        if screen_w <= 0.0 {
            screen_w = if perception.state == UserState::GAME { 60.0 } else { 30.0 };
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

        log::info!(
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
                if let Some(prompt) = self.get_screen_prompt(game_status, config).await {
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

    async fn get_screen_prompt(
        &self,
        game_status: &GameStatus,
        config: &ProactiveConfig,
    ) -> Option<String> {
        if config.vd_api_key.is_empty() {
            log::warn!("[StrategyDispatcher] VD_API_KEY is empty, skipping screenshot analysis.");
            return None;
        }

        // 截取桌面画面并压缩为 JPEG 字节
        let jpeg_bytes = match capture_screen_as_jpeg() {
            Some(bytes) => bytes,
            None => {
                log::error!("[StrategyDispatcher] Failed to capture desktop screenshot.");
                return None;
            }
        };

        let base64_image = base64::Engine::encode(&base64::prelude::BASE64_STANDARD, &jpeg_bytes);
        let image_url = format!("data:image/jpeg;base64,{}", base64_image);

        let analyze_prompt = "你是一个图像信息转述者，你将需要把你看到的画面描述给另一个AI让他理解用户的图片内容。用户开放了那个AI的自主窥屏功能，请获取桌面画面中的重点内容，用200字描述主体部分即可。如果你看到一个聊天窗口，有角色的立绘和对话框，不要描述这部分，只描述桌面上的其他内容。因为那部分是玩家与AI的聊天窗口。";

        log::info!("[StrategyDispatcher] Sending screenshot to Vision LLM ({}) for analysis...", config.vd_model);

        let client = Client::new();
        let payload = serde_json::json!({
            "model": &config.vd_model,
            "messages": [
                {
                    "role": "user",
                    "content": [
                        {"type": "text", "text": analyze_prompt},
                        {"type": "image_url", "image_url": {"url": image_url}}
                    ]
                }
            ],
            "max_tokens": 512
        });

        let res = client.post(&format!("{}/chat/completions", config.vd_base_url))
            .bearer_auth(&config.vd_api_key)
            .json(&payload)
            .send()
            .await;

        match res {
            Ok(response) => {
                if response.status().is_success() {
                    if let Ok(json_res) = response.json::<Value>().await {
                        if let Some(content) = json_res["choices"][0]["message"]["content"].as_str() {
                            let user_name = &game_status.player.user_name;
                            let ai_name = game_status
                                .current_role_id
                                .and_then(|rid| game_status.role_manager.get_loaded(rid))
                                .and_then(|role| role.display_name.clone())
                                .unwrap_or_else(|| "你".to_string());
                            
                            log::info!("[StrategyDispatcher] Screenshot analysis success: {}", content);
                            return Some(format!(
                                "{{ {} 偷看了一眼 {} 的电脑桌面信息: {} }}",
                                ai_name, user_name, content
                            ));
                        }
                    }
                } else {
                    let err_text = response.text().await.unwrap_or_default();
                    log::error!("[StrategyDispatcher] VLM API returned error status: {}", err_text);
                }
            }
            Err(e) => {
                log::error!("[StrategyDispatcher] Failed to send request to VLM: {:?}", e);
            }
        }

        None
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

/// 捕获桌面并将其转换为 JPEG 格式的辅助函数。
fn capture_screen_as_jpeg() -> Option<Vec<u8>> {
    #[cfg(target_os = "windows")]
    {
        use windows::Win32::Graphics::Gdi::{
            GetDC, ReleaseDC, CreateCompatibleDC, CreateCompatibleBitmap, SelectObject,
            BitBlt, GetDIBits, DeleteDC, DeleteObject, SRCCOPY, BITMAPINFOHEADER, DIB_RGB_COLORS,
        };
        use windows::Win32::UI::WindowsAndMessaging::{GetSystemMetrics, SM_CXSCREEN, SM_CYSCREEN};

        unsafe {
            let hdc_screen = GetDC(None);
            if hdc_screen.is_invalid() {
                return None;
            }
            let w = GetSystemMetrics(SM_CXSCREEN);
            let h = GetSystemMetrics(SM_CYSCREEN);
            if w <= 0 || h <= 0 {
                ReleaseDC(None, hdc_screen);
                return None;
            }
            
            let hdc_mem = CreateCompatibleDC(Some(hdc_screen));
            let hbitmap = CreateCompatibleBitmap(hdc_screen, w, h);
            
            let old_obj = SelectObject(hdc_mem, hbitmap.into());
            let _ = BitBlt(hdc_mem, 0, 0, w, h, Some(hdc_screen), 0, 0, SRCCOPY);
            
            // setup BITMAPINFO
            let mut bmi = windows::Win32::Graphics::Gdi::BITMAPINFO::default();
            bmi.bmiHeader.biSize = std::mem::size_of::<BITMAPINFOHEADER>() as u32;
            bmi.bmiHeader.biWidth = w;
            bmi.bmiHeader.biHeight = -h; // Negative height for top-down bitmap
            bmi.bmiHeader.biPlanes = 1;
            bmi.bmiHeader.biBitCount = 24;
            bmi.bmiHeader.biCompression = 0; // BI_RGB
            
            let mut buffer = vec![0u8; (w * h * 3) as usize];
            
            let lines = GetDIBits(
                hdc_screen,
                hbitmap,
                0,
                h as u32,
                Some(buffer.as_mut_ptr() as *mut _),
                &mut bmi,
                DIB_RGB_COLORS,
            );
            
            // cleanup GDI objects
            SelectObject(hdc_mem, old_obj);
            let _ = DeleteObject(hbitmap.into());
            let _ = DeleteDC(hdc_mem);
            ReleaseDC(None, hdc_screen);
            
            if lines <= 0 {
                return None;
            }
            
            // Swap B and R to get RGB from BGR
            for chunk in buffer.chunks_exact_mut(3) {
                chunk.swap(0, 2);
            }
            
            // Compress image to 1024x768 for faster Vision API upload and fewer tokens
            let img = image::ImageBuffer::<image::Rgb<u8>, _>::from_raw(w as u32, h as u32, buffer)?;
            let dynamic_img = image::DynamicImage::ImageRgb8(img);
            let resized = dynamic_img.resize(1024, 768, image::imageops::FilterType::Triangle);
            
            let mut jpeg_bytes = Vec::new();
            let mut cursor = std::io::Cursor::new(&mut jpeg_bytes);
            let _ = resized.write_to(&mut cursor, image::ImageFormat::Jpeg);
            
            Some(jpeg_bytes)
        }
    }

    #[cfg(not(target_os = "windows"))]
    {
        None
    }
}
