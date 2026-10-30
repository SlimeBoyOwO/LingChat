use serde::Serialize;
use tauri::{AppHandle, Manager};

use crate::AppState;
use crate::ai_service::game_system::auto_save;
use crate::ai_service::game_system::game_status::GameStatusSnapshot;
use crate::ai_service::game_system::player_identity::IdentityStore;
use crate::api::game::WebInitData;
use crate::api::game::build_web_init_data;
use crate::config::AppConfig;
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
    pub identity_name: Option<String>,
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

fn resolve_script_progress(
    script_status: &crate::ai_service::types::ScriptStatus,
    player_chapter: Option<&str>,
    player_event_index: Option<i32>,
    player_line_count: Option<i32>,
    player_vars_json: Option<&str>,
) -> (String, String, i32, String) {
    let anchor = match (
        player_chapter,
        player_event_index,
        player_line_count,
        player_vars_json,
    ) {
        (Some(chapter), Some(event_index), Some(line_count), Some(vars_json)) => {
            serde_json::from_str::<serde_json::Map<String, serde_json::Value>>(vars_json)
                .ok()
                .map(|vars| (chapter.to_string(), event_index, line_count, vars))
        },
        _ => None,
    };

    let (chapter, event_index, vars) = match anchor {
        Some((chapter, event_index, line_count, mut vars)) => {
            // 记录玩家已读到的台词条数（内部键），读档侧据此截断台词，加载时会剔除。
            vars.insert("__line_count".to_string(), serde_json::json!(line_count));
            (chapter, event_index, vars)
        },
        None => (
            script_status.current_chapter_key.clone(),
            script_status.current_event_process,
            script_status.vars.clone(),
        ),
    };

    (
        script_status.path_key().replace('\\', "/"),
        chapter,
        event_index,
        serde_json::to_string(&vars).unwrap_or_default(),
    )
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

    // 身份名解析：一次查索引表 + 一次查绑定表（替代逐档查询的 N+1）。
    use crate::db::entities::{player_identity, save_identity};
    use sea_orm::entity::prelude::*;
    let identity_names: std::collections::HashMap<String, String> = player_identity::Entity::find()
        .all(db)
        .await
        .map_err(|e| format!("读取身份索引失败: {}", e))?
        .into_iter()
        .map(|m| (m.identity_id, m.name))
        .collect();
    let save_identity_names: std::collections::HashMap<i32, String> =
        save_identity::Entity::find()
            .all(db)
            .await
            .map_err(|e| format!("读取存档身份绑定失败: {}", e))?
            .into_iter()
            .filter_map(|m| {
                identity_names
                    .get(&m.identity_id)
                    .map(|name| (m.save_id, name.clone()))
            })
            .collect();

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
            let identity_name = save_identity_names.get(&s.id).cloned();

            SaveListItem {
                id: s.id,
                title: s.title,
                create_date: format_datetime(&s.create_date),
                update_date: format_datetime(&s.update_date),
                last_message,
                screenshot,
                identity_name,
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
    player_chapter: Option<String>,
    player_event_index: Option<i32>,
    player_line_count: Option<i32>,
    player_vars_json: Option<String>,
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

    {
        let identity_id = service.game_status.lock().await.player.identity_id.clone();
        if let Some(ref id) = identity_id {
            if !id.trim().is_empty() {
                SaveRepo::upsert_save_identity(db, save_id, id)
                    .await
                    .map_err(|e| format!("记录身份失败: {}", e))?;
            }
        }
    }

    // 复制截图到 screenshots 目录
    if let Some(ref path) = screenshot_path {
        let _ = save_screenshot_file(save_id, path).await;
    }

    // 2. 同步台词
    if !lines.is_empty() {
        SaveRepo::sync_lines(db, save_id, &lines, false)
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

    {
        let mut gs = service.game_status.lock().await;
        gs.active_save_id = Some(save_id);
        if let Err(e) = SaveRepo::bind_line_ids(db, save_id, &mut gs.line_list).await {
            tracing::warn!("回填台词行 id 失败: {e}");
        }
    }

    // 6. 持久化 MemoryBank
    service
        .persist_memory_banks(save_id)
        .await
        .map_err(|e| format!("保存记忆库失败: {}", e))?;

    // 7. 持久化剧本状态（若有）
    let script_status_snapshot = service.game_status.lock().await.script_status.clone();
    match script_status_snapshot {
        Some(script_status) => {
            let (script_folder, chapter, event_index, vars_json) = resolve_script_progress(
                &script_status,
                player_chapter.as_deref(),
                player_event_index,
                player_line_count,
                player_vars_json.as_deref(),
            );
            let _ = SaveRepo::upsert_running_script(
                db,
                save_id,
                &script_folder,
                &vars_json,
                &chapter,
                event_index,
            )
            .await
            .map_err(|e| eprintln!("[SAVE_WARN] create_save: 保存剧本状态失败: {}", e));
        },
        None => {
            // 当前无剧本在跑：清掉该存档早年关联的旧进度行，避免读档误续跑已结束的剧本
            if let Err(e) = SaveRepo::clear_running_script(db, save_id).await {
                eprintln!("[SAVE_WARN] create_save: 清理旧剧本状态失败: {}", e);
            }
        },
    }

    let baseline_hash = auto_save::hash_of_real_lines(&lines);
    drop(service);
    state
        .auto_save_manager
        .lock()
        .await
        .set_baseline(baseline_hash);

    Ok(CreateSaveResponse {
        save_id,
        message: "存档创建成功".into(),
    })
}

#[tauri::command]
pub async fn load_save(app: AppHandle, save_id: i32) -> Result<WebInitData, String> {
    let state = app.state::<AppState>();
    let db = &state.db;

    crate::api::script::abort_running_script(&app).await;

    let mut service = state.ai_service.lock().await;

    // 1. 获取存档
    let save_model = SaveRepo::get_save_by_id(db, save_id)
        .await
        .map_err(|e| format!("查询存档失败: {}", e))?
        .ok_or_else(|| format!("存档 {} 不存在", save_id))?;

    // 2. 读取剧本进度并构造恢复版 ScriptStatus（若有）
    let mut resume_line_count: Option<usize> = None;
    let resume_script: Option<crate::ai_service::types::ScriptStatus> =
        match save_model.running_script_id {
            Some(rs_id) => match SaveRepo::get_running_script(db, rs_id).await {
                Ok(Some(rs)) => {
                    // 统一分隔符：旧档可能存 Windows 反斜杠的 path_key，或旧版的 folder_key
                    let saved_key = rs.script_folder.replace('\\', "/");
                    let matched = service
                        .script_manager
                        .all_scripts
                        .values()
                        .find(|s| s.path_key().replace('\\', "/") == saved_key)
                        .or_else(|| {
                            service
                                .script_manager
                                .all_scripts
                                .values()
                                .find(|s| s.folder_key == saved_key)
                        })
                        .cloned();

                    match matched {
                        Some(mut script) => {
                            let mut vars = serde_json::from_str::<
                                serde_json::Map<String, serde_json::Value>,
                            >(&rs.variable_info)
                            .unwrap_or_default();
                            if let Some(v) = vars.remove("__line_count") {
                                resume_line_count = v.as_u64().map(|n| n as usize);
                            }
                            script.vars = vars;
                            script.current_chapter_key = rs.current_chapter;
                            script.current_event_process = rs.event_sequence;
                            Some(script)
                        },
                        None => {
                            tracing::warn!(
                                "[SaveAPI] 存档 {} 关联的剧本 '{}' 已不存在，降级为普通读档",
                                save_id,
                                rs.script_folder
                            );
                            None
                        },
                    }
                },
                Ok(None) => None,
                Err(e) => {
                    tracing::warn!("[SaveAPI] 读取剧本进度失败，降级为普通读档: {}", e);
                    None
                },
            },
            None => None,
        };

    // 3. 获取台词列表
    let mut line_list = SaveRepo::get_gameline_list(db, save_id)
        .await
        .map_err(|e| format!("读取台词失败: {}", e))?;

    if let Some(n) = resume_line_count {
        let keep = n.min(line_list.len());
        line_list.truncate(keep);
    }

    // 4. 获取主角 role_id
    let main_role_id = save_model
        .main_role_id
        .ok_or_else(|| "存档中未记录主角信息".to_string())?;

    // 5. 构建 PromptOptions
    let app_config = AppConfig::load(&app).unwrap_or_default();
    let prompt_options = PromptOptions::from_config(&app_config);

    // 6b. 恢复本存档使用的「我的身份」；必须放在 `init_game_status` 之前。
    // 老存档没有绑定记录时保持开局引导解析出的主身份。
    let save_identity_id = {
        let identity_store = IdentityStore::new(&crate::api::data_dir());
        match SaveRepo::get_save_identity(db, save_id).await {
            Ok(Some(id)) => {
                if identity_store.find_by_id(&id).is_some() {
                    if let Err(e) = identity_store.set_current_id(&id) {
                        tracing::warn!("恢复存档身份失败: {e}");
                    }
                    Some(id)
                } else {
                    tracing::warn!("存档引用的身份 {id} 不存在，沿用当前身份");
                    None
                }
            },
            Ok(None) => None,
            Err(e) => {
                tracing::warn!("读取存档身份失败: {e}");
                None
            },
        }
    };

    service
        .init_game_status(Some(main_role_id), prompt_options)
        .await
        .map_err(|e| format!("初始化游戏状态失败: {}", e))?;

    // 开局引导会优先按角色目录的默认身份装配；读档必须以 save_identity 为准，
    // 这里在载入台词前把本局身份覆盖回来，后续快照/好感度按这个身份解析。
    if let Some(id) = save_identity_id.as_deref() {
        let identity_store = IdentityStore::new(&crate::api::data_dir());
        if let Some(identity) = identity_store.find_by_id(id) {
            // 身份卡 prompt 为空时，沿用角色卡里抽出的「我的设定」，与开局引导的兜底一致。
            let fallback_prompt = {
                let gs = service.game_status.lock().await;
                gs.role_manager
                    .get_loaded(main_role_id)
                    .map(|role| {
                        let (_, extracted) = crate::utils::prompt::extract_player_setting(
                            role.settings.system_prompt.as_deref().unwrap_or_default(),
                        );
                        extracted.unwrap_or_default()
                    })
                    .unwrap_or_default()
            };
            {
                let mut gs = service.game_status.lock().await;
                gs.player.user_name = identity.display_name().to_string();
                gs.player.user_subtitle = identity.subtitle.clone();
                gs.player.user_prompt = if identity.prompt.trim().is_empty() {
                    fallback_prompt
                } else {
                    identity.prompt.clone()
                };
                gs.player.identity_id = Some(identity.id.clone());
                gs.player.relations = identity.relations.clone();
            }
            if let Err(e) = identity_store.set_current_id(&identity.id) {
                tracing::warn!("恢复存档身份失败: {e}");
            }
        }
    }

    let _ = service
        .restore_memory_banks(save_id)
        .await
        .map_err(|e| eprintln!("[SAVE_WARN] 恢复记忆库失败: {}", e));

    let baseline_hash = auto_save::hash_of_real_lines(&line_list);
    service
        .load_lines(line_list, main_role_id, Some(save_id))
        .await
        .map_err(|e| format!("载入台词失败: {}", e))?;

    // 8. 恢复 GameStatus 快照
    let snapshot: GameStatusSnapshot = serde_json::from_str(&save_model.status).unwrap_or_default();
    service.game_status.lock().await.apply_snapshot(&snapshot);

    service.game_status.lock().await.script_status = resume_script.clone();

    // 10. 返回前端初始化数据
    let result = build_web_init_data(&service, &app).await?;
    drop(service);
    // 续跑引擎必须在释放 ai_service 锁之后启动（spawn 内部会再锁 ai_service）
    if let Some(script) = resume_script {
        crate::api::script::spawn_script_execution(app.clone(), script).await;
    }
    state
        .auto_save_manager
        .lock()
        .await
        .set_baseline(baseline_hash);
    Ok(result)
}

#[tauri::command]
pub async fn update_save(
    app: AppHandle,
    save_id: i32,
    screenshot_path: Option<String>,
    player_chapter: Option<String>,
    player_event_index: Option<i32>,
    player_line_count: Option<i32>,
    player_vars_json: Option<String>,
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
    SaveRepo::sync_lines(db, save_id, &lines, false)
        .await
        .map_err(|e| format!("同步台词失败: {}", e))?;

    {
        let mut gs = service.game_status.lock().await;
        gs.active_save_id = Some(save_id);
        if let Err(e) = SaveRepo::bind_line_ids(db, save_id, &mut gs.line_list).await {
            tracing::warn!("回填台词行 id 失败: {e}");
        }
    }

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
    let script_status_snapshot = service.game_status.lock().await.script_status.clone();
    match script_status_snapshot {
        Some(script_status) => {
            let (script_folder, chapter, event_index, vars_json) = resolve_script_progress(
                &script_status,
                player_chapter.as_deref(),
                player_event_index,
                player_line_count,
                player_vars_json.as_deref(),
            );
            let _ = SaveRepo::upsert_running_script(
                db,
                save_id,
                &script_folder,
                &vars_json,
                &chapter,
                event_index,
            )
            .await
            .map_err(|e| eprintln!("[SAVE_WARN] update_save: 保存剧本状态失败: {}", e));
        },
        None => {
            // 当前无剧本在跑：清掉该存档早年关联的旧进度行，避免读档误续跑已结束的剧本
            if let Err(e) = SaveRepo::clear_running_script(db, save_id).await {
                eprintln!("[SAVE_WARN] update_save: 清理旧剧本状态失败: {}", e);
            }
        },
    }

    let baseline_hash = auto_save::hash_of_real_lines(&lines);
    drop(service);
    state
        .auto_save_manager
        .lock()
        .await
        .set_baseline(baseline_hash);

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
    let screenshot_path = super::data_dir()
        .join("screenshots")
        .join(format!("{}.png", save_id));
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
pub async fn update_save_title(app: AppHandle, save_id: i32, title: String) -> Result<(), String> {
    let state = app.state::<AppState>();
    let db = &state.db;
    SaveRepo::update_save_title(db, save_id, &title)
        .await
        .map_err(|e| format!("修改存档名称失败: {}", e))?;
    Ok(())
}

#[tauri::command]
pub async fn save_screenshot(save_id: i32, screenshot_path: String) -> Result<(), String> {
    save_screenshot_file(save_id, &screenshot_path).await
}

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
