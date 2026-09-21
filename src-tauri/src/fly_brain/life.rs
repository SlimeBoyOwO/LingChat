//! 果蝇生活世界：一只果蝇在 2D 圆盘牧场里过日子（觅食/进食/饥饿/昼夜作息）。
//!
//! 取代原小游戏集合（贪吃蛇/Pong/打砖块已删除）：大脑不再"玩游戏"，而是驱动一只
//! 果蝇的生活行为。世界模型是连续 2D 圆盘（半径 [`WORLD_RADIUS`] 单位，x/z 平面），
//! 时间以决策 tick 推进（每 tick = [`SIM_STEPS_PER_TICK`] 仿真步）：
//! - 世界钟：1 天 = [`DAY_LENGTH_TICKS`] tick（1x 速下 4 分钟）；time_of_day ∈ [0,1)，
//!   0=午夜、0.25=日出、0.5=正午、0.75=日落；sun_elevation = sin(2π·(tod−0.25))；
//!   世界从 tod=0.3（上午）开始；
//! - 食物：蜜源（花/果，kind 0/1 随机）由生成器定期补货——每 60~160 tick 抽一次，
//!   圆盘内面积均匀随机、离边界≥5、离果蝇≥8、与现存蜜源间距≥6（拒绝采样）；
//!   开局 3 个、上限 5 个（达到不再生成），被吃即移除不原地重生；全部种子化
//!   rng，确定性可复现；id 自增唯一。无食物时 food_bearing=None → 漫游分支；
//! - 每 tick：构造 percepts（Fly64 风格连续分级：最近食物方位角 bearing∈[-π,π]
//!   连续 rad、正=左，编码侧线性分级 clamp 到 ±143.5° 视场，无档位突变无死角；
//!   looming=同目标帧间距离差调制吸引电流；自身转向对侧光流电流；距边界 <5 时
//!   按外法线方位给 danger；hunger 递增、白天 ×1.5）→ BrainIO 编码注入 →
//!   150 步仿真 → BrainIO decide 读出转向（±0.35 rad/tick 命令）；
//! - 背景驱动 + 可复现噪声（Fly64 风格）：双侧视觉群恒定 tonic 0.18 + 种子化
//!   低频 EMA 噪声（幅度 0.22，逐 tick 更新，每神经元哈希增益去同步），所有
//!   状态都注入——睡眠/画面静止时脑仍有稀疏活动，夜间明显比白天安静；
//! - 运动平滑：转向角速度与前进速度各经 EMA（τ≈3 tick），限幅保留；
//! - 转向与速度 → speed = 基础速 × (0.5+DN 总放电归一) × energy 系数 →
//!   移动并 clamp 在圆盘内；
//! - 打转修复（与原作者 README 的现象分析一致）：本 tick 既无食物方位也无危险时
//!   （encode 只注入了对称光照电流，decide 被 DN 残余基线不对称主导会每 tick
//!   同向偏转），忽略 DN 转向输出——直行 + ±0.05 rad 低频漫游摆动（种子确定性），
//!   呈现悠闲巡航而非原地打转；有感知时保留原 MARGIN 判定；
//! - 光照注入：白天给左右视觉群加 ∝ max(0,sun_elevation)×0.5 的电流（昼夜节律），
//!   夜间无注入脑更安静；
//! - 进食：距食物 <2.0 → reward(1.0)、hunger=0、eating 2 tick、食物进入重生计时；
//! - 走路（walking）：进食结束后 ~20% 概率落地散步 40~120 tick（随机）后起飞
//!   （概率按验收区间定标，见 LAND_P_AFTER_EAT 注释）；
//!   白天飞行中每 tick 约 1/800 概率自发落地；走路时贴地（前端按 state 处理 y）、
//!   speed = 飞行基础速 ×0.25，转向仍走 DN 读出（无感知时沿用漫游规则），
//!   走路中靠近食物 <2.0 同样可以进食（吃完接着走）；散步中饥饿 ≥70 起飞觅食，
//!   夜间入睡转换从 walking 也允许。落地/起飞各记 land/takeoff 事件；
//! - 作息：sun_elevation<−0.2 且 energy<60 → sleeping（不注入 drive、speed=0、
//!   energy +2/tick）；sun_elevation>0 → 醒来（wake 事件）。白天飞行 energy 缓慢
//!   下降，<20 → resting 落地恢复（≥60 起飞）。hunger≥100 → 挨饿：hunger=50、
//!   energy−30（starve 事件；只罚身体不罚脑——资格迹归因窗口 ~0.5s，挨饿瞬间
//!   果蝇多在积极觅食，punish 会误压觅食突触越学越笨，学习梯度由进食 reward 提供）；
//! - 事件日志：{seq 自增, kind: eat|sleep|wake|starve|land|takeoff|punish,
//!   text 中文短句}，保留最近 8 条（"punish" 为预留 kind，当前未使用）。
//!
//! 本模块只管世界规则：仿真推进与配速在 worker.rs，感觉/运动映射在 brain_io.rs。

