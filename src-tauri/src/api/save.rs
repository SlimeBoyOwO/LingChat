use std::path::Path;

use serde::Serialize;
use tauri::{AppHandle, Manager};

use crate::AppState;
use crate::ai_service::game_system::game_status::GameStatusSnapshot;
use crate::api::game::WebInitData;
use crate::api::game::build_web_init_data;
use crate::config::AppConfig;
use crate::db::managers::memory_repo::MemoryRepo;
use crate::db::managers::role_repo::RoleRepo;
use crate::db::managers::save_repo::SaveRepo;
use crate::utils::prompt::PromptOptions;

// ========== 响应类型 ==========

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "snake_case")]
pub struct SaveListItem {
    pub id: i32,
    pub title: String,
    pub create_date: String,
    pub update_date: String,
    pub last_message: Option<String>,
    pub screenshot: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "snake_case")]
pub struct SaveListResponse {
    pub saves: Vec<SaveListItem>,
    pub total: u64,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "snake_case")]
pub struct CreateSaveResponse {
    pub save_id: i32,
    pub message: String,
}

// ========== 辅助函数 ==========

fn format_datetime(dt: &chrono::NaiveDateTime) -> String {
    dt.and_utc().to_rfc3339()
}

async fn save_screenshot_file(save_id: i32, source_path: &str) -> Result<(), String> {
    save_screenshot_to_dir(save_id, source_path, &super::data_dir().join("screenshots"))
}

fn save_screenshot_to_dir(
    save_id: i32,
    source_path: &str,
    screenshots_dir: &Path,
) -> Result<(), String> {
    std::fs::create_dir_all(screenshots_dir).map_err(|e| e.to_string())?;
    let dest_path = screenshots_dir.join(format!("{}.png", save_id));
    std::fs::copy(source_path, &dest_path)
        .map_err(|e| format!("复制截图文件失败: {} → {:?}: {}", source_path, dest_path, e))?;
    Ok(())
}

/// Production save creation orchestration, factored so the preview boundary is
/// regression-tested together with its DB-row and screenshot side effects.
pub(crate) async fn create_save_for_session(
    db: &sea_orm::DatabaseConnection,
    service: &mut crate::ai_service::service::AIService,
    title: &str,
    screenshot_path: Option<&str>,
    screenshots_dir: &Path,
) -> Result<i32, String> {
    // Retain this guard until both the row/file effects and immutable session
    // write complete. A preview transition therefore cannot slip between a
    // preliminary check and the first formal side effect.
    let _formal_gate = service
        .acquire_formal_session_gate()
        .await
        .map_err(|error| error.to_string())?;
    let save_id = SaveRepo::create_save(db, title)
        .await
        .map_err(|e| format!("创建存档失败: {}", e))?
        .id;
    if let Some(path) = screenshot_path {
        let _ = save_screenshot_to_dir(save_id, path, screenshots_dir);
    }
    let snapshot = service.capture_guarded_session_snapshot().await;
    service
        .persist_captured_formal_session(save_id, &snapshot)
        .await
        .map_err(|e| format!("保存会话失败: {}", e))?;
    Ok(save_id)
}

/// Production update orchestration; preview is rejected before existence
/// checks, screenshot copying, or any session persistence.
pub(crate) async fn update_save_for_session(
    db: &sea_orm::DatabaseConnection,
    service: &mut crate::ai_service::service::AIService,
    save_id: i32,
    screenshot_path: Option<&str>,
    screenshots_dir: &Path,
) -> Result<(), String> {
    let _formal_gate = service
        .acquire_formal_session_gate()
        .await
        .map_err(|error| error.to_string())?;
    SaveRepo::get_save_by_id(db, save_id)
        .await
        .map_err(|e| format!("查询存档失败: {}", e))?
        .ok_or_else(|| format!("存档 {} 不存在", save_id))?;
    if let Some(path) = screenshot_path {
        let _ = save_screenshot_to_dir(save_id, path, screenshots_dir);
    }
    let snapshot = service.capture_guarded_session_snapshot().await;
    service
        .persist_captured_formal_session(save_id, &snapshot)
        .await
        .map_err(|e| format!("保存会话失败: {}", e))?;
    Ok(())
}

// ========== Tauri 命令 ==========

