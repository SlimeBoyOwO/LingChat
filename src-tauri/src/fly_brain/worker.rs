//! 常驻仿真 worker：std::thread 连续运转 LIF 仿真，驱动果蝇生活世界（FlyLife）。
//!
//! 沿用初版 worker 的架构（连续仿真 + 渲染/数据解耦），游戏循环替换为生活模拟：
//! - 后台单线程让仿真**连续运转**：按真实时间配速，1 仿真步 = (1/speed) ms 墙钟
//!   （speed ∈ {0.5, 1, 2}；target = 墙钟 ms × speed − 已走步数，一次最多补 50 步，
//!   catch-up 后 sleep 1ms）；
//! - 每个决策 tick = 150 仿真步（固定）：tick 内 drive 恒定，tick 末 BrainIO decide
//!   读出转向 → life.advance 推进世界（移动/进食/饥饿/作息/奖惩）；
//! - 维护三条 600ms 脉冲滚动窗口通道：每步发放**计数**（brain_activity = 窗口
//!   发放率归一 0..1，前端萤火虫亮度）+ 完整发放**索引**（快照 spikes /
//!   spike_ages_ms / active_neurons（窗口内去重神经元数，常驻位图去重），
//!   >8000 等距抽样仅作用于 spikes/ages 展示通道）+ 视觉群**分组计数**
//!   （左/右眼群与 optic 全集，快照 vision 字段，复眼发光用）；
//! - 控制面：speed 倍率（0.5/1/2，配速每轮直接读 ctrl 即时生效）、cmd=restart
//!   （重置世界+果蝇+脑权重恢复 w0、可塑性状态清零）、plasticity 开关；
//! - 另持有归一化全脑点云 `positions`（fly_brain_positions 命令的数据源，
//!   左上角 3D 小窗用）。
//!
//! 快照契约见 [`FlyBrainSnapshot`]（JSON 键名逐字固定，前端按此开发）。

use std::collections::VecDeque;
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::{Arc, Mutex, MutexGuard};
use std::thread::JoinHandle;
use std::time::{Duration, Instant};

use serde::{Deserialize, Serialize};
use tracing::{info, warn};

use super::brain_io::{BrainIO, SENS_K};
use super::engine::Brain;
use super::graph::BrainGraph;
use super::learned;
use super::life::{self, FlyLife, SIM_STEPS_PER_TICK, WORLD_RADIUS};
use super::{FlyBrainControlResp, PositionsData};

const SPIKE_WINDOW_MS: u64 = 600; // 滚动放电窗口（仿真 ms）
const ACTIVITY_REF_RATE: f32 = 300.0; // 窗口平均发放率达到该值（/步）时 activity=1.0
const MAX_SPIKES: usize = 8000; // 快照 spikes 抽样上限
/// vision.left/right 满亮参考率（Hz/神经元）。标定依据（冒烟实测，昼夜快进
/// 200 tick）：被食物电流（1.5~2.0，饥饿放大至 ~4）刺激的侧群峰值 ~140-150Hz，
/// 仅受光照/递归驱动的对侧 ~20-80Hz，夜间睡眠 <1Hz。取 150：刺激侧 ≈0.95 不
/// 饱和、对侧 0.13~0.53、夜间 ≈0，左右对比与昼夜对比都保留（原拟 ÷50 会让
/// 两侧同时顶到 1.0，丢失不对称信息）。
const VISION_RATE_REF: f32 = 150.0;
/// vision.optic 满亮参考率（Hz/神经元）。标定依据（同一冒烟）：optic 全集
/// 77,873 个视叶神经元无直接电流注入，仅靠网络递归活动，白天觅食均值
/// ~1.4Hz（峰 ~1.8）、夜间睡眠 ~0.4Hz。取 3：白天 ≈0.45~0.6、夜间 ≈0.13，
/// 昼夜分明且白天不饱和。
const VISION_OPTIC_RATE_REF: f32 = 3.0;
const LIFE_SEED: u64 = 42; // 世界随机种子（食物布局固定）
const CATCH_UP_BATCH: u64 = 50; // 单次 catch-up 最多补的步数
const AUTOSAVE_INTERVAL: Duration = Duration::from_secs(60); // 学习权重自动保存间隔

