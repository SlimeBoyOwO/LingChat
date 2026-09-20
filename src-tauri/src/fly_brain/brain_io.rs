//! 脑 IO 映射：感觉编码（游戏状态 → 注入电流）与运动读出（DN 放电 → 动作）。
//!
//! 移植自 fly-snake `snake.py` 的 `BrainIO` 类（190-365 行）。IO 映射是人为工程约定
//! （不是生物学事实）：
//! - 感觉群：优先 super_class ∈ {visual_projection, visual_centrifugal} 的视觉投射
//!   神经元（不足 2*SENS_K 时依次回退 sensory → optic → 全脑），按 side 标注分
//!   左/右（任一侧不足 8 个时回退 coords 左右轴中位数），每侧固定种子抽 SENS_K 个；
//! - 运动读出：super_class == "descending" 的 DN 群（不足 10 个回退 "motor"），
//!   同样分左右，比较决策窗口内两群放电总数决定 直行/左转/右转；
//! - 校准（calibrate）：3 次 200 步探针测增益基线（norm_l/norm_r）与左右符号
//!   （flip），使"食物在左 → 左转"成立。
//!
//! 与 Python 的差异：抽样用 rand 0.8 的 `StdRng::seed_from_u64(0)`（不要求与
//! Python 的 PCG64 序列一致），其余规则逐行对齐 snake.py 298-365。

use rand::SeedableRng;
use rand::rngs::StdRng;
use rand::seq::SliceRandom;
use tracing::info;

use super::engine::Brain;

// ---- 参数常量（与 snake.py 49-53 行一致）----
pub const SENS_K: usize = 200; // 每侧视觉群选取的神经元数
pub const CURR_FOOD: f32 = 1.5; // 食物吸引电流
pub const CURR_DANGER: f32 = 2.0; // 危险回避电流（略强，优先保命）
pub const MARGIN: f64 = 0.05; // DN 决策死区：归一化后 |L-R|/max(L,R) 小于此值直行
const CALIBRATE_SIM_MS: usize = 200; // 校准探针的仿真步数

/// 目标在控制轴的哪一侧（food_rel 语义）。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Rel {
    Left,
    Right,
    Front,
}

/// 三方向危险标志。
#[derive(Debug, Clone, Copy, Default)]
pub struct Danger {
    pub front: bool,
    pub left: bool,
    pub right: bool,
}

/// 统一感知（任何世界把状态整理成 percepts 即可复用 BrainIO 编码；
/// 语义对齐 snake.py 的 encode_percepts 文档）。BrainIO 只依赖 engine，
/// 世界/游戏实现（如 life.rs）依赖本类型。
#[derive(Debug, Clone, Copy)]
pub struct Percepts {
    pub food_rel: Option<Rel>,
    pub danger: Danger,
    /// 用于破对称的奇偶。
    pub steps: u64,
    /// 饥饿增强觅食电流（无饥饿机制时传 0）。
    pub hunger: i64,
}

/// 脑 IO 映射（感觉群 / DN 群索引 + 校准参数）。
///
/// `sens_left` / `sens_right` 为公有字段：左右两个感觉群（各 SENS_K=200 个
/// visual_projection+visual_centrifugal 神经元）的索引，正是接收食物方位/危险/
/// 光照电流的那些神经元——worker 用它们构建逐神经元视觉分组标志（0=非视觉
/// 1=左眼群 2=右眼群），把"复眼"放电暴露给前端。super_class=="optic" 的全脑
/// 视叶神经元全集可从 `engine::Brain::super_class` 直接判定（worker 侧构建位图）。
pub struct BrainIO {
    /// 左眼群神经元索引（≤SENS_K 个）。
    pub sens_left: Vec<u32>,
    /// 右眼群神经元索引（≤SENS_K 个）。
    pub sens_right: Vec<u32>,
    pub dn_left: Vec<u32>,
    pub dn_right: Vec<u32>,
    pub sens_source: String,
    pub dn_source: String,
    pub norm_l: f64,
    pub norm_r: f64,
    pub flip: bool,
}

