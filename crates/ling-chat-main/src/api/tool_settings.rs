//! 聊天工具的用户配置命令（网页搜索等）。

use std::sync::Arc;
use std::sync::atomic::Ordering;
use std::time::Instant;

use sea_orm::DatabaseConnection;
use serde::Serialize;
use tauri::Manager;

use ling_chat_notes::embedding::Embedder;
use ling_chat_notes::retrieval::NotesVectorIndex;
use ling_chat_notes::{Note, NotesStore};

use crate::AppState;
use crate::ai_service::embedding_remote::{WarmingProgress, emit_warming};
use crate::ai_service::message_system::generator::GeneratorSource;
use crate::ai_service::skill_agent::command_executor;
use crate::ai_service::tools::builtin::web_search::WebSearchTool;
use crate::ai_service::tools::executor::{Tool, ToolContext};
use crate::ai_service::tools::permissions::CONFIG_FILE_NAME;
use crate::ai_service::tools::settings::ToolSettings;
use crate::api::character::read_character_settings;
use crate::db::managers::role_repo::RoleRepo;

/// 读取当前工具配置。
#[tauri::command]
pub async fn get_tool_settings(app: tauri::AppHandle) -> Result<ToolSettings, String> {
    let state = app.state::<AppState>();
    Ok(state.tool_settings.get())
}

/// 设置页使用的运行时诊断信息。尤其用于 Android：工具配置按设备保存，
/// 仅看开关无法区分“当前角色没有权限”和“当前模型不支持原生工具调用”。
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ToolRuntimeInfo {
    platform: &'static str,
    model_configured: bool,
    native_tool_calls_supported: bool,
    command_available: bool,
    file_ops_app_sandbox_only: bool,
    registered_tool_count: usize,
    allowed_tools: Vec<String>,
}

#[tauri::command]
pub async fn get_tool_runtime_info(app: tauri::AppHandle) -> Result<ToolRuntimeInfo, String> {
    let state = app.state::<AppState>();
    let llm = crate::ai_service::llm::slot_snapshot(&state.chat.llm).await;
    let model_configured = llm.is_some();
    let native_tool_calls_supported = llm
        .as_ref()
        .map(|client| client.supports_streaming_tools())
        .unwrap_or(false);

    let game_status = {
        let service = state.ai_service.lock().await;
        service.game_status.clone()
    };
    let role_name = {
        let mut game_status = game_status.lock().await;
        match game_status.current_role_id {
            Some(role_id) => game_status
                .get_role(&state.db, role_id)
                .await
                .ok()
                .and_then(|role| role.display_name.clone()),
            None => None,
        }
    };

    let mut allowed_tools = role_name
        .as_deref()
        .map(|name| {
            state
                .tool_registry
                .allowed_tools(GeneratorSource::UserChat, Some(name))
                .into_iter()
                .collect::<Vec<_>>()
        })
        .unwrap_or_default();
    allowed_tools.sort();

    let platform = if cfg!(target_os = "android") {
        "android"
    } else if cfg!(target_os = "ios") {
        "ios"
    } else if cfg!(target_os = "windows") {
        "windows"
    } else if cfg!(target_os = "macos") {
        "macos"
    } else {
        "desktop"
    };

    Ok(ToolRuntimeInfo {
        platform,
        model_configured,
        native_tool_calls_supported,
        command_available: cfg!(desktop),
        file_ops_app_sandbox_only: cfg!(any(target_os = "android", target_os = "ios")),
        registered_tool_count: state.tool_registry.definitions().len(),
        allowed_tools,
    })
}

