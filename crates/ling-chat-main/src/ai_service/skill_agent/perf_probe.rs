//! 性能探针（仅测试编译）：一轮 agent 回合的本地 CPU 准备链路计时 + 优化前后行为对拍。
#![cfg(test)]

use crate::ai_service::skill_agent::role::{Handoff, TaskKind};
use crate::ai_service::skill_agent::router::{QueueItem, RoutePlan};
use crate::ai_service::skill_agent::stage::budget::{self, budget_allows_send};
use crate::ai_service::skill_agent::stage::prompt;
use crate::ai_service::skill_agent::skills;
use crate::ai_service::types::LlmMessage;
use std::path::{Path, PathBuf};
use std::time::Instant;

fn repo_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("..")
        .join("..")
        .canonicalize()
        .expect("repo root")
}

fn skills_dir() -> PathBuf {
    repo_root().join("data").join("game_data").join("skills")
}

/// 混合中英文的典型历史：中文正文 + 英文回复 + tool 结果，接近真实创作会话。
fn sample_messages(n: usize) -> Vec<LlmMessage> {
    let cn = "这是一段用于性能测量的中文剧本正文，描述了角色之间的对话与场景切换。".repeat(40);
    let en = "This is a sample English payload used for token estimation benchmarks. ".repeat(30);
    let mut out = Vec::with_capacity(n + 1);
    out.push(LlmMessage::system("系统提示与工具说明".repeat(200)));
    for i in 0..n {
        match i % 4 {
            0 => out.push(LlmMessage::user(format!("请把第 {i} 章写出来。{cn}"))),
            1 => out.push(LlmMessage::assistant(format!("好的，我来处理。{en}"))),
            2 => out.push(LlmMessage {
                role: "tool".into(),
                content: cn.clone(),
                tool_calls: None,
                tool_call_id: Some(format!("call_{i}")),
                image_data_url: None,
            }),
            _ => out.push(LlmMessage::user(format!("继续，把剩下的写完。{cn}"))),
        }
    }
    out
}

fn measure(name: &str, warmup: usize, runs: usize, mut f: impl FnMut()) {
    for _ in 0..warmup {
        f();
    }
    let mut times = Vec::with_capacity(runs);
    for _ in 0..runs {
        let t = Instant::now();
        f();
        times.push(t.elapsed().as_secs_f64() * 1000.0);
    }
    let total: f64 = times.iter().sum();
    let min = times.iter().copied().fold(f64::INFINITY, f64::min);
    let max = times.iter().copied().fold(f64::NEG_INFINITY, f64::max);
    println!(
        "PERF|{name}|{runs}|{total:.3}|{:.4}|{min:.4}|{max:.4}",
        total / runs as f64
    );
}

fn checksum(messages: &[LlmMessage]) -> u64 {
    let mut h: u64 = 1469598103934665603;
    for m in messages {
        for b in m.role.bytes().chain(m.content.bytes()) {
            h ^= b as u64;
            h = h.wrapping_mul(1099511628211);
        }
    }
    h
}

fn route_plan() -> RoutePlan {
    RoutePlan {
        items: vec![
            QueueItem { kind: TaskKind::Outline, target: String::new() },
            QueueItem { kind: TaskKind::DraftChapter, target: "01".into() },
            QueueItem { kind: TaskKind::WriteChapter, target: "01".into() },
            QueueItem { kind: TaskKind::Polish, target: String::new() },
        ],
        boundary: None,
        reason: None,
    }
}

fn task_block_input<'a>(
    plan: &'a RoutePlan,
    dir: &'a Path,
    allowed: &'a [&'a str],
    skipped: &'a [String],
) -> prompt::TaskBlockInput<'a> {
    prompt::TaskBlockInput {
        task: TaskKind::DraftChapter,
        handoff: Handoff::Proceed,
        plan,
        skills_dir: dir,
        land: true,
        item_index: 1,
        skipped,
        is_last: false,
        allowed,
    }
}