/// 果蝇快照（fly 字段）。
#[derive(Debug, Clone, Serialize)]
pub struct FlySnap {
    pub x: f32,
    pub z: f32,
    pub heading: f32,
    pub speed: f32,
    pub state: String,
    pub hunger: f32,
    pub energy: f32,
}

/// 蜜源快照（foods 数组元素；只列当前存在的——被吃移除、补货随机新位置）。
#[derive(Debug, Clone, Serialize)]
pub struct FoodSnap {
    pub id: u32,
    pub x: f32,
    pub z: f32,
    pub kind: u8,
}

/// 事件快照（events 数组元素）。
#[derive(Debug, Clone, Serialize)]
pub struct EventSnap {
    pub seq: u64,
    pub kind: String,
    pub text: String,
}

/// 视觉群放电快照（vision 字段）：近 600ms 窗口内该群「每神经元平均发放率 Hz」
/// ÷参考率归一 clamp 到 0..1（参考率标定见常量注释）。
#[derive(Debug, Clone, Serialize)]
pub struct VisionSnap {
    /// 左眼群（sens_left，200 个视觉投射神经元）。
    pub left: f32,
    /// 右眼群（sens_right）。
    pub right: f32,
    /// optic 全集（super_class=="optic" 的 77,873 个视叶神经元）。
    pub optic: f32,
}

/// tick 末发布的生活状态（快照中除 sim_time/brain_activity 外的全部字段）。
#[derive(Debug, Clone, Serialize)]
pub struct LifeStatePub {
    pub time_of_day: f32,
    pub sun_elevation: f32,
    pub is_night: bool,
    pub tick: u64,
    pub speed: f32,
    pub plasticity: bool,
    pub weights_changed: u64,
    pub spikes_total: u64,
    pub fly: FlySnap,
    pub foods: Vec<FoodSnap>,
    pub events: Vec<EventSnap>,
    pub world_radius: f32,
    pub day_length_ticks: u32,
    /// 食物设置：开局蜜源数 / 数量上限（设置面板显示用）。
    pub food_init: u32,
    pub food_max: u32,
}

/// `fly_brain_state` 的完整快照（键名逐字对齐 IPC 契约）。
#[derive(Debug, Clone, Serialize)]
pub struct FlyBrainSnapshot {
    pub time_of_day: f32,
    pub sun_elevation: f32,
    pub is_night: bool,
    pub tick: u64,
    pub sim_time: u64,
    pub speed: f32,
    pub plasticity: bool,
    pub weights_changed: u64,
    pub brain_activity: f32,
    pub spikes_total: u64,
    /// 600ms 窗口内发放神经元索引扁平数组（>8000 等距抽样）。
    pub spikes: Vec<u32>,
    /// 与 spikes 对齐的距快照时刻 ms（int16）。
    pub spike_ages_ms: Vec<i16>,
    /// 近 600ms 窗口内发放过的去重神经元数（对窗口全量计数，不受抽样影响）。
    pub active_neurons: u32,
    pub vision: VisionSnap,
    pub fly: FlySnap,
    pub foods: Vec<FoodSnap>,
    pub events: Vec<EventSnap>,
    pub world_radius: f32,
    pub day_length_ticks: u32,
    /// 食物设置：开局蜜源数 / 数量上限（设置面板显示用）。
    pub food_init: u32,
    pub food_max: u32,
}

/// 控制面状态：命令线程写入，worker 读取并应用。
#[derive(Debug)]
struct CtrlState {
    /// 配速倍率（0.5/1/2；配速循环每轮直接读，即时生效）。
    speed: f32,
    plasticity: bool,
    dirty_plasticity: bool,
    restart: bool,
    /// 立即保存学习权重（learned_save 命令）。
    save_weights: bool,
    /// 重置学习权重到出厂 w0（learned_reset 命令；文件由命令侧删除）。
    reset_learned: bool,
    /// 食物设置（开局数/上限；命令侧合并后的当前值，set_food 置位时 worker 应用）。
    food_init: usize,
    food_max: usize,
    set_food: bool,
}

