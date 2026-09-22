//! 阶段推导与按阶段上下文配置。
//!
//! 阶段由文件系统事实推导，不落库、不强制流程。
//! 本模块只装配上下文：运行中唯一阻塞等待用户的确认是命令审批，它由工具入口触发，
//! 与阶段无关；这里的「确认门」全部只是提示词。

use std::path::{Path, PathBuf};

use crate::ai_service::types::LlmMessage;
use crate::utils::script_paths;

/// 设计稿在剧本包内的相对路径。点号目录不会被引擎扫描，也不进编辑器枚举。
pub const DESIGN_REL_PATH: &str = ".agent/design.md";

// 相对技能目录（`data/game_data/skills`）的材料路径。
const HUB_DOC: &str = "lingchat-script-editor/SKILL.md";
const WRITER_DOC: &str = "script-writer/SKILL.md";
const TRANSFORMER_DOC: &str = "script-transformer/SKILL.md";
const OPTIMIZER_DOC: &str = "script-optimizer/SKILL.md";
const EVENT_REFERENCE: &str = "lingchat-script-editor/references/event-reference.md";
const CHAPTER_TEMPLATE: &str = "lingchat-script-editor/assets/templates/chapter_template.yaml";

/// 创作阶段。按「上下文配置是否相同」划分，S1–S3 合并为 [`Stage::Setup`]。
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
    /// 本阶段的行为要求（粒度等），注入在材料之前。
    pub directive: &'static str,
}

// 各阶段的行为要求。
const DIRECTIVE_SETUP: &str = "本阶段是设计与大纲：产出设计稿（大纲 + 章节设计 + 素材诉求）。\
    \n- 设计稿落地后**停下等用户确认**，不要接着开始写章节";

const DIRECTIVE_FORGE: &str = "本阶段是逐章编写，**粒度是「一轮一章」**：\
    \n- 一轮只写一章，写完立即停下等用户确认；不要在一条消息里并行写多章\
    \n- 也不要写完一章后不等确认就接着写下一章\
    \n- 通用规则里的「任务必须完成到产出物为止」指的是**当前这一章的产出物**，不是整部剧本\
    \n- **不要调用 `validate_script`**：全剧没写完时它必然报 `chapter_end.dangling`、\
     `graph.unreachable`、`variable.never_read` 等一批「尚未写完」的假错，\
     会把你引向提前补写后续章节；校验留到交付阶段";

const DIRECTIVE_POLISH: &str = "本阶段是校验与修复：\
    \n- 按诊断逐条修复；需要**新编剧情内容**才能补上的缺口交回用户，不要自行编造\
    \n- 修复完成、校验通过后停下汇报";

const DIRECTIVE_MODIFY: &str = "本阶段是修改既有剧本：\
    \n- 按用户提出的修改需求**逐项**修改，改动完成后停下等用户确认\
    \n- **不要自行扩大改动范围**，也不要顺手重写未被要求改动的章节";

/// 取阶段配置。会落到剧本 YAML 的阶段都带 `TRANSFORMER_DOC`（事件字段与 YAML 规范）。
pub fn profile(stage: Stage) -> StageProfile {
    // 创作类阶段需要推理；机械落盘与按诊断码表修复不需要。
    match stage {
        // 未绑定剧本时做的正是 hub 的 0/1/2 阶段（入口判断 + 类型 + 大纲），归属 writer
        Stage::Routing => StageProfile {
            thinking: None,
            system_materials: &[HUB_DOC, WRITER_DOC],
            directive: "",
        },
        Stage::Setup => StageProfile {
            thinking: Some(true),
            system_materials: &[HUB_DOC, WRITER_DOC, TRANSFORMER_DOC],
            directive: DIRECTIVE_SETUP,
        },
        Stage::Forge => StageProfile {
            thinking: Some(false),
            system_materials: &[HUB_DOC, TRANSFORMER_DOC, EVENT_REFERENCE, CHAPTER_TEMPLATE],
            directive: DIRECTIVE_FORGE,
        },
        Stage::Polish => StageProfile {
            thinking: Some(false),
            system_materials: &[HUB_DOC, TRANSFORMER_DOC, OPTIMIZER_DOC],
            directive: DIRECTIVE_POLISH,
        },
        Stage::Modify => StageProfile {
            thinking: Some(true),
            system_materials: &[HUB_DOC, WRITER_DOC, TRANSFORMER_DOC, OPTIMIZER_DOC],
            directive: DIRECTIVE_MODIFY,
        },
    }
}