/// 保存工具配置：写盘 + 热更新 + 同步权限矩阵。
///
/// 网页搜索启用且凭据来源可用时，自动放开 default 角色组的 `web_search`
/// 权限；Codex 使用 OAuth，Kimi 可在执行时复用当前官方 Kimi Code 对话凭据。
#[tauri::command]
pub async fn save_tool_settings(
    app: tauri::AppHandle,
    mut settings: ToolSettings,
) -> Result<(), String> {
    let state = app.state::<AppState>();
    let data_dir = super::data_dir();
    let previous_identity = state.tool_settings.get().embedding.vector_identity();
    settings.normalize();
    settings.apply_platform_constraints();
    settings.save(&data_dir).map_err(|e| e.to_string())?;
    state.tool_settings.update(settings.clone());
    // 激活缓存容量随设置热更新；缩小时会立即裁掉旧条目。
    state.tool_rag.set_capacity(settings.cache_capacity());

    // 向量化后端总是按新配置重新选路（保存即清除熔断态）；只有真正影响向量的
    // 字段变了才作废语料缓存并预热——改超时这类操作不该消耗额度。
    state.embedding.reset();
    if previous_identity != settings.embedding.vector_identity() {
        state.tool_rag.invalidate_embeddings();
        spawn_embedding_warm(&app);
    }

    // 同步权限矩阵：启用的工具组/web_search 放开给 default 角色组，关闭的收回
    state.tool_registry.update_permissions(|permissions| {
        settings.sync_to_permissions(permissions);
        if let Err(e) = permissions.save(&data_dir.join(CONFIG_FILE_NAME)) {
            tracing::warn!("保存工具权限配置失败: {e}");
        }
    });
    Ok(())
}

/// 直接执行一次网页搜索（供设置页「测试搜索」按钮使用）。
#[tauri::command]
pub async fn test_web_search(app: tauri::AppHandle, query: String) -> Result<String, String> {
    let state = app.state::<AppState>();
    let tool = WebSearchTool::new(state.tool_settings.clone());
    let context =
        ToolContext::new(["web_search".to_string()].into_iter().collect()).with_app(app.clone());
    let result = tool
        .execute(&context, serde_json::json!({ "query": query }))
        .await
        .map_err(|e| e.to_string())?;
    Ok(result.to_string())
}

/// 嵌入自检：嵌入一句固定文本，回报生效后端、维度与耗时，供设置页验证配置。
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct EmbeddingTestResult {
    /// 生效后端标识，如 `local` 或 `cloud openai/text-embedding-3-small`。
    backend: String,
    /// 本次是否实际走了云端。
    cloud: bool,
    dimension: usize,
    elapsed_ms: u64,
}

#[tauri::command]
pub async fn test_embedding(app: tauri::AppHandle) -> Result<EmbeddingTestResult, String> {
    let state = app.state::<AppState>();
    if !state.embedding.is_enabled() {
        return Err("向量化后端不可用（未配置云端，且本地模型不可用）".into());
    }
    let backend = state.embedding.active_backend_label();
    let cloud = backend.starts_with("cloud");
    let embedder: Arc<dyn Embedder> = state.embedding.clone();
    let started = Instant::now();
    let vector = tokio::task::spawn_blocking(move || embedder.embed("语义检索自检：今天天气不错"))
        .await
        .map_err(|e| format!("自检任务异常: {e}"))?
        .map_err(|e| e.to_string())?;
    let result = EmbeddingTestResult {
        backend,
        cloud,
        dimension: vector.len(),
        elapsed_ms: started.elapsed().as_millis() as u64,
    };
    tracing::info!(
        "[embedding] 自检通过: {} 维度 {} 耗时 {} ms",
        result.backend,
        result.dimension,
        result.elapsed_ms
    );
    Ok(result)
}

/// 手动触发一次向量化预热（设置页的「重新向量化」按钮）。
#[tauri::command]
pub async fn rebuild_embeddings(app: tauri::AppHandle) -> Result<(), String> {
    spawn_embedding_warm(&app);
    Ok(())
}