impl Default for CtrlState {
    fn default() -> Self {
        CtrlState {
            speed: 1.0,
            plasticity: false, // 对齐参考实现的默认（启动不开启可塑性）
            dirty_plasticity: false,
            restart: false,
            save_weights: false,
            reset_learned: false,
            food_init: life::DEFAULT_INIT_FOODS,
            food_max: life::DEFAULT_MAX_FOODS,
            set_food: false,
        }
    }
}

/// worker 与命令线程共享的状态。
pub struct Shared {
    life_state: Mutex<LifeStatePub>,
    /// (sim_now, 当步发放计数) 滚动窗口；brain_activity 用。
    spike_log: Mutex<VecDeque<(u64, u32)>>,
    /// (sim_now, 当步发放索引) 滚动窗口；快照 spikes/spike_ages_ms/active_neurons 用。
    spike_idx_log: Mutex<VecDeque<(u64, Vec<u32>)>>,
    /// 活跃神经元去重位图（常驻 n 字节，快照时置位计数后复位）。
    active_bitmap: Mutex<Vec<u8>>,
    /// (sim_now, 左, 右, optic) 视觉群分组计数滚动窗口；快照 vision 用（第三通道）。
    vision_log: Mutex<VecDeque<(u64, u32, u32, u32)>>,
    /// 三个视觉群的神经元数（left/right/optic），归一化分母。
    vision_ns: (u32, u32, u32),
    sim_now: AtomicU64,
    ctrl: Mutex<CtrlState>,
    /// 归一化全脑点云（fly_brain_positions 数据源，只读）。
    pub positions: PositionsData,
    /// learned_save 的应答通道：最近一次保存结果（Ok=字节数，Err=原因）。
    save_ack: Mutex<Option<Result<u64, String>>>,
    /// 保存结果序号（每次尝试 +1；命令侧据此等待本次保存完成）。
    save_seq: AtomicU64,
}

fn lock<'a, T>(m: &'a Mutex<T>) -> MutexGuard<'a, T> {
    m.lock().unwrap_or_else(|e| e.into_inner())
}

/// 视觉神经群放电追踪：逐神经元分组标志（u8：0=非视觉 1=左眼群 2=右眼群，
/// 139,255 字节）+ optic 布尔位图（super_class=="optic" 全集）。
/// 左/右眼群即 BrainIO 的 sens_left/sens_right（接收食物方位/危险/光照电流的
/// 400 个视觉投射神经元）；optic 全集来自 `Brain::super_class`。
pub struct VisionTracker {
    group: Vec<u8>,
    optic: Vec<u8>,
    pub n_left: u32,
    pub n_right: u32,
    pub n_optic: u32,
}

impl VisionTracker {
    pub fn new(io: &BrainIO, brain: &Brain) -> Self {
        let mut group = vec![0u8; brain.n];
        for &i in &io.sens_left {
            group[i as usize] = 1;
        }
        for &i in &io.sens_right {
            group[i as usize] = 2;
        }
        let mut optic = vec![0u8; brain.n];
        let mut n_optic = 0u32;
        for (i, sc) in brain.super_class.iter().enumerate() {
            if sc.as_str() == "optic" {
                optic[i] = 1;
                n_optic += 1;
            }
        }
        VisionTracker {
            group,
            optic,
            n_left: io.sens_left.len() as u32,
            n_right: io.sens_right.len() as u32,
            n_optic,
        }
    }

    /// 统计一步发放的 (左眼群, 右眼群, optic) 计数。
    #[inline]
    pub fn count(&self, spikes: &[u32]) -> (u32, u32, u32) {
        let (mut l, mut r, mut o) = (0u32, 0u32, 0u32);
        for &s in spikes {
            let s = s as usize;
            match self.group[s] {
                1 => l += 1,
                2 => r += 1,
                _ => {},
            }
            o += self.optic[s] as u32;
        }
        (l, r, o)
    }
}

/// 窗口合计 → vision 字段值：每神经元平均发放率 Hz ÷ 参考率归一 clamp 0..1。
fn vision_level(sum: u32, n: u32, rate_ref: f32) -> f32 {
    if n == 0 {
        return 0.0;
    }
    let hz = sum as f32 / n as f32 / (SPIKE_WINDOW_MS as f32 / 1000.0);
    (hz / rate_ref).clamp(0.0, 1.0)
}

