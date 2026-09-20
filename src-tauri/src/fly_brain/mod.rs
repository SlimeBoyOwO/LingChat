//! 果蝇全脑 LIF 脉冲网络驱动「模拟果蝇生活」——后端模块。
//!
//! 移植自参考项目 fly-snake（纯 numpy + stdlib 的果蝇全脑仿真），完整编译进主程序，
//! 通过 Tauri 命令为前端 3D 牧场场景服务：
//! - [`graph`]：graph.npz（果蝇半脑连接组，139,255 神经元 / 2,484,367 边）加载；
//! - [`engine`]：LIF 脉冲仿真引擎（只移植发放门控快速路径；**有意省略** int8 量化
//!   与 full-matrix 全图稀疏乘对照路径，详见 engine.rs 文档）；
//! - [`brain_io`]：感觉编码 / DN 运动读出 / 开机校准（只依赖 engine；percepts
//!   类型定义在此，世界实现方可复用）；
//! - [`life`]：果蝇生活世界（觅食/进食/饥饿/昼夜作息；替代已删除的小游戏集合）；
//! - [`worker`]：std::thread 常驻连续仿真（真实时间配速，0.5/1/2 倍率）+ 快照 +
//!   控制面。
//!
//! Tauri 命令层在 `crate::api::fly_brain`（IPC 契约见该文件）。本模块持有全局状态
//! [`FlyBrainState`]（在 setup 阶段 `app.manage`）。
//!
//! 其他移植约定：
//! - 日志用仓库统一的 `tracing`（参考实现是 print，且 log crate 非本仓库直接依赖）；
//! - `fly_brain_positions`（3D 小窗点云）在未 enter 时**懒加载**：只解压 graph.npz
//!   中的 coords/super_class/meta_lr_axis 三个条目生成归一化点云并缓存（避免为出
//!   点云而完整加载 ~90MB 图数据）；enter 后改用 worker 持有的实时数据。

pub mod brain_io;
pub mod engine;
pub mod graph;
pub mod life;
pub mod worker;

use std::path::PathBuf;
use std::sync::{Arc, Mutex};
use std::time::Instant;

use serde::Serialize;
use tracing::info;

use engine::Brain;
use graph::{BrainGraph, PositionsSource};

// ─── IPC 响应类型（JSON 键名逐字对齐 IPC 契约，不做 camelCase 重命名）───

/// `fly_brain_enter` 响应。
#[derive(Debug, Clone, Serialize)]
pub struct FlyBrainEnterResp {
    pub ok: bool,
    pub load_ms: u64,
}

/// `fly_brain_exit` 响应。
#[derive(Debug, Clone, Serialize)]
pub struct FlyBrainExitResp {
    pub ok: bool,
}

/// `fly_brain_positions` 响应（左上角 3D 小窗的全脑点云）。
#[derive(Debug, Clone, Serialize)]
pub struct FlyBrainPositionsResp {
    /// super_class 名数组（按 class_id 索引）。
    pub classes: Vec<String>,
    /// 全部神经元 [x,y,z,class_id] f32 小端的 base64（n×16 字节）。
    pub data_b64: String,
}

/// `fly_brain_control` 响应。
#[derive(Debug, Clone, Serialize)]
pub struct FlyBrainControlResp {
    pub ok: bool,
    pub speed: f32,
    pub plasticity: bool,
}

// ─── 归一化点云（positions.bin 语义，对齐 web_server.py 47-60/90-105 行）───

/// 归一化神经元点云：1%~99% 分位截断 → [0,1] → 居中 [-0.5,0.5]，y 翻转（背侧朝上），
/// x 用 meta_lr_axis 轴。`bin` 为每点 [x,y,z,class_id] f32 LE（class_id 索引 classes）。
pub struct PositionsData {
    pub classes: Vec<String>,
    pub bin: Vec<u8>,
    /// 全长 [0,1] 归一化坐标（按神经元索引取坐标用；无效点为 0.0）。
    pub xs_f: Vec<f32>,
    pub ys_f: Vec<f32>,
}

