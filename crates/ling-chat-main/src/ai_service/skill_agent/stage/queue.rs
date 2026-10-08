//! 队列账本：`.agent/queue.json` 是真相，`queue.md` / `queue-done.md` 是它的视图。

use super::evidence::{Evidence, artifact_exists, entry_evidence};
use super::prompt::render_text_of;
use super::{AGENT_DIR, QUEUE_DONE_REL_PATH, QUEUE_REL_PATH};
use crate::ai_service::skill_agent::role::TaskKind;
use crate::ai_service::skill_agent::router::QueueItem;
use serde::{Deserialize, Deserializer, Serialize, Serializer};
use std::path::Path;

/// 队列的机器状态（真相）：serde 序列化的 `Vec<QueueEntry>`，markdown 那份只是它的视图。
pub(super) const QUEUE_JSON_REL_PATH: &str = ".agent/queue.json";

/// 归档的机器状态。与 [`QUEUE_JSON_REL_PATH`] 同构，只是装着了结过的项。
const QUEUE_DONE_JSON_REL_PATH: &str = ".agent/queue-done.json";

/// 归档保留条数上限：这是给人回看的流水，不该无限长。
const QUEUE_DONE_KEEP: usize = 50;

/// 视图里的登记时刻只到秒（`[write_chapter@1790664196]`）；状态里存的是完整时刻（纳秒）。
const NANOS_PER_SEC: u64 = 1_000_000_000;

/// 账本里的一项，只在本轮有效；JSON 里就是这几个字段。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub(super) struct QueueEntry {
    pub(super) mark: Mark,
    #[serde(serialize_with = "kind_key", deserialize_with = "parse_kind")]
    pub(super) kind: TaskKind,
    /// 目标（章号 / 大纲 / 素材表…），空串 = 未指明；去重与物证核对都只看它，不从正文里反解。
    pub(super) target: String,
    /// 登记时刻（Unix 纳秒）。存满精度，于是"这一项是不是本轮的"就是一次相等比较。
    pub(super) registered_at: u64,
    /// 登记那一刻产物在不在盘上。**删除唯一的成功信号就是它**：登记时在、核账时不见了 = 删掉了；
    #[serde(default)]
    pub(super) existed_at_register: bool,
}

/// `TaskKind` 归 `role.rs` 所有（不在那儿派生 serde），JSON 里只存它的 key。
fn kind_key<S: Serializer>(kind: &TaskKind, s: S) -> Result<S::Ok, S::Error> {
    s.serialize_str(kind.key())
}