impl Shared {
    /// 应用控制命令并返回当前逻辑状态（speed 吸附到 0.5/1/2）。
    pub fn apply_control(
        &self,
        speed: Option<f64>,
        cmd: Option<String>,
        plasticity: Option<bool>,
    ) -> FlyBrainControlResp {
        let mut c = lock(&self.ctrl);
        if let Some(p) = plasticity {
            c.plasticity = p;
            c.dirty_plasticity = true;
        }
        if let Some(s) = speed {
            c.speed = if s < 0.75 {
                0.5
            } else if s < 1.5 {
                1.0
            } else {
                2.0
            };
        }
        if cmd.as_deref() == Some("restart") {
            c.restart = true;
        }
        FlyBrainControlResp {
            ok: true,
            speed: c.speed,
            plasticity: c.plasticity,
        }
    }

    /// 请求立即保存学习权重，返回当前结果序号（命令侧据此等待）。
    pub fn request_save(&self) -> u64 {
        let seq = self.save_seq.load(Ordering::Relaxed);
        lock(&self.ctrl).save_weights = true;
        seq
    }

    /// 读取保存结果（seq 为序号；None=尚无结果）。
    pub fn save_result(&self) -> Option<(u64, Result<u64, String>)> {
        let seq = self.save_seq.load(Ordering::Relaxed);
        lock(&self.save_ack).clone().map(|r| (seq, r))
    }

    /// 请求把工作权重重置为出厂 w0（learned_reset 命令；文件由命令侧删除）。
    pub fn request_learned_reset(&self) {
        lock(&self.ctrl).reset_learned = true;
    }

    /// 应用食物设置（food_config 命令）：与当前值合并、钳制、置标志位，
    /// worker 在 tick 边界应用到世界并持久化。返回生效的 (开局数, 上限)。
    pub fn apply_food_config(&self, init: Option<u32>, max: Option<u32>) -> (usize, usize) {
        let mut c = lock(&self.ctrl);
        let (i, m) = life::clamp_food_config(
            init.map(|v| v as usize).unwrap_or(c.food_init),
            max.map(|v| v as usize).unwrap_or(c.food_max),
        );
        c.food_init = i;
        c.food_max = m;
        c.set_food = true;
        (i, m)
    }

    /// 组装完整快照：生活状态 + sim_time + brain_activity（窗口发放率归一）
    /// + spikes/spike_ages_ms（索引窗口，>8000 等距抽样，语义与初版一致）。
    pub fn snapshot(&self) -> FlyBrainSnapshot {
        let ls = lock(&self.life_state).clone();
        let sim_now = self.sim_now.load(Ordering::Relaxed);
        let window_spikes: u64 = {
            let log = lock(&self.spike_log);
            log.iter().map(|&(_, c)| c as u64).sum()
        };
        let brain_activity =
            (window_spikes as f32 / (SPIKE_WINDOW_MS as f32 * ACTIVITY_REF_RATE)).clamp(0.0, 1.0);
        // 滚动窗口内发放：扁平索引 + 距现在 ms
        let log = lock(&self.spike_idx_log).clone();
        let total: usize = log.iter().map(|(_, v)| v.len()).sum();
        let mut idx: Vec<u32> = Vec::with_capacity(total.min(MAX_SPIKES));
        let mut ages: Vec<i16> = Vec::with_capacity(total.min(MAX_SPIKES));
        for (t, spikes) in &log {
            let age = sim_now.saturating_sub(*t) as i16;
            for &s in spikes {
                idx.push(s);
                ages.push(age);
            }
        }
        // 窗口全量发放的去重神经元数（常驻位图两趟：置位计数 → 仅对本批索引复位；
        // 位图在两次快照间保持全零，无每步清零开销）
        let active_neurons = {
            let mut bm = lock(&self.active_bitmap);
            let mut cnt = 0u32;
            for &s in &idx {
                let b = &mut bm[s as usize];
                if *b == 0 {
                    *b = 1;
                    cnt += 1;
                }
            }
            for &s in &idx {
                bm[s as usize] = 0;
            }
            cnt
        };
        if idx.len() > MAX_SPIKES {
            let step = idx.len().div_ceil(MAX_SPIKES);
            idx = idx.into_iter().step_by(step).collect();
            ages = ages.into_iter().step_by(step).collect();
        }
        // 视觉群窗口合计 → 归一化 vision
        let (vl, vr, vo) = {
            let log = lock(&self.vision_log);
            log.iter().fold((0u32, 0u32, 0u32), |acc, &(_, l, r, o)| {
                (acc.0 + l, acc.1 + r, acc.2 + o)
            })
        };
        let vision = VisionSnap {
            left: vision_level(vl, self.vision_ns.0, VISION_RATE_REF),
            right: vision_level(vr, self.vision_ns.1, VISION_RATE_REF),
            optic: vision_level(vo, self.vision_ns.2, VISION_OPTIC_RATE_REF),
        };
        FlyBrainSnapshot {
            time_of_day: ls.time_of_day,
            sun_elevation: ls.sun_elevation,
            is_night: ls.is_night,
            tick: ls.tick,
            sim_time: sim_now,
            speed: ls.speed,
            plasticity: ls.plasticity,
            weights_changed: ls.weights_changed,
            brain_activity,
            spikes_total: ls.spikes_total,
            spikes: idx,
            spike_ages_ms: ages,
            active_neurons,
            vision,
            fly: ls.fly,
            foods: ls.foods,
            events: ls.events,
            world_radius: ls.world_radius,
            day_length_ticks: ls.day_length_ticks,
            food_init: ls.food_init,
            food_max: ls.food_max,
        }
    }
}

