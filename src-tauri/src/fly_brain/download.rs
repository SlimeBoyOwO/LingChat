//! 基础权重下载/更新编排：FlyWire 官方源 → raw/ → 离线构建 → 原子替换 graph.npz。
//!
//! - 下载复用仓库通用工具（`utils::download::build_download_client` /
//!   `download_to_file`，流式写盘 + 200ms/1MB 节流的进度回调），官方源
//!   （storage.googleapis.com）不通时给出带网络/代理提示的明确错误
//!   （TODO: 镜像回退未实现）；
//! - 构建用本模块 [`crate::fly_brain::build_graph::build_from_raw`]（CPU 密集，
//!   放 spawn_blocking）；
//! - 新 graph.npz 先写 `graph.npz.new` 再原子替换（Windows 先删目标）；替换后若
//!   worker 在跑则自动 `exit`，下次 `fly_brain_enter` 重载新图（简单可靠）；
//! - 全程后台任务（不阻塞 invoke），进度经 `fly-brain:model-progress` 事件推送
//!   （契约见 [`ModelProgressEvent`]），并同步更新 `FlyBrainState.model_dl` 供
//!   `fly_brain_model_status` 查询。

use std::sync::{Arc, Mutex};

use serde::Serialize;
use tauri::{AppHandle, Emitter, Manager};
use tracing::{info, warn};

use super::FlyBrainState;
use super::build_graph::{DEFAULT_MIN_SYN, build_from_raw};

/// 事件名（仓库 domain:name 惯例）。
pub const EVENT_MODEL_PROGRESS: &str = "fly-brain:model-progress";

/// 官方源（FlyWire Codex FAFB v783）。
const BASE_URL: &str = "https://storage.googleapis.com/flywire-data/codex/data/fafb/783";
const FILES: [(&str, &str); 3] = [
    ("connections.csv.gz", "connections.csv.gz"),
    ("coordinates.csv.gz", "coordinates.csv.gz"),
    ("classification.csv.gz", "classification.csv.gz"),
];

/// `fly-brain:model-progress` 事件载荷（键名逐字对齐契约）。
#[derive(Debug, Clone, Serialize)]
pub struct ModelProgressEvent {
    pub stage: String, // "download"|"build"|"done"|"error"
    pub file: String,
    pub downloaded_bytes: u64,
    pub total_bytes: u64,
    pub percent: f32,
    pub message: String,
}

/// model_status 的 download 字段（也是 FlyBrainState.model_dl 的内容）。
#[derive(Debug, Clone, Serialize)]
pub struct ModelDownloadState {
    pub active: bool,
    pub stage: String,
    pub percent: f32,
}

impl ModelDownloadState {
    pub fn busy() -> Self {
        ModelDownloadState {
            active: true,
            stage: "download".to_string(),
            percent: 0.0,
        }
    }
}

fn emit(app: &AppHandle, ev: &ModelProgressEvent) {
    let _ = app.emit(EVENT_MODEL_PROGRESS, ev);
}

/// 更新 model_dl 状态（State 临时值与 MutexGuard 生命周期解耦的写法）。
fn set_dl(app: &AppHandle, s: ModelDownloadState) {
    let state = app.state::<FlyBrainState>();
    let mut g = state.model_dl.lock().unwrap_or_else(|e| e.into_inner());
    *g = Some(s);
}

fn set_stage(
    app: &AppHandle,
    stage: &str,
    file: &str,
    done: u64,
    total: u64,
    percent: f32,
    msg: &str,
) {
    emit(
        app,
        &ModelProgressEvent {
            stage: stage.to_string(),
            file: file.to_string(),
            downloaded_bytes: done,
            total_bytes: total,
            percent,
            message: msg.to_string(),
        },
    );
}

/// 是否已有下载/构建在进行（busy 判定）。
pub fn is_busy(dl: &Mutex<Option<ModelDownloadState>>) -> bool {
    dl.lock()
        .map(|g| g.as_ref().map(|s| s.active).unwrap_or(false))
        .unwrap_or(false)
}

