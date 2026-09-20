//! 剧本创作阶段推导与按阶段上下文配置。
//!
//! 阶段完全由文件系统事实推导：不落库、不强制流程，只用于组装上下文与切换思考模式。

use std::path::{Path, PathBuf};

use crate::ai_service::types::LlmMessage;
use crate::utils::script_paths;

/// 设计稿在剧本包内的相对路径。
///
/// 放在点号目录下：剧本枚举（`enumerate_script_keys`）与章节遍历（`walk_chapters`）
/// 都会跳过点号开头的目录，因此不会干扰引擎、校验器与编辑器。
pub const DESIGN_REL_PATH: &str = ".agent/design.md";

// 相对技能目录（`data/game_data/skills`）的材料路径。
const HUB_DOC: &str = "lingchat-script-editor/SKILL.md";
const WRITER_DOC: &str = "script-writer/SKILL.md";
const TRANSFORMER_DOC: &str = "script-transformer/SKILL.md";
const OPTIMIZER_DOC: &str = "script-optimizer/SKILL.md";
const EVENT_REFERENCE: &str = "lingchat-script-editor/references/event-reference.md";
const CHAPTER_TEMPLATE: &str = "lingchat-script-editor/assets/templates/chapter_template.yaml";

/// 创作阶段。
///
/// 按「上下文配置是否相同」划分，不与产品流程的小节一一对应：
/// 类型选择 / 大纲 / 内容设计三节的配置一致，合并为 [`Stage::Setup`]。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Stage {
    /// 未绑定剧本。
    #[default]
    Routing,
    /// 无剧本包，或设计稿尚未列出章节。
    Setup,
    /// 设计稿已声明章节，但尚未写完。
    Forge,
    /// 计划章节齐备。
    Polish,
    /// 有剧本包、无设计稿、且已有章节：修改既有剧本。
    Modify,
}

impl Stage {
    /// 注入系统提示的单行标识。
    pub fn label(self) -> &'static str {
        match self {
            Stage::Routing => "未绑定剧本",
            Stage::Setup => "设计与大纲",
            Stage::Forge => "逐章编写",
            Stage::Polish => "校验与修复",
            Stage::Modify => "修改既有剧本",
        }
    }
}

/// 阶段推导结果，以及推导所依据的事实。
#[derive(Debug, Clone, Default)]
pub struct StageSnapshot {
    pub stage: Stage,
    /// 会话绑定的剧本 key（如 `standalone/我的剧本`、`character/角色/剧本`）。
    pub script_key: Option<String>,
    pub script_dir: Option<PathBuf>,
    /// 设计稿声明的章节 id。
    pub plan: Vec<String>,
    /// 磁盘上已有的章节 id。
    pub written: Vec<String>,
}

impl StageSnapshot {
    /// 下一个待写章节 id（仅 [`Stage::Forge`] 有意义）。
    pub fn next_chapter(&self) -> Option<&str> {
        self.plan
            .iter()
            .map(String::as_str)
            .find(|id| !self.written.iter().any(|w| w == id))
    }

    /// 设计稿中最后一个已落盘的章节 id。
    pub fn last_written(&self) -> Option<&str> {
        self.plan
            .iter()
            .map(String::as_str)
            .rfind(|id| self.written.iter().any(|w| w == id))
    }
}

/// 推导当前阶段。
pub fn derive(script_key: Option<&str>) -> StageSnapshot {
    let Some(key) = script_key else {
        return StageSnapshot::default();
    };
    let Ok(script_dir) = script_paths::resolve_script_dir(key) else {
        // 包还不存在 → 创建流程
        return StageSnapshot {
            stage: Stage::Setup,
            script_key: Some(key.to_string()),
            ..Default::default()
        };
    };

    let written = script_paths::enumerate_chapter_ids(&script_dir);
    let plan = read_plan(&script_dir);

    let stage = if plan.is_empty() {
        // 无设计稿：已有章节说明是既存剧本，否则还在设计与大纲阶段
        if written.is_empty() {
            Stage::Setup
        } else {
            Stage::Modify
        }
    } else if plan.iter().all(|id| written.iter().any(|w| w == id)) {
        Stage::Polish
    } else {
        Stage::Forge
    };

    StageSnapshot {
        stage,
        script_key: Some(key.to_string()),
        script_dir: Some(script_dir),
        plan,
        written,
    }
}