/// 运行中的 worker 句柄（FlyBrainState 持有；shutdown 时停线程释放脑）。
pub struct Running {
    pub shared: Arc<Shared>,
    stop: Arc<AtomicBool>,
    handle: Option<JoinHandle<()>>,
}

impl Running {
    /// 置停止标志并 join（worker 每批 ≤50 步检查一次，毫秒级响应）。
    pub fn shutdown(mut self) {
        self.stop.store(true, Ordering::Relaxed);
        if let Some(h) = self.handle.take() {
            if h.join().is_err() {
                warn!("果蝇脑 worker 线程 panic");
            }
        }
        info!("果蝇脑 worker 已停止，脑数据已释放");
    }
}

/// 从生活世界与脑状态组装发布用快照（initial 与 end_tick 共用）。
fn build_life_state(life: &FlyLife, brain: &Brain, speed: f32, spikes_total: u64) -> LifeStatePub {
    let (food_init, food_max) = life.food_config();
    LifeStatePub {
        time_of_day: life.time_of_day(),
        sun_elevation: life.sun_elevation(),
        is_night: life.is_night(),
        tick: life.tick,
        speed,
        plasticity: brain.plastic,
        weights_changed: brain.plastic_stats.weights_changed,
        spikes_total,
        fly: FlySnap {
            x: life.fly.x,
            z: life.fly.z,
            heading: life.fly.heading,
            speed: life.fly.speed,
            state: life.fly.state.as_str().to_string(),
            hunger: life.fly.hunger,
            energy: life.fly.energy,
        },
        foods: life
            .foods
            .iter()
            .map(|f| FoodSnap {
                id: f.id,
                x: f.x,
                z: f.z,
                kind: f.kind,
            })
            .collect(),
        events: life
            .events
            .iter()
            .map(|e| EventSnap {
                seq: e.seq,
                kind: e.kind.to_string(),
                text: e.text.clone(),
            })
            .collect(),
        world_radius: WORLD_RADIUS,
        day_length_ticks: life.day_length_ticks,
        food_init: food_init as u32,
        food_max: food_max as u32,
    }
}

/// 构建脑 + BrainIO + 校准 + 生活世界 + 归一化点云，然后 spawn 常驻仿真线程。
/// （learned_weights.bin 持久化禁用；测试/冒烟用）
pub fn start(graph: BrainGraph) -> Result<Running, String> {
    start_with_dir(graph, None)
}

