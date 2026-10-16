//! 物证层：盘上有什么、写到哪一步、产出物自检，所有事实只从这一处读盘。

use super::queue::{Mark, QueueEntry, epoch_nanos};
use super::{ASSETS_REL_PATH, CHAPTER_DETAILS_REL_PATH, DESIGN_REL_PATH, StageSnapshot};
use crate::ai_service::game_system::script_engine::validate;
use crate::ai_service::skill_agent::role::TaskKind;
use crate::utils::script_modes::CONSTRAINTS_REL_PATH;
use crate::utils::script_modes::{self, Mode, ScriptModes};
use crate::utils::script_paths;
use std::path::{Path, PathBuf};

/// 这一项的物证状态：只有"这一轮真的做出来的"才打勾，"本来就在盘上"（`Existing`）不算做完也不算欠。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum Evidence {
    Done,
    Existing,
    Open,
}

impl Evidence {
    pub(super) fn of_change(changed: bool) -> Self {
        if changed { Self::Done } else { Self::Open }
    }

    /// 归档时打的记号（还欠着的 `Open` 不进归档）。
    pub(super) fn mark(self) -> Mark {
        match self {
            Self::Done => Mark::Done,
            _ => Mark::Existing,
        }
    }
}

/// 登记那一刻这一项的产物在不在盘上（只看有没有，不看内容）。给账本记 `existed_at_register` 用 ——
pub(super) fn artifact_exists(dir: &Path, kind: TaskKind, target: &str) -> bool {
    match kind {
        TaskKind::WriteChapter | TaskKind::ReviseChapter => {
            chapter_file(dir, target).is_some_and(|p| p.is_file())
        },
        TaskKind::CheckAssets => dir.join(ASSETS_REL_PATH).is_file(),
        TaskKind::Outline | TaskKind::ReviseOutline => dir.join(DESIGN_REL_PATH).is_file(),
        TaskKind::DraftChapter => {
            let keys = chapter_keys(target);
            !keys.is_empty() && {
                let text = read_or_empty(&dir.join(CHAPTER_DETAILS_REL_PATH));
                keys.iter()
                    .all(|k| chapter_titled(&text, std::slice::from_ref(k)))
            }
        },
        _ => false,
    }
}

/// 判定"这一轮做出来的"：产出型（转成 YAML）看 `Chapters/<id>.yaml` 在不在且登记之后被改过（在但没动给 `Existing`，不算欠）；
pub(super) fn entry_evidence(dir: &Path, entry: &QueueEntry) -> Evidence {
    let at = entry.registered_at;
    match entry.kind {
        TaskKind::WriteChapter => match chapter_file(dir, &entry.target) {
            Some(p) if changed_after(&p, at) => Evidence::Done,
            Some(p) if p.exists() => Evidence::Existing,
            _ => Evidence::Open,
        },
        TaskKind::ReviseChapter => {
            let file = chapter_file(dir, &entry.target);
            let here = file.as_deref().is_some_and(Path::is_file);
            if here && file.as_deref().is_some_and(|p| changed_after(p, at)) {
                Evidence::Done
            } else if !here && entry.existed_at_register {
                Evidence::Done
            } else {
                Evidence::Open
            }
        },
        TaskKind::CheckAssets => Evidence::of_change(changed_after(&dir.join(ASSETS_REL_PATH), at)),
        TaskKind::Outline | TaskKind::ReviseOutline => {
            let keys = chapter_keys(&entry.target);
            if keys.is_empty() {
                return Evidence::of_change(changed_after(&dir.join(DESIGN_REL_PATH), at));
            }
            chapter_section_evidence(&dir.join(DESIGN_REL_PATH), &keys, at)
        },
        TaskKind::DraftChapter => {
            let keys = chapter_keys(&entry.target);
            if keys.is_empty() {
                return Evidence::Open;
            }
            chapter_section_evidence(&dir.join(CHAPTER_DETAILS_REL_PATH), &keys, at)
        },
        _ => Evidence::Open,
    }
}

/// 目标里的章号（列表与区间都拆开，归一成 [`chapter_key`] 的口径）；空 = 没点名章节，物证要认全否则那项永远挂着。
fn chapter_keys(target: &str) -> Vec<String> {
    chapter_ids_in(target)
        .iter()
        .map(|t| chapter_key(t))
        .filter(|k| !k.is_empty())
        .collect()
}

