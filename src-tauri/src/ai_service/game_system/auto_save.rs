use std::hash::{DefaultHasher, Hash, Hasher};
use std::sync::Arc;
use std::time::Duration;

use chrono::Local;
use sea_orm::DatabaseConnection;
use serde::Serialize;
use tauri::{AppHandle, Emitter, WebviewWindow};
use tokio::sync::Mutex;

use crate::ai_service::service::SharedAIService;
use crate::db::managers::save_repo::SaveRepo;

const AUTO_SAVE_PREFIX: &str = "自动存档";
const AUTO_SAVE_INTERVAL_SECS: u64 = 300; // 5 minutes
const EXIT_SAVE_TIMEOUT_SECS: u64 = 5;

/// Payload emitted to frontend after each successful auto-save.
#[derive(Debug, Clone, Serialize)]
struct AutoSaveEventPayload {
    save_id: i32,
    title: String,
    timestamp: String,
}

pub struct AutoSaveManager {
    app: AppHandle,
    db: DatabaseConnection,
    ai_service: SharedAIService,
    /// Hash of line_list at the moment of the last successful auto-save.
    last_saved_hash: Option<u64>,
    /// Resolved auto-save slot ID (lazily found or created on first save).
    auto_save_id: Option<i32>,
}

impl AutoSaveManager {
    pub fn new(app: AppHandle, db: DatabaseConnection, ai_service: SharedAIService) -> Self {
        Self {
            app,
            db,
            ai_service,
            last_saved_hash: None,
            auto_save_id: None,
        }
    }

    // ========== Periodic Loop ==========

    /// Run the periodic auto-save loop (every 5 minutes).  Never returns.
    pub async fn run_periodic(manager: Arc<Mutex<Self>>) {
        let mut interval = tokio::time::interval(Duration::from_secs(AUTO_SAVE_INTERVAL_SECS));
        loop {
            interval.tick().await;
            let mut mgr = manager.lock().await;
            if let Err(e) = mgr.perform_save().await {
                tracing::warn!("[AutoSave] 自动存档失败: {}", e);
            }
        }
    }

    // ========== Close Handler ==========

    /// Register a close-requested handler on the main window that performs a
    /// final auto-save before allowing the window to actually close.
    pub fn setup_close_handler(
        app: AppHandle,
        window: WebviewWindow,
        manager: Arc<Mutex<Self>>,
    ) {
        window.clone().on_window_event(move |event| {
            if let tauri::WindowEvent::CloseRequested { api, .. } = event {
                // Prevent the window from closing immediately
                api.prevent_close();

                let mgr = manager.clone();
                let ah = app.clone();

                tauri::async_runtime::spawn(async move {
                    tracing::info!("[AutoSave] 正在执行退出前自动存档...");

                    let save_result = tokio::time::timeout(
                        Duration::from_secs(EXIT_SAVE_TIMEOUT_SECS),
                        async {
                            let mut mgr = mgr.lock().await;
                            mgr.perform_exit_save().await
                        },
                    )
                    .await;

                    match save_result {
                        Ok(Ok(())) => tracing::info!("[AutoSave] 退出前存档完成"),
                        Ok(Err(ref e)) => tracing::error!("[AutoSave] 退出前存档失败: {}", e),
                        Err(_) => tracing::warn!("[AutoSave] 退出前存档超时（{} 秒），放弃等待", EXIT_SAVE_TIMEOUT_SECS),
                    }

                    // Drop the manager lock before exiting
                    drop(save_result);

                    // 退出整个应用程序（destroy 只会销毁窗口，不会结束进程）
                    let _ = ah.exit(0);
                });
            }
        });
    }

    // ========== Core Save Logic ==========

