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
//!   vision: { left, right, optic }（视觉群 600ms 窗口发放率归一 0..1，复眼发光用）,
//!   fly: { x, z, heading, speed, state, hunger, energy },
//!   foods: [{ id, x, z, kind }], events: [{ seq, kind, text }],
//!   world_radius, day_length_ticks }`。建议 5~20Hz 轮询。
//! - `invoke("fly_brain_control", { payload })` → `{ ok, speed, plasticity }`：
//!   payload = `{ speed?: 0.5|1|2, cmd?: "restart", plasticity?: bool }`；
//!   restart = 重置世界+果蝇+脑权重恢复 w0（可塑性状态清零）。
//!
//! enter/exit 可能阻塞数百毫秒到数秒（磁盘 + 大内存分配），放 `spawn_blocking`；
//! state/control 只动共享状态，直接执行。

use serde::Deserialize;
use tauri::{AppHandle, Manager};

use crate::fly_brain::worker::FlyBrainSnapshot;
use crate::fly_brain::{
    FlyBrainControlResp, FlyBrainEnterResp, FlyBrainExitResp, FlyBrainPositionsResp, FlyBrainState,
};

/// `fly_brain_control` 的载荷。
#[derive(Debug, Deserialize)]
pub struct FlyBrainControlPayload {
    pub speed: Option<f64>,
    pub cmd: Option<String>,
    pub plasticity: Option<bool>,
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