/// 这些章是否全部都在稿子里；都在再分"这一轮改过"（`Done`）与"本来就在"（`Existing`）。
fn chapter_section_evidence(path: &Path, keys: &[String], at: u64) -> Evidence {
    let text = read_or_empty(path);
    if !keys
        .iter()
        .all(|k| chapter_titled(&text, std::slice::from_ref(k)))
    {
        return Evidence::Open;
    }
    if changed_after(path, at) {
        Evidence::Done
    } else {
        Evidence::Existing
    }
}

/// 这一项在账本上的状态，翻成一句注入给模型的进度话术；不写"完成 / 未完成"，勾只代表产物本轮被改过。
pub(super) fn mark_phrase(mark: Mark) -> &'static str {
    match mark {
        Mark::Done => "本轮做出来了（这个产物本轮被改过）",
        Mark::Existing => "盘上本来就有、这一轮没动它",
        Mark::Open => "还没核对到产物（可能没做成；这一项若本来就是删除类，那是正常的）",
        Mark::Dropped => "上一轮没做完就放下的（本轮不重做）",
    }
}

/// 这一份文件在 `at`（这一项的登记时刻）之后被改过吗。
fn changed_after(path: &Path, at: u64) -> bool {
    std::fs::metadata(path)
        .and_then(|m| m.modified())
        .map(|m| epoch_nanos(m) >= at)
        .unwrap_or(false)
}

pub(super) fn read_or_empty(path: &Path) -> String {
    std::fs::read_to_string(path).unwrap_or_default()
}

/// 这份稿子里有没有点名这一章的小节（`## 第4章 · …` / `## 第4章`）：只看标题行，章节号归一成第一个数字串再比。
fn chapter_titled(text: &str, keys: &[String]) -> bool {
    text.lines()
        .map(str::trim_start)
        .filter(|line| line.starts_with('#'))
        .any(|head| keys.contains(&chapter_key(head)))
}

/// 本轮前面几项预计会产出什么：承接判断在开跑前一次做完，不并进事实就会把这些项误判成做不了。
#[derive(Default)]
pub struct Pending {
    /// 预计会落盘的章节。
    pub(super) landed: Vec<String>,
    /// 预计会写进 `.agent/chapter-details.md` 的章节。
    pub(super) drafted: Vec<String>,
    /// 预计会写出或改动设计稿。
    pub(super) design: bool,
}

impl Pending {
    /// 这一项做完会多出什么（目标里的章节号已归一）。
    pub fn of(kind: TaskKind, target: &str) -> Self {
        let ids = chapter_ids_in(target);
        match kind {
            TaskKind::WriteChapter => Self {
                landed: ids,
                ..Default::default()
            },
            TaskKind::DraftChapter => Self {
                drafted: ids,
                ..Default::default()
            },
            TaskKind::Outline => Self {
                design: true,
                ..Default::default()
            },
            TaskKind::ReviseOutline => Self {
                design: true,
                ..Default::default()
            },
            _ => Self::default(),
        }
    }

    pub fn add(&mut self, other: Self) {
        self.landed.extend(other.landed);
        self.drafted.extend(other.drafted);
        self.design |= other.design;
    }
}

/// 承接判断要用的事实 = 磁盘上的 + 本轮前面几项预计会产出的（`pending`）；判据 [`crate::ai_service::skill_agent::role::reconcile`] 不变。
pub fn facts_of(
    snap: &StageSnapshot,
    target: Option<&str>,
    pending: &Pending,
) -> crate::ai_service::skill_agent::role::ScriptFacts {
    let wanted: Vec<String> = target.map(chapter_ids_in).unwrap_or_default();
    let same_chapter = |a: &str, b: &str| {
        let (ka, kb) = (chapter_key(a), chapter_key(b));
        if ka.is_empty() || kb.is_empty() {
            a.trim() == b.trim()
        } else {
            ka == kb
        }
    };
    let landed = |id: &str| {
        snap.written.iter().any(|w| same_chapter(w, id))
            || pending.landed.iter().any(|w| same_chapter(w, id))
    };
    let present: Vec<bool> = wanted.iter().map(|t| landed(t.as_str())).collect();
    let target_exists = if wanted.is_empty() {
        None
    } else {
        Some(present.iter().any(|p| *p))
    };
    let has_design = pending.design  // 「文件在」与「读得出章节」是两件事：前者决定能不能改它，后者只是格式提醒，别当前置用。
        || snap
            .script_dir
            .as_deref()
            .and_then(|d| std::fs::metadata(d.join(DESIGN_REL_PATH)).ok())
            .is_some_and(|m| m.len() > 0);
    let next = snap.plan.iter().find(|id| !landed(id.as_str()));
    let has_draft = pending.drafted.iter().any(|d| wanted.contains(d))
        || snap
            .script_dir
            .as_deref()
            .is_some_and(|d| chapter_draft_present(d, &wanted));
    crate::ai_service::skill_agent::role::ScriptFacts {
        has_design,
        has_plan: !snap.plan.is_empty(),
        has_written: !snap.written.is_empty() || !pending.landed.is_empty(),
        has_next: next.is_some(),
        target_exists,
        target_partial: present.iter().any(|p| *p) && present.iter().any(|p| !*p),
        target_is_next: next.is_some_and(|n| wanted.iter().any(|w| same_chapter(w, n))),
        has_draft,
    }
}