impl PositionsData {
    pub fn from_brain(brain: &Brain) -> Option<Self> {
        let coords = brain.coords.as_deref()?;
        Some(Self::build(
            brain.n,
            coords,
            &brain.super_class,
            brain.meta_lr_axis.unwrap_or(0),
        ))
    }

    /// 从原始 (n,3) C 序坐标与 super_class 标注构建。
    pub fn build(n: usize, coords: &[f32], super_class: &[String], lr_axis: usize) -> Self {
        // valid = ~isnan(coords[:, 0])
        let valid: Vec<bool> = (0..n).map(|i| !coords[i * 3].is_nan()).collect();
        let idx_valid: Vec<usize> = (0..n).filter(|&i| valid[i]).collect();

        let xs: Vec<f32> = idx_valid.iter().map(|&i| coords[i * 3 + lr_axis]).collect();
        let ys: Vec<f32> = idx_valid.iter().map(|&i| coords[i * 3 + 1]).collect();
        let zs: Vec<f32> = idx_valid.iter().map(|&i| coords[i * 3 + 2]).collect();
        let nx = percentile_norm(&xs);
        // y 翻转：背侧朝上
        let ny: Vec<f32> = percentile_norm(&ys).into_iter().map(|v| 1.0 - v).collect();
        let nz = percentile_norm(&zs);

        // classes = sorted(set(super_class[valid]))，cid 按类名索引
        let mut classes: Vec<String> = idx_valid.iter().map(|&i| super_class[i].clone()).collect();
        classes.sort();
        classes.dedup();
        let class_id = |name: &str| -> f32 {
            classes
                .binary_search_by(|c| c.as_str().cmp(name))
                .unwrap_or(0) as f32
        };

        // 全长版本（按索引取坐标用），无效点为 0.0
        let mut xs_f = vec![0f32; n];
        let mut ys_f = vec![0f32; n];
        let mut bin = Vec::with_capacity(n * 16);
        let mut p = 0usize; // pos_in_valid（对齐 Python 的 cumsum(valid)-1）
        for i in 0..n {
            let (x, y, z, cid) = if valid[i] {
                let (vx, vy, vz) = (nx[p] - 0.5, ny[p] - 0.5, nz[p] - 0.5);
                xs_f[i] = nx[p];
                ys_f[i] = ny[p];
                p += 1;
                (vx, vy, vz, class_id(&super_class[i]))
            } else {
                (0.0, 0.0, 0.0, 0.0)
            };
            for v in [x, y, z, cid] {
                bin.extend_from_slice(&v.to_le_bytes());
            }
        }
        PositionsData {
            classes,
            bin,
            xs_f,
            ys_f,
        }
    }

    pub fn resp(&self) -> FlyBrainPositionsResp {
        use base64::Engine;
        FlyBrainPositionsResp {
            classes: self.classes.clone(),
            data_b64: base64::engine::general_purpose::STANDARD.encode(&self.bin),
        }
    }
}

/// numpy 默认（线性插值）分位数。
fn percentile(sorted: &[f32], p: f64) -> f64 {
    if sorted.is_empty() {
        return 0.0;
    }
    if sorted.len() == 1 {
        return sorted[0] as f64;
    }
    let rank = (sorted.len() - 1) as f64 * p / 100.0;
    let lo = rank.floor() as usize;
    let hi = (lo + 1).min(sorted.len() - 1);
    let frac = rank - lo as f64;
    sorted[lo] as f64 * (1.0 - frac) + sorted[hi] as f64 * frac
}

/// 1%~99% 分位截断 → clip 到 [0,1]。
fn percentile_norm(v: &[f32]) -> Vec<f32> {
    let mut sorted: Vec<f32> = v.to_vec();
    sorted.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));
    let lo = percentile(&sorted, 1.0);
    let hi = percentile(&sorted, 99.0);
    let span = ((hi - lo).max(1e-9)) as f32;
    v.iter()
        .map(|&x| (((x as f64 - lo) as f32) / span).clamp(0.0, 1.0))
        .collect()
}

// ─── 全局状态 ───

