//! LAN 同步 HTTP 服务端。
//!
//! 基于 axum 提供轻量 HTTP API：
//! - GET  /health       — 健康检查
//! - GET  /manifest     — 返回 CompleteManifest
//! - GET  /file?path=   — 下载文件（流式）
//! - POST /push-file?path= — 接收文件（原子写入）
//! - POST /push-delete?path= — 接收删除指令

use std::path::PathBuf;

use axum::{
    body::Body,
    extract::{DefaultBodyLimit, Query, State as AxumState},
    http::StatusCode,
    response::{IntoResponse, Json, Response},
    routing::{get, post},
    Router,
};
use futures_util::StreamExt;
use serde::Deserialize;
use tokio::net::TcpListener;
use tokio::sync::oneshot;
use tokio_util::io::ReaderStream;
use tracing::{error, info};

use crate::api::{self, data_dir};
use crate::data_update::manifest::DataManifest;

use super::manifest as sync_manifest;
use super::messages::{CompleteManifest, DeviceIdentity};

// ─── 服务端状态 ──────────────────────────────────────────────

/// axum 共享状态。
#[derive(Clone)]
struct ServerState {
    device_id: String,
    data_dir: PathBuf,
}

/// 全局服务关闭信号。
static SHUTDOWN_TX: std::sync::Mutex<Option<oneshot::Sender<()>>> = std::sync::Mutex::new(None);

// ─── 查询参数 ────────────────────────────────────────────────

#[derive(Deserialize)]
struct FileQuery {
    path: String,
}

// ─── 公开 API ────────────────────────────────────────────────

/// 启动 axum HTTP 服务，绑定随机端口，返回实际端口号。
pub async fn start_server(
    app: tauri::AppHandle,
    identity: &DeviceIdentity,
) -> Result<u16, String> {
    let state = ServerState {
        device_id: identity.device_id.clone(),
        data_dir: data_dir(),
    };

    let router = Router::new()
        .route("/health", get(health_handler))
        .route("/manifest", get(manifest_handler))
        .route("/file", get(file_handler))
        .route("/push-file", post(push_file_handler))
        .route("/push-delete", post(push_delete_handler))
        .route("/db-records", get(db_records_handler))
        .route("/db-records", post(db_records_push_handler))
        .layer(DefaultBodyLimit::disable())
        .with_state(state);

    // 绑定随机端口
    let listener = TcpListener::bind("0.0.0.0:0")
        .await
        .map_err(|e| format!("绑定端口失败: {e}"))?;

    let port = listener
        .local_addr()
        .map_err(|e| format!("获取端口失败: {e}"))?
        .port();

    let (tx, rx) = oneshot::channel::<()>();

    // 存储关闭信号
    {
        let mut guard = SHUTDOWN_TX
            .lock()
            .map_err(|e| format!("锁失败: {e}"))?;
        *guard = Some(tx);
    }

    // 后台运行服务
    let _app = app.clone();
    tauri::async_runtime::spawn(async move {
        info!("axum 服务启动在端口 {}", port);
        axum::serve(listener, router)
            .with_graceful_shutdown(async {
                let _ = rx.await;
            })
            .await
            .unwrap_or_else(|e| error!("axum 服务错误: {e}"));
    });

    Ok(port)
}

/// 停止 axum HTTP 服务。
pub async fn stop_server() -> Result<(), String> {
    let tx = {
        let mut guard = SHUTDOWN_TX
            .lock()
            .map_err(|e| format!("锁失败: {e}"))?;
        guard.take()
    };

    if let Some(tx) = tx {
        let _ = tx.send(());
        info!("已发送 axum 关闭信号");
    }

    Ok(())
}

// ─── 端点处理器 ──────────────────────────────────────────────

/// GET /db-records — 导出全部数据库记录（只读连接，不阻塞主 DB）。
async fn db_records_handler(
    AxumState(state): AxumState<ServerState>,
) -> Result<Json<super::messages::DbRecords>, AppError> {
    let records = super::db_sync::export_all_records(&state.data_dir)
        .await
        .map_err(|e| AppError(StatusCode::INTERNAL_SERVER_ERROR, e))?;
    Ok(Json(records))
}