/// 细节稿里有没有点名这一章的小节：落盘只把已写好的剧情转成 YAML，第一问就是编剧写了没有。
fn chapter_draft_present(dir: &Path, ids: &[String]) -> bool {
    let wanted: Vec<String> = ids.iter().map(|id| chapter_key(id)).collect();
    if wanted.iter().all(String::is_empty) {
        return false;
    }
    chapter_titled(&read_or_empty(&dir.join(CHAPTER_DETAILS_REL_PATH)), &wanted)
}

/// 章节号归一：取第一个数字串去掉前导零；不含数字的（`Intro/x`、`end`）返回空串，不参与比较。
fn chapter_key(raw: &str) -> String {
    let mut run = String::new();
    for ch in raw.chars() {
        if ch.is_ascii_digit() {
            run.push(ch);
        } else if !run.is_empty() {
            break;
        }
    }
    run.trim_start_matches('0').to_string()
}

/// 用户点名的章节 id 列表（`第 3 章` 归一成 `3`）；区间与列表都要拆开，按单个 id 查不到会误判成还没落盘。
pub fn chapter_ids_in(raw: &str) -> Vec<String> {
    /// 一个区间最多展开这么多章，防止 `01-9999` 这种写法把内存撑爆。
    const MAX_SPAN: u32 = 200;

    let mut out: Vec<String> = Vec::new();
    for part in raw.split([',', '，', '、', ';', '；', '和', '与', ' ']) {
        let p = normalize_chapter_token(part);
        if p.is_empty() {
            continue;
        }
        let Some(dash) = p.find(['-', '~', '–', '—']) else {
            out.push(p);
            continue;
        };
        let dash_len = p[dash..].chars().next().map_or(1, char::len_utf8);
        let (a, b) = (
            p[..dash].trim(),
            p[dash + dash_len..]
                .trim_start_matches(['-', '~', '–', '—'])
                .trim(),
        );
        match (a.parse::<u32>(), b.parse::<u32>()) {
            (Ok(start), Ok(end))
                if start <= end
                    && end - start <= MAX_SPAN
                    && a.chars().all(|c| c.is_ascii_digit())
                    && b.chars().all(|c| c.is_ascii_digit()) =>
            {
                let width = a.len().max(b.len());
                for n in start..=end {
                    out.push(format!("{:0width$}", n, width = width));
                }
            },
            _ => out.push(p),
        }
    }
    out.dedup();
    out
}

/// `第 3 章` / `03节` / `"03"` → `03`（去空白与引号，剥掉"第/章/节"）。
fn normalize_chapter_token(raw: &str) -> String {
    let t = raw
        .trim()
        .trim_matches(['"', '\'', '`', '《', '》', '「', '」']);
    let t = t.strip_prefix('第').unwrap_or(t);
    let t = t
        .strip_suffix('章')
        .or_else(|| t.strip_suffix('节'))
        .unwrap_or(t);
    t.trim().to_string()
}

pub(super) fn parse_plan(markdown: &str) -> Vec<String> {
    let lines: Vec<&str> = markdown.lines().collect();
    let mut plan: Vec<String> = Vec::new();
    for (start, end) in block_ranges(&lines) {
        if let Some(id) = block_id(&lines[start + 1..end]) {
            if !plan.iter().any(|p| p == id) {
                plan.push(id.to_string());
            }
        }
    }
    plan
}

pub(super) fn extract_chapter_block(markdown: &str, id: &str) -> Option<String> {
    let lines: Vec<&str> = markdown.lines().collect();
    for (start, end) in block_ranges(&lines) {
        if block_id(&lines[start + 1..end]) == Some(id) {
            return Some(lines[start..end].join("\n").trim_end().to_string());
        }
    }
    None
}