/// 优化前后必须逐字节一致的 golden 输出；只比对语义，不比对仅供展示的 token 数值。
#[test]
fn perf_golden() {
    for (name, n, window) in [
        ("small", 8usize, 8 * 1024usize),
        ("medium", 40, 128 * 1024),
        ("large", 120, 1024 * 1024),
    ] {
        let msgs = sample_messages(n);
        let est = budget::estimate_tokens(&msgs);
        let upper = budget::estimate_tokens_upper_bound(&msgs);
        assert!(upper >= est, "上界必须不小于精确值: {name}");
        let cap = window
            .saturating_sub((128 * 1024usize).min(window / 8))
            .max(1);
        let trigger = cap * 80 / 100;
        println!("BOUND|{name}|{est}|{upper}|{trigger}|{}", upper < trigger);
        let mut work = msgs.clone();
        let outcome = budget::apply_context_budget(&mut work, window);
        let label = match &outcome {
            budget::BudgetOutcome::Untouched { .. } => "Untouched",
            budget::BudgetOutcome::Trimmed { .. } => "Trimmed",
            budget::BudgetOutcome::TooLong { .. } => "TooLong",
        };
        println!(
            "GOLDEN|{name}|{est}|{label}|{}|{}|{}",
            budget_allows_send(&outcome),
            outcome.note().unwrap_or_default().len(),
            checksum(&work)
        );
    }

    // 上界不能被任何 UTF-8 宽度击穿：CJK / 韩文 / 全角(3字节) / emoji(4字节) / 拉丁扩展(2字节) / ASCII / 空。
    let cases: Vec<(&str, String)> = vec![
        ("cjk", "中文剧本正文与角色对白。".repeat(500)),
        ("ascii", "plain ascii text for estimation. ".repeat(500)),
        ("emoji", "😀🎉🚀".repeat(500)),
        ("hangul", "한글텍스트".repeat(500)),
        ("fullwidth", "ＡＢＣ１２３".repeat(500)),
        ("latin_ext", "éèêëàçüö".repeat(500)),
        ("empty", String::new()),
    ];
    for (name, text) in cases {
        let m = vec![LlmMessage::user(text)];
        let exact = budget::estimate_tokens(&m);
        let upper = budget::estimate_tokens_upper_bound(&m);
        assert!(upper >= exact, "上界被击穿: {name} ({upper} < {exact})");
        println!("BOUND_CASE|{name}|{exact}|{upper}");
    }
}

