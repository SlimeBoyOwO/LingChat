pub mod manager;
pub mod triggers;
pub mod types;

use tauri::Emitter;
use tokio::sync::Mutex;

use crate::achievements::manager::AchievementManager;
use crate::achievements::types::Achievement;

/// 解锁一个成就并向前端广播 `achievement:unlocked` 事件。
///
/// 命令（`api/achievement.rs::unlock_achievement`）与剧本事件
/// （`unlock_achievement` 事件处理器）共用，保证两处行为一致：
/// 解锁即落盘（`AchievementManager::unlock` 内部保存）+ 推送成就弹窗。
/// 返回 None 表示成就不存在或已解锁（不重复广播）。
pub async fn unlock_and_emit(
    app: &tauri::AppHandle,
    manager: &Mutex<AchievementManager>,
    achievement_id: &str,
) -> Result<Option<Achievement>, String> {
    let mut mgr = manager.lock().await;
    if let Some(achievement) = mgr.unlock(achievement_id) {
        app.emit("achievement:unlocked", &achievement)
            .map_err(|e| format!("发送成就事件失败: {}", e))?;
        Ok(Some(achievement))
    } else {
        Ok(None)
    }
}
