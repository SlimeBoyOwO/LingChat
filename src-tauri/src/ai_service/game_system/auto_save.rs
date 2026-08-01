use std::hash::{DefaultHasher, Hash, Hasher};
use std::sync::Arc;
use std::time::Duration;

use chrono::Local;
use sea_orm::{DatabaseConnection, TransactionTrait};
use serde::Serialize;
use tauri::{AppHandle, Emitter, WebviewWindow};
use tokio::sync::Mutex;

use crate::ai_service::service::SharedAIService;
use crate::db::managers::save_repo::{SaveRepo, AUTO_SAVE_PREFIX};

// 台词已改为逐条落盘（见 game_status::add_line），周期自动存档只兜底
// 快照/记忆库/剧本变量/截图，不需要 5 分钟一次，120s 足够
const AUTO_SAVE_INTERVAL_SECS: u64 = 120;
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
    /// 已写入 settings.json 的"当前进行"槽 id（只有槽变化才重写文件，避免每 120s 全量写）。
    persisted_save_id: Option<i32>,
}

impl AutoSaveManager {
    pub fn new(app: AppHandle, db: DatabaseConnection, ai_service: SharedAIService) -> Self {
        Self {
            app,
            db,
            ai_service,
            last_saved_hash: None,
            persisted_save_id: None,
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
    pub fn setup_close_handler(app: AppHandle, window: WebviewWindow, manager: Arc<Mutex<Self>>) {
        window.clone().on_window_event(move |event| {
            if let tauri::WindowEvent::CloseRequested { api, .. } = event {
                // Prevent the window from closing immediately
                api.prevent_close();

                let mgr = manager.clone();
                let ah = app.clone();

                tauri::async_runtime::spawn(async move {
                    tracing::info!("[AutoSave] 正在执行退出前自动存档...");

                    let save_result =
                        tokio::time::timeout(Duration::from_secs(EXIT_SAVE_TIMEOUT_SECS), async {
                            let mut mgr = mgr.lock().await;
                            mgr.perform_exit_save().await
                        })
                        .await;

                    match save_result {
                        Ok(Ok(())) => tracing::info!("[AutoSave] 退出前存档完成"),
                        Ok(Err(ref e)) => tracing::error!("[AutoSave] 退出前存档失败: {}", e),
                        Err(_) => tracing::warn!(
                            "[AutoSave] 退出前存档超时（{} 秒），放弃等待",
                            EXIT_SAVE_TIMEOUT_SECS
                        ),
                    }

                    // Drop the manager lock before exiting
                    drop(save_result);

                    // 通知前端存档已完成，由前端决定是否退出
                    let _ = ah.emit("app:close-ready", ());
                });
            }
        });
    }

    // ========== Core Save Logic ==========

    /// Perform a save if line_list is non-empty and has changed since last save.
    pub(crate) async fn perform_save(&mut self) -> Result<(), String> {
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

        let mut service = self.ai_service.lock().await;

        // 3. 确定写入目标：galgame 语义下，自动保存永远写"当前进行"槽 active_save_id，
        //    不再另开一个平行"自动存档"槽去抢 active_save_id（旧行为会让手动档/读档对象
        //    在 120s 后被搬家，变成过期半截）。没有当前进行（新世界第一条对话）时才创建
        //    当前角色的自动槽。
        let (save_id, is_auto_slot) = {
            let mut gs = service.game_status.lock().await;
            let mut save_id = match gs.active_save_id {
                Some(id) => id,
                None => {
                    let id = SaveRepo::find_or_create_auto_save_slot(&self.db, gs.main_role_id)
                        .await
                        .map_err(|e| format!("查找/创建自动存档槽失败: {}", e))?;
                    gs.active_save_id = Some(id);
                    id
                }
            };
            // 目标槽可能已被用户删除 → 回退到当前角色的自动槽
            let is_auto_slot = match SaveRepo::get_save_by_id(&self.db, save_id)
                .await
                .map_err(|e| format!("查询存档失败: {}", e))?
            {
                Some(m) => m.title.starts_with(AUTO_SAVE_PREFIX),
                None => {
                    let id = SaveRepo::find_or_create_auto_save_slot(&self.db, gs.main_role_id)
                        .await
                        .map_err(|e| format!("重建自动存档槽失败: {}", e))?;
                    save_id = id;
                    gs.active_save_id = Some(id);
                    true
                }
            };
            (save_id, is_auto_slot)
        };

        // 4. 一次性读取台词、快照、主角（减少锁持有时长）
        let (lines, snapshot, main_role_id) = {
            let gs = service.game_status.lock().await;
            (gs.line_list.clone(), gs.to_snapshot(), gs.main_role_id)
        };

        // 5. 事务：同步台词（智能 diff）+ 写入快照，整体原子
        let txn = self
            .db
            .begin()
            .await
            .map_err(|e| format!("开启事务失败: {}", e))?;
        SaveRepo::sync_lines(&txn, save_id, &lines)
            .await
            .map_err(|e| format!("同步台词失败: {}", e))?;

        let snapshot_json =
            serde_json::to_string(&snapshot).map_err(|e| format!("序列化状态失败: {}", e))?;
        SaveRepo::update_save_status(&txn, save_id, &snapshot_json)
            .await
            .map_err(|e| format!("保存状态失败: {}", e))?;
        txn.commit()
            .await
            .map_err(|e| format!("提交事务失败: {}", e))?;

        // 6. 只对自动槽做"会话性维护"（刷新标题时间戳 + 对齐主角）；
        //    用户命名/读档的槽保持原样，标题不能被自动刷新覆盖。
        if is_auto_slot {
            let new_title = format!(
                "{} {}",
                AUTO_SAVE_PREFIX,
                Local::now().format("%Y-%m-%d %H:%M:%S")
            );
            SaveRepo::update_save_title(&self.db, save_id, &new_title)
                .await
                .map_err(|e| format!("更新自动存档标题失败: {}", e))?;
            SaveRepo::update_save_main_role(&self.db, save_id, main_role_id)
                .await
                .map_err(|e| format!("设置主角失败: {}", e))?;
        }

        // 7. Persist memory banks
        service
            .persist_memory_banks(save_id)
            .await
            .map_err(|e| format!("保存记忆库失败: {}", e))?;

        // 8. Persist script state (if running)
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

        // 9. Update tracking state
        self.last_saved_hash = Some(current_hash);

        // 10. 记录"当前进行"（per-role），供启动/继续恢复。只有槽变化才写 settings.json，
        //     避免每 120s 全量重写一次文件。
        if self.persisted_save_id != Some(save_id) {
            if let Some(rid) = main_role_id {
                crate::config::set_last_save_id(&self.app, rid, save_id);
                self.persisted_save_id = Some(save_id);
            }
        }

        // 11. Emit event to frontend
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
    pub(crate) async fn perform_exit_save(&mut self) -> Result<(), String> {
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

}