/// POST /db-records — 接收数据库记录并暂存（导入在下次启动时生效）。
async fn db_records_push_handler(
    AxumState(state): AxumState<ServerState>,
    Json(records): Json<super::messages::DbRecords>,
) -> Result<Json<serde_json::Value>, AppError> {
    super::db_sync::stage_db_records(&state.data_dir, &records)
        .map_err(|e| AppError(StatusCode::INTERNAL_SERVER_ERROR, e))?;

    Ok(Json(serde_json::json!({
        "ok": true,
        "staged": true,
        "message": "数据库记录已暂存，将在下次启动时导入"
    })))
}

/// GET /health — 健康检查。
async fn health_handler(AxumState(state): AxumState<ServerState>) -> Json<serde_json::Value> {
    let manifest = load_local_manifest(&state.data_dir);
    let version = manifest.as_ref().map_or(0, |m| m.data_version);

    Json(serde_json::json!({
        "ok": true,
        "deviceId": state.device_id,
        "dataVersion": version,
    }))
}

/// GET /manifest — 返回完整文件清单。
async fn manifest_handler(
    AxumState(state): AxumState<ServerState>,
) -> Result<Json<CompleteManifest>, AppError> {
    let manifest = load_local_manifest(&state.data_dir);

    let complete = sync_manifest::build_complete_manifest(
        &state.data_dir,
        manifest.as_ref(),
        &state.device_id,
    )
    .map_err(|e| AppError(StatusCode::INTERNAL_SERVER_ERROR, format!("扫描失败: {e}")))?;

    Ok(Json(complete))
}

/// GET /file?path=... — 流式传输单个文件。
async fn file_handler(
    AxumState(state): AxumState<ServerState>,
    Query(query): Query<FileQuery>,
) -> Result<Response, AppError> {
    // 构造路径并检查文件是否存在（先于 canonicalize，避免 Windows 上对不存在路径报错）
    let file_path = state.data_dir.join(&query.path);

    if !file_path.is_file() {
        return Err(AppError(StatusCode::NOT_FOUND, "文件不存在".to_string()));
    }

    // 文件存在 → 可以安全 canonicalize
    api::validate_path_in_base(&file_path, &state.data_dir)
        .map_err(|e| AppError(StatusCode::FORBIDDEN, e))?;

    let file = tokio::fs::File::open(&file_path)
        .await
        .map_err(|e| AppError(StatusCode::NOT_FOUND, format!("无法打开文件: {e}")))?;

    let size = file
        .metadata()
        .await
        .map(|m| m.len())
        .unwrap_or(0);

    let stream = ReaderStream::new(file);
    let body = Body::from_stream(stream);

    let response = Response::builder()
        .header("Content-Type", "application/octet-stream")
        .header("Content-Length", size.to_string())
        .body(body)
        .map_err(|e| AppError(StatusCode::INTERNAL_SERVER_ERROR, format!("构建响应失败: {e}")))?;

    Ok(response)
}