use std::collections::VecDeque;
use std::f32::consts::{FRAC_PI_6, PI, TAU};

use rand::rngs::StdRng;
use rand::{Rng, SeedableRng};

use super::brain_io::{BrainIO, Danger, Percepts, SENS_K};
use super::engine::Brain;

pub const WORLD_RADIUS: f32 = 40.0; // 圆盘世界半径（单位）
pub const DAY_LENGTH_TICKS: u32 = 1600; // 1 天的 tick 数（1x 速下 4 分钟）
pub const SIM_STEPS_PER_TICK: u32 = 150; // 每 tick 的仿真步数
const START_TOD: f32 = 0.3; // 世界起始时刻（上午）
const EAT_DIST: f32 = 2.0; // 进食距离
const DANGER_DIST: f32 = 5.0; // 边界危险距离
const FRONT_CONE: f32 = FRAC_PI_6; // 前方锥半角（±30°）
const TURN_RATE: f32 = 0.35; // 每 tick 转向幅度（rad）
const BASE_SPEED: f32 = 0.8; // 基础速度（单位/tick）
const HUNGER_RATE_NIGHT: f32 = 0.25; // 饥饿增速（夜间，每 tick）
const HUNGER_DAY_FACTOR: f32 = 1.5; // 白天饥饿增速倍率
const ENERGY_DRAIN_DAY: f32 = 0.2; // 白天飞行精力消耗（每 tick）
const ENERGY_DRAIN_NIGHT: f32 = 0.05; // 夜间飞行精力消耗
const ENERGY_RECOVER: f32 = 2.0; // 睡觉/休息精力恢复（每 tick）
const REST_ENTER: f32 = 20.0; // 精力低于此值落地休息
const REST_LEAVE: f32 = 60.0; // 休息恢复到该值起飞
const SLEEP_ELEVATION: f32 = -0.2; // 太阳高度低于此值且精力不足 → 睡觉
const LIGHT_CURR_FACTOR: f32 = 0.5; // 光照注入电流系数
// ---- 蜜源补货（随机生成 + 数量限制；取代原地重生）----
const INIT_FOODS: usize = 6; // 开局蜜源数
const MAX_FOODS: usize = 10; // 数量上限（达到不再生成）
const SPAWN_INTERVAL_MIN: u32 = 50; // 补货抽签间隔最短 tick
const SPAWN_INTERVAL_MAX: u32 = 130; // 补货抽签间隔最长 tick
const SPAWN_BOUNDARY_MARGIN: f32 = 5.0; // 离边界（圆周）最小距离
const SPAWN_FLY_DIST: f32 = 8.0; // 离果蝇当前位置最小距离（避免刷脸）
const SPAWN_FOOD_DIST: f32 = 6.0; // 与现存蜜源最小间距
const SPAWN_TRIES: u32 = 30; // 单次补货的拒绝采样次数上限
const EVENT_CAP: usize = 8; // 事件日志保留条数
// ---- 无感知巡航（打转修复）----
const WANDER_AMP: f32 = 0.05; // 漫游摆动幅度（rad/tick，远小于 TURN_RATE）
const WANDER_PERIOD: f32 = 53.0; // 漫游摆动周期（tick，低频正弦）
// ---- 地面走路 ----
const WALK_SPEED_FACTOR: f32 = 0.25; // 走路速度 = 飞行基础速 ×0.25（贴地悠闲散步）
const WALK_MIN_TICKS: u32 = 40; // 散步最短 tick
const WALK_MAX_TICKS: u32 = 120; // 散步最长 tick
const LAND_P_AFTER_EAT: f64 = 0.2; // 进食结束后落地散步概率（定标见下行冒烟记录：
// 觅食成功率高（~8 次进食/600 tick）下，0.35 会使
//   walking 占白天 ~53% 超出验收区间上限 40%；
//   0.2 落在 ~25-35%。任务文本原为"~35%"的近似值）
const LAND_P_SPONT: f64 = 1.0 / 800.0; // 白天飞行中每 tick 自发落地概率
const WALK_TAKEOFF_HUNGER: f32 = 70.0; // 散步中饥饿达到此值起飞觅食
// ---- 背景驱动 + 可复现噪声（Fly64 风格：画面静止时脑也活着）----
// 定标出处：Fly64 动力学 `v ← exp(-dt/0.1)·v + 1.5·W·spikes + 0.180 + noise + retina`
// （全局 tonic 0.180；噪声为种子化 Bernoulli 背景活动 1.2Hz、幅度 0.22）。
// 我们 tick=150ms（其 20ms），噪声改为逐 tick 低频 EMA 随机游走（双侧视觉群各一
// 通道）× 每神经元固定增益（Knuth 哈希 0.6~1.4 去同步），全部为亚阈值电流，
// 靠网络递归放大为稀疏活动。冒烟实测标定见报告。
const BG_TONIC: f32 = 0.18; // 恒定背景电流（每视觉神经元）
const BG_NOISE_AMP: f32 = 0.22; // 低频噪声幅度（每侧视觉群一个通道）
const BG_NOISE_EMA: f32 = 0.85; // 噪声 EMA 系数（每 tick 更新，≈6.7 tick 时间常数）
// ---- 运动平滑（Fly64 操纵杆平滑/死区/限幅的对应）----
const MOTION_EMA_ALPHA: f32 = 1.0 / 3.0; // 转向/速度 EMA（τ≈3 tick；Fly64 为
//   0.78/0.22，即 τ≈4.5 @ 其 20ms tick）
// ---- 近食减速 ----
// EMA 平滑后转向角速度爬升变缓，转弯半径 v/ω 变大；不减速时果蝇会在蜜源外侧
// 以 ~3 单位半径绕圈、进不了 <2.0 的进食圈（冒烟实测 A 段仅 1 次进食+2 次挨饿）。
// 距食物 <6 单位时线性减速到 ~45%：转弯半径减半至 ~1.6 < 2.0，恢复咬食能力。
// 果蝇逼近目标减速也是自然行为。
const NEAR_SLOW_DIST: f32 = 6.0; // 近食减速起始距离
const NEAR_SLOW_MIN: f32 = 0.45; // 贴近时的速度系数下限