fn parse_kind<'de, D: Deserializer<'de>>(d: D) -> Result<TaskKind, D::Error> {
    let raw = String::deserialize(d)?;
    TaskKind::parse(&raw).ok_or_else(|| serde::de::Error::custom(format!("未知任务类型：{raw}")))
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub(super) enum Mark {
    /// `- [ ]` 这一轮排的，还没做完。
    Open,
    /// `- [x]` 这一轮真的做出来了（物证在登记之后被改写过）。
    Done,
    /// `- [=]` 东西本来就在盘上、这一轮没动过：不算做完也不算欠（只有产出型会落到这里）。
    Existing,
    /// `- [~]` 没做完就被放下了：既不算做完也不再执行，只留一行痕，免得磁盘上看不出它没做成。
    Dropped,
}

/// 把本轮的账落盘：上一轮的项出队留痕、登记本轮的项、按磁盘事实打勾、做完的搬进归档，状态先写、视图后渲染。
pub fn write_queue(
    dir: &Path,
    items: &[QueueItem],
    registered_at: std::time::SystemTime,
) -> std::io::Result<()> {
    let now = epoch_nanos(registered_at);

    let (kept, stale): (Vec<_>, Vec<_>) = read_state(dir, QUEUE_JSON_REL_PATH)
        .into_iter()
        .partition(|e| e.registered_at == now);
    let mut closed: Vec<QueueEntry> = stale
        .into_iter()
        .map(|e| QueueEntry {
            mark: if e.mark == Mark::Open {
                Mark::Dropped
            } else {
                e.mark
            },
            ..e
        })
        .collect();
    let (done, mut kept): (Vec<_>, Vec<_>) = kept.into_iter().partition(|e| e.mark == Mark::Done);
    closed.extend(done);

    for item in items {
        let target = item.target.trim();
        if !is_judgeable(item.kind, target) {
            continue;
        }
        if kept
            .iter()
            .any(|e| e.kind == item.kind && e.target == target)
        {
            continue;
        }
        kept.push(QueueEntry {
            mark: Mark::Open,
            kind: item.kind,
            target: target.to_string(),
            registered_at: now,
            existed_at_register: artifact_exists(dir, item.kind, target),
        });
    }

    let (settled, open): (Vec<_>, Vec<_>) = kept
        .into_iter()
        .map(|e| (entry_evidence(dir, &e), e))
        .partition(|(ev, _)| *ev != Evidence::Open);
    let open: Vec<QueueEntry> = open.into_iter().map(|(_, e)| e).collect();
    closed.extend(settled.into_iter().map(|(ev, e)| QueueEntry {
        mark: ev.mark(),
        ..e
    }));

    if closed.is_empty()
        && open.is_empty()
        && !dir.join(QUEUE_JSON_REL_PATH).exists()
        && !dir.join(QUEUE_REL_PATH).exists()
    {
        return Ok(());
    }

    ensure_agent_dir(dir)?;
    write_state(dir, QUEUE_JSON_REL_PATH, &open)?;
    write_atomic(&dir.join(QUEUE_REL_PATH), &render_view(QUEUE_HEADER, &open))?;
    archive_queue_done(dir, &closed)
}

/// 队列与归档都写在 `.agent/` 下，而包刚诞生时这个目录还不存在，由代码自己保证它在。
fn ensure_agent_dir(dir: &Path) -> std::io::Result<()> {
    if dir.is_dir() {
        std::fs::create_dir_all(dir.join(AGENT_DIR))?;
    }
    Ok(())
}

/// 先写同目录下的 `.tmp` 临时文件再改名：这一轮要写多次，撞上中断会丢账本；`.tmp` 结尾不会被产物枚举认领。
fn write_atomic(path: &Path, text: &str) -> std::io::Result<()> {
    let name = path
        .file_name()
        .map(|n| n.to_string_lossy().into_owned())
        .unwrap_or_default();
    let tmp = path.with_file_name(format!("{name}.tmp"));
    std::fs::write(&tmp, text)?;
    match std::fs::rename(&tmp, path) {
        Ok(()) => Ok(()),
        Err(e) => {
            let _ = std::fs::remove_file(&tmp);
            Err(e)
        },
    }
}

/// 登记时刻与文件时刻都按 Unix 纳秒比："是不是同一轮"就是一次相等比较，改动判定不留时间容差。
pub(super) fn epoch_nanos(at: std::time::SystemTime) -> u64 {
    at.duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_nanos() as u64)
        .unwrap_or_default()
}

/// 这一项在账本上还欠着吗：登记了且仍是 `Open`。不在账本里的（不产出可核对产物、或已经了结）返回 false。
pub fn still_owed(dir: &Path, kind: TaskKind, target: &str) -> bool {
    let target = target.trim();
    read_state(dir, QUEUE_JSON_REL_PATH)
        .into_iter()
        .any(|e| e.kind == kind && e.target == target && e.mark == Mark::Open)
}

/// 这一类的"做完"有没有物证可核对；没有的不进账本，章节类还要求目标非空。
fn is_judgeable(kind: TaskKind, target: &str) -> bool {
    match kind {
        TaskKind::WriteChapter | TaskKind::ReviseChapter | TaskKind::DraftChapter => {
            !target.trim().is_empty()
        },
        TaskKind::Outline | TaskKind::ReviseOutline | TaskKind::CheckAssets => true,
        _ => false,
    }
}

fn render_text(entry: &QueueEntry) -> String {
    render_text_of(entry.kind, &entry.target)
}

