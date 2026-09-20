//! 果蝇生活世界：一只果蝇在 2D 圆盘牧场里过日子（觅食/进食/饥饿/昼夜作息）。
//!
//! 取代原小游戏集合（贪吃蛇/Pong/打砖块已删除）：大脑不再"玩游戏"，而是驱动一只
//! 果蝇的生活行为。世界模型是连续 2D 圆盘（半径 [`WORLD_RADIUS`] 单位，x/z 平面），
//! 时间以决策 tick 推进（每 tick = [`SIM_STEPS_PER_TICK`] 仿真步）：
//! - 世界钟：1 天 = [`DAY_LENGTH_TICKS`] tick（1x 速下 4 分钟）；time_of_day ∈ [0,1)，
//!   0=午夜、0.25=日出、0.5=正午、0.75=日落；sun_elevation = sin(2π·(tod−0.25))；
//!   世界从 tod=0.3（上午）开始；
//! - 食物：4 个蜜源（花/果，kind 0/1），种子化随机散布（StdRng 固定种子），被吃后
//!   200~400 tick（随机）原地重生；
//! - 每 tick：构造 percepts（最近食物方位 ±30° 锥 = front，其余按转向较短一侧
//!   给 left/right（含正后方，无方向死角）；距边界 <5 时按外法线方位给 danger；
//!   hunger 递增、白天 ×1.5）→ BrainIO 编码注入 → 150 步仿真 → BrainIO decide
//!   读出转向（±0.35 rad/tick）→ speed = 基础速 × (0.5+DN 总放电归一) × energy
//!   系数 → 移动并 clamp 在圆盘内；
//! - 打转修复（与原作者 README 的现象分析一致）：本 tick 既无食物方位也无危险时
//!   （encode 只注入了对称光照电流，decide 被 DN 残余基线不对称主导会每 tick
//!   同向偏转），忽略 DN 转向输出——直行 + ±0.05 rad 低频漫游摆动（种子确定性），
//!   呈现悠闲巡航而非原地打转；有感知时保留原 MARGIN 判定；
//! - 光照注入：白天给左右视觉群加 ∝ max(0,sun_elevation)×0.5 的电流（昼夜节律），
//!   夜间无注入脑更安静；
//! - 进食：距食物 <2.0 → reward(1.0)、hunger=0、eating 2 tick、食物进入重生计时；
//! - 作息：sun_elevation<−0.2 且 energy<60 → sleeping（不注入 drive、speed=0、
//!   energy +2/tick）；sun_elevation>0 → 醒来（wake 事件）。白天飞行 energy 缓慢
//!   下降，<20 → resting 落地恢复（≥60 起飞）。hunger≥100 → 挨饿：punish(−1.0)、
//!   hunger=50、energy−30（starve 事件）；
//! - 事件日志：{seq 自增, kind: eat|sleep|wake|starve|punish, text 中文短句}，
//!   保留最近 8 条（"punish" 为预留 kind，当前由 starve 承载惩罚语义）。
//!
//! 本模块只管世界规则：仿真推进与配速在 worker.rs，感觉/运动映射在 brain_io.rs。

use std::collections::VecDeque;
use std::f32::consts::{FRAC_PI_6, PI, TAU};

use rand::rngs::StdRng;
use rand::{Rng, SeedableRng};

use super::brain_io::{BrainIO, Danger, Percepts, Rel};
use super::engine::Brain;

pub const WORLD_RADIUS: f32 = 40.0; // 圆盘世界半径（单位）
pub const DAY_LENGTH_TICKS: u32 = 1600; // 1 天的 tick 数（1x 速下 4 分钟）
pub const SIM_STEPS_PER_TICK: u32 = 150; // 每 tick 的仿真步数
const START_TOD: f32 = 0.3; // 世界起始时刻（上午）
const FOOD_COUNT: u32 = 4; // 蜜源数量
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
const FOOD_RESPAWN_MIN: u32 = 200; // 食物重生最短 tick
const FOOD_RESPAWN_SPAN: u32 = 200; // 重生随机跨度（200~400）
const EVENT_CAP: usize = 8; // 事件日志保留条数
// ---- 无感知巡航（打转修复）----
const WANDER_AMP: f32 = 0.05; // 漫游摆动幅度（rad/tick，远小于 TURN_RATE）
const WANDER_PERIOD: f32 = 53.0; // 漫游摆动周期（tick，低频正弦）

/// 果蝇状态（snapshot 序列化为小写字符串）。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FlyState {
    Flying,
    Foraging,
    Eating,
    Resting,
    Sleeping,
}

impl FlyState {
    pub fn as_str(self) -> &'static str {
        match self {
            FlyState::Flying => "flying",
            FlyState::Foraging => "foraging",
            FlyState::Eating => "eating",
            FlyState::Resting => "resting",
            FlyState::Sleeping => "sleeping",
        }
    }
}

