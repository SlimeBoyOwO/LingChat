//! 果蝇全脑 LIF 脉冲仿真引擎。
//!
//! 移植自 fly-snake `brain.py` 的 `Brain` 类（纯 numpy 参考实现的 Rust 直译）：
//! - dt = 1 ms 固定步长，LIF 精确指数积分，阈值发放复位 + 5ms 不应期；
//! - 突触电流一阶指数衰减，突触传递 1 步延迟（上一步发放者本步沿出边传播）；
//! - 只移植**发放门控（gated）快速路径**：每步仅处理上一步发放神经元的出边
//!   （出边 CSR `out_indptr`/`out_edges` + 顺序 scatter-add，替代 numpy 的
//!   `np.bincount`），O(E_active)。
//! - 可塑性（奖惩）：惰性衰减资格迹 + reward/punish 时折算调制权重，
//!   clamp 到初始权重的 [0.3x, 3.0x]，并对 DA 能突触前神经元群注入 50ms 电流。
//!
//! 有意省略（与参考实现的差异点，模块级约定）：
//! - **不移植 int8 量化**（`quantize="int8"`）：那是嵌入式省内存路径，桌面端不需要；
//! - **不移植 full-matrix 路径**（`_propagate_full`，`np.add.reduceat` 全图稀疏乘）：
//!   那是 gated 路径的对照参考实现，O(E) 每步慢一个数量级；
//! - 浮点求和顺序与 numpy 向量化不完全一致（scatter-add 是顺序累加），
//!   数值上存在最后一个 ulp 级别的差异，对混沌脉冲系统无影响。

use tracing::{info, warn};

use super::graph::BrainGraph;

// ---- 参数常量（与 brain.py 35-53 行一致）----
pub const DT_MS: f32 = 1.0; // 固定步长 1 ms
pub const TAU_M: f32 = 10.0; // 膜时间常数 ms
pub const TAU_SYN: f32 = 3.0; // 突触电流时间常数 ms
pub const V_REST: f32 = 0.0; // 静息电位（相对值）
pub const V_TH: f32 = 1.0; // 阈值
pub const V_RESET: f32 = 0.0; // 复位电位
pub const REFRACT_STEPS: u8 = 5; // 不应期 5 ms

/// exp(-DT/TAU_SYN)，与 numpy float32 结果逐位一致。
pub const SYN_DECAY: f32 = 0.716_531_3;
/// exp(-DT/TAU_M)。
pub const MEM_DECAY: f32 = 0.904_837_4;
/// exp(-DT/ELIG_TAU_MS)，post_trace 的每步衰减。
pub const ELIG_STEP_DECAY: f32 = 0.998_002;

// ---- 可塑性（奖惩）参数 ----
pub const ELIG_TAU_MS: f32 = 500.0; // 资格迹时间常数 ms
pub const PLASTIC_ETA: f32 = 0.02; // 学习率 η
pub const W_MIN_F: f32 = 0.3; // 权重 clamp 下限（初始值的倍数）
pub const W_MAX_F: f32 = 3.0; // 权重 clamp 上限
pub const DA_STIM_STEPS: u32 = 50; // reward/punish 后 DA 神经元刺激时长 ms
pub const DA_STIM_CURR: f32 = 0.8; // DA 刺激电流
pub const ELIG_EPS: f32 = 1e-3; // 参与调制的资格迹阈值
pub const NT_DA: u8 = 4; // nt_type == DA 的编码

/// 资格迹衰减查表长度（dt 为整数步号差；表内覆盖 exp(-dt/500) 到 ~2e-8，
/// 与逐步调用 f32::exp 结果逐位一致，省去高活动期每步上万次 exp 调用）。
const ELIG_DECAY_TAB_N: u64 = 10000;

/// 可塑性统计（对齐 brain.py 的 plastic_stats dict）。
#[derive(Debug, Clone, Default)]
pub struct PlasticStats {
    pub weights_changed: u64,
    pub mean_abs_dw: f32,
    pub rewards: u64,
    pub punishes: u64,
}