/// 后台预热：逐角色全量嵌入笔记，并把 Tool RAG 的示例问句预热进缓存。
///
/// 预热是「用额度换体验」的动作，因此只在配置确实影响向量时被触发；期间检索照常
/// 走当前可用后端，不阻塞对话。同一时刻只允许一个任务，新的一次保存会取消上一个。
pub(crate) fn spawn_embedding_warm(app: &tauri::AppHandle) {
    let state = app.state::<AppState>();
    if !state.embedding.is_enabled() {
        tracing::info!("[embedding] 向量化后端不可用，跳过预热");
        return;
    }

    let backend = state.embedding.active_backend_label();
    let embedder: Arc<dyn Embedder> = state.embedding.clone();
    let warming = state.embedding_warming.clone();
    let tool_rag = state.tool_rag.clone();
    let registry = state.tool_registry.clone();
    let db = state.db.clone();
    let data_dir = super::data_dir();
    let app = app.clone();

    let (generation, cancel) = warming.begin();
    let task_warming = warming.clone();
    let handle = tauri::async_runtime::spawn(async move {
        let roles = collect_role_notes(&db, &data_dir).await;
        let definitions = registry.definitions();
        let total = roles.iter().map(|(_, notes)| notes.len()).sum::<usize>()
            + crate::ai_service::tools::tool_rag::corpus_size(&definitions);
        let index = Arc::new(NotesVectorIndex::new(&data_dir));
        let mut done = 0usize;
        let mut failed = 0usize;

        emit(&app, &backend, 0, total, 0, false);
        for (role, notes) in roles {
            if cancel.load(Ordering::Relaxed) {
                tracing::info!("[embedding] 预热已取消（配置被新的保存覆盖）");
                return;
            }
            let index = index.clone();
            let embedder = embedder.clone();
            let count = notes.len();
            let label = role.clone();
            let outcome =
                tokio::task::spawn_blocking(move || index.warm(embedder.as_ref(), &role, &notes))
                    .await;
            match outcome {
                Ok(Ok(synced)) => {
                    tracing::debug!("[embedding] 角色 {label} 预热: 新增 {synced} 条");
                },
                Ok(Err(e)) => {
                    tracing::warn!("[embedding] 角色 {label} 预热失败: {e}");
                    failed += count;
                },
                Err(e) => {
                    tracing::warn!("[embedding] 角色 {label} 预热任务异常: {e}");
                    failed += count;
                },
            }
            done += count;
            emit(&app, &backend, done.min(total), total, failed, false);
        }

        if !cancel.load(Ordering::Relaxed) {
            match tool_rag.warm(&definitions).await {
                Ok((tools, texts)) => {
                    tracing::info!(
                        "[embedding] Tool RAG 语料预热: {tools} 个工具 / {texts} 条语料"
                    );
                },
                Err(e) => {
                    tracing::warn!("[embedding] Tool RAG 语料预热失败: {e}");
                    failed += 1;
                },
            }
        }
        emit(&app, &backend, total, total, failed, true);
        task_warming.finish(generation);
    });
    warming.attach(generation, handle);
}

/// 逐角色读取笔记（按权威展示名定位笔记文件，与 memory_get_notes 同一口径）。
async fn collect_role_notes(
    db: &DatabaseConnection,
    data_dir: &std::path::Path,
) -> Vec<(String, Vec<Note>)> {
    let Ok(roles) = RoleRepo::get_all_main_roles(db).await else {
        tracing::warn!("[embedding] 预热时查询角色列表失败");
        return Vec::new();
    };
    let store = NotesStore::new(data_dir);
    let mut out = Vec::new();
    for role in roles {
        let name =
            read_character_settings(role.resource_folder.as_deref().unwrap_or_default()).ai_name;
        let notes = store.load(&name).unwrap_or_default();
        if !notes.is_empty() {
            out.push((name, notes));
        }
    }
    out
}

fn emit(
    app: &tauri::AppHandle,
    backend: &str,
    done: usize,
    total: usize,
    failed: usize,
    finished: bool,
) {
    emit_warming(
        app,
        &WarmingProgress {
            backend: backend.to_string(),
            done,
            total,
            failed,
            finished,
        },
    );
}