/// POST /push-file?path=... — 接收文件（流式写入 + 原子 rename）。
async fn push_file_handler(
    AxumState(state): AxumState<ServerState>,
    Query(query): Query<FileQuery>,
    body: Body,
) -> Result<Json<serde_json::Value>, AppError> {
    let file_path = state.data_dir.join(&query.path);

    // 文件可能尚不存在 → 先创建父目录，再校验父目录而非文件本身
    if let Some(parent) = file_path.parent() {
        std::fs::create_dir_all(parent).map_err(|e| {
            AppError(
                StatusCode::INTERNAL_SERVER_ERROR,
                format!("创建目录失败: {e}"),
            )
        })?;
        api::validate_path_in_base(&parent.to_path_buf(), &state.data_dir)
            .map_err(|e| AppError(StatusCode::FORBIDDEN, e))?;
    }

    // 原子写入：先流式写 .tmp，再 rename；若 rename 失败则暂存
    let tmp_path = file_path.with_extension(format!(
        "{}.tmp",
        file_path
            .extension()
            .map(|e| format!(".{}", e.to_string_lossy()))
            .unwrap_or_default()
    ));

    // 流式写入 + 边写边算 SHA-256
    let mut dest = tokio::fs::File::create(&tmp_path)
        .await
        .map_err(|e| AppError(StatusCode::INTERNAL_SERVER_ERROR, format!("创建临时文件失败: {e}")))?;

    use sha2::{Digest, Sha256};
    let mut hasher = Sha256::new();
    let mut stream = body.into_data_stream();
    while let Some(chunk) = stream.next().await {
        let chunk =
            chunk.map_err(|e| AppError(StatusCode::INTERNAL_SERVER_ERROR, format!("接收数据失败: {e}")))?;
        tokio::io::AsyncWriteExt::write_all(&mut dest, &chunk)
            .await
            .map_err(|e| AppError(StatusCode::INTERNAL_SERVER_ERROR, format!("写入临时文件失败: {e}")))?;
        hasher.update(&chunk);
    }

    // 确保数据落盘
    tokio::io::AsyncWriteExt::flush(&mut dest)
        .await
        .map_err(|e| AppError(StatusCode::INTERNAL_SERVER_ERROR, format!("flush 失败: {e}")))?;
    drop(dest);

    let sha256 = format!("{:x}", hasher.finalize());

    if let Err(e) = std::fs::rename(&tmp_path, &file_path) {
        // 目标文件被锁定（如 SQLite DB），回退到暂存
        match super::staging::stage_file(&state.data_dir, &query.path, &tmp_path) {
            Ok(()) => {
                let _ = std::fs::remove_file(&tmp_path);
                return Ok(Json(serde_json::json!({
                    "ok": true,
                    "staged": true,
                    "sha256": sha256,
                })));
            }
            Err(se) => {
                return Err(AppError(
                    StatusCode::INTERNAL_SERVER_ERROR,
                    format!("重命名失败且暂存失败: {} (rename: {}, stage: {})", query.path, e, se),
                ));
            }
        }
    }

    Ok(Json(serde_json::json!({
        "ok": true,
        "staged": false,
        "sha256": sha256,
    })))
}

/// POST /push-delete?path=... — 接收删除指令（软删除到 .trash/）。
async fn push_delete_handler(
    AxumState(state): AxumState<ServerState>,
    Query(query): Query<FileQuery>,
) -> Result<Json<serde_json::Value>, AppError> {
    let file_path = state.data_dir.join(&query.path);

    if !file_path.exists() {
        // 文件已不存在，幂等返回成功（无需校验路径）
        return Ok(Json(serde_json::json!({ "ok": true })));
    }

    // 文件存在 → 可以安全 canonicalize
    api::validate_path_in_base(&file_path, &state.data_dir)
        .map_err(|e| AppError(StatusCode::FORBIDDEN, e))?;

    // 软删除到 .trash/
    let trash_dir = state.data_dir.join(".trash");
    std::fs::create_dir_all(&trash_dir).map_err(|e| {
        AppError(
            StatusCode::INTERNAL_SERVER_ERROR,
            format!("创建 .trash 目录失败: {e}"),
        )
    })?;

    let file_name = file_path
        .file_name()
        .unwrap_or_default()
        .to_string_lossy()
        .to_string();
    let trash_path = trash_dir.join(&file_name);

    // 如果回收站已存在同名文件，追加时间戳
    let trash_path = if trash_path.exists() {
        trash_dir.join(format!(
            "{}_{}",
            file_name,
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap_or_default()
                .as_secs()
        ))
    } else {
        trash_path
    };

    std::fs::rename(&file_path, &trash_path).map_err(|e| {
        AppError(
            StatusCode::INTERNAL_SERVER_ERROR,
            format!("移动到 .trash 失败: {e}"),
        )
    })?;

    Ok(Json(serde_json::json!({ "ok": true })))
}

// ─── 辅助 ────────────────────────────────────────────────────

/// 加载本地 data_manifest.json（如果存在）。
fn load_local_manifest(data_dir: &std::path::Path) -> Option<DataManifest> {
    let manifest_path = data_dir.join("data_manifest.json");
    if !manifest_path.exists() {
        return None;
    }
    let content = std::fs::read_to_string(&manifest_path).ok()?;
    serde_json::from_str::<DataManifest>(&content).ok()
}

// ─── 错误类型 ────────────────────────────────────────────────

struct AppError(StatusCode, String);

impl IntoResponse for AppError {
    fn into_response(self) -> Response {
        let body = Json(serde_json::json!({
            "error": self.1,
        }));
        (self.0, body).into_response()
    }
}