/// 同 `start`，附加 fly_brain 数据目录（含 learned_weights.bin 的持久化）。
pub fn start_with_dir(graph: BrainGraph, dir: Option<PathBuf>) -> Result<Running, String> {
    let t0 = Instant::now();
    let mut brain = Brain::new(graph);
    // 学习权重：若存在且边数匹配则覆盖 weight（w0 出厂基线不动）
    let learned_path = dir.as_ref().map(|d| d.join("learned_weights.bin"));
    if let Some(p) = &learned_path {
        match learned::load_into(p, &mut brain) {
            Ok(true) => info!("已恢复上次学习权重"),
            Ok(false) => {},
            Err(e) => warn!("学习权重加载失败（忽略）: {e}"),
        }
    }
    // 食物设置：读取持久化配置（无文件/损坏则用出厂默认）
    let food_cfg_path = dir.as_ref().map(|d| d.join("food_config.json"));
    let (food_init, food_max) = food_cfg_path
        .as_ref()
        .and_then(|p| load_food_config(p))
        .unwrap_or((life::DEFAULT_INIT_FOODS, life::DEFAULT_MAX_FOODS));
    let positions = PositionsData::from_brain(&brain).ok_or("图缺少 coords，无法生成点云")?;
    let mut io = BrainIO::new(&brain, SENS_K, 0);
    io.calibrate(&mut brain);
    let vision = VisionTracker::new(&io, &brain);
    let life = FlyLife::new_with_food(LIFE_SEED, food_init, food_max);
    info!(
        "果蝇脑准备就绪（加载/校准耗时 {}ms），启动生活 worker 线程",
        t0.elapsed().as_millis()
    );

    let shared = Arc::new(Shared {
        life_state: Mutex::new(build_life_state(&life, &brain, 1.0, 0)),
        spike_log: Mutex::new(VecDeque::new()),
        spike_idx_log: Mutex::new(VecDeque::new()),
        active_bitmap: Mutex::new(vec![0u8; brain.n]),
        vision_log: Mutex::new(VecDeque::new()),
        vision_ns: (vision.n_left, vision.n_right, vision.n_optic),
        sim_now: AtomicU64::new(0),
        ctrl: Mutex::new(CtrlState {
            food_init,
            food_max,
            ..CtrlState::default()
        }),
        positions,
        save_ack: Mutex::new(None),
        save_seq: AtomicU64::new(0),
    });
    let stop = Arc::new(AtomicBool::new(false));

    let mut worker = Worker {
        brain,
        io,
        life,
        vision,
        learned_path,
        food_cfg_path,
        last_save: Instant::now(),
        last_saved_changed: 0,
        drive: Vec::new(),
        sim_now: 0,
        spikes_total: 0,
        tick_steps_left: 0,
    };
    let shared2 = Arc::clone(&shared);
    let stop2 = Arc::clone(&stop);
    let handle = std::thread::Builder::new()
        .name("fly-brain-worker".to_string())
        .spawn(move || worker.run(shared2, stop2))
        .map_err(|e| format!("spawn worker 失败: {e}"))?;
    info!(
        "果蝇生活 worker 线程已启动（每 tick {} 步仿真, 1x 实时配速）",
        SIM_STEPS_PER_TICK
    );
    Ok(Running {
        shared,
        stop,
        handle: Some(handle),
    })
}

struct Worker {
    brain: Brain,
    io: BrainIO,
    life: FlyLife,
    /// 视觉群分组标志（step 记录发放时顺带累加左/右/optic 计数）。
    vision: VisionTracker,
    /// learned_weights.bin 路径（None=持久化禁用）。
    learned_path: Option<PathBuf>,
    /// food_config.json 路径（None=食物设置不持久化）。
    food_cfg_path: Option<PathBuf>,
    /// 上次保存时间 / 上次保存时的 weights_changed（增量判定）。
    last_save: Instant,
    last_saved_changed: u64,
    drive: Vec<(u32, f32)>,
    /// 仿真时钟（ms，dt=1ms 步进）；与 Shared.sim_now 镜像。
    sim_now: u64,
    spikes_total: u64,
    tick_steps_left: u32,
}

