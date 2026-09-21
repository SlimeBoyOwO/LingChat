//! 果蝇大脑「模拟果蝇生活」的 Tauri 命令（IPC 契约层，薄封装；逻辑在 `crate::fly_brain`）。
//!
//! 前端用法（契约，键名逐字固定）：
//! - `invoke("fly_brain_enter")` → `{ ok, load_ms }`：幂等；首次调用加载 graph.npz
//!   （约 1~2s）+ 校准 + 启动常驻仿真 worker，已启动直接返回 ok。
//! - `invoke("fly_brain_exit")` → `{ ok }`：停 worker 并释放脑（约 90MB+）。
//! - `invoke("fly_brain_positions")` → `{ classes, data_b64 }`：左上角 3D 小窗的
//!   全脑点云，139,255 点 [x,y,z,class_id] f32 小端（1~99 分位截断归一化、居中
//!   [-0.5,0.5]、y 翻转）的 base64；未 enter 时懒加载只读点云（见 fly_brain/mod.rs
//!   文档），挂载时取一次即可。
//! - `invoke("fly_brain_state")` → 快照：`{ time_of_day, sun_elevation, is_night,
//!   tick, sim_time, speed, plasticity, weights_changed, brain_activity,
//!   spikes_total, spikes, spike_ages_ms,
//!   active_neurons（600ms 窗口内发放过的去重神经元数，状态卡显示用）,
//!   vision: { left, right, optic }（视觉群 600ms 窗口发放率归一 0..1，复眼发光用）,
//!   fly: { x, z, heading, speed, state, hunger, energy }
//!   （state ∈ flying/foraging/eating/resting/sleeping/walking，walking=贴地散步），
//!   foods: [{ id, x, z, kind }], events: [{ seq, kind, text }],
//!   world_radius, day_length_ticks }`。建议 5~20Hz 轮询。
//! - `invoke("fly_brain_control", { payload })` → `{ ok, speed, plasticity }`：
//!   payload = `{ speed?: 0.5|1|2, cmd?: "restart", plasticity?: bool }`；
//!   restart = 重置世界+果蝇+脑权重恢复 w0（可塑性状态清零）。
//!
//! 权重管理（基础权重下载/构建 + 学习权重持久化）：
//! - `invoke("fly_brain_model_status")` → `{ base: { present, size_bytes,
//!   version("FlyWire FAFB v783") }, learned: { present, size_bytes, saved_at|null,
//!   rewards, punishes }, running_weights_changed|null, download|null }`；
//! - `invoke("fly_brain_model_download")` → `{ ok, error|null }`：后台下载 FlyWire
//!   官方 CSV → raw/ 离线构建 → 原子替换 graph.npz（进行中返回 ok:false error:"busy"）；
//!   进度经事件 `fly-brain:model-progress`（payload `{ stage: "download"|"build"|
//!   "done"|"error", file, downloaded_bytes, total_bytes, percent, message }`）推送；
//! - `invoke("fly_brain_learned_save")` → `{ ok, size_bytes }`：worker 立即落盘
//!   学习权重（learned_weights.bin；worker 未运行/从未开启可塑性 → ok:false）；
//! - `invoke("fly_brain_learned_reset")` → `{ ok }`：删 learned 文件，
//!   worker 在跑则同时重置为出厂权重。
//!
//! 食物设置（选图页「食物设置」面板；持久化到 food_config.json）：
//! - `invoke("fly_brain_food_config", { payload })` → `{ ok, init_foods, max_foods }`：
//!   payload = `{ init_foods?: 1..=上限, max_foods?: 1..=20 }`（None 保持当前值）；
//!   上限即时生效（超出立即裁减），低于开局数立即补足，restart/重进按开局数生成；
//!   快照附带当前 `food_init`/`food_max` 字段。
//!
//! enter/exit 可能阻塞数百毫秒到数秒（磁盘 + 大内存分配），放 `spawn_blocking`；
//! state/control 只动共享状态，直接执行。

use serde::Deserialize;
use tauri::{AppHandle, Manager};

use crate::fly_brain::worker::FlyBrainSnapshot;
use crate::fly_brain::{
    FlyBrainControlResp, FlyBrainEnterResp, FlyBrainExitResp, FlyBrainPositionsResp, FlyBrainState,
    FoodConfigResp, LearnedResetResp, LearnedSaveResp, ModelDownloadResp, ModelStatusResp,
};

/// `fly_brain_control` 的载荷。
#[derive(Debug, Deserialize)]
pub struct FlyBrainControlPayload {
    pub speed: Option<f64>,
    pub cmd: Option<String>,
    pub plasticity: Option<bool>,
}

/// `fly_brain_food_config` 的载荷（None 字段保持当前值；上限硬顶 20）。
#[derive(Debug, Deserialize)]
pub struct FlyBrainFoodConfigPayload {
    pub init_foods: Option<u32>,
    pub max_foods: Option<u32>,
}

#[tauri::command]
pub async fn fly_brain_enter(app: AppHandle) -> Result<FlyBrainEnterResp, String> {
    tauri::async_runtime::spawn_blocking(move || app.state::<FlyBrainState>().enter())
        .await
        .map_err(|e| format!("任务失败: {e}"))?
}

#[tauri::command]
pub async fn fly_brain_exit(app: AppHandle) -> Result<FlyBrainExitResp, String> {
    tauri::async_runtime::spawn_blocking(move || app.state::<FlyBrainState>().exit())
        .await
        .map_err(|e| format!("任务失败: {e}"))?
}

#[tauri::command]
pub async fn fly_brain_positions(app: AppHandle) -> Result<FlyBrainPositionsResp, String> {
    tauri::async_runtime::spawn_blocking(move || app.state::<FlyBrainState>().positions())
        .await
        .map_err(|e| format!("任务失败: {e}"))?
}

#[tauri::command]
pub async fn fly_brain_state(app: AppHandle) -> Result<FlyBrainSnapshot, String> {
    app.state::<FlyBrainState>().state()
}

#[tauri::command]
pub async fn fly_brain_control(
    app: AppHandle,
    payload: FlyBrainControlPayload,
) -> Result<FlyBrainControlResp, String> {
    app.state::<FlyBrainState>()
        .control(payload.speed, payload.cmd, payload.plasticity)
}

// ─── 权重管理 ───

#[tauri::command]
pub async fn fly_brain_model_status(app: AppHandle) -> Result<ModelStatusResp, String> {
    app.state::<FlyBrainState>().model_status()
}

#[tauri::command]
pub async fn fly_brain_model_download(app: AppHandle) -> Result<ModelDownloadResp, String> {
    app.state::<FlyBrainState>().model_download(&app)
}

#[tauri::command]
pub async fn fly_brain_learned_save(app: AppHandle) -> Result<LearnedSaveResp, String> {
    // 需要等待 worker 在 tick 边界完成落盘（≤2.5s），放 blocking 线程
    tauri::async_runtime::spawn_blocking(move || app.state::<FlyBrainState>().learned_save())
        .await
        .map_err(|e| format!("任务失败: {e}"))?
}

#[tauri::command]
pub async fn fly_brain_learned_reset(app: AppHandle) -> Result<LearnedResetResp, String> {
    app.state::<FlyBrainState>().learned_reset()
}

#[tauri::command]
pub async fn fly_brain_food_config(
    app: AppHandle,
    payload: FlyBrainFoodConfigPayload,
) -> Result<FoodConfigResp, String> {
    app.state::<FlyBrainState>()
        .food_config(payload.init_foods, payload.max_foods)
}