fn block_ranges(lines: &[&str]) -> Vec<(usize, usize)> {
    let starts: Vec<usize> = lines
        .iter()
        .enumerate()
        .filter(|(_, l)| l.trim_start().starts_with("##"))
        .map(|(i, _)| i)
        .collect();
    starts
        .iter()
        .enumerate()
        .map(|(n, &s)| (s, starts.get(n + 1).copied().unwrap_or(lines.len())))
        .collect()
}

fn block_id<'a>(block: &[&'a str]) -> Option<&'a str> {
    let first = block.iter().map(|l| l.trim()).find(|l| !l.is_empty())?;
    let rest = first.strip_prefix("id:")?;
    Some(rest.trim().trim_matches(&['"', '\''][..]).trim())
}

/// 取章节末尾的注释块，即 `SKILL.md`「状态注释原则」要求记录的收尾状态。
pub(super) fn tail_state(chapter_text: &str) -> String {
    let mut tail: Vec<&str> = Vec::new();
    for line in chapter_text.lines().rev() {
        let t = line.trim();
        if t.starts_with('#') {
            tail.push(line);
        } else if !t.is_empty() {
            break;
        }
    }
    tail.reverse();
    tail.join("\n")
}

pub(super) fn read_chapter(script_dir: &Path, id: &str) -> String {
    chapter_file(script_dir, id)
        .and_then(|p| std::fs::read_to_string(p).ok())
        .unwrap_or_default()
}

fn chapter_file(script_dir: &Path, id: &str) -> Option<PathBuf> {
    script_paths::resolve_chapter_file(script_dir, id, false).ok()
}

pub(crate) fn chapter_id_of_path(path: &str) -> Option<String> {
    let normalized = path.replace('\\', "/");
    let segs: Vec<&str> = normalized.split('/').collect();
    let at = segs
        .iter()
        .skip(1)
        .position(|s| s.eq_ignore_ascii_case("chapters"))?
        + 1;
    let tail = segs[at + 1..].join("/");
    let id = tail
        .strip_suffix(".yaml")
        .or_else(|| tail.strip_suffix(".yml"))?;
    (!id.is_empty()).then(|| id.to_string())
}

/// 从剧本包内任意写入路径反推 key，会话绑定 / 归属反推 / 外来包拦截共用这一个入口。
pub fn script_key_of(path: &str, known: &[String]) -> Option<String> {
    let normalized = path.replace('\\', "/");
    let (_, tail) = normalized.split_once("/scripts/")?;
    if let Some(root) = ["/.agent/", "/Chapters/"]
        .into_iter()
        .find_map(|a| tail.split_once(a).map(|(root, _)| root))
        .filter(|root| !root.is_empty())
    {
        return Some(root.to_string());
    }
    if let Some(root) = tail
        .strip_suffix("/story_config.yaml")
        .filter(|root| !root.is_empty())
    {
        return Some(root.to_string());
    }
    let mut keys: Vec<&String> = known.iter().collect();
    keys.sort_by_key(|k| std::cmp::Reverse(k.len()));
    keys.into_iter()
        .find(|k| normalized.contains(&format!("/scripts/{}/", k.trim_matches('/'))))
        .cloned()
}

/// 这个剧本用不用人物卡（卡片声明过模式、角色卡羁绊冒险、或已建 `characters/`），决定交接单要不要带角色卡模式那一行。
pub(super) fn uses_character_cards(
    snap: &StageSnapshot,
    script_dir: &Path,
    constraints: &str,
) -> bool {
    if script_modes::declares_cast(constraints) {
        return true;
    }
    if snap
        .script_key
        .as_deref()
        .is_some_and(|k| k.replace('\\', "/").starts_with("character/"))
    {
        return true;
    }
    std::fs::read_dir(script_dir.join("characters")).is_ok_and(|mut d| d.next().is_some())
}

#[derive(Debug, Default, PartialEq, Eq)]
pub struct ChapterCheck {
    pub errors: Vec<String>,
    pub warnings: Vec<String>,
    /// 本章引用了但磁盘上没有的素材名（允许缺失时只登记，不阻断）。
    pub missing_assets: Vec<String>,
    pub mode: Mode,
}

impl ChapterCheck {
    pub fn is_empty(&self) -> bool {
        self.errors.is_empty() && self.warnings.is_empty()
    }