/// 一个阶段的上下文配置。
pub struct StageProfile {
    /// 思考模式覆盖；`None` 表示不干预，沿用 provider 默认。
    pub thinking: Option<bool>,
    /// 稳定材料：相对技能目录的路径，注入系统提示。
    pub system_materials: &'static [&'static str],
}

/// 取阶段对应的配置。
///
/// 凡是会落到剧本 YAML 的阶段（建工程 / 写章节 / 改章节 / 修章节）都带 `TRANSFORMER_DOC`
/// —— 它承载事件字段与 YAML 规范；缺了模型只能自行摸索，会去翻引擎源码或全盘搜索。
pub fn profile(stage: Stage) -> StageProfile {
    // 创作类阶段需要推理；机械落盘与按诊断码表修复不需要。
    match stage {
        Stage::Routing => StageProfile {
            thinking: None,
            system_materials: &[HUB_DOC],
        },
        Stage::Setup => StageProfile {
            thinking: Some(true),
            system_materials: &[HUB_DOC, WRITER_DOC, TRANSFORMER_DOC],
        },
        Stage::Forge => StageProfile {
            thinking: Some(false),
            system_materials: &[HUB_DOC, TRANSFORMER_DOC, EVENT_REFERENCE, CHAPTER_TEMPLATE],
        },
        Stage::Polish => StageProfile {
            thinking: Some(false),
            system_materials: &[HUB_DOC, TRANSFORMER_DOC, OPTIMIZER_DOC],
        },
        Stage::Modify => StageProfile {
            thinking: Some(true),
            system_materials: &[HUB_DOC, WRITER_DOC, TRANSFORMER_DOC, OPTIMIZER_DOC],
        },
    }
}

/// 读取并拼接稳定材料，作为可注入系统提示的块。
pub fn load_system_materials(skills_dir: &Path, stage: Stage) -> String {
    let mut out = String::new();
    for rel in profile(stage).system_materials {
        let path = skills_dir.join(rel);
        let Ok(text) = std::fs::read_to_string(&path) else {
            // 材料缺失会让模型失去依据转而自行摸索，必须显式暴露而不是静默跳过。
            tracing::warn!("[skill_agent] 阶段材料缺失: {}", path.display());
            continue;
        };
        out.push_str("\n\n===== ");
        out.push_str(rel);
        out.push_str(" =====\n");
        out.push_str(text.trim_end());
    }
    out
}

/// 组装本轮的动态材料。
///
/// 刻意不落库：每轮由 `run_chat` 重新拼装并追加在对话尾部，避免污染前缀缓存。
pub fn build_run_materials(snap: &StageSnapshot) -> String {
    let Some(dir) = snap.script_dir.as_deref() else {
        return String::new();
    };
    let design = std::fs::read_to_string(dir.join(DESIGN_REL_PATH)).unwrap_or_default();

    match snap.stage {
        Stage::Setup | Stage::Modify if !design.is_empty() => {
            format!("\n\n【现有设计稿】\n{}", design.trim_end())
        },
        Stage::Forge => {
            let mut out = String::new();
            if let Some(id) = snap.next_chapter() {
                if let Some(block) = extract_chapter_block(&design, id) {
                    out.push_str(&format!("\n\n【待写章节 · {}】\n{}", id, block));
                }
            }
            if let Some(prev) = snap.last_written() {
                let tail = tail_state(&read_chapter(dir, prev));
                if !tail.is_empty() {
                    out.push_str(&format!("\n\n【上一章（{}）收尾状态】\n{}", prev, tail));
                }
            }
            out
        },
        _ => String::new(),
    }
}

