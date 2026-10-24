use rand::Rng;

use crate::config::proactive::{
    DEFAULT_EXPECTED_INTERVAL_MINUTES, MAX_EXPECTED_INTERVAL_MINUTES, MIN_EXPECTED_INTERVAL_MINUTES,
};

/// 主动对话轮询周期（秒）。必须与 ProactiveSystem 后台 Loop 的 interval 一致，
/// 下面的期望间隔换算全部以它为时间单位。
pub const LOOP_INTERVAL_SECS: f64 = 30.0;

/// 兴趣超过它才可能触发主动对话。
const TRIGGER_THRESHOLD: f64 = 50.0;
/// 兴趣上限的初始值，也是用户每次发言后恢复到上限值。
const INITIAL_INTEREST_CAP: f64 = 100.0;
/// 增长上限 / 增长下限，沿用历史上固定的 5~10 所用比例。
const GROWTH_RATIO: f64 = 2.0;

/// 把配置里的期望间隔收进合法区间。NaN 会让下游的 gen_range panic，退回默认值；
/// 正负无穷由 clamp 自然落到上下界。
pub fn clamp_expected_interval(minutes: f64) -> f64 {
    if minutes.is_nan() {
        DEFAULT_EXPECTED_INTERVAL_MINUTES
    } else {
        minutes.clamp(MIN_EXPECTED_INTERVAL_MINUTES, MAX_EXPECTED_INTERVAL_MINUTES)
    }
}

/// 由每轮增长均值还原随机上下限（保持上限 = 2 倍下限，即原 5~10 的形状）。
fn growth_bounds(mean_growth: f64) -> (f64, f64) {
    let growth_min = 2.0 * mean_growth / (1.0 + GROWTH_RATIO);
    (growth_min, growth_min * GROWTH_RATIO)
}

/// 求「每轮兴趣增长均值为 mean_growth」时，从兴趣归零到触发主动对话的期望轮数。
///
/// 每轮兴趣增加 U[a, b]（b = 2a、均值 mean_growth）并截断在上限 100，随后以
/// p = clamp((interest + status_mod - 50) / 50, 0, 1) 的概率掷骰触发。
/// 记 E[x] 为当前兴趣为 x 时的期望剩余轮数：
///     E[x] = 1 + E_g[(1 - p(y)) * E[min(y, 100)]],  y = x + g
/// y >= x 恒成立，所以从上限往下逐点回代即可解出整张表（无需迭代收敛）。
fn expected_cycles(mean_growth: f64, status_mod: f64) -> f64 {
    let (growth_min, growth_max) = growth_bounds(mean_growth);

    // 网格步长必须明显细于最小增长量，否则小幅度增长会被网格抹平
    let step = ((growth_max - growth_min) / 8.0).clamp(0.02, 0.25);
    let points = (INITIAL_INTEREST_CAP / step).ceil() as usize + 1;

    // 中点法对增长量积分：被积函数只在截断处有一个折点，取样点数不必多
    const SAMPLES: usize = 16;
    let sample_step = (growth_max - growth_min) / SAMPLES as f64;
    let growths: Vec<f64> = (0..SAMPLES)
        .map(|i| growth_min + (i as f64 + 0.5) * sample_step)
        .collect();

    let mut table = vec![0.0_f64; points];
    for i in (0..points).rev() {
        let interest = i as f64 * step;
        let mut no_trigger = 0.0;
        for &growth in &growths {
            let next = (interest + growth).min(INITIAL_INTEREST_CAP);
            let prob =
                ((next + status_mod - TRIGGER_THRESHOLD) / TRIGGER_THRESHOLD).clamp(0.0, 1.0);
            // 必触发时剩余轮数为 0，本就不参与累加
            if prob < 1.0 {
                no_trigger += (1.0 - prob) * interpolate(&table, next / step);
            }
        }
        table[i] = 1.0 + no_trigger / SAMPLES as f64;
    }
    table[0]
}

/// 在兴趣网格上线性插值取 E[value]。
fn interpolate(table: &[f64], index: f64) -> f64 {
    let last = table.len() - 1;
    let lower = (index.floor() as usize).min(last);
    let upper = (lower + 1).min(last);
    let frac = (index - lower as f64).clamp(0.0, 1.0);
    table[lower] * (1.0 - frac) + table[upper] * frac
}