/// graph.npz 的运行时路径（`data/third_party/fly_brain/graph.npz`）。
pub fn graph_path() -> PathBuf {
    crate::data_dir::get_data_dir()
        .join("third_party")
        .join("fly_brain")
        .join("graph.npz")
}

/// 果蝇脑全局状态：持有运行中的 worker（脑数据归 worker 线程所有），
/// 以及未 enter 时懒加载的 positions 缓存。
#[derive(Default)]
pub struct FlyBrainState {
    running: Mutex<Option<worker::Running>>,
    positions_cache: Mutex<Option<Arc<PositionsData>>>,
}

impl FlyBrainState {
    /// 幂等进入：首次调用加载 graph.npz + 构建 BrainIO + 校准 + spawn worker。
    /// 已启动时直接返回 ok（load_ms=0）。
    pub fn enter(&self) -> Result<FlyBrainEnterResp, String> {
        let mut guard = self.running.lock().map_err(|e| format!("锁失败: {e}"))?;
        if guard.is_some() {
            return Ok(FlyBrainEnterResp {
                ok: true,
                load_ms: 0,
            });
        }
        let t0 = Instant::now();
        let path = graph_path();
        if !path.exists() {
            return Err(format!("未找到果蝇脑图数据: {}", path.display()));
        }
        let graph = BrainGraph::load(&path)?;
        let running = worker::start(graph)?;
        let load_ms = t0.elapsed().as_millis() as u64;
        info!("果蝇脑 enter: 加载+校准+启动 worker 共耗时 {}ms", load_ms);
        *guard = Some(running);
        Ok(FlyBrainEnterResp { ok: true, load_ms })
    }

    /// 退出：停 worker（stop 标志 + join），释放脑。未启动也返回 ok。
    pub fn exit(&self) -> Result<FlyBrainExitResp, String> {
        let running = {
            self.running
                .lock()
                .map_err(|e| format!("锁失败: {e}"))?
                .take()
        };
        if let Some(r) = running {
            r.shutdown();
        }
        Ok(FlyBrainExitResp { ok: true })
    }

    /// 快照（未启动返回 Err("not started")）。
    pub fn state(&self) -> Result<worker::FlyBrainSnapshot, String> {
        let guard = self.running.lock().map_err(|e| format!("锁失败: {e}"))?;
        let r = guard.as_ref().ok_or_else(|| "not started".to_string())?;
        Ok(r.shared.snapshot())
    }

    /// 归一化点云（3D 小窗用）。enter 后用 worker 的实时数据；未 enter 时懒加载
    /// 并缓存（只读 coords/super_class/meta 三个 npz 条目，模块文档已注明此约定）。
    pub fn positions(&self) -> Result<FlyBrainPositionsResp, String> {
        {
            let guard = self.running.lock().map_err(|e| format!("锁失败: {e}"))?;
            if let Some(r) = guard.as_ref() {
                return Ok(r.shared.positions.resp());
            }
        }
        let mut cache = self
            .positions_cache
            .lock()
            .map_err(|e| format!("锁失败: {e}"))?;
        if cache.is_none() {
            let t0 = Instant::now();
            let src = PositionsSource::load(&graph_path())?;
            let data = PositionsData::build(
                src.n,
                &src.coords,
                &src.super_class,
                src.meta_lr_axis.unwrap_or(0),
            );
            info!(
                "果蝇脑 positions 懒加载完成（{}ms, {} 点）",
                t0.elapsed().as_millis(),
                data.xs_f.len()
            );
            *cache = Some(Arc::new(data));
        }
        Ok(cache.as_ref().expect("刚填充").resp())
    }

    /// 控制命令（未启动返回 Err("not started")）。
    pub fn control(
        &self,
        speed: Option<f64>,
        cmd: Option<String>,
        plasticity: Option<bool>,
    ) -> Result<FlyBrainControlResp, String> {
        let guard = self.running.lock().map_err(|e| format!("锁失败: {e}"))?;
        let r = guard.as_ref().ok_or_else(|| "not started".to_string())?;
        Ok(r.shared.apply_control(speed, cmd, plasticity))
    }
}