/// 全脑 LIF 仿真器。拥有图数据与全部运行时状态。
pub struct Brain {
    pub n: usize,
    // ---- 静态图（post 排序 CSR + 出边 CSR）----
    pub indices: Vec<u32>,
    pub indptr: Vec<i64>,
    pub weight: Vec<f32>,
    pub edge_nt: Option<Vec<u8>>,
    pub out_indptr: Vec<i64>,
    pub out_edges: Vec<u32>,
    /// 每条边的突触后索引（由 indptr 展开，缓存复用；对齐 `_post_per_edge`）。
    pub post_per_edge: Vec<u32>,
    // ---- 神经元标注（BrainIO / positions 用，只读）----
    pub coords: Option<Vec<f32>>,
    pub super_class: Vec<String>,
    pub side: Vec<String>,
    pub meta_lr_axis: Option<usize>,
    // ---- 运行时状态 ----
    pub v: Vec<f32>,
    pub i_syn: Vec<f32>,
    pub refract: Vec<u8>,
    /// 决策窗口内累计发放（读出用；`reset_window_counts` 清空）。
    pub spike_counts: Vec<i32>,
    last_spikes: Vec<u32>,
    pub step_count: u64,
    // ---- 可塑性（enable_plasticity 分配；disable 保留数组仅停更新）----
    pub plastic: bool,
    elig: Vec<f32>,
    /// 上次触边的 step_count；用 u64 防回绕（Python 用 uint32，长期运行会溢出）。
    elig_t: Vec<u64>,
    w0: Vec<f32>,
    post_trace: Vec<f32>,
    changed: Vec<bool>,
    /// 多巴胺能突触前神经元集合（edge_nt == NT_DA 的 pre，升序去重）。
    da_pre: Vec<u32>,
    da_stim_left: u32,
    last_eids: Option<Vec<u32>>,
    pub plastic_stats: PlasticStats,
    /// exp(-dt/500) 查表（0..=10000 整数步）。
    elig_decay_tab: Vec<f32>,
}

impl Brain {
    /// 从图数据构建（权重/CSR 直接 move 进来，零拷贝）。
    pub fn new(graph: BrainGraph) -> Self {
        let n = graph.n;
        let e = graph.weight.len();
        // 每条边的突触后索引：CSR 行指针展开
        let mut post_per_edge = Vec::with_capacity(e);
        for i in 0..n {
            let cnt = (graph.indptr[i + 1] - graph.indptr[i]) as usize;
            post_per_edge.extend(std::iter::repeat(i as u32).take(cnt));
        }
        info!(
            "果蝇脑引擎就绪: {} 神经元, {} 边（gated 路径, float32）",
            n, e
        );
        let elig_decay_tab: Vec<f32> = (0..=ELIG_DECAY_TAB_N)
            .map(|d| (-(d as f32) / ELIG_TAU_MS).exp())
            .collect();
        Brain {
            n,
            indices: graph.indices,
            indptr: graph.indptr,
            weight: graph.weight,
            edge_nt: graph.edge_nt,
            out_indptr: graph.out_indptr,
            out_edges: graph.out_edges,
            post_per_edge,
            coords: graph.coords,
            super_class: graph.super_class,
            side: graph.side,
            meta_lr_axis: graph.meta_lr_axis,
            v: vec![0.0; n],
            i_syn: vec![0.0; n],
            refract: vec![0; n],
            spike_counts: vec![0; n],
            last_spikes: Vec::new(),
            step_count: 0,
            plastic: false,
            elig: Vec::new(),
            elig_t: Vec::new(),
            w0: Vec::new(),
            post_trace: Vec::new(),
            changed: Vec::new(),
            da_pre: Vec::new(),
            da_stim_left: 0,
            last_eids: None,
            plastic_stats: PlasticStats::default(),
            elig_decay_tab,
        }
    }

    /// 资格迹衰减因子 exp(-dt/500)，整数步号差查表（表外回退直接 exp）。
    #[inline]
    fn elig_decay(&self, dt: u64) -> f32 {
        if dt <= ELIG_DECAY_TAB_N {
            self.elig_decay_tab[dt as usize]
        } else {
            (-(dt as f32) / ELIG_TAU_MS).exp()
        }
    }