/// 返回当前 LingChat 进程是否已通过 Windows UAC 获得管理员令牌。
#[tauri::command]
pub fn get_tool_elevation_status() -> bool {
    command_executor::is_current_process_elevated()
}

/// 使用 Windows 正常 RunAs 流程启动管理员实例；成功启动后退出当前标准权限实例。
#[tauri::command]
pub async fn restart_tool_process_as_admin(app: tauri::AppHandle) -> Result<(), String> {
    if command_executor::is_current_process_elevated() {
        return Ok(());
    }
    tokio::task::spawn_blocking(command_executor::launch_current_process_as_admin)
        .await
        .map_err(|error| format!("管理员重启任务异常: {error}"))?
        .map_err(|error| error.to_string())?;
    // 使用独立系统线程而不是 Tauri async runtime：`app.exit(0)` 会关闭窗口并触发
    // 退出存档，但某些仍在运行的后台任务可能让进程继续残留。重启场景下必须保证
    // 旧 PID 最终释放，否则已提权的辅助进程会一直等不到启动新实例的时机。
    std::thread::spawn(move || {
        std::thread::sleep(std::time::Duration::from_millis(150));
        app.exit(0);
        std::thread::sleep(std::time::Duration::from_secs(5));
        tracing::warn!("管理员重启时旧进程未在宽限期内退出，正在结束残留进程");
        std::process::exit(0);
    });
    Ok(())
}

/// 主聊天 `execute_command` 的审批回调：前端弹窗后把用户决定送回等待中的工具。
#[tauri::command]
pub async fn resolve_command_approval(
    app: tauri::AppHandle,
    request_id: String,
    approved: bool,
) -> Result<(), String> {
    tracing::info!(
        "[approval] resolve_command_approval 收到回传: request_id={request_id} approved={approved}"
    );
    let state = app.state::<AppState>();
    let request = state
        .chat_command_approvals
        .lock()
        .await
        .remove(&request_id);
    match request {
        Some(request) => {
            let _ = request.tx.send(approved);
            Ok(())
        },
        None => {
            tracing::warn!(
                "[approval] resolve_command_approval 未找到请求: request_id={request_id}"
            );
            Err("审批请求不存在或已过期".into())
        },
    }
}

/// 主聊天文件写入/编辑审批回调。
#[tauri::command]
pub async fn resolve_file_change_approval(
    app: tauri::AppHandle,
    request_id: String,
    approved: bool,
) -> Result<(), String> {
    tracing::info!(
        "[approval] resolve_file_change_approval 收到回传: request_id={request_id} approved={approved}"
    );
    let state = app.state::<AppState>();
    let request = state
        .chat_file_change_approvals
        .lock()
        .await
        .remove(&request_id);
    match request {
        Some(request) => {
            let _ = request.tx.send(approved);
            Ok(())
        },
        None => {
            tracing::warn!(
                "[approval] resolve_file_change_approval 未找到请求: request_id={request_id}"
            );
            Err("文件修改审批请求不存在或已过期".into())
        },
    }
}

/// 主聊天 `delete_file` 的审批回调：前端确认后把决定送回等待中的删除工具。
#[tauri::command]
pub async fn resolve_file_delete_approval(
    app: tauri::AppHandle,
    request_id: String,
    approved: bool,
) -> Result<(), String> {
    tracing::info!(
        "[approval] resolve_file_delete_approval 收到回传: request_id={request_id} approved={approved}"
    );
    let state = app.state::<AppState>();
    let request = state
        .chat_file_delete_approvals
        .lock()
        .await
        .remove(&request_id);
    match request {
        Some(request) => {
            let _ = request.tx.send(approved);
            Ok(())
        },
        None => {
            tracing::warn!(
                "[approval] resolve_file_delete_approval 未找到请求: request_id={request_id}"
            );
            Err("删除审批请求不存在或已过期".into())
        },
    }
}