#[test]
fn perf_probe() {
    let dir = skills_dir();
    assert!(dir.is_dir(), "skills dir: {}", dir.display());
    let msgs = sample_messages(40);
    println!(
        "PERF_META|messages={}|bytes={}|est_tokens={}",
        msgs.len(),
        msgs.iter().map(|m| m.content.len()).sum::<usize>(),
        budget::estimate_tokens(&msgs)
    );

    measure("estimate_tokens_40msg", 2, 30, || {
        std::hint::black_box(budget::estimate_tokens(&msgs));
    });

    let mut pool: Vec<Vec<LlmMessage>> = (0..32).map(|_| msgs.clone()).collect();
    let pool_len = pool.len();
    let mut k = 0usize;
    measure("apply_budget_128k_40msg", 2, 30, || {
        let slot = &mut pool[k % pool_len];
        k += 1;
        std::hint::black_box(budget::apply_context_budget(slot, 128 * 1024));
    });

    measure("apply_budget_8k_40msg_fresh_clone", 2, 20, || {
        let mut m = msgs.clone();
        std::hint::black_box(budget::apply_context_budget(&mut m, 8 * 1024));
    });

    measure("find_all_skills", 2, 20, || {
        std::hint::black_box(skills::find_all_skills(&dir));
    });

    let infos = skills::find_all_skills(&dir);
    measure("build_skills_xml", 2, 20, || {
        std::hint::black_box(skills::build_skills_xml(&infos));
    });

    measure("hub_doc", 2, 20, || {
        std::hint::black_box(prompt::hub_doc(&dir));
    });

    let plan = route_plan();
    let allowed: Vec<&str> = vec!["read_skill", "read_file", "write_file", "list_files"];
    let skipped: Vec<String> = Vec::new();
    measure("build_task_block", 2, 20, || {
        let input = task_block_input(&plan, &dir, &allowed, &skipped);
        std::hint::black_box(prompt::build_task_block(&input));
    });

    // 旧版单 agent：一回合只扫一次技能、建一次 system、一次 budget 检查。
    measure("turn_old_single_agent", 2, 10, || {
        let infos = skills::find_all_skills(&dir);
        let xml = skills::build_skills_xml(&infos);
        let mut m = msgs.clone();
        std::hint::black_box((xml.len(), budget::apply_context_budget(&mut m, 128 * 1024)));
    });

    // 旧版单 agent 的真实口径：一回合扫一次技能、读一次 SKILL.md、准备一次消息；没有 budget 检查。
    measure("turn_old_single_agent_no_budget", 2, 10, || {
        let infos = skills::find_all_skills(&dir);
        let xml = skills::build_skills_xml(&infos);
        let doc = std::fs::read_to_string(dir.join("lingchat-script-editor/SKILL.md"))
            .unwrap_or_default();
        let m = msgs.clone();
        std::hint::black_box((xml.len(), doc.len(), m.len()));
    });

    // 对照：新架构但保留「每项重扫技能」的旧结构（用于量化重复扫盘的代价）。
    measure("turn_new_multi_role_repeat_scan", 2, 10, || {
        let hub = prompt::hub_doc(&dir);
        std::hint::black_box(hub.map(|h| h.len()).unwrap_or(0));
        for _ in 0..4 {
            let infos = skills::find_all_skills(&dir);
            let xml = skills::build_skills_xml(&infos);
            std::hint::black_box(xml.len());
            let input = task_block_input(&plan, &dir, &allowed, &skipped);
            let block = prompt::build_task_block(&input);
            std::hint::black_box(block.len());
            for _ in 0..2 {
                let mut m = msgs.clone();
                std::hint::black_box(budget::apply_context_budget(&mut m, 128 * 1024));
            }
        }
    });

    // 优化后结构：技能索引一轮只扫一次（对应 core.rs 的 OnceLock 改动）+ 每项的 budget 检查。
    measure("turn_new_multi_role_4items", 2, 10, || {
        let hub = prompt::hub_doc(&dir);
        std::hint::black_box(hub.map(|h| h.len()).unwrap_or(0));
        let infos = skills::find_all_skills(&dir);
        let xml = skills::build_skills_xml(&infos);
        for _ in 0..4 {
            std::hint::black_box(xml.len());
            let input = task_block_input(&plan, &dir, &allowed, &skipped);
            let block = prompt::build_task_block(&input);
            std::hint::black_box(block.len());
            for _ in 0..2 {
                let mut m = msgs.clone();
                std::hint::black_box(budget::apply_context_budget(&mut m, 128 * 1024));
            }
        }
    });

    // 更贴近真实：每完成一项历史增长 8 条；同样按优化后结构（一轮扫一次技能）。
    let growing: Vec<Vec<LlmMessage>> = (0..4).map(|i| sample_messages(40 + i * 8)).collect();
    measure("turn_new_multi_role_growing_history", 2, 10, || {
        let hub = prompt::hub_doc(&dir);
        std::hint::black_box(hub.map(|h| h.len()).unwrap_or(0));
        let infos = skills::find_all_skills(&dir);
        let xml = skills::build_skills_xml(&infos);
        for (i, hist) in growing.iter().enumerate() {
            std::hint::black_box((i, xml.len()));
            let input = task_block_input(&plan, &dir, &allowed, &skipped);
            let block = prompt::build_task_block(&input);
            std::hint::black_box(block.len());
            for _ in 0..2 {
                let mut m = hist.clone();
                std::hint::black_box(budget::apply_context_budget(&mut m, 128 * 1024));
            }
        }
    });
}

// ============================================================================
// 前缀稳定化（C2）：拆分契约测试。以下全部为追加内容，不改动本文件上方任何已有行。
// 交叉对拍对象是两份彼此独立的实现：prompt::build_task_block 与 prompt::build_task_parts。
// ============================================================================

/// 4 项计划：第 0 项无 target（Outline），第 1~3 项都是 `DraftChapter` 且 target 各不相同，
/// 前三个用例可以「只改 item_index / target」而落在同一个桶里。
fn parts_plan() -> RoutePlan {
    RoutePlan {
        items: vec![
            QueueItem {
                kind: TaskKind::Outline,
                target: String::new(),
            },
            QueueItem {
                kind: TaskKind::DraftChapter,
                target: "01".into(),
            },
            QueueItem {
                kind: TaskKind::DraftChapter,
                target: "02".into(),
            },
            QueueItem {
                kind: TaskKind::DraftChapter,
                target: "03".into(),
            },
        ],
        boundary: None,
        reason: None,
    }
}