    /// Perform a save if line_list is non-empty and has changed since last save.
    async fn perform_save(&mut self) -> Result<(), String> {
        // 1. Compute current hash (returns None if line_list is empty)
        let current_hash = self.compute_line_hash().await;

        let current_hash = match current_hash {
            Some(h) => h,
            None => {
                // line_list is empty — nothing to save
                return Ok(());
            }
        };

        // 2. Skip if unchanged since last save
        if self.last_saved_hash == Some(current_hash) {
            return Ok(());
        }

        // 3. Find or create the auto-save slot
        let save_id = self.find_or_create_slot().await?;

        // 4. Perform the actual save
        let mut service = self.ai_service.lock().await;
        let lines = service.game_status.lock().await.line_list.clone();

        // 4a. Sync lines (smart diff)
        SaveRepo::sync_lines(&self.db, save_id, &lines)
            .await
            .map_err(|e| format!("同步台词失败: {}", e))?;

        // 4b. Set active save
        service.game_status.lock().await.active_save_id = Some(save_id);

        // 4c. Write GameStatus snapshot
        let snapshot = service.game_status.lock().await.to_snapshot();
        let snapshot_json =
            serde_json::to_string(&snapshot).map_err(|e| format!("序列化状态失败: {}", e))?;
        SaveRepo::update_save_status(&self.db, save_id, &snapshot_json)
            .await
            .map_err(|e| format!("保存状态失败: {}", e))?;

        // 4d. Persist memory banks
        service
            .persist_memory_banks(save_id)
            .await
            .map_err(|e| format!("保存记忆库失败: {}", e))?;

        // 4e. Persist script state (if running)
        if let Some(ref script_status) = service.game_status.lock().await.script_status {
            let vars_json = serde_json::to_string(&script_status.vars).unwrap_or_default();
            let _ = SaveRepo::upsert_running_script(
                &self.db,
                save_id,
                &script_status.folder_key,
                &vars_json,
                &script_status.current_chapter_key,
                script_status.current_event_process,
            )
            .await
            .map_err(|e| {
                tracing::warn!("[AutoSave] 保存剧本状态失败: {}", e);
            });
        }

        drop(service);

        // 5. Update tracking state
        self.last_saved_hash = Some(current_hash);

        // 6. Emit event to frontend
        let now = Local::now();
        let title = format!("{} {}", AUTO_SAVE_PREFIX, now.format("%Y-%m-%d %H:%M:%S"));
        let timestamp = now.format("%H:%M:%S").to_string();

        let _ = self.app.emit(
            "save:auto-saved",
            AutoSaveEventPayload {
                save_id,
                title,
                timestamp,
            },
        );

        tracing::info!("[AutoSave] 自动存档完成 save_id={}", save_id);
        Ok(())
    }

    /// Exit save: force a save regardless of change detection.
    async fn perform_exit_save(&mut self) -> Result<(), String> {
        // Reset hash to force save even if nothing changed
        self.last_saved_hash = None;
        self.perform_save().await
    }

    // ========== Helpers ==========

    /// Compute a hash of the current line_list contents.
    /// Returns `None` if the list is empty (nothing to save).
    async fn compute_line_hash(&self) -> Option<u64> {
        let service = self.ai_service.lock().await;
        let lines = &service.game_status.lock().await.line_list;

        // 初始化时 line_list 自带一条 system 台词（角色人设），
        // 只有大于 1 条时才说明有实际对话发生，才需要自动存档。
        if lines.len() <= 1 {
            return None;
        }

        let mut hasher = DefaultHasher::new();
        for line in lines {
            line.base.content.hash(&mut hasher);
            line.base.sender_role_id.hash(&mut hasher);
            line.base.attribute.as_str().hash(&mut hasher);
        }

        Some(hasher.finish())
    }

    /// Find the existing auto-save slot by title prefix, or create a new one.
    /// Updates the title with the current timestamp.
    async fn find_or_create_slot(&mut self) -> Result<i32, String> {
        // Try to find an existing auto-save by prefix
        // Read current main_role_id once (used in both branches)
        let main_id = {
            let service = self.ai_service.lock().await;
            let gs = service.game_status.lock().await;
            gs.main_role_id
        };

        if let Ok(Some(existing)) =
            SaveRepo::find_save_by_title_prefix(&self.db, AUTO_SAVE_PREFIX).await
        {
            let save_id = existing.id;
            let new_title = format!(
                "{} {}",
                AUTO_SAVE_PREFIX,
                Local::now().format("%Y-%m-%d %H:%M:%S")
            );
            SaveRepo::update_save_title(&self.db, save_id, &new_title)
                .await
                .map_err(|e| format!("更新自动存档标题失败: {}", e))?;
            // 每次存档都同步 main_role_id，防止切角色后指向旧角色
            SaveRepo::update_save_main_role(&self.db, save_id, main_id)
                .await
                .map_err(|e| format!("设置主角失败: {}", e))?;
            self.auto_save_id = Some(save_id);
            return Ok(save_id);
        }

        // Create a new auto-save slot
        let title = format!(
            "{} {}",
            AUTO_SAVE_PREFIX,
            Local::now().format("%Y-%m-%d %H:%M:%S")
        );
        let model = SaveRepo::create_save(&self.db, &title)
            .await
            .map_err(|e| format!("创建自动存档失败: {}", e))?;
        let save_id = model.id;

        SaveRepo::update_save_main_role(&self.db, save_id, main_id)
            .await
            .map_err(|e| format!("设置主角失败: {}", e))?;

        self.auto_save_id = Some(save_id);
        Ok(save_id)
    }
}