impl BrainIO {
    pub fn new(brain: &Brain, sens_k: usize, seed: u64) -> Self {
        let n = brain.n;
        let sc = &brain.super_class;
        let side = &brain.side;

        // ---- 感觉群：视觉投射优先 ----
        let wanted: [&[&str]; 3] = [
            &["visual_projection", "visual_centrifugal"],
            &["sensory"],
            &["optic"],
        ];
        let mut pool: Vec<bool> = vec![false; n];
        let mut sens_source = String::new();
        let mut found = false;
        for w in wanted {
            let mut cnt = 0usize;
            for (i, s) in sc.iter().enumerate() {
                if w.contains(&s.as_str()) {
                    pool[i] = true;
                    cnt += 1;
                }
            }
            if cnt >= 2 * sens_k {
                sens_source = w.join("+");
                found = true;
                break;
            }
            pool.fill(false);
        }
        if !found {
            pool.fill(true);
            sens_source = "all(回退)".to_string();
        }

        // ---- 左右划分：side 列优先，缺失用坐标左右轴中位数 ----
        let coords = brain.coords.as_deref();
        let lr_axis = brain.meta_lr_axis;
        let split_lr = |mask: &[bool]| -> (Vec<u32>, Vec<u32>) {
            let idx: Vec<u32> = mask
                .iter()
                .enumerate()
                .filter_map(|(i, &m)| m.then_some(i as u32))
                .collect();
            let mut left: Vec<u32> = Vec::new();
            let mut right: Vec<u32> = Vec::new();
            for &i in &idx {
                match side[i as usize].as_str() {
                    "left" => left.push(i),
                    "right" => right.push(i),
                    _ => {},
                }
            }
            if left.len() < 8 || right.len() < 8 {
                // side 标注不足，回退坐标中位数
                if let (Some(axis), Some(cd)) = (lr_axis, coords) {
                    let mut vals: Vec<(u32, f32)> = idx
                        .iter()
                        .map(|&i| (i, cd[i as usize * 3 + axis]))
                        .filter(|(_, c)| !c.is_nan())
                        .collect();
                    vals.sort_by(|a, b| a.1.partial_cmp(&b.1).unwrap_or(std::cmp::Ordering::Equal));
                    let med = if vals.is_empty() {
                        0.0
                    } else {
                        // 中位数（偶数取两中点均值，对齐 np.median）
                        let m = vals.len();
                        if m % 2 == 1 {
                            vals[m / 2].1
                        } else {
                            (vals[m / 2 - 1].1 + vals[m / 2].1) / 2.0
                        }
                    };
                    left = vals
                        .iter()
                        .filter(|(_, c)| *c < med)
                        .map(|(i, _)| *i)
                        .collect();
                    right = vals
                        .iter()
                        .filter(|(_, c)| *c >= med)
                        .map(|(i, _)| *i)
                        .collect();
                }
            }
            (left, right)
        };

        let (pl, pr) = split_lr(&pool);
        let mut rng = StdRng::seed_from_u64(seed);
        let sens_left: Vec<u32> = pl
            .choose_multiple(&mut rng, sens_k.min(pl.len()))
            .copied()
            .collect();
        let sens_right: Vec<u32> = pr
            .choose_multiple(&mut rng, sens_k.min(pr.len()))
            .copied()
            .collect();

        // ---- DN 群 ----
        let dn_count = sc.iter().filter(|s| s.as_str() == "descending").count();
        let wanted_dn = if dn_count < 10 { "motor" } else { "descending" };
        let dn_source = if dn_count < 10 {
            "motor(回退)".to_string()
        } else {
            "descending".to_string()
        };
        let dn_mask: Vec<bool> = sc.iter().map(|s| s.as_str() == wanted_dn).collect();
        let (dn_left, dn_right) = split_lr(&dn_mask);

        info!(
            "果蝇脑 IO: 感觉群来源 super_class={}: 左 {} / 右 {}; DN 群 {}: 左 {} / 右 {}",
            sens_source,
            sens_left.len(),
            sens_right.len(),
            dn_source,
            dn_left.len(),
            dn_right.len()
        );

        BrainIO {
            sens_left,
            sens_right,
            dn_left,
            dn_right,
            sens_source,
            dn_source,
            norm_l: 1.0,
            norm_r: 1.0,
            flip: false,
        }
    }