/// 按「随项变」的那几维参数化构造 `TaskBlockInput`；其余字段固定，便于单点对比。
#[allow(clippy::too_many_arguments)]
fn parts_input<'a>(
    plan: &'a RoutePlan,
    dir: &'a Path,
    allowed: &'a [&'a str],
    skipped: &'a [String],
    task: TaskKind,
    handoff: Handoff,
    item_index: usize,
    is_last: bool,
    land: bool,
) -> prompt::TaskBlockInput<'a> {
    prompt::TaskBlockInput {
        task,
        handoff,
        plan,
        skills_dir: dir,
        land,
        item_index,
        skipped,
        is_last,
        allowed,
    }
}

/// 4 组不同输入（不同 item_index / target / handoff / is_last / land / allowed）逐字对拍两个实现，
/// 并做字符数守恒：每个字符恰好落在一个 Part 里，段丢失或段重复都会把等式打破。
#[test]
fn prefix_parts_golden_identical() {
    let dir = skills_dir();
    assert!(dir.is_dir(), "skills dir: {}", dir.display());
    let plan = parts_plan();
    let wide: Vec<&str> = vec!["read_skill", "read_file", "write_file", "list_files"];
    let narrow: Vec<&str> = vec!["read_skill", "read_file"];
    let no_skipped: Vec<String> = Vec::new();
    let some_skipped: Vec<String> = vec!["盘点素材（还没有素材目录）".to_string()];

    let cases = vec![
        parts_input(
            &plan,
            &dir,
            &wide,
            &no_skipped,
            TaskKind::DraftChapter,
            Handoff::Proceed,
            1,
            false,
            true,
        ),
        parts_input(
            &plan,
            &dir,
            &wide,
            &no_skipped,
            TaskKind::DraftChapter,
            Handoff::Proceed,
            2,
            true,
            true,
        ),
        parts_input(
            &plan,
            &dir,
            &narrow,
            &some_skipped,
            TaskKind::WriteChapter,
            Handoff::Prerequisite {
                first: TaskKind::Outline,
                why: "还没有设计稿，先把设计稿写出来才谈得上改它",
            },
            3,
            false,
            false,
        ),
        parts_input(
            &plan,
            &dir,
            &wide,
            &some_skipped,
            TaskKind::Outline,
            Handoff::Explain("这一项做不了，先说清楚不动文件"),
            0,
            true,
            false,
        ),
    ];

    for (i, input) in cases.iter().enumerate() {
        let parts = prompt::build_task_parts(input);
        let golden = prompt::build_task_block(input);
        assert_eq!(
            parts.ordered, golden,
            "第 {i} 组：parts.ordered 必须与 build_task_block 逐字相同"
        );
        assert_eq!(
            parts.fixed.len() + parts.varying.len(),
            parts.ordered.len(),
            "第 {i} 组：fixed + varying 的字符数必须与 ordered 守恒"
        );
    }
}

/// 同一个桶（同 TaskKind / allowed / land / user_msg / plan.len）里只挪 item_index 与 target：
/// fixed 必须逐字像素级相同（前缀可复用），varying 必须互不相同；而 allowed 一变，fixed 就该分叉。
#[test]
fn prefix_fixed_stable_within_bucket() {
    let dir = skills_dir();
    assert!(dir.is_dir(), "skills dir: {}", dir.display());
    let plan = parts_plan();
    let allowed: Vec<&str> = vec!["read_skill", "read_file", "write_file", "list_files"];
    let narrow: Vec<&str> = vec!["read_skill", "read_file"];
    let no_skipped: Vec<String> = Vec::new();

    let a = parts_input(
        &plan, &dir, &allowed, &no_skipped, TaskKind::DraftChapter, Handoff::Proceed, 1, false,
        true,
    );
    let b = parts_input(
        &plan, &dir, &allowed, &no_skipped, TaskKind::DraftChapter, Handoff::Proceed, 2, false,
        true,
    );
    let c = parts_input(
        &plan, &dir, &allowed, &no_skipped, TaskKind::DraftChapter, Handoff::Proceed, 3, false,
        true,
    );
    let pa = prompt::build_task_parts(&a);
    let pb = prompt::build_task_parts(&b);
    let pc = prompt::build_task_parts(&c);

    assert_eq!(pa.fixed, pb.fixed, "同桶（只换 item_index/target）fixed 必须逐字相同");
    assert_eq!(pb.fixed, pc.fixed, "同桶（只换 item_index/target）fixed 必须逐字相同");
    assert_ne!(pa.varying, pb.varying, "不同 item_index 的 varying 必须不同");
    assert_ne!(pb.varying, pc.varying, "不同 item_index 的 varying 必须不同");
    assert_ne!(pa.varying, pc.varying, "不同 item_index 的 varying 必须不同");

    // allowed 不同 = 另一个桶：工具清单进了 fixed，预期分叉，这不是 bug。
    let d = parts_input(
        &plan, &dir, &narrow, &no_skipped, TaskKind::DraftChapter, Handoff::Proceed, 1, false,
        true,
    );
    let pd = prompt::build_task_parts(&d);
    assert_ne!(
        pd.fixed, pa.fixed,
        "allowed 不同时 fixed 应当分叉（工具清单在固定段内）"
    );
}