/// 果蝇状态（snapshot 序列化为小写字符串）。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FlyState {
    Flying,
    Foraging,
    Eating,
    Resting,
    Sleeping,
    /// 地面走路：贴地散步（speed = 飞行基础速 ×0.25），转向仍走 DN 读出。
    Walking,
}

impl FlyState {
    pub fn as_str(self) -> &'static str {
        match self {
            FlyState::Flying => "flying",
            FlyState::Foraging => "foraging",
            FlyState::Eating => "eating",
            FlyState::Resting => "resting",
            FlyState::Sleeping => "sleeping",
            FlyState::Walking => "walking",
        }
    }
}

/// 蜜源（花/果）。被吃后从 vec 移除；补货在随机新位置生成（见 try_spawn_food）。
#[derive(Debug, Clone)]
pub struct Food {
    pub id: u32,
    pub x: f32,
    pub z: f32,
    pub kind: u8,
}

/// 果蝇本体。
#[derive(Debug, Clone)]
pub struct Fly {
    pub x: f32,
    pub z: f32,
    pub heading: f32,
    pub speed: f32,
    pub hunger: f32,
    pub energy: f32,
    pub state: FlyState,
    eating_left: u32,
    /// 剩余散步 tick（walking 倒计时；起飞/入睡时清零）。
    walk_left: u32,
    /// 转向角速度 EMA（rad/tick；运动平滑，τ≈3 tick）。
    turn_ema: f32,
    /// 前进速度 EMA（单位/tick）。
    speed_ema: f32,
}

/// 生活事件（kind 取自 eat|sleep|wake|starve|land|takeoff|punish）。
#[derive(Debug, Clone)]
pub struct LifeEvent {
    pub seq: u64,
    pub kind: &'static str,
    pub text: String,
}