    // ---------- 可塑性（奖惩） ----------
    // 生物学参照：果蝇蘑菇体多巴胺神经元（PPL1 惩罚 / PAM 奖励）门控
    // KC→MBON 突触可塑性。这里是诚实的工程近似，不是真实果蝇学习规则：
    //  - 每条边存惰性衰减的资格迹：突触前发放时按突触后近期活动迹增量累积
    //    （e += post_trace[post]），奖惩时才按指数衰减折算到当前值；
    //  - reward/punish：W *= (1 + η·sign·e)，clamp 到初始权重 [0.3x, 3.0x]；
    //  - 生物风味：奖惩同时对 nt_type==DA 的突触前神经元群注入 50ms 电流。
    pub fn enable_plasticity(&mut self) -> bool {
        if self.plastic {
            return true;
        }
        let e = self.indices.len();
        self.elig = vec![0.0; e];
        self.elig_t = vec![0; e];
        self.w0.clone_from(&self.weight);
        self.post_trace = vec![0.0; self.n];
        self.changed = vec![false; e];
        // 多巴胺能突触前神经元集合（升序去重，对齐 np.unique）
        self.da_pre.clear();
        if let Some(nt) = &self.edge_nt {
            let mut v: Vec<u32> = self
                .indices
                .iter()
                .zip(nt.iter())
                .filter_map(|(&pre, &t)| (t == NT_DA).then_some(pre))
                .collect();
            v.sort_unstable();
            v.dedup();
            info!("可塑性开启: DA 突触前神经元 {} 个", v.len());
            self.da_pre = v;
        } else {
            warn!("图无 edge_nt（旧 npz），DA 刺激关闭");
        }
        self.da_stim_left = 0;
        self.last_eids = None;
        self.plastic = true;
        true
    }

    /// 停用更新（保留已学权重与统计，可重新 enable 继续）。
    pub fn disable_plasticity(&mut self) {
        self.plastic = false;
    }

    pub fn reward(&mut self, strength: f32) {
        self.modulate(strength.abs());
    }

    pub fn punish(&mut self, strength: f32) {
        self.modulate(-strength.abs());
    }

    fn modulate(&mut self, s: f32) {
        if !self.plastic {
            return;
        }
        let now = self.step_count;
        for e in 0..self.elig.len() {
            let actual = self.elig[e] * self.elig_decay(now - self.elig_t[e]);
            if actual > ELIG_EPS {
                let w0 = self.w0[e];
                // clamp 到初始权重的 [0.3x, 3.0x]（w0 可能为负，取 min/max 包住）
                let lo = (w0 * W_MIN_F).min(w0 * W_MAX_F);
                let hi = (w0 * W_MIN_F).max(w0 * W_MAX_F);
                self.weight[e] = (self.weight[e] * (1.0 + PLASTIC_ETA * s * actual)).clamp(lo, hi);
                self.changed[e] = true;
            }
        }
        if s > 0.0 {
            self.plastic_stats.rewards += 1;
        } else {
            self.plastic_stats.punishes += 1;
        }
        self.refresh_plastic_stats();
        if !self.da_pre.is_empty() {
            self.da_stim_left = DA_STIM_STEPS;
        }
    }

    fn refresh_plastic_stats(&mut self) {
        let mut cnt = 0u64;
        let mut sum = 0.0f64;
        for e in 0..self.changed.len() {
            if self.changed[e] {
                cnt += 1;
                sum += (self.weight[e] - self.w0[e]).abs() as f64;
            }
        }
        self.plastic_stats.weights_changed = cnt;
        if cnt > 0 {
            self.plastic_stats.mean_abs_dw = (sum / cnt as f64) as f32;
        }
    }

    /// step 内的可塑性记账：用本次传播收集到的出边更新资格迹。
    fn plasticity_tick(&mut self) {
        let Some(eids) = &self.last_eids else { return };
        let now = self.step_count;
        for &e in eids {
            let e = e as usize;
            let decay = self.elig_decay(now - self.elig_t[e]);
            let post = self.post_per_edge[e] as usize;
            self.elig[e] = self.elig[e] * decay + self.post_trace[post];
            self.elig_t[e] = now;
        }
    }

    // ---------- 状态复位 ----------
    pub fn reset_state(&mut self, clear_counts: bool) {
        self.v.fill(0.0);
        self.i_syn.fill(0.0);
        self.refract.fill(0);
        self.step_count = 0;
        self.last_spikes.clear();
        if self.plastic {
            // 清空资格迹与活动迹（保留已学权重与统计）
            self.post_trace.fill(0.0);
            self.elig.fill(0.0);
            self.elig_t.fill(0);
            self.da_stim_left = 0;
            self.last_eids = None;
        }
        if clear_counts {
            self.spike_counts.fill(0);
        }
    }

    /// 清空窗口放电计数（每个决策窗口开始时调用）。
    pub fn reset_window_counts(&mut self) {
        self.spike_counts.fill(0);
    }

    /// 是否开启过可塑性（w0 基线已分配；学习权重持久化的保存门槛）。
    pub fn w0_is_empty(&self) -> bool {
        self.w0.is_empty()
    }