/// 由「期望间隔（分钟）」反推每轮兴趣增长的随机范围，返回 (下限, 上限)。
///
/// 校正点取兴趣上限 100、状态修正 0（即用户刚发过言、兴趣归零、一天里的首次
/// 主动搭话）。状态修正在运行时由用户活动决定：打游戏/冲浪会把间隔压短约
/// 三成，挂机/工作会拉长约两成，这里的校正点取的是中性场景。
pub fn growth_range_for_interval(minutes: f64) -> (f64, f64) {
    let target_cycles = clamp_expected_interval(minutes) * 60.0 / LOOP_INTERVAL_SECS;
    growth_bounds(solve_mean_growth(target_cycles))
}

/// 二分求根。E[轮数] 关于每轮增长均值单调递减，故可用二分。
fn solve_mean_growth(target_cycles: f64) -> f64 {
    let (mut low, mut high) = (0.05_f64, 400.0_f64);
    for _ in 0..32 {
        let mid = (low + high) / 2.0;
        if expected_cycles(mid, 0.0) > target_cycles {
            low = mid;
        } else {
            high = mid;
        }
    }
    (low + high) / 2.0
}

pub struct InterestManager {
    pub interest: f64,
    pub max_interest_cap: f64,
    pub initial_max_cap: f64,
    pub status_mod: i32,
    pub max_proactive_count: i32,
    pub decay_step: f64,
    pub proactive_times: i32,
    /// 每轮兴趣增长的随机范围，由「期望间隔」反推得到。
    growth_range: (f64, f64),
}

impl InterestManager {
    pub fn new(max_proactive_count: i32, expected_interval_minutes: f64) -> Self {
        // 计算 decay_step: 50.0 / max_proactive_count
        // 注意：需要处理 max_proactive_count 为 0 的情况，防止除以零导致 panic 或产生 NaN/Inf
        let decay_step = if max_proactive_count > 0 {
            50.0 / max_proactive_count as f64
        } else {
            0.0 // 如果最大次数为0，则不衰减
        };

        let mut manager = Self {
            interest: 0.0,
            max_interest_cap: 100.0,
            initial_max_cap: 100.0,
            status_mod: 0,
            max_proactive_count,
            decay_step,
            proactive_times: 0,
            growth_range: (0.0, 0.0),
        };
        manager.set_expected_interval(expected_interval_minutes);
        manager
    }

    pub fn update_from_config(&mut self, max_proactive_count: i32, expected_interval_minutes: f64) {
        self.max_proactive_count = max_proactive_count;
        // 当配置更新时，同步更新 decay_step
        self.decay_step = if max_proactive_count > 0 {
            50.0 / max_proactive_count as f64
        } else {
            0.0
        };
        self.set_expected_interval(expected_interval_minutes);
    }

    /// 由期望间隔重算增长范围。配置重载会反复调用，范围没变就不重复记日志。
    fn set_expected_interval(&mut self, minutes: f64) {
        let growth_range = growth_range_for_interval(minutes);
        if growth_range == self.growth_range {
            return;
        }
        tracing::info!(
            "[Engagement] Expected interval {:.1} min -> interest growth {:.2}~{:.2}/cycle",
            clamp_expected_interval(minutes),
            growth_range.0,
            growth_range.1
        );
        self.growth_range = growth_range;
    }

    pub fn update_interest(&mut self) {
        let mut rng = rand::thread_rng();
        let growth = rng.gen_range(self.growth_range.0..self.growth_range.1);
        self.interest = (self.interest + growth).min(self.max_interest_cap);
        tracing::info!(
            "[Engagement] Interest grown by {:.2}. Current: {:.2}/{:.2}",
            growth,
            self.interest,
            self.max_interest_cap
        );
    }

    pub fn should_trigger_talk(&self) -> bool {
        if self.proactive_times >= self.max_proactive_count {
            return false;
        }

        if self.interest <= 50.0 {
            return false;
        }

        let mut rng = rand::thread_rng();
        let prob = (self.interest + self.status_mod as f64 - 50.0) / 50.0;
        let roll = rng.gen_range(0.0..1.0);
        let triggered = roll < prob;

        tracing::info!(
            "[Engagement] Trigger check: prob={:.2}, roll={:.2}, triggered={}",
            prob,
            roll,
            triggered
        );

        triggered
    }

    pub fn reset_interest(&mut self) {
        self.interest = 0.0;
        self.proactive_times += 1;
        self.decay_max_interest_cap();
    }

    pub fn decay_max_interest_cap(&mut self) {
        self.max_interest_cap = (self.max_interest_cap - self.decay_step).max(50.0);
        tracing::info!("[Engagement] Cap decayed to {:.2}", self.max_interest_cap);
    }

    pub fn restore_max_interest_cap(&mut self) {
        self.max_interest_cap = self.initial_max_cap;
        self.proactive_times = 0;
        self.interest = 0.0;
    }

    pub fn set_status_mod(&mut self, val: i32) {
        self.status_mod = val;
    }
}