/// 果蝇生活世界。
pub struct FlyLife {
    seed: u64,
    /// 一天的 tick 数（冒烟测试可临时改小，快照原样透出）。
    pub day_length_ticks: u32,
    pub tick: u64,
    rng: StdRng,
    pub fly: Fly,
    pub foods: Vec<Food>,
    pub events: VecDeque<LifeEvent>,
    event_seq: u64,
    /// 本 tick 的食物方位角输入（rad，正=左，连续；foraging 判定用；
    /// build_drive 时更新）。
    food_bearing: Option<f32>,
    /// looming 追踪：上一 tick 最近食物的 (id, 距离)。
    food_track: Option<(u32, f32)>,
    /// 本 tick 最近食物的距离（近食减速用；build_drive 时更新）。
    food_dist: Option<f32>,
    /// 本 tick 是否有方向性感知（食物方位或危险；build_drive 时更新）。
    /// false 时忽略 DN 转向输出（打转修复，见 advance 注释）。
    had_percept: bool,
    /// 漫游摆动相位（种子派生，确定性）。
    wander_phase: f32,
    /// 上一 tick 实际转向角速度（rad/tick，正=左；光流通道用）。
    last_turn_rate: f32,
    /// 背景噪声通道（左/右视觉群各一，低频 EMA 随机游走，种子化可复现）。
    bg_noise_l: f32,
    bg_noise_r: f32,
    /// 噪声专用 rng（与世界 rng 分流，互不干扰序列）。
    noise_rng: StdRng,
    /// 蜜源自增 id（保证唯一；不回收）。
    next_food_id: u32,
    /// 下一次补货抽签的 tick（冒烟测试可覆写以控制窗口）。
    pub next_spawn_at: u64,
}

impl FlyLife {
    pub fn new(seed: u64) -> Self {
        let mut life = FlyLife {
            seed,
            day_length_ticks: DAY_LENGTH_TICKS,
            tick: 0,
            rng: StdRng::seed_from_u64(seed),
            fly: Fly {
                x: 0.0,
                z: 0.0,
                heading: 0.0,
                speed: 0.0,
                hunger: 20.0,
                energy: 100.0,
                state: FlyState::Flying,
                eating_left: 0,
                walk_left: 0,
                turn_ema: 0.0,
                speed_ema: 0.0,
            },
            foods: Vec::new(),
            events: VecDeque::with_capacity(EVENT_CAP + 1),
            event_seq: 0,
            food_bearing: None,
            food_track: None,
            food_dist: None,
            had_percept: false,
            wander_phase: 0.0,
            last_turn_rate: 0.0,
            bg_noise_l: 0.0,
            bg_noise_r: 0.0,
            noise_rng: StdRng::seed_from_u64(seed.wrapping_mul(0x9E3779B97F4A7C15) ^ 0x42),
            next_food_id: 0,
            next_spawn_at: 0,
        };
        // 开局 INIT_FOODS 个蜜源（同一套拒绝采样生成器，果蝇在原点）
        for _ in 0..INIT_FOODS {
            life.try_spawn_food();
        }
        // 首次补货抽签间隔
        life.next_spawn_at = life.rng.gen_range(SPAWN_INTERVAL_MIN..=SPAWN_INTERVAL_MAX) as u64;
        life.fly.heading = life.rng.gen_range(0.0..TAU);
        life.wander_phase = life.rng.gen_range(0.0..TAU);
        life
    }

    /// 重置世界与果蝇（restart）：同样的种子布局、世界钟归零、事件清空。
    pub fn reset(&mut self) {
        let day_length = self.day_length_ticks; // 测试覆盖保留
        *self = FlyLife::new_with_day_length(self.seed, day_length);
    }

    fn new_with_day_length(seed: u64, day_length_ticks: u32) -> Self {
        let mut life = FlyLife::new(seed);
        life.day_length_ticks = day_length_ticks;
        life
    }

    /// 蜜源补货：圆盘内面积均匀随机（r = √u·(R−5)）拒绝采样——离边界 ≥5、
    /// 离果蝇当前位置 ≥8（避免刷脸）、与现存蜜源间距 ≥6；最多 SPAWN_TRIES 次，
    /// 失败则等下一轮抽签。kind 随机（0 花/1 果）。全部走种子化 rng（确定性）。
    fn try_spawn_food(&mut self) {
        for _ in 0..SPAWN_TRIES {
            let ang = self.rng.gen_range(0.0..TAU);
            let r = self.rng.gen_range(0.0..1.0f32).sqrt() * (WORLD_RADIUS - SPAWN_BOUNDARY_MARGIN);
            let (x, z) = (ang.cos() * r, ang.sin() * r);
            if (x - self.fly.x).hypot(z - self.fly.z) < SPAWN_FLY_DIST {
                continue;
            }
            if self
                .foods
                .iter()
                .any(|f| (f.x - x).hypot(f.z - z) < SPAWN_FOOD_DIST)
            {
                continue;
            }
            let kind = self.rng.gen_range(0..2) as u8;
            let id = self.next_food_id;
            self.next_food_id += 1;
            self.foods.push(Food { id, x, z, kind });
            return;
        }
    }