impl Worker {
    /// 连续仿真主循环 + 真实时间配速（speed 倍率直接读 ctrl，即时生效）。
    fn run(&mut self, shared: Arc<Shared>, stop: Arc<AtomicBool>) {
        let start = Instant::now();
        let mut paced: u64 = 0; // 已按配速执行的步数
        while !stop.load(Ordering::Relaxed) {
            let speed = lock(&shared.ctrl).speed;
            let target = (start.elapsed().as_secs_f64() * 1000.0 * speed as f64) as u64;
            let mut batch = 0;
            while paced < target && batch < CATCH_UP_BATCH {
                if self.tick_steps_left == 0 {
                    self.begin_tick(&shared);
                }
                self.brain.step(&self.drive);
                self.sim_now += 1;
                shared.sim_now.store(self.sim_now, Ordering::Relaxed);
                let nsp = {
                    let sp = self.brain.last_spikes();
                    if sp.is_empty() {
                        0
                    } else {
                        let (vl, vr, vo) = self.vision.count(sp);
                        lock(&shared.vision_log).push_back((self.sim_now, vl, vr, vo));
                        lock(&shared.spike_idx_log).push_back((self.sim_now, sp.to_vec()));
                        sp.len() as u32
                    }
                };
                if nsp > 0 {
                    lock(&shared.spike_log).push_back((self.sim_now, nsp));
                    self.spikes_total += nsp as u64;
                }
                self.tick_steps_left -= 1;
                paced += 1;
                batch += 1;
                if self.tick_steps_left == 0 {
                    self.end_tick(&shared);
                }
            }
            if paced >= target {
                std::thread::sleep(Duration::from_millis(1));
            }
        }
        // worker 停止前落盘学习权重（有增量才写；plastic 关但已学过也保留成果）
        if learned::has_learned_state(&self.brain)
            && self.brain.plastic_stats.weights_changed > self.last_saved_changed
        {
            self.save_learned_now(&shared);
        }
        info!("果蝇脑 worker 主循环退出");
    }

    /// 保存学习权重并更新应答通道（autosave / save 命令 / exit 落盘共用）。
    fn save_learned_now(&mut self, shared: &Shared) {
        let Some(path) = &self.learned_path else {
            return;
        };
        let result = if learned::has_learned_state(&self.brain) {
            learned::save(path, &self.brain)
        } else {
            Err("从未开启可塑性，无可学权重".to_string())
        };
        if result.is_ok() {
            self.last_saved_changed = self.brain.plastic_stats.weights_changed;
            self.last_save = Instant::now();
        } else if let Err(e) = &result {
            warn!("学习权重保存失败: {e}");
        }
        *lock(&shared.save_ack) = Some(result);
        shared.save_seq.fetch_add(1, Ordering::Relaxed);
    }

    /// 一个决策 tick 的开始：应用控制 → 编码驱动 → 清空窗口计数。
    fn begin_tick(&mut self, shared: &Shared) {
        self.apply_ctrl(shared);
        self.drive = self.life.build_drive(&self.io);
        self.brain.reset_window_counts();
        self.spikes_total = 0;
        self.tick_steps_left = SIM_STEPS_PER_TICK;
    }

    /// 一个决策 tick 的结束：裁剪三条放电窗口 → DN 读出 → 推进世界 → 发布快照。
    fn end_tick(&mut self, shared: &Shared) {
        {
            let mut log = lock(&shared.spike_log);
            while let Some((t, _)) = log.front() {
                if *t + SPIKE_WINDOW_MS < self.sim_now {
                    log.pop_front();
                } else {
                    break;
                }
            }
        }
        {
            let mut log = lock(&shared.spike_idx_log);
            while let Some((t, _)) = log.front() {
                if *t + SPIKE_WINDOW_MS < self.sim_now {
                    log.pop_front();
                } else {
                    break;
                }
            }
        }
        {
            let mut log = lock(&shared.vision_log);
            while let Some((t, ..)) = log.front() {
                if *t + SPIKE_WINDOW_MS < self.sim_now {
                    log.pop_front();
                } else {
                    break;
                }
            }
        }
        let (action, l, r) = self.io.decide(&self.brain);
        self.life.advance(action, l, r, &self.io, &mut self.brain);
        let speed = lock(&shared.ctrl).speed;
        *lock(&shared.life_state) =
            build_life_state(&self.life, &self.brain, speed, self.spikes_total);
        // 学习权重自动保存：可塑性开启且有增量时每 60 秒落盘一次
        if self.brain.plastic
            && self.brain.plastic_stats.weights_changed > self.last_saved_changed
            && self.last_save.elapsed() >= AUTOSAVE_INTERVAL
        {
            self.save_learned_now(shared);
        }
    }