/// 渲染阶段块（阶段名 + 行为要求 + 角色指令），拼在系统提示末尾以保前缀缓存。
pub fn build_stage_block(skills_dir: &Path, stage: Stage) -> String {
    let profile = profile(stage);
    let mut out = format!("\n\n【当前阶段】{}", stage.label());
    if !profile.directive.is_empty() {
        out.push('\n');
        out.push_str(profile.directive);
    }
    out.push_str(
        "\n\n以下技能文档是本阶段**必须遵守的角色指令**，不是参考资料。\
         \n已注入的文档无需重复调用 read_skill；文档里引用到的其他角色技能，\
         需要时仍用 read_skill 加载。",
    );
    out.push_str(&load_system_materials(skills_dir, stage));
    out
}

/// 读取阶段材料，冠以「角色指令」来源头（区别于看起来像附件的文件转储）。
fn load_system_materials(skills_dir: &Path, stage: Stage) -> String {
    let mut out = String::new();
    for rel in profile(stage).system_materials {
        let path = skills_dir.join(rel);
        match std::fs::read_to_string(&path) {
            Ok(text) => {
                out.push_str(&format!("\n\n【角色指令 · {}】\n", rel));
                out.push_str(text.trim_end());
            },
            // 缺失必须让模型看见：只打日志的话，它会凭记忆补规范且无人知情
            Err(_) => {
                tracing::warn!("[skill_agent] 阶段材料缺失: {}", path.display());
                out.push_str(&format!(
                    "\n\n【角色指令缺失 · {rel}】\n本文件读取失败。其中的规范不得凭记忆代替，\
                     也不要静默继续；先告知用户技能文件缺失（可能需要在数据同步里重新勾选）。\n"
                ));
            },
        }
    }
    out
}

/// 组装本轮动态材料（待写章节 + 上一章收尾状态）；不落库，每轮重算。
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

/// 解析设计稿的章节列表：`##` 块内第一个非空行为 `id:` 才算章节。
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
    chapter_file(script_dir, id)
        .and_then(|p| std::fs::read_to_string(p).ok())
        .unwrap_or_default()
}

/// 章节 id → 磁盘文件。布局与合法性交给 `script_paths`，避免两处各写一遍规则。
fn chapter_file(script_dir: &Path, id: &str) -> Option<PathBuf> {
    script_paths::resolve_chapter_file(script_dir, id, false).ok()
}

/// 丢弃已被超越的章节写入轮次，避免历史里堆积整章 YAML。
///
/// 整轮丢弃（含 tool 回应）以保持历史结构合法；只改内存，DB 不动。
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