    // ---- 世界钟 ----
    pub fn time_of_day(&self) -> f32 {
        (START_TOD + self.tick as f32 / self.day_length_ticks as f32).rem_euclid(1.0)
    }
    pub fn sun_elevation(&self) -> f32 {
        (TAU * (self.time_of_day() - 0.25)).sin()
    }
    pub fn is_night(&self) -> bool {
        self.sun_elevation() < 0.0
    }

    /// 本 tick 的食物方位角输入（rad，正=左；build_drive 时更新；调试用）。
    pub fn food_bearing(&self) -> Option<f32> {
        self.food_bearing
    }

    /// 本 tick 是否有方向性感知（build_drive 时更新；打转修复的判定依据）。
    pub fn had_percept(&self) -> bool {
        self.had_percept
    }

    // ---- 感知 ----
    /// 最近的蜜源：(foods 索引, 距离)。foods 只存当前存在的蜜源（被吃即移除）。
    fn nearest_food(&self) -> Option<(usize, f32)> {
        let mut best: Option<(usize, f32)> = None;
        for (i, f) in self.foods.iter().enumerate() {
            let d = (f.x - self.fly.x).hypot(f.z - self.fly.z);
            if best.map_or(true, |(_, bd)| d < bd) {
                best = Some((i, d));
            }
        }
        best
    }

    /// 世界向量 (dx,dz) 相对朝向的连续方位角（rad，wrap 到 [-π,π]，正=左）。
    /// 连续分级编码的基础：正后方不再置 None，任何方位都有明确角度输入
    /// （分级时在编码侧 clamp 到较近一侧，消灭打转死角）。
    fn bearing_to(&self, dx: f32, dz: f32) -> f32 {
        let mut rel = dz.atan2(dx) - self.fly.heading;
        while rel > PI {
            rel -= TAU;
        }
        while rel < -PI {
            rel += TAU;
        }
        rel
    }

    /// 边界危险：距边界 <5 时按外法线方位给 front/left/right（至多一个方向，
    /// 离散三向保留原版设计——危险回避不需要连续量）。
    fn boundary_danger(&self) -> Danger {
        let d = self.fly.x.hypot(self.fly.z);
        let mut danger = Danger::default();
        if d > 1e-3 && WORLD_RADIUS - d < DANGER_DIST {
            let b = self.bearing_to(self.fly.x / d, self.fly.z / d);
            if b.abs() <= FRONT_CONE {
                danger.front = true;
            } else if b > 0.0 {
                danger.left = true;
            } else {
                danger.right = true;
            }
        }
        danger
    }

    /// Knuth 乘法哈希 → 每神经元固定背景增益 [0.6, 1.4]（去同步，无状态确定性）。
    fn bg_gain(i: u32) -> f32 {
        let h = i.wrapping_mul(2654435761) ^ 0x9E3779B9;
        0.6 + 0.8 * ((h >> 8) as f32 / 16_777_216.0)
    }

    /// 背景驱动：双侧视觉群恒定 tonic + 低频噪声（EMA 随机游走，逐 tick 更新）。
    /// 所有状态都注入（含睡眠——对应 Fly64「画面静止时脑也活着」）。
    fn background_drive(&mut self, io: &BrainIO) -> Vec<(u32, f32)> {
        self.bg_noise_l = self.bg_noise_l * BG_NOISE_EMA
            + self.noise_rng.gen_range(-1.0..1.0) * (1.0 - BG_NOISE_EMA);
        self.bg_noise_r = self.bg_noise_r * BG_NOISE_EMA
            + self.noise_rng.gen_range(-1.0..1.0) * (1.0 - BG_NOISE_EMA);
        let cl = (BG_TONIC + BG_NOISE_AMP * self.bg_noise_l).max(0.0);
        let cr = (BG_TONIC + BG_NOISE_AMP * self.bg_noise_r).max(0.0);
        let mut drive = Vec::with_capacity(SENS_K * 2 + 8);
        for &i in &io.sens_left {
            drive.push((i, cl * Self::bg_gain(i)));
        }
        for &i in &io.sens_right {
            drive.push((i, cr * Self::bg_gain(i)));
        }
        drive
    }