    pub fn render(&self) -> String {
        let mut out = String::new();
        if !self.errors.is_empty() {
            out.push_str("\n\n[章节自检] 这一章还不算写完，先改这些：\n");
            for e in &self.errors {
                out.push_str(&format!("- {}\n", e));
            }
            out.push_str("改好后重新 write_file 覆盖它（不要 append）。\n");
        }
        if !self.warnings.is_empty() {
            out.push_str("\n[章节自检] 记下来即可，不阻断：\n");
            for w in &self.warnings {
                out.push_str(&format!("- {}\n", w));
            }
        }
        if !self.missing_assets.is_empty() {
            out.push_str(&format!(
                "缺的素材（{}）已经记进 .agent/assets.md，别等交付才发现。\n",
                self.missing_assets.join("、")
            ));
            out.push_str(&self.mode.switch_hint("素材"));
        }
        out
    }
}

/// 章节写入后的轻量自检，不适用或无问题时返回 `None`；不做整剧本校验（断链诊断会误导模型去补后续章节）。
pub fn check_written_chapter(
    snap: &StageSnapshot,
    path: &str,
    data_dir: &Path,
) -> Option<ChapterCheck> {
    let (dir, id) = chapter_of_write(snap, path)?;
    check_chapter(dir, &id, data_dir)
}

/// 认出「刚写的是本会话这个剧本的哪一章」；认不出就不产生自检（数据目录也留到确认之后再取）。
fn chapter_of_write<'a>(snap: &'a StageSnapshot, path: &str) -> Option<(&'a Path, String)> {
    let dir = snap.script_dir.as_deref()?;
    let id = chapter_id_of_path(path)?;
    let normalized = path.replace('\\', "/").to_lowercase();
    let key = snap.script_key.as_deref()?;
    if !normalized.contains(&format!("{}/chapters/", key.to_lowercase())) {
        return None;
    }
    Some((dir, id))
}

fn check_chapter(dir: &Path, id: &str, data_dir: &Path) -> Option<ChapterCheck> {
    let file = chapter_file(dir, id)?;
    let value = match crate::utils::yaml_file::read_yaml_as_json(&file) {
        Ok(v) => v,
        Err(e) => {
            return Some(ChapterCheck {
                errors: vec![format!("`{}` 不是可解析的 YAML：{}", id, e)],
                ..Default::default()
            });
        },
    };

    let constraints = std::fs::read_to_string(dir.join(CONSTRAINTS_REL_PATH)).unwrap_or_default();
    let mut check = ChapterCheck {
        errors: chapter_shape_problems(&value),
        mode: ScriptModes::parse(&constraints).asset,
        ..Default::default()
    };

    let findings = validate::check_chapter_assets(data_dir, dir, id, &value);
    for d in findings {
        if let Some(name) = validate::missing_asset_name(&d) {
            if !check.missing_assets.iter().any(|m| m == name) {
                check.missing_assets.push(name.to_string());
            }
        }
        let at = d.event_index.map(|i| i + 1).unwrap_or_default();
        let line = format!("第 {} 个事件 · {}", at, d.message);
        if check.mode == Mode::OnlyExisting {
            // 松紧由用户声明的素材模式决定：只有"只用已有"才算错误，"允许缺失"是用户允许的缺口。
            check.errors.push(line);
        } else {
            check.warnings.push(line);
        }
    }
    let design = std::fs::read_to_string(dir.join(DESIGN_REL_PATH)).unwrap_or_default();
    if let Some(block) = extract_chapter_block(&design, id) {
        let appearing = appearing_cast(&value);
        for who in declared_cast(&block) {
            if !appearing.iter().any(|a| a.eq_ignore_ascii_case(&who)) {
                check.warnings.push(format!(
                    "设计稿说本章「{who}」登场，但这一章里没有它出场\
                     （没有任何事件的 `character` 是 {who}）"
                ));
            }
        }
    }
    if !check.missing_assets.is_empty() && !script_modes::declares_asset(&constraints) {
        check.warnings.push(format!(
            "`素材模式`还没声明：这一轮先问用户「只用已有」还是「允许缺失、之后补」，\
             写进 {}（格式 `- 素材模式：只用已有`）。\
             未声明时缺失只按警告算，但这正是用户最在意的那类错。",
            CONSTRAINTS_REL_PATH
        ));
    }

    (!check.is_empty()).then_some(check)
}