#[tauri::command]
pub async fn list_saves(
    app: AppHandle,
    page: Option<u64>,
    page_size: Option<u64>,
) -> Result<SaveListResponse, String> {
    let state = app.state::<AppState>();
    let db = &state.db;
    let page = page.unwrap_or(1);
    let page_size = page_size.unwrap_or(50);

    let total = SaveRepo::count_saves(db)
        .await
        .map_err(|e| format!("查询存档总数失败: {}", e))?;

    let saves = SaveRepo::list_saves(db, page, page_size)
        .await
        .map_err(|e| format!("查询存档列表失败: {}", e))?;

    // 1. 获取所有 last_message_id 并批量查询内容
    let last_msg_ids: Vec<i32> = saves.iter().filter_map(|s| s.last_message_id).collect();
    let mut lines_map = std::collections::HashMap::new();
    if !last_msg_ids.is_empty() {
        use crate::db::entities::line;
        use sea_orm::entity::prelude::*;
        if let Ok(lines) = line::Entity::find()
            .filter(line::Column::Id.is_in(last_msg_ids))
            .all(db)
            .await
        {
            for l in lines {
                lines_map.insert(l.id, l.content);
            }
        }
    }

    let data_dir = super::data_dir();
    let screenshots_dir = data_dir.join("screenshots");

    let items: Vec<SaveListItem> = saves
        .into_iter()
        .map(|s| {
            let last_message = s.last_message_id.and_then(|id| lines_map.get(&id).cloned());
            let screenshot_path = screenshots_dir.join(format!("{}.png", s.id));
            let screenshot = if screenshot_path.exists() {
                Some(screenshot_path.to_string_lossy().to_string())
            } else {
                None
            };

            SaveListItem {
                id: s.id,
                title: s.title,
                create_date: format_datetime(&s.create_date),
                update_date: format_datetime(&s.update_date),
                last_message,
                screenshot,
            }
        })
        .collect();

    Ok(SaveListResponse {
        saves: items,
        total,
    })
}

#[tauri::command]
pub async fn create_save(
    app: AppHandle,
    title: String,
    screenshot_path: Option<String>,
) -> Result<CreateSaveResponse, String> {
    let state = app.state::<AppState>();
    let db = &state.db;
    let mut service = state.ai_service.lock().await;
    let save_id = create_save_for_session(
        db,
        &mut service,
        &title,
        screenshot_path.as_deref(),
        &super::data_dir().join("screenshots"),
    )
    .await?;

    Ok(CreateSaveResponse {
        save_id,
        message: "存档创建成功".into(),
    })
}

#[tauri::command]
pub async fn load_save(app: AppHandle, save_id: i32) -> Result<WebInitData, String> {
    let state = app.state::<AppState>();
    let db = &state.db;

    let mut service = state.ai_service.lock().await;
    // Loading replaces runtime history/banks and imports settings; reject it
    // before any DB/runtime/import side effect while a preview owns GameStatus.
    let _formal_gate = service
        .acquire_formal_session_gate()
        .await
        .map_err(|error| error.to_string())?;

    // 1. 获取存档
    let save_model = SaveRepo::get_save_by_id(db, save_id)
        .await
        .map_err(|e| format!("查询存档失败: {}", e))?
        .ok_or_else(|| format!("存档 {} 不存在", save_id))?;

    // 2. 获取台词列表
    let line_list = SaveRepo::get_gameline_list(db, save_id)
        .await
        .map_err(|e| format!("读取台词失败: {}", e))?;

    // 3. 获取主角 role_id
    let main_role_id = save_model
        .main_role_id
        .ok_or_else(|| "存档中未记录主角信息".to_string())?;

    // 4. 加载角色设定
    let data_dir = crate::api::data_dir();
    let settings = RoleRepo::get_role_settings_by_id(db, &data_dir, main_role_id)
        .await
        .map_err(|e| format!("查询角色配置失败: {}", e))?
        .unwrap_or_else(|| {
            let mut s = crate::ai_service::types::CharacterSettings::default();
            s.character_id = Some(main_role_id);
            s
        });

    // 5. 构建 PromptOptions
    let app_config = AppConfig::load(&app).unwrap_or_default();
    let prompt_options = PromptOptions {
        output_sec_lang: app_config.llm_output_sec_lang,
        no_emotion_limit: app_config.no_emotion_limit_prompt,
    };

    // 6. 导入设定
    service
        .import_settings(settings.clone(), prompt_options)
        .await;

    // 7. 先恢复 MemoryBank：若在 load_lines（内部 sync_memories 会触发压缩检查）之后再恢复，
    //    后台全量重压会用旧库/旧指针把 DB 恢复的记忆库覆盖掉。
    service
        .restore_memory_banks(save_id)
        .await
        .map_err(|e| format!("恢复记忆库失败: {}", e))?;

    // 8. 载入台词（sync_memories 用恢复后的正确指针，只压缩存档点之后的增量）
    service
        .load_lines(line_list, main_role_id, Some(save_id))
        .await
        .map_err(|e| format!("载入台词失败: {}", e))?;

    // 9. 恢复 GameStatus 快照
    let snapshot: GameStatusSnapshot = serde_json::from_str(&save_model.status).unwrap_or_default();
    service.game_status.lock().await.apply_snapshot(&snapshot);

    // 10. 恢复剧本状态（若有）
    if let Some(rs_id) = save_model.running_script_id {
        let _ = SaveRepo::get_running_script(db, rs_id).await;
    }

    // 11. 返回前端初始化数据
    build_web_init_data(&service, &app).await
}