    /// 构造本 tick 的注入电流：背景 tonic+噪声（所有状态含睡眠）打底，
    /// 醒着时叠加 percepts 编码（连续分级食物方位 + looming + 光流 + 危险）
    /// 与白天光照。顺带维护 `had_percept` / `food_bearing` / looming 追踪。
    pub fn build_drive(&mut self, io: &BrainIO) -> Vec<(u32, f32)> {
        let mut drive = self.background_drive(io);
        if self.fly.state == FlyState::Sleeping {
            self.food_bearing = None;
            self.food_track = None;
            self.food_dist = None;
            self.had_percept = false;
            return drive; // 睡眠只有背景驱动
        }
        let elev = self.sun_elevation();
        let grounded = matches!(self.fly.state, FlyState::Eating | FlyState::Resting);
        if grounded {
            self.food_bearing = None;
            self.food_track = None;
            self.food_dist = None;
            self.had_percept = false;
        } else {
            // 最近食物：连续方位角 + looming（同目标帧间距离差，正=逼近）
            let (bearing, approach) = match self.nearest_food() {
                Some((i, dist)) => {
                    let f = &self.foods[i];
                    let b = self.bearing_to(f.x - self.fly.x, f.z - self.fly.z);
                    let ap = match self.food_track {
                        Some((pid, pd)) if pid == f.id => pd - dist,
                        _ => 0.0,
                    };
                    self.food_track = Some((f.id, dist));
                    self.food_dist = Some(dist);
                    (Some(b), ap)
                },
                None => {
                    self.food_track = None;
                    self.food_dist = None;
                    (None, 0.0)
                },
            };
            let danger = self.boundary_danger();
            self.food_bearing = bearing;
            self.had_percept = bearing.is_some() || danger.front || danger.left || danger.right;
            let percepts = Percepts {
                food_bearing: bearing,
                food_approach: approach,
                turn_rate: self.last_turn_rate,
                danger,
                steps: self.tick,
                hunger: self.fly.hunger as i64,
            };
            drive.extend(io.encode_percepts(&percepts));
        }
        // 光照注入：白天左右视觉群加 ∝ max(0,sun_elevation)×0.5 的电流（昼夜节律）
        let light = elev.max(0.0) * LIGHT_CURR_FACTOR;
        if light > 0.0 {
            for &i in io.sens_left.iter().chain(io.sens_right.iter()) {
                drive.push((i, light));
            }
        }
        drive
    }

    fn push_event(&mut self, kind: &'static str, text: impl Into<String>) {
        self.event_seq += 1;
        if self.events.len() == EVENT_CAP {
            self.events.pop_front();
        }
        self.events.push_back(LifeEvent {
            seq: self.event_seq,
            kind,
            text: text.into(),
        });
    }

