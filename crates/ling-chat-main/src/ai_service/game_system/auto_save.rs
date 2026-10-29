use std::hash::{DefaultHasher, Hash, Hasher};
use std::sync::Arc;
use std::time::{Duration, Instant};

use chrono::Local;
use sea_orm::DatabaseConnection;
use serde::Serialize;
use tauri::{AppHandle, Emitter, WebviewWindow};
use tokio::sync::Mutex;

use crate::ai_service::service::SharedAIService;
use crate::ai_service::types::GameLine;
use crate::config::AppConfig;
use crate::db::entities::line::LineAttribute;
use crate::db::managers::save_repo::SaveRepo;

const AUTO_SAVE_PREFIX: &str = "自动存档";
const EXIT_SAVE_TIMEOUT_SECS: u64 = 5;
const ENTER_GRACE_SECS: u64 = 60;

/// 一行是否属于「真实对话」——玩家发言或角色回复。
/// 人设 system 行、旁白/系统/剧情提示（内容以 [`NARRATION_TAG`] 开头）一律不计，
/// TODO+钦灵：这地方好像把自动存档中的辅助台词给删了？日后我检查一下
pub(crate) fn is_real_dialogue(line: &GameLine) -> bool {
    match line.attribute() {
        // 角色回复：assistant 且归属某角色（工具调用的空内容载体行 sender 为空，排除）
        LineAttribute::Assistant => line.base.sender_role_id.is_some(),
        // 玩家发言：sender_role_id==0（DB 不变量）且非旁白/系统/剧情内容
        LineAttribute::User => {
            line.base.sender_role_id == Some(0)
                && !line
                    .base
                    .content
                    .starts_with(crate::utils::prompt::NARRATION_TAG)
        },
        _ => false,
    }
}

/// 计算「真实对话」内容指纹；无真实对话返回 `None`。
/// 抽出为自由函数，供载入/手动存档路径在持有 `game_status` 锁时复用，避免与
/// 定时循环（manager→ai_service）反向取锁造成死锁。
pub fn hash_of_real_lines(lines: &[GameLine]) -> Option<u64> {
    let mut hasher = DefaultHasher::new();
    let mut real = 0u64;
    for line in lines {
        if !is_real_dialogue(line) {
            continue;
        }
        real += 1;
        line.base.content.hash(&mut hasher);
        line.base.sender_role_id.hash(&mut hasher);
        line.base.attribute.as_str().hash(&mut hasher);
    }
    if real == 0 {
        None
    } else {
        Some(hasher.finish())
    }
}

/// Payload emitted to frontend after each successful auto-save.
#[derive(Debug, Clone, Serialize)]
struct AutoSaveEventPayload {
    save_id: i32,
    title: String,
    timestamp: String,
}

#[derive(Clone, serde::Serialize)]
pub struct AutoSaveFailedPayload {
    pub message: String,
    pub timestamp: String,
}

pub struct AutoSaveManager {
    app: AppHandle,
    db: DatabaseConnection,
    ai_service: SharedAIService,
    /// Hash of line_list at the moment of the last successful auto-save.
    last_saved_hash: Option<u64>,
    /// Resolved auto-save slot ID (lazily found or created on first save).
    auto_save_id: Option<i32>,
    /// 进入新一局的冷却截止时刻，见 [`ENTER_GRACE_SECS`]。
    grace_until: Option<Instant>,
}

impl AutoSaveManager {
    pub fn new(app: AppHandle, db: DatabaseConnection, ai_service: SharedAIService) -> Self {
        Self {
            app,
            db,
            ai_service,
            last_saved_hash: None,
            auto_save_id: None,
            grace_until: None,
        }
    }

    // ========== Periodic Loop ==========