/// 消息顺序契约：system → … → user(varying) → user(own_task)，own_task 永远是最后一条。
#[test]
fn prefix_own_task_is_last() {
    use crate::ai_service::skill_agent::core::item_messages;

    let dir = skills_dir();
    let plan = parts_plan();
    let allowed: Vec<&str> = vec!["read_skill", "read_file"];
    let no_skipped: Vec<String> = Vec::new();
    let input = parts_input(
        &plan, &dir, &allowed, &no_skipped, TaskKind::DraftChapter, Handoff::Proceed, 1, false,
        true,
    );
    let parts = prompt::build_task_parts(&input);
    assert!(!parts.varying.is_empty(), "varying 为空就测不到「倒数第二」");

    let own_task = "这一项是：起草第 1 章正文，写完就收尾。";
    let base: Vec<LlmMessage> = Vec::new();
    let msgs = item_messages(
        String::from("SYSTEM-PROMPT"),
        &base,
        "",
        String::new(),
        &parts.varying,
        own_task,
    );

    assert_eq!(msgs.len(), 3, "digest / materials 为空时只 push system + varying + own_task");
    let last = msgs.last().expect("最后一条必须存在");
    assert_eq!(last.role, "user");
    assert_eq!(last.content, own_task, "最后一条必须是 own_task 原文");
    let second_last = &msgs[msgs.len() - 2];
    assert_eq!(second_last.role, "user", "varying 也必须是 user 角色");
    assert_eq!(second_last.content, parts.varying, "varying 必须是倒数第二条");
}

/// 「留给最后一项问」只出现在中间项；最后一项换成收尾分支，不该再有这个标记。
#[test]
fn prefix_deferred_mark_kept() {
    let dir = skills_dir();
    let plan = parts_plan();
    let allowed: Vec<&str> = vec!["read_skill", "read_file"];
    let no_skipped: Vec<String> = Vec::new();

    let middle = parts_input(
        &plan, &dir, &allowed, &no_skipped, TaskKind::DraftChapter, Handoff::Proceed, 1, false,
        true,
    );
    let last = parts_input(
        &plan, &dir, &allowed, &no_skipped, TaskKind::DraftChapter, Handoff::Proceed, 3, true,
        true,
    );
    let pm = prompt::build_task_parts(&middle);
    let pl = prompt::build_task_parts(&last);
    let mark = prompt::DEFERRED_MARK;

    assert!(
        pm.varying.contains(mark),
        "中间项（is_last=false）的 varying 必须含 {mark}"
    );
    assert!(
        !pl.varying.contains(mark),
        "最后一项（is_last=true）的 varying 不应含 {mark}"
    );
}