    /// 开机校准：测量"刺激某侧视觉群 → DN 左右群响应"的传递函数。
    /// 1) 增益归一化：对称刺激测基线 (nl0, nr0)，之后读数除以它，对称输入 → 直行；
    /// 2) 符号标定：单侧刺激测 DN 左右响应，决定是否交换 DN 左右标签（flip）。
    pub fn calibrate(&mut self, brain: &mut Brain) {
        fn probe(
            brain: &mut Brain,
            dn_left: &[u32],
            dn_right: &[u32],
            groups: &[(&[u32], f32)],
        ) -> (i64, i64) {
            brain.reset_state(true);
            let mut drive: Vec<(u32, f32)> = Vec::new();
            for (grp, cur) in groups {
                for &i in *grp {
                    drive.push((i, *cur));
                }
            }
            for _ in 0..CALIBRATE_SIM_MS {
                brain.step(&drive);
            }
            let l = dn_left
                .iter()
                .map(|&i| brain.spike_counts[i as usize] as i64)
                .sum();
            let r = dn_right
                .iter()
                .map(|&i| brain.spike_counts[i as usize] as i64)
                .sum();
            brain.reset_state(true);
            (l, r)
        }

        let (nl0, nr0) = probe(
            brain,
            &self.dn_left,
            &self.dn_right,
            &[(&self.sens_left, CURR_FOOD), (&self.sens_right, CURR_FOOD)],
        );
        self.norm_l = nl0.max(1) as f64;
        self.norm_r = nr0.max(1) as f64;
        let (ll, lr) = probe(
            brain,
            &self.dn_left,
            &self.dn_right,
            &[(&self.sens_left, CURR_FOOD)],
        );
        let (rl, rr) = probe(
            brain,
            &self.dn_left,
            &self.dn_right,
            &[(&self.sens_right, CURR_FOOD)],
        );
        let eff_l = ll as f64 / self.norm_l - lr as f64 / self.norm_r;
        let eff_r = rr as f64 / self.norm_r - rl as f64 / self.norm_l;
        // 若单侧刺激主要推动同侧 DN（eff_l+eff_r>0），默认规则"左>右 → 右转"
        // 会让蛇背离食物，交换 DN 左右标签使"食物在左 → 左转"成立。
        self.flip = (eff_l + eff_r) > 0.0;
        info!(
            "果蝇脑校准: 基线 DN 左={} 右={} | 左刺激->(左{},右{}) 右刺激->(左{},右{}) | eff_l={:.2} eff_r={:.2} | flip={}",
            nl0, nr0, ll, lr, rl, rr, eff_l, eff_r, self.flip
        );
    }

    /// 通用感觉编码：percepts → 注入电流列表（规则逐行对齐 snake.py 298-347）。
    pub fn encode_percepts(&self, percepts: &Percepts) -> Vec<(u32, f32)> {
        let mut drive: Vec<(u32, f32)> = Vec::with_capacity(SENS_K * 2);
        // 生物风味：饥饿放大觅食电流（饥饿 50 → 2 倍，90 → 2.8 倍）
        let food_curr = CURR_FOOD * (1.0 + percepts.hunger as f32 / 50.0);
        let danger = &percepts.danger;

        let add = |group: &[u32], cur: f32, drive: &mut Vec<(u32, f32)>| {
            for &i in group {
                drive.push((i, cur));
            }
        };

        // 危险优先：前方被堵（环绕模式下只有身体算危险）时压制食物吸引，
        // 否则容易盯着食物撞上去
        let food_scale = if danger.front { 0.3 } else { 1.0 };

        // 食物：吸引（同侧），电流随饥饿放大
        match percepts.food_rel {
            Some(Rel::Left) => add(&self.sens_left, food_curr * food_scale, &mut drive),
            Some(Rel::Right) => add(&self.sens_right, food_curr * food_scale, &mut drive),
            Some(Rel::Front) => {
                add(&self.sens_left, food_curr * 0.6 * food_scale, &mut drive);
                add(&self.sens_right, food_curr * 0.6 * food_scale, &mut drive);
            },
            None => {},
        }
        // 危险：回避（刺激对侧）
        if danger.left {
            add(&self.sens_right, CURR_DANGER, &mut drive);
        }
        if danger.right {
            add(&self.sens_left, CURR_DANGER, &mut drive);
        }
        if danger.front {
            if danger.left && !danger.right {
                add(&self.sens_right, CURR_DANGER, &mut drive); // 只剩右边可走
            } else if danger.right && !danger.left {
                add(&self.sens_left, CURR_DANGER, &mut drive);
            } else if !danger.left && !danger.right {
                // 两侧都自由：按步数奇偶打破对称，避免直行撞墙
                let grp = if percepts.steps % 2 == 0 {
                    &self.sens_left
                } else {
                    &self.sens_right
                };
                add(grp, CURR_DANGER, &mut drive);
            }
            // 三侧全堵：不必刺激，等撞
        }
        drive
    }

    /// 读窗口放电计数（增益归一化 + flip），返回 (action, left, right)。
    pub fn decide(&self, brain: &Brain) -> (u8, i64, i64) {
        let l: i64 = self
            .dn_left
            .iter()
            .map(|&i| brain.spike_counts[i as usize] as i64)
            .sum();
        let r: i64 = self
            .dn_right
            .iter()
            .map(|&i| brain.spike_counts[i as usize] as i64)
            .sum();
        let mut ln = l as f64 / self.norm_l;
        let mut rn = r as f64 / self.norm_r;
        if self.flip {
            std::mem::swap(&mut ln, &mut rn);
        }
        let m = ln.max(rn).max(1e-9);
        let action = if (ln - rn) / m > MARGIN {
            2 // 左 DN 活跃 → 右转
        } else if (rn - ln) / m > MARGIN {
            1 // 右 DN 活跃 → 左转
        } else {
            0
        };
        (action, l, r)
    }
}