#[tauri::command]
pub async fn update_save(
    app: AppHandle,
    save_id: i32,
    screenshot_path: Option<String>,
) -> Result<(), String> {
    let state = app.state::<AppState>();
    let db = &state.db;
    let mut service = state.ai_service.lock().await;
    update_save_for_session(
        db,
        &mut service,
        save_id,
        screenshot_path.as_deref(),
        &super::data_dir().join("screenshots"),
    )
    .await
}

#[tauri::command]
pub async fn delete_save(app: AppHandle, save_id: i32) -> Result<(), String> {
    let state = app.state::<AppState>();
    let db = &state.db;

    let service = state.ai_service.lock().await;
    let _formal_gate = service
        .acquire_formal_session_gate()
        .await
        .map_err(|error| error.to_string())?;

    // 1. 删除 MemoryBank
    MemoryRepo::delete_for_save(db, save_id)
        .await
        .map_err(|e| format!("删除记忆库失败: {}", e))?;

    // 2. Delete every legacy running-script row for this save. Do not discard
    // repository errors: partial cleanup must be observable to the caller.
    SaveRepo::clear_running_script_for_save(db, save_id)
        .await
        .map_err(|e| format!("删除运行剧本失败: {}", e))?;

    // 删除关联的截图文件；a file-system failure is likewise not safe to hide.
    let screenshot_path = super::data_dir()
        .join("screenshots")
        .join(format!("{}.png", save_id));
    if screenshot_path.exists() {
        std::fs::remove_file(&screenshot_path).map_err(|e| format!("删除存档截图失败: {}", e))?;
    }

    // 3. 删除存档（级联删除关联的 line / line_perception）
    let deleted = SaveRepo::delete_save(db, save_id)
        .await
        .map_err(|e| format!("删除存档失败: {}", e))?;

    if !deleted {
        return Err(format!("存档 {} 不存在", save_id));
    }

    // 4. 若当前活跃存档是被删除的，清除标记
    if service.game_status.lock().await.active_save_id == Some(save_id) {
        service.game_status.lock().await.active_save_id = None;
    }

    Ok(())
}

#[tauri::command]
pub async fn update_save_title(app: AppHandle, save_id: i32, title: String) -> Result<(), String> {
    let state = app.state::<AppState>();
    let db = &state.db;
    let service = state.ai_service.lock().await;
    let _formal_gate = service
        .acquire_formal_session_gate()
        .await
        .map_err(|error| error.to_string())?;
    SaveRepo::update_save_title(db, save_id, &title)
        .await
        .map_err(|e| format!("修改存档名称失败: {}", e))?;
    Ok(())
}

#[tauri::command]
pub async fn save_screenshot(
    app: AppHandle,
    save_id: i32,
    screenshot_path: String,
) -> Result<(), String> {
    let state = app.state::<AppState>();
    let service = state.ai_service.lock().await;
    let _formal_gate = service
        .acquire_formal_session_gate()
        .await
        .map_err(|error| error.to_string())?;
    save_screenshot_file(save_id, &screenshot_path).await
}

/// 直接通过 HWND 截图主窗口（Windows）。
///
/// 跳过所有窗口枚举（`EnumWindows` / `Window::all()`），
/// 用 Tauri 拿到的原生 HWND 直接 GDI 截图 → 写入临时 PNG → 返回路径。
#[cfg(target_os = "windows")]
#[tauri::command]
pub async fn capture_main_window_screenshot(app: AppHandle) -> Result<String, String> {
    let window = app
        .get_webview_window("main")
        .ok_or_else(|| "main window not found".to_string())?;

    let hwnd = window
        .hwnd()
        .map_err(|e| format!("获取窗口句柄失败: {}", e))?;

    // HWND.0 → *mut c_void → usize → u32（Windows 句柄是 32 位值）
    let id = hwnd.0 as usize as u32;

    let image = tauri_plugin_screenshots::windows::capture_own_window(id)?;

    let temp_dir = std::env::temp_dir();
    let temp_path = temp_dir.join(format!("lingchat_screenshot_{}.png", std::process::id()));
    image
        .save(&temp_path)
        .map_err(|e| format!("保存截图失败: {}", e))?;

    tracing::info!(
        "[capture_main_window_screenshot] Captured → {}",
        temp_path.display()
    );
    Ok(temp_path.to_string_lossy().to_string())
}

/// 非 Windows 平台的占位实现（该命令始终可注册，但在非 Windows 上返回错误）。
#[cfg(not(target_os = "windows"))]
#[tauri::command]
pub async fn capture_main_window_screenshot(_app: AppHandle) -> Result<String, String> {
    Err("capture_main_window_screenshot is only available on Windows".to_string())
}