/// 启动后台下载+构建任务（调用方已做 busy 判定）。
pub fn spawn_model_download(app: AppHandle) {
    tauri::async_runtime::spawn(async move {
        if let Err(e) = run(&app).await {
            warn!("果蝇脑模型下载/构建失败: {e}");
            let msg = format!(
                "{e}（官方源 storage.googleapis.com；请检查网络/代理，可尝试设置 HTTPS_PROXY）"
            );
            set_stage(&app, "error", "", 0, 0, 0.0, &msg);
            set_dl(
                &app,
                ModelDownloadState {
                    active: false,
                    stage: "error".to_string(),
                    percent: 0.0,
                },
            );
        }
    });
}

async fn run(app: &AppHandle) -> Result<(), String> {
    let dir = crate::data_dir::get_data_dir()
        .join("third_party")
        .join("fly_brain");
    let raw_dir = dir.join("raw");
    std::fs::create_dir_all(&raw_dir).map_err(|e| format!("mkdir raw: {e}"))?;

    // ---- 阶段 1：下载三个 csv.gz ----
    let client = crate::utils::download::build_download_client()?;
    for (i, (name, url_name)) in FILES.iter().enumerate() {
        let url = format!("{BASE_URL}/{url_name}");
        let dest = raw_dir.join(name);
        info!("下载 {} -> {}", url, dest.display());
        let app2 = app.clone();
        let name2 = name.to_string();
        let cb: Arc<dyn Fn(crate::utils::download::DownloadProgress) + Send + Sync> =
            Arc::new(move |p: crate::utils::download::DownloadProgress| {
                set_dl(
                    &app2,
                    ModelDownloadState {
                        active: true,
                        stage: "download".to_string(),
                        percent: p.percent,
                    },
                );
                emit(
                    &app2,
                    &ModelProgressEvent {
                        stage: "download".to_string(),
                        file: name2.clone(),
                        downloaded_bytes: p.bytes_done,
                        total_bytes: p.total_bytes,
                        percent: p.percent,
                        message: format!("下载中 ({}/{})…", i + 1, FILES.len()),
                    },
                );
            });
        crate::utils::download::download_to_file(&client, &url, &dest, None, Some(cb), 0).await?;
    }

    // ---- 阶段 2：离线构建 graph.npz.new ----
    set_stage(
        app,
        "build",
        "graph.npz",
        0,
        0,
        0.0,
        "解析 CSV 构建图（约十秒）…",
    );
    set_dl(
        app,
        ModelDownloadState {
            active: true,
            stage: "build".to_string(),
            percent: 0.0,
        },
    );
    let raw2 = raw_dir.clone();
    let out_new = dir.join("graph.npz.new");
    let stats = tauri::async_runtime::spawn_blocking(move || {
        build_from_raw(&raw2, &out_new, DEFAULT_MIN_SYN)
    })
    .await
    .map_err(|e| format!("构建任务 join: {e}"))??;

    // ---- 阶段 3：原子替换 + 若在跑则自动 exit 让下次 enter 重载 ----
    let graph_path = super::graph_path();
    if graph_path.exists() {
        std::fs::remove_file(&graph_path).map_err(|e| format!("删除旧 graph.npz: {e}"))?;
    }
    std::fs::rename(dir.join("graph.npz.new"), &graph_path)
        .map_err(|e| format!("替换 graph.npz: {e}"))?;

    let state = app.state::<FlyBrainState>();
    let was_running = state.running.lock().map(|g| g.is_some()).unwrap_or(false);
    if was_running {
        info!("新 graph.npz 已替换，自动停止 worker（下次 enter 重载）");
        let _ = state.exit();
    }

    set_stage(
        app,
        "done",
        "graph.npz",
        0,
        0,
        100.0,
        &format!(
            "完成：{} 神经元 {} 边{}",
            stats.n,
            stats.edges,
            if was_running {
                "（已停止仿真，重新进入生效）"
            } else {
                ""
            }
        ),
    );
    set_dl(
        app,
        ModelDownloadState {
            active: false,
            stage: "done".to_string(),
            percent: 100.0,
        },
    );
    Ok(())
}