    /// 把权重恢复到初始值 w0 并清零全部可塑性状态（restart 用）。
    /// 不影响 plastic 开关本身与 DA 突触前集合；从未开启过可塑性时为空操作。
    pub fn reset_learned_weights(&mut self) {
        if self.w0.is_empty() {
            return;
        }
        self.weight.clone_from(&self.w0);
        self.elig.fill(0.0);
        self.elig_t.fill(0);
        self.post_trace.fill(0.0);
        self.changed.fill(false);
        self.da_stim_left = 0;
        self.last_eids = None;
        self.plastic_stats = PlasticStats::default();
    }

    /// 上一步发放神经元索引（只读）。
    pub fn last_spikes(&self) -> &[u32] {
        &self.last_spikes
    }

    // ---------- 核心仿真 ----------
    /// 推进 1 ms。drive: [(神经元索引, 注入电流)]。返回后可用 `last_spikes()` 读当步发放。
    pub fn step(&mut self, drive: &[(u32, f32)]) {
        // 1) 突触电流指数衰减
        for x in self.i_syn.iter_mut() {
            *x *= SYN_DECAY;
        }
        if self.plastic {
            for x in self.post_trace.iter_mut() {
                *x *= ELIG_STEP_DECAY;
            }
            if self.da_stim_left > 0 {
                // DA 刺激（reward/punish 后的短电流）
                for &i in &self.da_pre {
                    self.i_syn[i as usize] += DA_STIM_CURR;
                }
                self.da_stim_left -= 1;
            }
        }

        // 2) 上一步发放经突触传递（发放门控：只处理有发放的突触前的出边）
        if !self.last_spikes.is_empty() {
            self.propagate_gated();
        } else if self.plastic {
            self.last_eids = None;
        }

        // 2.5) 可塑性记账：突触前发放边按突触后近期活动累积资格迹
        if self.plastic {
            self.plasticity_tick();
        }

        // 3) 外加驱动电流
        for &(i, c) in drive {
            self.i_syn[i as usize] += c;
        }

        // 4) LIF 膜电位更新（精确积分，不应期内冻结）+ 5) 阈值判定 + 复位 + 不应期
        // 与 numpy 两趟向量化逐元素等价：active（refract==0）先积分再判阈值，
        // 不应期神经元 v 冻结、计数减 1。
        let one_minus_mem = 1.0 - MEM_DECAY;
        // 复用 last_spikes 的容量，避免每步重新分配
        let mut spikes = std::mem::take(&mut self.last_spikes);
        spikes.clear();
        for i in 0..self.n {
            if self.refract[i] == 0 {
                // v_inf = V_REST + i_syn（把电流折合为电位），向 v_inf 指数趋近
                let v = self.v[i] * MEM_DECAY + (V_REST + self.i_syn[i]) * one_minus_mem;
                if v >= V_TH {
                    self.v[i] = V_RESET;
                    self.refract[i] = REFRACT_STEPS;
                    self.spike_counts[i] += 1;
                    if self.plastic {
                        self.post_trace[i] += 1.0;
                    }
                    spikes.push(i as u32);
                } else {
                    self.v[i] = v;
                }
            } else {
                self.refract[i] -= 1;
            }
        }

        self.last_spikes = spikes;
        self.step_count += 1;
    }

    /// 快速路径：收集发放神经元的全部出边，顺序 scatter-add 到突触后
    /// （替代 numpy 的 repeat/arange 展开 + bincount 聚合）。
    fn propagate_gated(&mut self) {
        let total: usize = self
            .last_spikes
            .iter()
            .map(|&s| (self.out_indptr[s as usize + 1] - self.out_indptr[s as usize]) as usize)
            .sum();
        if total == 0 {
            self.last_eids = None;
            return;
        }
        // 复用 last_eids 的容量（塑料关闭时只是每步覆盖，逻辑不变）
        let mut eids = self.last_eids.take().unwrap_or_default();
        eids.clear();
        eids.reserve(total);
        for &s in &self.last_spikes {
            let a = self.out_indptr[s as usize] as usize;
            let b = self.out_indptr[s as usize + 1] as usize;
            eids.extend_from_slice(&self.out_edges[a..b]);
        }
        for &e in &eids {
            let post = self.post_per_edge[e as usize] as usize;
            self.i_syn[post] += self.weight[e as usize];
        }
        self.last_eids = Some(eids);
    }
}