    /// Run the periodic auto-save loop.  Never returns.
    ///
    /// 每轮先 sleep 再存：避免 `tokio::time::interval` 首次 `tick()` 立即返回导致启动抢跑，
    /// 并让开关与间隔每轮热重读（改设置无需重启）。
    pub async fn run_periodic(manager: Arc<Mutex<Self>>) {
        loop {
            let (enabled, interval_secs) = {
                let mgr = manager.lock().await;
                let cfg = AppConfig::load(&mgr.app).unwrap_or_default();
                (cfg.auto_save_enabled, cfg.auto_save_interval_secs)
            };

            tokio::time::sleep(Duration::from_secs(interval_secs as u64)).await;

            if !enabled {
                continue;
            }

            let mut mgr = manager.lock().await;
            if let Err(e) = mgr.perform_save().await {
                tracing::warn!("[AutoSave] 自动存档失败: {e}");
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

    /// Perform a save only when there is real dialogue whose content changed since last save.
    async fn perform_save(&mut self) -> Result<(), String> {
        // 0. 开关：关闭时定时与退出存档都跳过
        let cfg = AppConfig::load(&self.app).unwrap_or_default();
        if !cfg.auto_save_enabled {
            return Ok(());
        }

        // 0b. 冷却期：刚进入一局，先别动上一局的自动存档
        if let Some(until) = self.grace_until {
            if Instant::now() < until {
                return Ok(());
            }
        }

        // 1. Compute current hash (returns None if there is no real dialogue)
        let current_hash = self.compute_line_hash().await;

        let current_hash = match current_hash {
            Some(h) => h,
            None => {
                // 无真实对话（主菜单/仅人设/仅旁白）— 绝不覆盖旧档
                return Ok(());
            },
        };

        // 2. Skip if unchanged since last save
        if self.last_saved_hash == Some(current_hash) {
            return Ok(());
        }

        // 3. 解析写目标（方案 A）：已绑档优先，否则解析自动存档位。
        //    已存在的档位此刻只取 id 不写库：标题/主角/时间戳要等台词写成功之后再刷
        //    （见 4b），否则写失败会把旧档刷新成"刚刚存过"的样子。
        let (bound, lines, main_id) = {
            let service = self.ai_service.lock().await;
            let gs = service.game_status.lock().await;
            (gs.active_save_id, gs.line_list.clone(), gs.main_role_id)
        };
        let (save_id, bound_target) = self.resolve_save_target(bound, &lines).await?;

        // 4. Perform the actual save
        let mut service = self.ai_service.lock().await;

        // 4a. Sync lines (smart diff)。自动存档位只跟踪当前这一局，允许在无共同锚点
        //     （切过角色、开过新档）时整体接管重建，而不是永久拒绝写入。
        if let Err(e) = SaveRepo::sync_lines(&self.db, save_id, &lines, true).await {
            let _ = self.app.emit(
                "save:auto-save-failed",
                AutoSaveFailedPayload {
                    message: e.to_string(),
                    timestamp: Local::now().format("%H:%M:%S").to_string(),
                },
            );
            return Err(format!("同步台词失败: {}", e));
        }

        // 4b. 台词落库成功后再刷新档位元数据，与内容同进同退；
        //     写的是绑定档时保持用户自己的标题与主角（与快速存档一致）。
        let now = Local::now();
        let title = format!("{} {}", AUTO_SAVE_PREFIX, now.format("%Y-%m-%d %H:%M:%S"));
        if !bound_target {
            SaveRepo::update_save_title(&self.db, save_id, &title)
                .await
                .map_err(|e| format!("更新自动存档标题失败: {}", e))?;
            SaveRepo::update_save_main_role(&self.db, save_id, main_id)
                .await
                .map_err(|e| format!("设置主角失败: {}", e))?;
        }

        // 4c. Set active save
        {
            let mut gs = service.game_status.lock().await;
            gs.active_save_id = Some(save_id);
            // 回填行 id：让后续锚点按 id 命中（重建人设行后仍能安全落档）。
            if let Err(e) = SaveRepo::bind_line_ids(&self.db, save_id, &mut gs.line_list).await {
                tracing::warn!("[AutoSave] 回填台词行 id 失败: {e}");
            }
        }

        // 记录本局使用的「我的身份」，否则读自动存档会沿用全局当前身份。
        {
            let identity_id = service.game_status.lock().await.player.identity_id.clone();
            if let Some(ref id) = identity_id {
                if !id.trim().is_empty() {
                    if let Err(e) = SaveRepo::upsert_save_identity(&self.db, save_id, id).await {
                        tracing::warn!("[AutoSave] 记录本局身份失败: {e}");
                    }
                }
            }
        }

        // 4d. Write GameStatus snapshot
        let snapshot = service.game_status.lock().await.to_snapshot();
        let snapshot_json =
            serde_json::to_string(&snapshot).map_err(|e| format!("序列化状态失败: {}", e))?;
        SaveRepo::update_save_status(&self.db, save_id, &snapshot_json)
            .await
            .map_err(|e| format!("保存状态失败: {}", e))?;

        // 4e. Persist memory banks
        service
            .persist_memory_banks(save_id)
            .await
            .map_err(|e| format!("保存记忆库失败: {}", e))?;

        // 4f. Persist script state (if running)
        let script_status_snapshot = service.game_status.lock().await.script_status.clone();
        match script_status_snapshot {
            Some(script_status) => {
                let vars_json = serde_json::to_string(&script_status.vars).unwrap_or_default();
                // 与手动存档统一：写 path_key() 并归一化分隔符，读档时按同一规则回匹配
                let script_folder = script_status.path_key().replace('\\', "/");
                let _ = SaveRepo::upsert_running_script(
                    &self.db,
                    save_id,
                    &script_folder,
                    &vars_json,
                    &script_status.current_chapter_key,
                    script_status.current_event_process,
                )
                .await
                .map_err(|e| {
                    tracing::warn!("[AutoSave] 保存剧本状态失败: {}", e);
                });
            },
            None => {
                // 当前无剧本在跑：清掉该存档早年关联的旧进度行，避免读档误续跑已结束的剧本
                if let Err(e) = SaveRepo::clear_running_script(&self.db, save_id).await {
                    tracing::warn!("[AutoSave] 清理旧剧本状态失败: {}", e);
                }
            },
        }

        drop(service);

        // 5. Update tracking state
        self.last_saved_hash = Some(current_hash);

        // 6. Emit event to frontend
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

    /// Exit save: go through the same gate as the periodic save.
    /// 不再 reset 强制写：开关关闭 / 无真实对话 / 内容自上次存档未变化时跳过，
    /// 避免停在主菜单关程序也盖一次旧自动存档。
    async fn perform_exit_save(&mut self) -> Result<(), String> {
        self.perform_save().await
    }

    // ========== Helpers ==========

    /// 载入存档 / 手动存档成功后同步基线：把 `last_saved_hash` 设为已给定内容的 hash，
    /// 使下一次 tick 不会把刚载入（或刚落盘）的内容当成新变化重存一次而覆盖旧自动存档。
    pub fn set_baseline(&mut self, hash: Option<u64>) {
        self.last_saved_hash = hash;
    }

    /// 起算进入新一局的冷却。主菜单进游戏、切角色、清空对话时调用，
    /// 冷却期内不给自动存档，让玩家有机会先从存档列表读回上一局的自动存档。
    pub fn arm_grace(&mut self) {
        self.grace_until = Some(Instant::now() + Duration::from_secs(ENTER_GRACE_SECS));
    }

    /// Compute a hash of the current real-dialogue contents (see [`hash_of_real_lines`]).
    async fn compute_line_hash(&self) -> Option<u64> {
        let service = self.ai_service.lock().await;
        let lines = &service.game_status.lock().await.line_list;
        hash_of_real_lines(lines)
    }

    /// 解析自动存档写目标（方案 A）：已绑档优先——绑定档可安全写入时直接写用户
    /// 自己的档，不改标题、不改 main_role（与快速存档一致）；否则走上游的自动存档
    /// 位解析（已存在的槽只取 id，新建空白档期写标题与主角）。返回 (档 id, 是否绑定档)。
    async fn resolve_save_target(
        &mut self,
        bound: Option<i32>,
        lines: &[GameLine],
    ) -> Result<(i32, bool), String> {
        if let Some(id) = bound {
            let ok = SaveRepo::can_sync_into(&self.db, id, lines)
                .await
                .map_err(|e| format!("检查存档可写性失败: {e}"))?;
            if ok {
                return Ok((id, true));
            }
        }
        Ok((self.resolve_slot().await?, false))
    }

    /// 解析自动存档位：已有档位直接返回其 id，不动它的任何元数据（标题、主角、
    /// 时间戳统一由写台词成功后的 `perform_save` 4b 刷新）；没有档位则新建一位，
    /// 并写好标题与主角，保证刚建的空档也能被正常读回。
    async fn resolve_slot(&mut self) -> Result<i32, String> {
        // 按标题前缀找已有自动存档位
        if let Ok(Some(existing)) =
            SaveRepo::find_save_by_title_prefix(&self.db, AUTO_SAVE_PREFIX).await
        {
            self.auto_save_id = Some(existing.id);
            return Ok(existing.id);
        }

        // 新建档位：标题带当前时间，主角取当前主角色
        let main_id = {
            let service = self.ai_service.lock().await;
            let gs = service.game_status.lock().await;
            gs.main_role_id
        };
        let title = format!(
            "{} {}",
            AUTO_SAVE_PREFIX,
            Local::now().format("%Y-%m-%d %H:%M:%S")
        );
        let model = SaveRepo::create_save(&self.db, &title)
            .await
            .map_err(|e| format!("创建自动存档失败: {}", e))?;

        SaveRepo::update_save_main_role(&self.db, model.id, main_id)
            .await
            .map_err(|e| format!("设置主角失败: {}", e))?;

        self.auto_save_id = Some(model.id);
        Ok(model.id)
    }
}