/// 拆段不能丢字也不该重复：把 21 段拆成两份再拼回来，手册与各条规则文案都必须各出现且只出现一次。
#[test]
fn prefix_no_text_loss() {
    let dir = skills_dir();
    assert!(dir.is_dir(), "skills dir: {}", dir.display());
    let plan = parts_plan();
    let allowed: Vec<&str> = vec!["read_skill", "read_file", "write_file", "list_files"];
    let no_skipped: Vec<String> = Vec::new();
    // land=false 且 is_last=false：把 DICTATE_NOTE 与 _5.md 两条条件文案也纳入覆盖。
    let input = parts_input(
        &plan, &dir, &allowed, &no_skipped, TaskKind::DraftChapter, Handoff::Proceed, 1, false,
        false,
    );
    let parts = prompt::build_task_parts(&input);
    let split = format!("{}{}", parts.fixed, parts.varying);

    let features = [
        ("_1.md 一项一段", "【一项一段，各报各的】"),
        ("_2.md 对不上就别做", "【对不上用户的意思就别做】"),
        ("_2.md 已有的不要重做", "【已有的不要重做，也不要加码】"),
        ("_2.md 承接材料只是参考", "【承接材料只是参考】"),
        ("_3.md 听用户的", "【听用户的】"),
        ("ACTION_VOCAB", "【两个动作词，别混用】"),
        ("DICTATE_NOTE", "【本轮不产出能跑的章节】"),
        ("_5.md 中间项别提问", "**你是中间项：不要向用户提问**"),
        ("回执怎么写", "【回执怎么写】"),
        ("职责边界", "【职责边界】"),
        ("BOUNDARY_BASELINE", "把**本轮排给你的任务**全部做完再收尾"),
        ("SKILL.md 标题", "# 创作（Writer）"),
        ("SKILL.md 元信息", "name: script-writer"),
    ];
    for (name, needle) in features {
        assert_eq!(
            split.matches(needle).count(),
            1,
            "{name}：fixed+varying 里必须恰好出现一次"
        );
        assert_eq!(
            parts.ordered.matches(needle).count(),
            1,
            "{name}：ordered 里必须恰好出现一次"
        );
    }
}

/// 最大固定段必须是整本角色手册：DraftChapter → materials() = ["script-writer/SKILL.md"]（17,808 B）。
#[test]
fn prefix_manual_in_fixed() {
    let dir = skills_dir();
    assert!(dir.is_dir(), "skills dir: {}", dir.display());
    let plan = parts_plan();
    let allowed: Vec<&str> = vec!["read_skill", "read_file"];
    let no_skipped: Vec<String> = Vec::new();
    let input = parts_input(
        &plan, &dir, &allowed, &no_skipped, TaskKind::DraftChapter, Handoff::Proceed, 1, false,
        false,
    );
    let parts = prompt::build_task_parts(&input);

    let title = "【角色指令 · script-writer/SKILL.md】";
    assert!(
        parts.fixed.contains(title),
        "整本手册的标题必须在固定段：{title}"
    );
    assert!(
        parts.fixed.len() > 10_000,
        "整本手册（约 17.8 KB）必须进固定段，实际只有 {} B",
        parts.fixed.len()
    );
    assert!(
        !parts.varying.contains(title),
        "手册标题不该出现在随项变段"
    );
}

/// skipped 逐轮变化只进 varying；固定段不许复述用户原话。
#[test]
fn prefix_skipped_in_varying_not_fixed() {
    let dir = skills_dir();
    assert!(dir.is_dir(), "skills dir: {}", dir.display());
    let plan = parts_plan();
    let allowed: Vec<&str> = vec!["read_skill", "read_file", "write_file", "list_files"];
    let no_skipped: Vec<String> = Vec::new();
    let some_skipped: Vec<String> = vec!["盘点素材（还没有素材目录）".to_string()];

    let a = parts_input(
        &plan, &dir, &allowed, &no_skipped, TaskKind::DraftChapter, Handoff::Proceed, 1, false,
        true,
    );
    let b = parts_input(
        &plan, &dir, &allowed, &some_skipped, TaskKind::DraftChapter, Handoff::Proceed, 1, false,
        true,
    );
    let pa = prompt::build_task_parts(&a);
    let pb = prompt::build_task_parts(&b);

    assert_eq!(
        pa.fixed, pb.fixed,
        "skipped 变化（换轮）不许影响固定段，否则跨轮前缀复用拿不到"
    );
    assert_ne!(pa.varying, pb.varying, "skipped 必须进 varying");
    assert!(
        pb.varying.contains("【队列里做不了的项】"),
        "skipped 的文案必须落在 varying 里"
    );
    assert!(
        !pb.fixed.contains("【队列里做不了的项】"),
        "固定段里不许再出现 skipped 文案"
    );
    assert!(
        !pb.fixed.contains("【用户这一轮的原话】"),
        "固定段里不许再复述用户原话（原话已在 base_history 的 user 消息里）"
    );
    let golden = prompt::build_task_block(&b);
    assert_eq!(pb.ordered, golden, "两份实现仍须逐字对拍");
}