    /// 一个决策 tick 的世界推进：转向 → 移动 → 进食 → 饥饿/精力 → 作息状态机。
    /// `action`/`l`/`r` 来自 BrainIO.decide；进食 reward 直接作用于脑。
    pub fn advance(&mut self, action: u8, l: i64, r: i64, io: &BrainIO, brain: &mut Brain) {
        let elev = self.sun_elevation();
        let day = elev > 0.0;
        let airborne = matches!(self.fly.state, FlyState::Flying | FlyState::Foraging);
        let walking = self.fly.state == FlyState::Walking;
        let mobile = airborne || walking; // 会动的状态（飞行/走路）

        // 1) 转向（飞行与走路状态；action 1=左转, 2=右转, 0=直行；
        //    EMA 平滑 τ≈3 tick，限幅天然保留——EMA 永不超出命令幅值 ±TURN_RATE）
        if !mobile {
            // 落地即清转向 EMA（不起滑），起飞从 0 平滑加速
            self.fly.turn_ema = 0.0;
            self.last_turn_rate = 0.0;
        } else if self.had_percept {
            // 有方向性感知：采信 DN 读出（MARGIN 死区判定保留在 BrainIO::decide，
            // 与 snake.py 一致，此处不误用/不重复判定），EMA 平滑后应用
            let raw = match action {
                1 => TURN_RATE,
                2 => -TURN_RATE,
                _ => 0.0,
            };
            self.fly.turn_ema += (raw - self.fly.turn_ema) * MOTION_EMA_ALPHA;
            self.fly.heading = (self.fly.heading + self.fly.turn_ema).rem_euclid(TAU);
            self.last_turn_rate = self.fly.turn_ema;
        } else {
            // 无感知（既无食物方位也无危险）：encode 只注入了对称光照/背景电流，
            // decide() 会被 DN 左右群的残余基线不对称主导，每 tick 同向转
            // TURN_RATE → 原地打转（与原作者 README 记载的现象一致）。
            // 忽略 DN 转向输出：直行 + 低频漫游摆动（±WANDER_AMP）。
            // 不走 EMA：避免上一有感知 tick 的残余转向打破防打转保护。
            let w =
                WANDER_AMP * (TAU * (self.tick as f32 / WANDER_PERIOD) + self.wander_phase).sin();
            self.fly.turn_ema = w;
            self.fly.heading = (self.fly.heading + w).rem_euclid(TAU);
            self.last_turn_rate = w;
        }

        // 2) 速度（EMA 平滑）：飞行 = 基础速 × (0.5+DN 总放电归一) × energy 系数；
        //    走路 = 基础速 ×0.25（贴地散步，不随 DN/精力缩放）；
        //    其余落地状态即停（EMA 清零，起飞从 0 平滑加速，不起滑）
        let dn_norm = ((l + r) as f64 / (io.norm_l + io.norm_r).max(1.0)).clamp(0.0, 1.0) as f32;
        let energy_factor = 0.3 + 0.7 * (self.fly.energy / 100.0).clamp(0.0, 1.0);
        // 近食减速：距食物 <6 单位线性减到 ~45%（缩小 EMA 平滑后的转弯半径，
        // 保证能进入 <2.0 进食圈；逼近减速也是自然行为）
        let near_slow = match (airborne, self.food_dist) {
            (true, Some(d)) if d < NEAR_SLOW_DIST => {
                NEAR_SLOW_MIN + (1.0 - NEAR_SLOW_MIN) * (d / NEAR_SLOW_DIST)
            },
            _ => 1.0,
        };
        let speed_target = if airborne {
            BASE_SPEED * (0.5 + dn_norm) * energy_factor * near_slow
        } else if walking {
            BASE_SPEED * WALK_SPEED_FACTOR
        } else {
            0.0
        };
        if mobile {
            self.fly.speed_ema += (speed_target - self.fly.speed_ema) * MOTION_EMA_ALPHA;
        } else {
            self.fly.speed_ema = 0.0;
        }
        self.fly.speed = self.fly.speed_ema;

        // 3) 移动并 clamp 在圆盘内（沿边界滑动）
        self.fly.x += self.fly.heading.cos() * self.fly.speed;
        self.fly.z += self.fly.heading.sin() * self.fly.speed;
        let d = self.fly.x.hypot(self.fly.z);
        if d > WORLD_RADIUS {
            self.fly.x *= WORLD_RADIUS / d;
            self.fly.z *= WORLD_RADIUS / d;
        }

        // 4) 进食判定（飞行/走路中距食物 <2.0）：reward、hunger=0、eating 2 tick、
        //    食物移除（补货交给生成器在随机新位置进行，见 step 8）
        if mobile {
            if let Some((fi, dist)) = self.nearest_food() {
                if dist < EAT_DIST {
                    let kind = self.foods[fi].kind;
                    self.foods.swap_remove(fi);
                    brain.reward(1.0);
                    self.fly.hunger = 0.0;
                    self.fly.state = FlyState::Eating;
                    self.fly.eating_left = 2;
                    self.push_event(
                        "eat",
                        if kind == 0 {
                            "吃到了团子！"
                        } else {
                            "捡到了赛钱！"
                        },
                    );
                }
            }
        }

        // 5) 饥饿（进食 tick 不增长）与挨饿后果
        if self.fly.state != FlyState::Eating {
            let rate = if day {
                HUNGER_RATE_NIGHT * HUNGER_DAY_FACTOR
            } else {
                HUNGER_RATE_NIGHT
            };
            self.fly.hunger = (self.fly.hunger + rate).min(100.0);
        }
        if self.fly.hunger >= 100.0 {
            // 挨饿只罚身体、不罚脑：资格迹只能归因到最近 ~0.5s 的活动，而挨饿瞬间
            // 果蝇往往正在积极觅食——此时 punish 会压低觅食突触权重，越学越不敢找食
            // （负向 reward shaping 事故）。觅食的学习梯度由进食 reward(1.0) 单边提供。
            self.fly.hunger = 50.0;
            self.fly.energy = (self.fly.energy - 30.0).max(0.0);
            self.push_event("starve", "饿坏了，体力受损…");
        }

        // 6) 精力与作息状态机
        match self.fly.state {
            FlyState::Eating => {
                self.fly.eating_left = self.fly.eating_left.saturating_sub(1);
                if self.fly.eating_left == 0 {
                    if self.fly.walk_left > 0 {
                        // 走路中吃的：吃完接着走
                        self.fly.state = FlyState::Walking;
                    } else if self.rng.gen::<f64>() < LAND_P_AFTER_EAT {
                        // 进食结束 ~35% 概率落地散步 40~120 tick
                        self.fly.state = FlyState::Walking;
                        self.fly.walk_left = self.rng.gen_range(WALK_MIN_TICKS..=WALK_MAX_TICKS);
                        self.push_event("land", "落地散散步");
                    } else {
                        self.fly.state = FlyState::Flying;
                    }
                }
            },
            FlyState::Resting => {
                self.fly.energy = (self.fly.energy + ENERGY_RECOVER).min(100.0);
                if elev < SLEEP_ELEVATION && self.fly.energy < 60.0 {
                    self.fall_asleep();
                } else if self.fly.energy >= REST_LEAVE {
                    self.fly.state = FlyState::Flying;
                }
            },
            FlyState::Sleeping => {
                self.fly.energy = (self.fly.energy + ENERGY_RECOVER).min(100.0);
                if elev > 0.0 {
                    self.fly.state = FlyState::Flying;
                    self.push_event("wake", "天亮了，起床觅食！");
                }
            },
            FlyState::Walking => {
                // 散步倒计时；走路不耗精力也不恢复（低强度活动）
                self.fly.walk_left = self.fly.walk_left.saturating_sub(1);
                if self.fly.hunger >= WALK_TAKEOFF_HUNGER {
                    // 饿了 → 起飞觅食
                    self.take_off();
                } else if elev < SLEEP_ELEVATION && self.fly.energy < 60.0 {
                    // 夜间入睡（走路本来就贴着地）
                    self.fall_asleep();
                } else if self.fly.walk_left == 0 {
                    // 散步结束 → 起飞
                    self.take_off();
                }
            },
            FlyState::Flying | FlyState::Foraging => {
                let drain = if day {
                    ENERGY_DRAIN_DAY
                } else {
                    ENERGY_DRAIN_NIGHT
                };
                self.fly.energy = (self.fly.energy - drain).max(0.0);
                if elev < SLEEP_ELEVATION && self.fly.energy < 60.0 {
                    self.fall_asleep();
                } else if self.fly.energy < REST_ENTER {
                    self.fly.state = FlyState::Resting;
                } else if day && self.rng.gen::<f64>() < LAND_P_SPONT {
                    // 白天飞行中每 tick 小概率自发落地散步
                    self.fly.state = FlyState::Walking;
                    self.fly.walk_left = self.rng.gen_range(WALK_MIN_TICKS..=WALK_MAX_TICKS);
                    self.push_event("land", "落地散散步");
                }
            },
        }

        // 7) flying/foraging 命名：白天且本 tick 有食物方位输入 → foraging
        if matches!(self.fly.state, FlyState::Flying | FlyState::Foraging) {
            self.fly.state = if day && self.food_bearing.is_some() {
                FlyState::Foraging
            } else {
                FlyState::Flying
            };
        }

        // 8) 蜜源补货：每 50~130 tick 抽一次签，未达上限则随机补 1~2 个到新位置
        if self.tick >= self.next_spawn_at {
            self.next_spawn_at =
                self.tick + self.rng.gen_range(SPAWN_INTERVAL_MIN..=SPAWN_INTERVAL_MAX) as u64;
            for _ in 0..self.rng.gen_range(1..=2) {
                if self.foods.len() < MAX_FOODS {
                    self.try_spawn_food();
                }
            }
        }

        self.tick += 1;
    }

    /// 起飞（散步结束/饿了去觅食）：清散步倒计时并记事件。
    fn take_off(&mut self) {
        self.fly.state = FlyState::Flying;
        self.fly.walk_left = 0;
        self.push_event("takeoff", "起飞啦！");
    }

    fn fall_asleep(&mut self) {
        self.fly.state = FlyState::Sleeping;
        self.fly.walk_left = 0; // 入睡即中断本次散步
        if self.fly.hunger >= 60.0 {
            self.push_event("sleep", "饿着肚子睡着了…");
        } else {
            self.push_event("sleep", "夜幕降临，睡着了…");
        }
    }
}