fn read_plan(script_dir: &Path) -> Vec<String> {
    std::fs::read_to_string(script_dir.join(DESIGN_REL_PATH))
        .map(|t| parse_plan(&t))
        .unwrap_or_default()
}

/// 解析设计稿的章节列表：每个 `##` 标题为一个块，块内**第一个非空行**为 `id:` 时
/// 才认作章节。把规则收紧到首行，避免大纲等正文里的 `id:` 被误判。
fn parse_plan(markdown: &str) -> Vec<String> {
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

/// 取出设计稿中 `id:` 等于 `id` 的块（含标题行，到下一个 `##` 为止）。
fn extract_chapter_block(markdown: &str, id: &str) -> Option<String> {
    let lines: Vec<&str> = markdown.lines().collect();
    for (start, end) in block_ranges(&lines) {
        if block_id(&lines[start + 1..end]) == Some(id) {
            return Some(lines[start..end].join("\n").trim_end().to_string());
        }
    }
    None
}

/// `##` 标题切出的块的行区间（区间从标题行开始，到下一个标题行之前）。
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

/// 块内第一个非空行若为 `id:` 行，返回其值。
fn block_id<'a>(block: &[&'a str]) -> Option<&'a str> {
    let first = block.iter().map(|l| l.trim()).find(|l| !l.is_empty())?;
    let rest = first.strip_prefix("id:")?;
    Some(rest.trim().trim_matches(&['"', '\''][..]).trim())
}

