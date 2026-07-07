use chrono::Local;
use rand::Rng;

pub struct InterestManager {
    pub interest: f64,
    pub max_interest_cap: f64,
    pub initial_max_cap: f64,
    pub status_mod: i32,
    pub max_proactive_count: i32,
    pub decay_step: f64,
    pub proactive_times: i32,
    pub trigger_threshold: f64,
    pub last_reset_date: String,
}

impl InterestManager {
    pub fn new(max_proactive_count: i32, trigger_threshold: f64, decay_step: f64) -> Self {
        Self {
            interest: 0.0,
            max_interest_cap: 100.0,
            initial_max_cap: 100.0,
            status_mod: 0,
            max_proactive_count,
            decay_step,
            proactive_times: 0,
            trigger_threshold,
            last_reset_date: Local::now().format("%Y-%m-%d").to_string(),
        }
    }

    pub fn update_from_config(
        &mut self,
        max_proactive_count: i32,
        trigger_threshold: f64,
        decay_step: f64,
    ) {
        self.max_proactive_count = max_proactive_count;
        self.trigger_threshold = trigger_threshold;
        self.decay_step = decay_step;
    }

    /// 根据用户状态更新兴趣度。
    /// 用户越活跃（游戏/浏览）增长越快；工作/挂机时增长更慢。
    pub fn update_interest(&mut self, status_mod: i32) {
        self.status_mod = status_mod;

        let mut rng = rand::thread_rng();
        let base_growth = rng.gen_range(5.0..10.0);
        // 状态修正：正 modifier 加速，负 modifier 减速，但不会让增长低于 1
        let growth = (base_growth + status_mod as f64 * 0.3).max(1.0);

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

        if self.interest <= self.trigger_threshold {
            return false;
        }

        let mut rng = rand::thread_rng();
        // prob 从 0 到 1 线性增长：刚好到阈值时为 0，达到 cap 时为 1
        let range = self.max_interest_cap - self.trigger_threshold;
        let prob = if range <= 0.0 {
            1.0
        } else {
            (self.interest + self.status_mod as f64 - self.trigger_threshold) / range
        };
        let prob = prob.clamp(0.0, 1.0);
        let roll = rng.gen_range(0.0..1.0);
        let triggered = roll < prob;

        tracing::info!(
            "[Engagement] Trigger check: threshold={:.2}, prob={:.2}, roll={:.2}, triggered={}",
            self.trigger_threshold,
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
        // 衰减后至少保留触发阈值或 20 分，避免上限掉到 0 导致永远无法触发
        let floor = self.trigger_threshold.max(20.0);
        self.max_interest_cap = (self.max_interest_cap - self.decay_step).max(floor);
        tracing::info!("[Engagement] Cap decayed to {:.2}", self.max_interest_cap);
    }

    pub fn restore_max_interest_cap(&mut self) {
        self.max_interest_cap = self.initial_max_cap;
        self.proactive_times = 0;
        self.interest = 0.0;
        self.last_reset_date = Local::now().format("%Y-%m-%d").to_string();
    }

    /// 跨天时重置每日计数与上限，确保每天最多触发 `max_proactive_count` 次。
    pub fn check_daily_reset(&mut self) {
        let today = Local::now().format("%Y-%m-%d").to_string();
        if self.last_reset_date != today {
            tracing::info!(
                "[Engagement] New day reset: {} -> {}, restoring cap and proactive count",
                self.last_reset_date,
                today
            );
            self.max_interest_cap = self.initial_max_cap;
            self.proactive_times = 0;
            self.interest = 0.0;
            self.last_reset_date = today;
        }
    }
}
