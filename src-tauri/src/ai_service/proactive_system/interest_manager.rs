use rand::Rng;

pub struct InterestManager {
    pub interest: f64,
    pub max_interest_cap: f64,
    pub initial_max_cap: f64,
    pub status_mod: i32,
    pub max_proactive_count: i32,
    pub decay_step: f64,
    pub proactive_times: i32,
}

impl InterestManager {
    pub fn new(max_proactive_count: i32) -> Self {
        Self {
            interest: 0.0,
            max_interest_cap: 100.0,
            initial_max_cap: 100.0,
            status_mod: 0,
            max_proactive_count,
            decay_step: 50.0,
            proactive_times: 0,
        }
    }

    pub fn update_from_config(&mut self, max_proactive_count: i32) {
        self.max_proactive_count = max_proactive_count;
    }

    pub fn update_interest(&mut self) {
        let mut rng = rand::thread_rng();
        let growth = rng.gen_range(5.0..10.0);
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
        self.max_interest_cap = (self.max_interest_cap - self.decay_step).max(0.0);
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