/// 队列视图 / 归档视图的头部。
const QUEUE_HEADER: &str = include_str!("../prompts/stage_queue_header.md");

const QUEUE_DONE_HEADER: &str = "# 已归档（`[x]` 代码按磁盘事实核对过的、这一轮真做出来的，或上一轮已经勾过的；\
    `[=]` 本来就在盘上、这一轮没动过（不算欠）；`[~]` 没做完就被放下的 —— 不再执行，也没当成做完）\n\n";

/// 渲染一份视图：队列画"还没做完的"，归档画了结过的（行格式 `- [ ] [kind@epoch] 正文`）。
fn render_view(header: &str, entries: &[QueueEntry]) -> String {
    let mut out = String::from(header);
    for entry in entries {
        let mark = match entry.mark {
            Mark::Open => ' ',
            Mark::Done => 'x',
            Mark::Existing => '=',
            Mark::Dropped => '~',
        };
        let at = entry.registered_at / NANOS_PER_SEC;
        out.push_str(&format!(
            "- [{mark}] [{}@{at}] {}\n",
            entry.kind.key(),
            render_text(entry)
        ));
    }
    out
}

/// 读机器状态。文件不在 / 读不动 / 解析失败 → 空账：宁可从这一轮重新记，也不拿半份状态去判。
pub(super) fn read_state(dir: &Path, rel: &str) -> Vec<QueueEntry> {
    std::fs::read_to_string(dir.join(rel))
        .ok()
        .and_then(|text| serde_json::from_str(&text).ok())
        .unwrap_or_default()
}

/// 写机器状态（JSON），同样先写临时文件再改名。
fn write_state(dir: &Path, rel: &str, entries: &[QueueEntry]) -> std::io::Result<()> {
    let json = serde_json::to_string_pretty(entries)
        .map_err(|e| std::io::Error::new(std::io::ErrorKind::InvalidData, e))?;
    write_atomic(&dir.join(rel), &json)
}

/// 归档去重只看 `(kind, target, mark)`：同一件事、同一个结论只留一行（登记时刻不同也算同一件）。
fn same_conclusion(a: &QueueEntry, b: &QueueEntry) -> bool {
    a.kind == b.kind && a.target == b.target && a.mark == b.mark
}

/// 把了结的项搬进归档状态并重渲染归档视图，只留最近 [`QUEUE_DONE_KEEP`] 条。
fn archive_queue_done(dir: &Path, closed: &[QueueEntry]) -> std::io::Result<()> {
    if closed.is_empty() {
        return Ok(());
    }
    let mut entries = read_state(dir, QUEUE_DONE_JSON_REL_PATH);
    let fresh: Vec<QueueEntry> = closed
        .iter()
        .filter(|e| !entries.iter().any(|old| same_conclusion(old, e)))
        .cloned()
        .collect();
    entries.extend(fresh);
    if entries.len() > QUEUE_DONE_KEEP {
        entries.drain(..entries.len() - QUEUE_DONE_KEEP);
    }
    write_state(dir, QUEUE_DONE_JSON_REL_PATH, &entries)?;
    write_atomic(
        &dir.join(QUEUE_DONE_REL_PATH),
        &render_view(QUEUE_DONE_HEADER, &entries),
    )
}

/// 队列里还欠着的事，从机器状态算（`[=]` 不算欠、早已归档）。
pub(super) fn read_queue_progress(dir: &Path) -> Option<String> {
    let pending: Vec<String> = read_state(dir, QUEUE_JSON_REL_PATH)
        .into_iter()
        .filter(|e| e.mark == Mark::Open)
        .map(|e| render_text(&e))
        .collect();
    if pending.is_empty() {
        return None;
    }
    let mut out = format!("还欠 {} 项", pending.len());
    if let Some(next) = pending.first() {
        out.push_str(&format!("（最早一项：{}）", next));
    }
    out.push_str(
        "。这里列的就是还没做完的：用户说了不用做、或者已经改口做别的，\
         说一声就行，别去改队列文件（那是渲染出来的，改了不生效）",
    );
    Some(out)
}
