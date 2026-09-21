//! 果蝇觅食行为无头冒烟：复现/量化「不主动找食物」。
//!
//! 用法：`cargo run --release --example fly_seeking_smoke -- [graph.npz 路径] [tick数]`
//! 默认路径为运行目录的 graph.npz，默认 1500 tick。
//!
//! 指标：eat/starve 事件计数、每 100 tick 打印状态（含最近食物距离 food_dist、
//! 位置、朝向、状态）；觅食命中率 = food_dist 下降的 tick 占比（空中且有食物时）。

use std::path::PathBuf;

use ling_chat_lib::fly_brain::brain_io::{BrainIO, SENS_K};
use ling_chat_lib::fly_brain::engine::Brain;
use ling_chat_lib::fly_brain::graph::BrainGraph;
use ling_chat_lib::fly_brain::life::{FlyLife, SIM_STEPS_PER_TICK};

fn run_case(tag: &str, graph_path: &PathBuf, ticks: u32, init: usize, max: usize, plastic: bool) {
    let graph = BrainGraph::load(graph_path).expect("load graph");
    let mut brain = Brain::new(graph);
    let mut io = BrainIO::new(&brain, SENS_K, 0);
    io.calibrate(&mut brain);
    if plastic {
        brain.enable_plasticity();
    }
    let mut life = FlyLife::new_with_food(42, init, max);

    let (mut eats, mut starves) = (0u32, 0u32);
    let mut last_seq = 0u64;
    let (mut seek_ticks, mut seek_down) = (0u32, 0u32);
    let mut prev_dist: Option<f32> = None;
    let mut dist_sum = 0.0f64;
    let mut dist_samples = 0u64;

    for _ in 0..ticks {
        let drive = life.build_drive(&io);
        brain.reset_window_counts();
        for _ in 0..SIM_STEPS_PER_TICK {
            brain.step(&drive);
        }
        let (action, l, r) = io.decide(&brain);
        life.advance(action, l, r, &io, &mut brain);

        for ev in &life.events {
            if ev.seq > last_seq {
                last_seq = ev.seq;
                match ev.kind {
                    "eat" => {
                        eats += 1;
                        println!("  [tick {:>5}] 🍡 {}", life.tick, ev.text);
                    },
                    "starve" => {
                        starves += 1;
                        println!("  [tick {:>5}] 💀 {}", life.tick, ev.text);
                    },
                    _ => {},
                }
            }
        }

        let airborne = matches!(life.fly.state.as_str(), "flying" | "foraging");
        if airborne && !life.foods.is_empty() {
            let d = life
                .foods
                .iter()
                .map(|f| (f.x - life.fly.x).hypot(f.z - life.fly.z))
                .fold(f32::INFINITY, f32::min);
            dist_sum += d as f64;
            dist_samples += 1;
            if let Some(p) = prev_dist {
                seek_ticks += 1;
                if d < p {
                    seek_down += 1;
                }
            }
            prev_dist = Some(d);
        } else {
            prev_dist = None;
        }

        if life.tick % 100 == 0 {
            let d = prev_dist.map(|v| format!("{v:5.1}")).unwrap_or("  —  ".into());
            println!(
                "  [tick {:>5}] state={:<8} hunger={:5.1} energy={:5.1} foods={:2} dist={} pos=({:6.1},{:6.1})",
                life.tick,
                life.fly.state.as_str(),
                life.fly.hunger,
                life.fly.energy,
                life.foods.len(),
                d,
                life.fly.x,
                life.fly.z,
            );
        }
    }

    let ratio = if seek_ticks > 0 {
        seek_down as f64 / seek_ticks as f64
    } else {
        0.0
    };
    let mean_dist = if dist_samples > 0 {
        dist_sum / dist_samples as f64
    } else {
        f64::NAN
    };
    println!(
        "== {tag}: ticks={ticks} init={init} max={max} plastic={plastic} | eat={eats} starve={starves} | 趋近率={:.1}% | 平均最近距离={:.2}",
        ratio * 100.0,
        mean_dist,
    );
}

fn main() {
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| "info".into()),
        )
        .init();
    let mut args = std::env::args().skip(1);
    let graph_path = PathBuf::from(args.next().unwrap_or_else(|| {
        "../LingChat-rust-main/bin/data/third_party/fly_brain/graph.npz".to_string()
    }));
    let ticks: u32 = args.next().and_then(|s| s.parse().ok()).unwrap_or(1500);
    let plastic = args.next().map(|s| s == "plastic").unwrap_or(false);
    println!("graph = {} plastic = {plastic}", graph_path.display());
    run_case("默认 6/10", &graph_path, ticks, 6, 10, plastic);
    run_case("拉满 20/20", &graph_path, ticks, 20, 20, plastic);
}
