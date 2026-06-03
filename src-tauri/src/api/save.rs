use serde::Serialize;
use tauri::{AppHandle, Manager};

use crate::ai_service::game_system::game_status::GameStatusSnapshot;
use crate::api::game::build_web_init_data;
use crate::api::game::WebInitData;
use crate::config::AppConfig;
use crate::db::managers::role_repo::RoleRepo;
use crate::db::managers::save_repo::SaveRepo;
use crate::utils::prompt::PromptOptions;
use crate::AppState;

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
    let screenshots_dir = super::data_dir().join("screenshots");
    std::fs::create_dir_all(&screenshots_dir).map_err(|e| e.to_string())?;
    let dest_path = screenshots_dir.join(format!("{}.png", save_id));
    std::fs::copy(source_path, &dest_path)
        .map_err(|e| format!("复制截图文件失败: {} → {:?}: {}", source_path, dest_path, e))?;
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
    let lines = service.game_status.lock().await.line_list.clone();

    // 1. 创建 save 行
    let save_model = SaveRepo::create_save(db, &title)
        .await
        .map_err(|e| format!("创建存档失败: {}", e))?;
    let save_id = save_model.id;

    // 复制截图到 screenshots 目录
    if let Some(ref path) = screenshot_path {
        let _ = save_screenshot_file(save_id, path).await;
    }

    // 2. 同步台词
    if !lines.is_empty() {
        SaveRepo::sync_lines(db, save_id, &lines)
            .await
            .map_err(|e| format!("同步台词失败: {}", e))?;
    }

    // 3. 设置主角
    if let Some(main_id) = service.game_status.lock().await.main_role_id {
        SaveRepo::update_save_main_role(db, save_id, Some(main_id))
            .await
            .map_err(|e| format!("设置主角失败: {}", e))?;
    }

    // 4. 写入 GameStatus 快照
    let snapshot = service.game_status.lock().await.to_snapshot();
    let snapshot_json =
        serde_json::to_string(&snapshot).map_err(|e| format!("序列化状态失败: {}", e))?;
    SaveRepo::update_save_status(db, save_id, &snapshot_json)
        .await
        .map_err(|e| format!("保存状态失败: {}", e))?;

    // 5. 标记当前活跃存档
    service.game_status.lock().await.active_save_id = Some(save_id);

    // 6. 持久化 MemoryBank
    service
        .persist_memory_banks(save_id)
        .await
        .map_err(|e| format!("保存记忆库失败: {}", e))?;

    // 7. 持久化剧本状态（若有）
    if let Some(ref script_status) = service.game_status.lock().await.script_status {
        let vars_json = serde_json::to_string(&script_status.vars).unwrap_or_default();
        let _ = SaveRepo::upsert_running_script(
            db,
            save_id,
            &script_status.folder_key,
            &vars_json,
            &script_status.current_chapter_key,
            script_status.current_event_process,
        )
        .await
        .map_err(|e| eprintln!("[SAVE_WARN] create_save: 保存剧本状态失败: {}", e));
    }

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

    // 6. 导入设定并载入台词
    service
        .import_settings(settings.clone(), prompt_options)
        .await;
    service
        .load_lines(line_list, main_role_id, Some(save_id))
        .await
        .map_err(|e| format!("载入台词失败: {}", e))?;

    // 7. 恢复 GameStatus 快照
    let snapshot: GameStatusSnapshot = serde_json::from_str(&save_model.status).unwrap_or_default();
    service.game_status.lock().await.apply_snapshot(&snapshot);

    // 8. 恢复 MemoryBank
    let _ = service
        .restore_memory_banks(save_id)
        .await
        .map_err(|e| eprintln!("[SAVE_WARN] 恢复记忆库失败: {}", e));

    // 9. 恢复剧本状态（若有）
    if let Some(rs_id) = save_model.running_script_id {
        let _ = SaveRepo::get_running_script(db, rs_id).await;
    }

    // 10. 返回前端初始化数据
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

    // 1. 校验存档存在
    SaveRepo::get_save_by_id(db, save_id)
        .await
        .map_err(|e| format!("查询存档失败: {}", e))?
        .ok_or_else(|| format!("存档 {} 不存在", save_id))?;

    // 复制截图到 screenshots 目录
    if let Some(ref path) = screenshot_path {
        let _ = save_screenshot_file(save_id, path).await;
    }

    let lines = service.game_status.lock().await.line_list.clone();

    // 2. 同步台词（智能 diff）
    SaveRepo::sync_lines(db, save_id, &lines)
        .await
        .map_err(|e| format!("同步台词失败: {}", e))?;

    // 3. 标记活跃存档
    service.game_status.lock().await.active_save_id = Some(save_id);

    // 4. 更新 GameStatus 快照
    let snapshot = service.game_status.lock().await.to_snapshot();
    let snapshot_json =
        serde_json::to_string(&snapshot).map_err(|e| format!("序列化状态失败: {}", e))?;
    SaveRepo::update_save_status(db, save_id, &snapshot_json)
        .await
        .map_err(|e| format!("保存状态失败: {}", e))?;

    // 5. 持久化 MemoryBank
    service
        .persist_memory_banks(save_id)
        .await
        .map_err(|e| format!("保存记忆库失败: {}", e))?;

    // 6. 持久化剧本状态
    if let Some(ref script_status) = service.game_status.lock().await.script_status {
        let vars_json = serde_json::to_string(&script_status.vars).unwrap_or_default();
        let _ = SaveRepo::upsert_running_script(
            db,
            save_id,
            &script_status.folder_key,
            &vars_json,
            &script_status.current_chapter_key,
            script_status.current_event_process,
        )
        .await
        .map_err(|e| eprintln!("[SAVE_WARN] update_save: 保存剧本状态失败: {}", e));
    }

    Ok(())
}

#[tauri::command]
pub async fn delete_save(app: AppHandle, save_id: i32) -> Result<(), String> {
    let state = app.state::<AppState>();
    let db = &state.db;

    let service = state.ai_service.lock().await;

    // 1. 删除 MemoryBank
    SaveRepo::delete_memory_banks_by_save(db, save_id)
        .await
        .map_err(|e| format!("删除记忆库失败: {}", e))?;

    // 2. 删除 running_script 关联（若有）
    if let Ok(Some(save_model)) = SaveRepo::get_save_by_id(db, save_id).await {
        if let Some(rs_id) = save_model.running_script_id {
            let _ = SaveRepo::delete_running_script(db, rs_id).await;
        }
    }

    // 删除关联的截图文件
    let screenshot_path = super::data_dir().join("screenshots").join(format!("{}.png", save_id));
    if screenshot_path.exists() {
        let _ = std::fs::remove_file(screenshot_path);
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
pub async fn update_save_title(
    app: AppHandle,
    save_id: i32,
    title: String,
) -> Result<(), String> {
    let state = app.state::<AppState>();
    let db = &state.db;
    SaveRepo::update_save_title(db, save_id, &title)
        .await
        .map_err(|e| format!("修改存档名称失败: {}", e))?;
    Ok(())
}

#[tauri::command]
pub async fn save_screenshot(
    save_id: i32,
    screenshot_path: String,
) -> Result<(), String> {
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
    let temp_path =
        temp_dir.join(format!("lingchat_screenshot_{}.png", std::process::id()));
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