/// 蜜源（花/果）。被吃后 `available=false` 进入重生计时。
#[derive(Debug, Clone)]
pub struct Food {
    pub id: u32,
    pub x: f32,
    pub z: f32,
    pub kind: u8,
    pub available: bool,
    /// 重生 tick（仅 available=false 时有意义；测试可覆写以控制窗口）。
    pub respawn_at: u64,
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
}

/// 生活事件（kind 取自 eat|sleep|wake|starve|punish）。
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
    /// 本 tick 的食物方位输入（foraging 判定用；build_drive 时更新）。
    food_rel: Option<Rel>,
    /// 本 tick 是否有方向性感知（食物方位或危险；build_drive 时更新）。
    /// false 时忽略 DN 转向输出（打转修复，见 advance 注释）。
    had_percept: bool,
    /// 漫游摆动相位（种子派生，确定性）。
    wander_phase: f32,
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
            },
            foods: Vec::new(),
            events: VecDeque::with_capacity(EVENT_CAP + 1),
            event_seq: 0,
            food_rel: None,
            had_percept: false,
            wander_phase: 0.0,
        };
        life.scatter_foods();
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

    /// 种子化随机散布 4 个蜜源（距心 10~34 单位，kind 交替 0/1）。
    fn scatter_foods(&mut self) {
        self.foods.clear();
        for id in 0..FOOD_COUNT {
            let ang = self.rng.gen_range(0.0..TAU);
            let r = 10.0 + self.rng.gen_range(0.0..24.0);
            self.foods.push(Food {
                id,
                x: ang.cos() * r,
                z: ang.sin() * r,
                kind: (id % 2) as u8,
                available: true,
                respawn_at: 0,
            });
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

    /// 本 tick 的食物方位输入（build_drive 时更新；foraging 判定与调试用）。
    pub fn food_rel(&self) -> Option<Rel> {
        self.food_rel
    }

    /// 本 tick 是否有方向性感知（build_drive 时更新；打转修复的判定依据）。
    pub fn had_percept(&self) -> bool {
        self.had_percept
    }

    // ---- 感知 ----
    /// 最近的可用蜜源：(食物索引, 距离)。
    fn nearest_food(&self) -> Option<(usize, f32)> {
        let mut best: Option<(usize, f32)> = None;
        for (i, f) in self.foods.iter().enumerate() {
            if !f.available {
                continue;
            }
            let d = (f.x - self.fly.x).hypot(f.z - self.fly.z);
            if best.map_or(true, |(_, bd)| d < bd) {
                best = Some((i, d));
            }
        }
        best
    }

    /// 世界向量 (dx,dz) 相对朝向的方位：前方 ±30° = front，其余按符号给左/右。
    /// 正后方锥区不置 None：按「转向较短一侧」给 Left/Right（rel>0 左转更近、
    /// rel<0 右转更近），保证任何方位都有明确方向输入，消灭打转死角。
    fn rel_side(&self, dx: f32, dz: f32) -> Rel {
        let mut rel = dz.atan2(dx) - self.fly.heading;
        while rel > PI {
            rel -= TAU;
        }
        while rel < -PI {
            rel += TAU;
        }
        if rel.abs() <= FRONT_CONE {
            Rel::Front
        } else if rel > 0.0 {
            Rel::Left
        } else {
            Rel::Right
        }
    }

    /// 边界危险：距边界 <5 时按外法线方位给 front/left/right（至多一个方向）。
    fn boundary_danger(&self) -> Danger {
        let d = self.fly.x.hypot(self.fly.z);
        let mut danger = Danger::default();
        if d > 1e-3 && WORLD_RADIUS - d < DANGER_DIST {
            match self.rel_side(self.fly.x / d, self.fly.z / d) {
                Rel::Front => danger.front = true,
                Rel::Left => danger.left = true,
                Rel::Right => danger.right = true,
            }
        }
        danger
    }

    /// 构造本 tick 的注入电流（percepts 编码 + 光照注入）。
    /// sleeping 完全不注入；eating/resting 只保留光照（无觅食/避险转向输入）。
    /// 顺带维护 `had_percept`：本 tick 是否有方向性感知（食物方位或危险）。
    pub fn build_drive(&mut self, io: &BrainIO) -> Vec<(u32, f32)> {
        if self.fly.state == FlyState::Sleeping {
            self.food_rel = None;
            self.had_percept = false;
            return Vec::new();
        }
        let elev = self.sun_elevation();
        let grounded = matches!(self.fly.state, FlyState::Eating | FlyState::Resting);
        let mut drive = if grounded {
            self.food_rel = None;
            self.had_percept = false;
            Vec::new()
        } else {
            let food_rel = self.nearest_food().map(|(i, _)| {
                self.rel_side(self.foods[i].x - self.fly.x, self.foods[i].z - self.fly.z)
            });
            let danger = self.boundary_danger();
            self.food_rel = food_rel;
            self.had_percept = food_rel.is_some() || danger.front || danger.left || danger.right;
            let percepts = Percepts {
                food_rel,
                danger,
                steps: self.tick,
                hunger: self.fly.hunger as i64,
            };
            io.encode_percepts(&percepts)
        };
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
    /// `action`/`l`/`r` 来自 BrainIO.decide；reward/punish 直接作用于脑。
    pub fn advance(&mut self, action: u8, l: i64, r: i64, io: &BrainIO, brain: &mut Brain) {
        let elev = self.sun_elevation();
        let day = elev > 0.0;
        let airborne = matches!(self.fly.state, FlyState::Flying | FlyState::Foraging);

        // 1) 转向（仅飞行状态；action 1=左转, 2=右转, 0=直行）
        if airborne {
            if self.had_percept {
                // 有方向性感知：采信 DN 读出（MARGIN 死区判定保留在 BrainIO::decide，
                // 与 snake.py 一致，此处不误用/不重复判定）
                match action {
                    1 => self.fly.heading += TURN_RATE,
                    2 => self.fly.heading -= TURN_RATE,
                    _ => {},
                }
            } else {
                // 无感知（既无食物方位也无危险）：encode 只注入了对称光照电流，
                // decide() 会被 DN 左右群的残余基线不对称主导，每 tick 同向转
                // TURN_RATE → 原地打转（与原作者 README 记载的现象一致）。
                // 忽略 DN 转向输出：直行，并叠加低频平滑漫游摆动（±WANDER_AMP），
                // 呈现为悠闲巡航而非打转。
                self.fly.heading += WANDER_AMP
                    * (TAU * (self.tick as f32 / WANDER_PERIOD) + self.wander_phase).sin();
            }
            self.fly.heading = self.fly.heading.rem_euclid(TAU);
        }

        // 2) 速度 = 基础速 × (0.5+DN 总放电归一) × energy 系数；落地状态 speed=0
        let dn_norm = ((l + r) as f64 / (io.norm_l + io.norm_r).max(1.0)).clamp(0.0, 1.0) as f32;
        let energy_factor = 0.3 + 0.7 * (self.fly.energy / 100.0).clamp(0.0, 1.0);
        self.fly.speed = if airborne {
            BASE_SPEED * (0.5 + dn_norm) * energy_factor
        } else {
            0.0
        };

        // 3) 移动并 clamp 在圆盘内（沿边界滑动）
        self.fly.x += self.fly.heading.cos() * self.fly.speed;
        self.fly.z += self.fly.heading.sin() * self.fly.speed;
        let d = self.fly.x.hypot(self.fly.z);
        if d > WORLD_RADIUS {
            self.fly.x *= WORLD_RADIUS / d;
            self.fly.z *= WORLD_RADIUS / d;
        }

        // 4) 进食判定（飞行中距食物 <2.0）：reward、hunger=0、eating 2 tick、重生计时
        if airborne {
            if let Some((fi, dist)) = self.nearest_food() {
                if dist < EAT_DIST {
                    let kind = self.foods[fi].kind;
                    self.foods[fi].available = false;
                    self.foods[fi].respawn_at = self.tick
                        + FOOD_RESPAWN_MIN as u64
                        + self.rng.gen_range(0..FOOD_RESPAWN_SPAN as u64);
                    brain.reward(1.0);
                    self.fly.hunger = 0.0;
                    self.fly.state = FlyState::Eating;
                    self.fly.eating_left = 2;
                    self.push_event(
                        "eat",
                        if kind == 0 {
                            "吃到了花蜜！"
                        } else {
                            "吃到了甜果！"
                        },
                    );
                }
            }
        }

        // 5) 饥饿（进食 tick 不增长）与挨饿惩罚
        if self.fly.state != FlyState::Eating {
            let rate = if day {
                HUNGER_RATE_NIGHT * HUNGER_DAY_FACTOR
            } else {
                HUNGER_RATE_NIGHT
            };
            self.fly.hunger = (self.fly.hunger + rate).min(100.0);
        }
        if self.fly.hunger >= 100.0 {
            brain.punish(-1.0);
            self.fly.hunger = 50.0;
            self.fly.energy = (self.fly.energy - 30.0).max(0.0);
            self.push_event("starve", "饿坏了，遭到惩罚…");
        }

        // 6) 精力与作息状态机
        match self.fly.state {
            FlyState::Eating => {
                self.fly.eating_left = self.fly.eating_left.saturating_sub(1);
                if self.fly.eating_left == 0 {
                    self.fly.state = FlyState::Flying;
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
                }
            },
        }

        // 7) flying/foraging 命名：白天且本 tick 有食物方位输入 → foraging
        if matches!(self.fly.state, FlyState::Flying | FlyState::Foraging) {
            self.fly.state = if day && self.food_rel.is_some() {
                FlyState::Foraging
            } else {
                FlyState::Flying
            };
        }

        // 8) 食物重生
        for f in &mut self.foods {
            if !f.available && self.tick >= f.respawn_at {
                f.available = true;
            }
        }

        self.tick += 1;
    }

    fn fall_asleep(&mut self) {
        self.fly.state = FlyState::Sleeping;
        if self.fly.hunger >= 60.0 {
            self.push_event("sleep", "饿着肚子睡着了…");
        } else {
            self.push_event("sleep", "夜幕降临，睡着了…");
        }
    }
}