    /// tick 边界应用控制命令（restart / plasticity / save / reset_learned；
    /// speed 由配速循环直接读）。
    /// 死锁修复：先持锁取走标志位并清零，**立即放锁**，再执行慢操作
    /// （enable_plasticity 大数组分配、reset_learned_weights 全量拷贝、
    /// 学习权重文件 IO、spike 窗口清空）——worker 绝不在持有 ctrl 锁时
    /// 做这些重活，否则命令侧的 request_save/request_learned_reset/
    /// apply_control（都要拿同一把 ctrl 锁）会被堵成锁车队，在高负载下
    /// 表现为整个界面卡死。
    fn apply_ctrl(&mut self, shared: &Shared) {
        let (restart, dirty_plasticity, plasticity, save_weights, reset_learned, set_food) = {
            let mut c = lock(&shared.ctrl);
            let flags = (
                c.restart,
                c.dirty_plasticity,
                c.plasticity,
                c.save_weights,
                c.reset_learned,
                c.set_food,
            );
            c.restart = false;
            c.dirty_plasticity = false;
            c.save_weights = false;
            c.reset_learned = false;
            c.set_food = false;
            flags
        };
        if restart {
            self.life.reset();
            self.brain.reset_state(true);
            self.brain.reset_learned_weights();
            lock(&shared.spike_log).clear();
            lock(&shared.spike_idx_log).clear();
            lock(&shared.vision_log).clear();
        }
        if dirty_plasticity {
            if plasticity {
                self.brain.enable_plasticity();
            } else {
                self.brain.disable_plasticity();
            }
        }
        if save_weights {
            self.save_learned_now(shared);
        }
        if reset_learned {
            self.brain.reset_learned_weights();
            self.last_saved_changed = 0;
        }
        if set_food {
            let (init, max) = {
                let c = lock(&shared.ctrl);
                (c.food_init, c.food_max)
            };
            self.life.set_food_config(init, max);
            if let Some(p) = &self.food_cfg_path {
                if let Err(e) = save_food_config(p, init, max) {
                    warn!("食物设置保存失败: {e}");
                }
            }
        }
    }
}

// ─── 食物设置持久化（food_config.json；与 learned_weights.bin 同目录）───

/// 食物设置文件格式（JSON 两字段；损坏/缺字段视为无配置回退默认）。
#[derive(Debug, Serialize, Deserialize)]
struct FoodConfigFile {
    init_foods: usize,
    max_foods: usize,
}

/// 读取食物设置（无文件/损坏 → None，调用方回退默认）。钳制在读取侧不做，
/// 由 life 构造/set_food_config 统一负责。
fn load_food_config(path: &PathBuf) -> Option<(usize, usize)> {
    let text = std::fs::read_to_string(path).ok()?;
    let cfg: FoodConfigFile = serde_json::from_str(&text).ok()?;
    Some(life::clamp_food_config(cfg.init_foods, cfg.max_foods))
}

/// 原子写食物设置（先 .tmp 再删旧 rename，与 learned.rs 同套路）。
fn save_food_config(path: &PathBuf, init_foods: usize, max_foods: usize) -> Result<(), String> {
    let text = serde_json::to_string_pretty(&FoodConfigFile {
        init_foods,
        max_foods,
    })
    .map_err(|e| format!("序列化: {e}"))?;
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent).map_err(|e| format!("mkdir: {e}"))?;
    }
    let tmp = path.with_extension("tmp");
    std::fs::write(&tmp, text).map_err(|e| format!("写 {} 失败: {e}", tmp.display()))?;
    if path.exists() {
        std::fs::remove_file(path).map_err(|e| format!("删除旧文件失败: {e}"))?;
    }
    std::fs::rename(&tmp, path).map_err(|e| format!("rename 失败: {e}"))?;
    Ok(())
}
