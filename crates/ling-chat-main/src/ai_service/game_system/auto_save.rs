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
use super::save_service::{PlayerAnchor, SaveService, SaveTarget};

const AUTO_SAVE_PREFIX: &str = "自动存档";
const EXIT_SAVE_TIMEOUT_SECS: u64 = 5;
const ENTER_GRACE_SECS: u64 = 60;

/// 一行是否属于「真实对话」——玩家发言或角色回复。
/// 人设 system 行、旁白/系统/剧情提示（内容以 [`NARRATION_TAG`] 开头）一律不计，
/// 因此主菜单（仅有占位/旁白行）算不出内容，自动存档不会误触发覆盖旧档。
/// 用内容前缀而非 display_name 判旁白：剧本作者可给旁白事件自定义展示名，会绕过 display_name。
/// 已知边角：试玩会话（`preview_generation`）期间若正好 tick，预览台词会被计为真实对话写进主槽，本次不处理。
pub(crate) fn is_real_dialogue(line: &GameLine) -> bool {
    match line.attribute() {
        // 角色回复：assistant 且归属某角色（工具调用回填的前缀行 sender 为空，排除）
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

pub struct AutoSaveManager {
    app: AppHandle,
    ai_service: SharedAIService,
    /// Hash of line_list at the moment of the last successful auto-save.
    last_saved_hash: Option<u64>,
    /// 进入新一局的冷却截止时刻，见 [`ENTER_GRACE_SECS`]。
    grace_until: Option<Instant>,
}

impl AutoSaveManager {
    pub fn new(app: AppHandle, _db: DatabaseConnection, ai_service: SharedAIService) -> Self {
        Self {
            app,
            ai_service,
            last_saved_hash: None,
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

        let now = Local::now();
        let title = format!("{} {}", AUTO_SAVE_PREFIX, now.format("%Y-%m-%d %H:%M:%S"));
        let target = SaveTarget::Auto { prefix: AUTO_SAVE_PREFIX.into(), title: title.clone() };
        let Some(outcome) = SaveService::save(
            &self.ai_service, target, self.last_saved_hash, PlayerAnchor::default(),
        ).await.map_err(|e| format!("自动存档失败: {e:#}"))? else {
            return Ok(());
        };
        let save_id = outcome.save_id;
        self.last_saved_hash = outcome.baseline_hash;

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

}