/// 从写入路径反推剧本 key：`…/scripts/<key…>/story_config.yaml`。
pub fn script_key_of_story_config(path: &str) -> Option<String> {
    let normalized = path.replace('\\', "/");
    let (_, tail) = normalized.rsplit_once("/scripts/")?;
    let key = tail.strip_suffix("/story_config.yaml")?.trim_matches('/');
    (!key.is_empty()).then(|| key.to_string())
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
/// 不做整剧本校验：未写完时的断链诊断会误导模型去补后续章节。
pub fn check_written_chapter(snap: &StageSnapshot, path: &str) -> Option<String> {
    let dir = snap.script_dir.as_deref()?;
    let id = chapter_id_of_path(path)?;
    // 只认本会话绑定剧本自己的章节。写的是相对还是绝对路径不一定，所以用包含判断；
    // 大小写无关是因为 Windows 上实际目录名可能与绑定值不同。
    let normalized = path.replace('\\', "/").to_lowercase();
    let key = snap.script_key.as_deref()?;
    if !normalized.contains(&format!("{}/chapters/", key.to_lowercase())) {
        return None;
    }
    let file = chapter_file(dir, &id)?;
    let value = match crate::utils::yaml_file::read_yaml_as_json(&file) {
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
        assert_eq!(
            chapter_file(dir, "01").as_deref(),
            Some(Path::new("/pkg/Chapters/01.yaml"))
        );
        assert_eq!(
            chapter_file(dir, "Intro/intro").as_deref(),
            Some(Path::new("/pkg/Chapters/Intro/intro.yaml"))
        );
    }

    #[test]
    fn chapter_file_rejects_unsafe_ids() {
        let dir = Path::new("/pkg");
        assert_eq!(chapter_file(dir, "../story_config"), None);
        assert_eq!(chapter_file(dir, "end"), None);
    }

    #[test]
    fn thinking_off_for_mechanical_stages() {
        assert_eq!(profile(Stage::Forge).thinking, Some(false));
        assert_eq!(profile(Stage::Polish).thinking, Some(false));
        assert_eq!(profile(Stage::Setup).thinking, Some(true));
    }

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

    #[test]
    fn forge_directive_states_one_chapter_per_turn() {
        let d = profile(Stage::Forge).directive;
        assert!(d.contains("一轮只写一章"), "Forge 必须写明粒度：{d}");
        assert!(d.contains("多章"), "Forge 必须禁止多章：{d}");
    }

    #[test]
    fn forge_directive_discourages_whole_script_validation() {
        let d = profile(Stage::Forge).directive;
        assert!(
            d.contains("validate_script"),
            "Forge 必须提醒别调 validate_script：{d}"
        );
        assert!(
            d.contains("never_read") || d.contains("假错"),
            "应说清会报什么错：{d}"
        );
    }

    #[test]
    fn non_routing_stages_all_have_directives() {
        for stage in [Stage::Setup, Stage::Forge, Stage::Polish, Stage::Modify] {
            assert!(
                !profile(stage).directive.is_empty(),
                "{stage:?} 缺少行为要求"
            );
        }
        assert!(profile(Stage::Routing).directive.is_empty());
    }

    #[test]
    fn stage_block_marks_materials_as_directives() {
        let block = build_stage_block(Path::new("/nonexistent-skills"), Stage::Forge);
        assert!(block.contains("【当前阶段】"));
        assert!(block.contains("必须遵守的角色指令"));
        assert!(block.contains("一轮只写一章"));
        // 材料读不到时必须写进提示词，否则模型会凭记忆补规范
        assert!(block.contains("角色指令缺失"), "{block}");
    }

    #[test]
    fn routing_stage_includes_writer_doc() {
        assert!(
            profile(Stage::Routing)
                .system_materials
                .iter()
                .any(|m| m.starts_with("script-writer/")),
            "未绑定剧本时做的是 0/1/2 阶段，必须给 writer"
        );
    }

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

    /// 造一轮 write_file 的工具轮次。
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

    #[test]
    fn script_key_from_story_config_path() {
        assert_eq!(
            script_key_of_story_config(
                "game_data/scripts/character/DeepSeek/雨夜爆种/story_config.yaml"
            )
            .as_deref(),
            Some("character/DeepSeek/雨夜爆种")
        );
        // Windows 分隔符
        assert_eq!(
            script_key_of_story_config(
                r"D:\d\data\game_data\scripts\standalone\我的剧本\story_config.yaml"
            )
            .as_deref(),
            Some("standalone/我的剧本")
        );
        // flat 布局
        assert_eq!(
            script_key_of_story_config("game_data/scripts/我的剧本/story_config.yaml").as_deref(),
            Some("我的剧本")
        );
        // 章节文件 / .bak / 不在 scripts 下 → 一律不认
        assert_eq!(
            script_key_of_story_config("game_data/scripts/x/Chapters/01.yaml"),
            None
        );
        assert_eq!(
            script_key_of_story_config("game_data/scripts/x/story_config.yaml.bak"),
            None
        );
        assert_eq!(
            script_key_of_story_config("data/skills/foo/story_config.yaml"),
            None
        );
    }
}