/// 取章节末尾的注释块，即 `SKILL.md`「状态注释原则」要求记录的收尾状态。
fn tail_state(chapter_text: &str) -> String {
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

fn read_chapter(script_dir: &Path, id: &str) -> String {
    std::fs::read_to_string(chapter_file(script_dir, id)).unwrap_or_default()
}

/// 章节 id → 磁盘文件（id 用 `/` 分隔，不含扩展名）。
fn chapter_file(script_dir: &Path, id: &str) -> PathBuf {
    let mut path = script_dir.join("Chapters");
    let mut segs = id.split('/').peekable();
    while let Some(seg) = segs.next() {
        if segs.peek().is_none() {
            path.push(format!("{seg}.yaml"));
        } else {
            path.push(seg);
        }
    }
    path
}

/// 丢弃已被超越的章节写入轮次，避免历史里堆积整章 YAML。
///
/// 判据：该轮的全部工具调用都是 `write_file` 写入「已落盘、且不是当前待写」的章节。
/// 整轮丢弃（含 tool 回应）而不是只丢回应，历史结构保持合法；只改内存中的历史，
/// DB 不动，所以回溯仍然完整。
pub fn compact_history(history: Vec<LlmMessage>, snap: &StageSnapshot) -> Vec<LlmMessage> {
    if snap.stage != Stage::Forge {
        return history;
    }
    let current = snap.next_chapter();
    let mut out = Vec::with_capacity(history.len());
    let mut i = 0;
    while i < history.len() {
        if history[i].role == "assistant" && history[i].tool_calls.is_some() {
            let mut j = i + 1;
            while j < history.len() && history[j].role == "tool" {
                j += 1;
            }
            if !is_superseded_chapter_write(&history[i], snap, current) {
                out.extend_from_slice(&history[i..j]);
            }
            i = j;
        } else {
            out.push(history[i].clone());
            i += 1;
        }
    }
    out
}

fn is_superseded_chapter_write(
    msg: &LlmMessage,
    snap: &StageSnapshot,
    current: Option<&str>,
) -> bool {
    let Some(calls) = msg.tool_calls.as_ref().filter(|c| !c.is_empty()) else {
        return false;
    };
    calls.iter().all(|tc| {
        written_chapter_id(&tc.function.name, &tc.function.arguments)
            .is_some_and(|id| Some(id.as_str()) != current && snap.written.iter().any(|w| w == &id))
    })
}

/// 从写入路径里取出章节 id（`…/Chapters/<id>.yaml`，允许 `\` 分隔与子目录）。
fn chapter_id_of_path(path: &str) -> Option<String> {
    let normalized = path.replace('\\', "/");
    let tail = normalized.split("/Chapters/").nth(1)?;
    let id = tail
        .strip_suffix(".yaml")
        .or_else(|| tail.strip_suffix(".yml"))?;
    (!id.is_empty()).then(|| id.to_string())
}

/// 若这是一次写章节文件的调用，返回章节 id。
fn written_chapter_id(tool: &str, arguments: &str) -> Option<String> {
    if tool != "write_file" {
        return None;
    }
    let args: serde_json::Value = serde_json::from_str(arguments).ok()?;
    chapter_id_of_path(args.get("path")?.as_str()?)
}

/// 章节写入后的轻量结构自检；不适用或无问题时返回 `None`。
///
/// 只查与本章自身有关、必然可判的硬性要求（`SKILL.md` 6.2），刻意**不做整剧本校验**：
/// 落盘阶段后续章节尚未写完，全量校验会报一堆指向未写章节的断链，反而误导模型去补。
pub fn check_written_chapter(snap: &StageSnapshot, path: &str) -> Option<String> {
    let dir = snap.script_dir.as_deref()?;
    let id = chapter_id_of_path(path)?;
    // 只认本会话绑定剧本自己的章节
    let key = snap.script_key.as_deref()?;
    if !path
        .replace('\\', "/")
        .contains(&format!("{key}/Chapters/"))
    {
        return None;
    }
    let value = match crate::utils::yaml_file::read_yaml_as_json(&chapter_file(dir, &id)) {
        Ok(v) => v,
        Err(e) => return Some(format!("`{}` 不是可解析的 YAML：{}", id, e)),
    };
    let problems = chapter_shape_problems(&value);
    (!problems.is_empty()).then(|| format!("`{}` 结构有问题：{}", id, problems.join("；")))
}

/// 章节的硬性结构要求。
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

    // 引擎优先 next，其次 next_chapter；end_type 缺省即 linear
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

#[cfg(test)]
mod tests {
    use super::*;

    const DESIGN: &str = "\
# 设计稿

## 大纲
一些说明。
id: 不是章节首行

## 第1章 · 雨夜
id: 01
梗概: 相遇

## 第2章 · 分别
id: Intro/02
梗概: 告别
";

    #[test]
    fn plan_reads_only_blocks_whose_first_line_is_id() {
        assert_eq!(parse_plan(DESIGN), vec!["01", "Intro/02"]);
    }

    #[test]
    fn plan_ignores_duplicates() {
        assert_eq!(parse_plan("## A\nid: 01\n## B\nid: 01\n"), vec!["01"]);
    }

    #[test]
    fn plan_skips_heading_without_id() {
        assert!(parse_plan("## 只有标题\n正文\n").is_empty());
    }

    #[test]
    fn extract_block_includes_heading_and_stops_at_next() {
        let block = extract_chapter_block(DESIGN, "01").unwrap();
        assert!(block.starts_with("## 第1章 · 雨夜"));
        assert!(block.contains("id: 01"));
        assert!(block.contains("梗概: 相遇"));
        assert!(!block.contains("告别"));
    }

    #[test]
    fn extract_block_handles_missing() {
        assert!(extract_chapter_block(DESIGN, "99").is_none());
    }

    #[test]
    fn tail_state_keeps_trailing_comments() {
        let text =
            "name: 第一章\nevents:\n  - type: chapter_end\n\n# 背景：教室\n# 音乐：quiet.mp3\n";
        assert_eq!(tail_state(text), "# 背景：教室\n# 音乐：quiet.mp3");
    }

    #[test]
    fn tail_state_empty_without_comments() {
        assert_eq!(tail_state("name: x\nevents: []\n"), "");
    }

    #[test]
    fn chapter_file_supports_subdirs() {
        let dir = Path::new("/pkg");
        assert_eq!(chapter_file(dir, "01"), Path::new("/pkg/Chapters/01.yaml"));
        assert_eq!(
            chapter_file(dir, "Intro/intro"),
            Path::new("/pkg/Chapters/Intro/intro.yaml")
        );
    }

    #[test]
    fn thinking_off_for_mechanical_stages() {
        assert_eq!(profile(Stage::Forge).thinking, Some(false));
        assert_eq!(profile(Stage::Polish).thinking, Some(false));
        assert_eq!(profile(Stage::Setup).thinking, Some(true));
    }

    /// 会落到剧本 YAML 的阶段必须带落盘规范，否则模型会自行摸索、去翻源码或全盘搜索。
    #[test]
    fn stages_that_touch_yaml_include_transformer_doc() {
        for stage in [Stage::Setup, Stage::Forge, Stage::Polish, Stage::Modify] {
            assert!(
                profile(stage)
                    .system_materials
                    .iter()
                    .any(|m| m.starts_with("script-transformer/")),
                "{stage:?} 会写/改剧本 YAML，材料里必须含 script-transformer"
            );
        }
    }

    /// 阶段材料路径必须是「技能目录/…」形式，不能是靠 base dir 解析的裸相对路径。
    #[test]
    fn stage_materials_are_skill_relative() {
        for stage in [
            Stage::Routing,
            Stage::Setup,
            Stage::Forge,
            Stage::Polish,
            Stage::Modify,
        ] {
            for m in profile(stage).system_materials {
                assert!(
                    !m.starts_with("references/") && !m.starts_with("assets/"),
                    "{stage:?} 的材料 `{m}` 是裸相对路径，预注入后无法解析"
                );
            }
        }
    }

    #[test]
    fn derive_without_key_is_routing() {
        assert_eq!(derive(None).stage, Stage::Routing);
    }

    /// 造一轮「模型调 write_file 并拿到结果」的历史。
    fn write_round(call_id: &str, path: &str, tool: &str) -> Vec<LlmMessage> {
        let args = format!(r#"{{"path":"{}"}}"#, path.replace('\\', "\\\\"));
        let mut assistant = LlmMessage::assistant("我来写。");
        assistant.tool_calls = Some(vec![crate::ai_service::types::ToolCall {
            id: call_id.into(),
            type_: "function".into(),
            function: crate::ai_service::types::FunctionCall {
                name: tool.into(),
                arguments: args,
            },
        }]);
        vec![assistant, LlmMessage::tool_result(call_id, "已写入")]
    }

    fn forge(plan: &[&str], written: &[&str]) -> StageSnapshot {
        StageSnapshot {
            stage: Stage::Forge,
            script_key: None,
            script_dir: None,
            plan: plan.iter().map(|s| s.to_string()).collect(),
            written: written.iter().map(|s| s.to_string()).collect(),
        }
    }

    #[test]
    fn written_chapter_id_parses_path() {
        assert_eq!(
            written_chapter_id("write_file", r#"{"path":"/p/Chapters/01.yaml"}"#).as_deref(),
            Some("01")
        );
        assert_eq!(
            written_chapter_id("write_file", r#"{"path":"p\\Chapters\\Intro\\a.yaml"}"#).as_deref(),
            Some("Intro/a")
        );
        // 非章节文件 / 非写入工具 → 不认
        assert_eq!(
            written_chapter_id("write_file", r#"{"path":"/p/story_config.yaml"}"#),
            None
        );
        assert_eq!(
            written_chapter_id("read_file", r#"{"path":"/p/Chapters/01.yaml"}"#),
            None
        );
    }

    #[test]
    fn compact_drops_superseded_chapter_rounds() {
        let mut history = write_round("c1", "/p/Chapters/01.yaml", "write_file");
        history.push(LlmMessage::user("继续下一章"));
        // 01 已落盘、当前待写 02 → 01 那轮被压掉
        let out = compact_history(history.clone(), &forge(&["01", "02"], &["01"]));
        assert_eq!(out.len(), 1);
        assert_eq!(out[0].role, "user");
        // 当前待写就是 01 时，01 那轮保留
        let out = compact_history(history, &forge(&["01", "02"], &[]));
        assert_eq!(out.len(), 3);
    }

    #[test]
    fn compact_keeps_non_chapter_writes() {
        let mut history = write_round("c1", "/p/story_config.yaml", "write_file");
        history.push(LlmMessage::user("改配置"));
        let out = compact_history(history, &forge(&["01"], &["01"]));
        assert_eq!(out.len(), 3);
    }

    #[test]
    fn compact_is_noop_outside_forge() {
        let history = write_round("c1", "/p/Chapters/01.yaml", "write_file");
        let mut snap = forge(&["01", "02"], &["01"]);
        snap.stage = Stage::Polish;
        assert_eq!(compact_history(history, &snap).len(), 2);
    }

    #[test]
    fn chapter_id_of_path_handles_variants() {
        assert_eq!(
            chapter_id_of_path("/p/Chapters/01.yaml").as_deref(),
            Some("01")
        );
        assert_eq!(
            chapter_id_of_path("p\\Chapters\\Intro\\a.yml").as_deref(),
            Some("Intro/a")
        );
        assert_eq!(chapter_id_of_path("/p/story_config.yaml"), None);
        assert_eq!(chapter_id_of_path("/p/Chapters/01.txt"), None);
    }

    #[test]
    fn shape_accepts_minimal_valid_chapter() {
        let ok = serde_json::json!({
            "name": "雨夜",
            "events": [
                {"type": "narration", "text": "…"},
                {"type": "chapter_end", "next": "end"},
            ],
        });
        assert!(chapter_shape_problems(&ok).is_empty());
    }

    #[test]
    fn shape_flags_missing_name_or_events() {
        let no_name = serde_json::json!({"events": [{"type": "chapter_end", "next": "end"}]});
        assert!(
            chapter_shape_problems(&no_name)
                .iter()
                .any(|m| m.contains("name"))
        );

        let no_events = serde_json::json!({"name": "x"});
        assert!(
            chapter_shape_problems(&no_events)
                .iter()
                .any(|m| m.contains("events"))
        );

        let empty = serde_json::json!({"name": "x", "events": []});
        assert!(
            chapter_shape_problems(&empty)
                .iter()
                .any(|m| m.contains("空"))
        );
    }

    #[test]
    fn shape_requires_chapter_end_last() {
        let p = chapter_shape_problems(&serde_json::json!({
            "name": "x",
            "events": [
                {"type": "chapter_end", "next": "end"},
                {"type": "narration", "text": "还在说"},
            ],
        }));
        assert!(p.iter().any(|m| m.contains("chapter_end")));
    }

    #[test]
    fn shape_requires_next_only_for_linear() {
        let linear = serde_json::json!({"name": "x", "events": [{"type": "chapter_end"}]});
        assert!(
            chapter_shape_problems(&linear)
                .iter()
                .any(|m| m.contains("linear"))
        );

        // end_type 缺省即 linear；给了 next 就通过
        let with_next =
            serde_json::json!({"name": "x", "events": [{"type": "chapter_end", "next": "end"}]});
        assert!(chapter_shape_problems(&with_next).is_empty());

        // branching 不要求 next
        let branching = serde_json::json!({
            "name": "x",
            "events": [{"type": "chapter_end", "end_type": "branching"}],
        });
        assert!(chapter_shape_problems(&branching).is_empty());
    }

    #[test]
    fn check_written_chapter_ignores_other_scripts() {
        let snap = StageSnapshot {
            script_dir: Some(PathBuf::from("/p")),
            script_key: Some("standalone/A".into()),
            ..Default::default()
        };
        assert!(check_written_chapter(&snap, "/p/standalone/B/Chapters/01.yaml").is_none());
        // 未绑定剧本时不检查
        assert!(check_written_chapter(&StageSnapshot::default(), "/p/Chapters/01.yaml").is_none());
    }
}
