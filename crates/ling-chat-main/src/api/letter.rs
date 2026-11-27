//! 信纸：前端读取当前会话的信纸页、定位源文件与导出。

use serde::Serialize;
use tauri::AppHandle;

use crate::ai_service::tools::game_status_handle;
use crate::db::managers::save_letter_repo::{letter_path, read_letter};

#[derive(Serialize)]
pub struct LetterPage {
    pub id: String,
    pub content: String,
}

/// 只接受当前会话里的信纸 id，前端传来的 id 不会拼出别的路径。
async fn session_letter_path(app: &AppHandle, id: &str) -> Result<std::path::PathBuf, String> {
    let gs = game_status_handle(app).await;
    if !gs.lock().await.letters.iter().any(|item| item == id) {
        return Err(format!("信纸 {id} 不在当前会话中"));
    }
    Ok(letter_path(id))
}

/// 当前会话的全部信纸页（按页序）。读不到正文的页按空白页返回，保持页码不错位。
#[tauri::command]
pub async fn get_session_letters(app: AppHandle) -> Result<Vec<LetterPage>, String> {
    let ids = game_status_handle(&app).await.lock().await.letters.clone();
    Ok(ids
        .into_iter()
        .map(|id| {
            let content = read_letter(&id).unwrap_or_else(|e| {
                tracing::warn!("{e}");
                String::new()
            });
            LetterPage { id, content }
        })
        .collect())
}

#[tauri::command]
pub async fn get_letter_path(app: AppHandle, id: String) -> Result<String, String> {
    let path = session_letter_path(&app, &id).await?;
    Ok(path.to_string_lossy().into_owned())
}

/// 把信纸另存到用户选的位置。
#[tauri::command]
pub async fn export_letter(app: AppHandle, id: String, target: String) -> Result<(), String> {
    let path = session_letter_path(&app, &id).await?;
    std::fs::copy(&path, &target).map_err(|e| format!("导出信纸失败: {e}"))?;
    Ok(())
}