/// 设计稿里这一章声明登场的角色（`登场:` 那一行）；与 YAML 的 `character` 同口径，可直接比名字。
fn declared_cast(chapter_block: &str) -> Vec<String> {
    for line in chapter_block.lines() {
        let head = line.trim_start_matches(['-', '*', ' ', '\t']);
        let Some(rest) = head.strip_prefix("登场") else {
            continue;
        };
        let value = rest.trim_start_matches([':', '：']);
        return value
            .split([',', '，', '、', '/', ' '])
            .map(|s| s.trim())
            .filter(|s| !s.is_empty())
            .map(|s| s.to_string())
            .collect();
    }
    Vec::new()
}

/// 章节 YAML 里实际出现的角色：事件顶层的 `character` 字段（不按事件类型硬编码，扫键更不易漏）。
fn appearing_cast(value: &serde_json::Value) -> Vec<String> {
    let Some(events) = value.get("events").and_then(|v| v.as_array()) else {
        return Vec::new();
    };
    let mut out: Vec<String> = Vec::new();
    for ev in events {
        let Some(name) = ev.get("character").and_then(|v| v.as_str()) else {
            continue;
        };
        let name = name.trim();
        if !name.is_empty() && !out.iter().any(|c| c == name) {
            out.push(name.to_string());
        }
    }
    out
}

/// 把本章的素材缺口并进 `.agent/assets.md`，由代码维护，同名素材只登记一次。
pub fn update_assets_gap(dir: &Path, chapter: &str, missing: &[String]) -> std::io::Result<()> {
    if missing.is_empty() {
        return Ok(());
    }
    let path = dir.join(ASSETS_REL_PATH);
    let old = std::fs::read_to_string(&path).unwrap_or_default();
    let mut rows: Vec<String> = old
        .lines()
        .filter(|l| l.trim_start().starts_with("-「"))
        .map(|l| l.trim().to_string())
        .collect();

    for name in missing {
        let marker = format!("-「{name}」");
        if rows.iter().any(|r| r.starts_with(&marker)) {
            continue;
        }
        rows.push(format!("-「{name}」（第 {chapter} 章引用，磁盘上没有）"));
    }

    let mut out = String::from(
        "# 素材缺口表\n\n\
         由系统按章节自检维护：写完一章发现引用了磁盘上没有的素材就登记在这里。\n\
         处理方式：补素材 / 改剧情 / 明确接受这个素材没有（三选一，问用户）。\n\n",
    );
    out.push_str(&rows.join("\n"));
    out.push('\n');
    std::fs::write(path, out)
}

/// 已落盘章节引用的磁盘上没有的素材；与当前模式无关 —— 回答"换个模式会多出/少掉哪些要改的地方"。
pub(super) fn missing_assets_of_written(
    snap: &StageSnapshot,
    dir: &Path,
    data_dir: &Path,
) -> Vec<String> {
    let mut out = Vec::new();
    for id in &snap.written {
        let Some(file) = chapter_file(dir, id) else {
            continue;
        };
        let Ok(value) = crate::utils::yaml_file::read_yaml_as_json(&file) else {
            continue;
        };
        for d in validate::check_chapter_assets(data_dir, dir, id, &value) {
            if let Some(name) = validate::missing_asset_name(&d) {
                out.push(format!("{} {}", id, name));
            }
        }
    }
    out
}

fn chapter_shape_problems(value: &serde_json::Value) -> Vec<String> {
    let mut problems = Vec::new();

    if value
        .get("name")
        .and_then(|v| v.as_str())
        .unwrap_or("")
        .trim()
        .is_empty()
    {
        problems.push("缺少顶层 `name`".to_string());
    }

    let Some(events) = value.get("events").and_then(|v| v.as_array()) else {
        problems.push("缺少 `events` 列表".to_string());
        return problems;
    };
    let Some(last) = events.last() else {
        problems.push("`events` 是空的".to_string());
        return problems;
    };

    if last.get("type").and_then(|v| v.as_str()) != Some("chapter_end") {
        problems.push(format!(
            "最后一个事件是 `{}`，必须以 `chapter_end` 收尾",
            last.get("type")
                .and_then(|v| v.as_str())
                .unwrap_or("(缺失)")
        ));
        return problems;
    }

    let end_type = last
        .get("end_type")
        .and_then(|v| v.as_str())
        .unwrap_or("linear");
    if end_type == "linear"
        && last.get("next").and_then(|v| v.as_str()).is_none()
        && last.get("next_chapter").and_then(|v| v.as_str()).is_none()
    {
        problems.push(
            "`chapter_end` 是 linear 却没给 `next_chapter` / `next`（结尾写 \"end\"）".to_string(),
        );
    }

    problems
}
