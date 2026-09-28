//! 阶段推导与按阶段上下文配置。
//!
//! 阶段由文件系统事实推导，不落库、不强制流程。
//! 本模块只装配上下文：运行中唯一阻塞等待用户的确认是命令审批，它由工具入口触发，
//! 与阶段无关；这里的「确认门」全部只是提示词。

use std::path::{Path, PathBuf};

use crate::ai_service::types::LlmMessage;
use crate::api::script_editor::validate;
use crate::utils::script_paths;

/// 流程产物所在目录。点号目录不会被引擎扫描，也不进编辑器枚举；
/// 助手面板的「详情」浮窗直接列这一层，所以加新产物不用改前端。
pub const AGENT_DIR: &str = ".agent";

/// 设计稿在剧本包内的相对路径。
pub const DESIGN_REL_PATH: &str = ".agent/design.md";

/// 用户约束卡片。素材模式等"问过用户才知道"的事实记在这里。
pub const CONSTRAINTS_REL_PATH: &str = ".agent/constraints.md";

/// 任务队列。这一版只读不写：写它的是流程 Agent（尚未实现）。
pub const QUEUE_REL_PATH: &str = ".agent/queue.md";

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

/// 组装本轮动态材料（待写章节 + 上一章收尾状态 + 落盘进度）；不落库，每轮重算。
pub fn build_run_materials(snap: &StageSnapshot) -> String {
    let Some(dir) = snap.script_dir.as_deref() else {
        return String::new();
    };
    let design = std::fs::read_to_string(dir.join(DESIGN_REL_PATH)).unwrap_or_default();

    let mut out = match snap.stage {
        Stage::Setup | Stage::Modify if !design.is_empty() => {
            format!("\n\n【现有设计稿】\n{}", design.trim_end())
        },
        Stage::Forge => {
            let mut forge = String::new();
            if let Some(id) = snap.next_chapter() {
                if let Some(block) = extract_chapter_block(&design, id) {
                    forge.push_str(&format!("\n\n【待写章节 · {}】\n{}", id, block));
                }
            }
            if let Some(prev) = snap.last_written() {
                let tail = tail_state(&read_chapter(dir, prev));
                if !tail.is_empty() {
                    forge.push_str(&format!("\n\n【上一章（{}）收尾状态】\n{}", prev, tail));
                }
            }
            forge
        },
        _ => String::new(),
    };
    out.push_str(&progress_block(snap, dir, &crate::api::data_dir()));
    out
}

/// 交接单里的进度事实：素材模式 + 已落盘清单 + 素材缺口影响面 + 下一章是否已有内容 + 队列剩余。
///
/// 「下一章是否已有内容」补的是删章那一脚：阶段按文件事实推，代码只能说"还没写"，
/// 而磁盘上那一章可能真有内容，模型得知道写下去会覆盖什么。
/// `data_dir` 由调用方给（判素材要用它），这样这条逻辑离开全局静态也能测。
fn progress_block(snap: &StageSnapshot, dir: &Path, data_dir: &Path) -> String {
    let mode = read_asset_mode(dir);
    let mut lines: Vec<String> = vec![format!(
        "素材模式：{}（随时可改，说一句就行）",
        mode.describe()
    )];

    lines.push(format!(
        "已落盘：{}",
        if snap.written.is_empty() {
            "（无）".to_string()
        } else {
            snap.written.join(" ")
        }
    ));

    // 换模式的影响面：已落盘的章节不会跟着重写，所以先把要改的地方摊开
    if !snap.written.is_empty() {
        let missing = missing_assets_of_written(snap, dir, data_dir);
        if !missing.is_empty() {
            lines.push(asset_impact_line(mode, &missing));
        }
    }

    if let Some(after) = chapter_after(snap) {
        let exists = snap.written.iter().any(|w| w == &after);
        lines.push(format!(
            "下一章 {}：{}",
            after,
            if exists {
                "磁盘上已有内容（本轮改的是它前面，别顺手覆盖它）"
            } else {
                "尚未落盘"
            }
        ));
    }
    if let Some(queue) = read_queue_progress(dir) {
        lines.push(format!("队列：{}", queue));
    }

    let mut out = String::from("\n\n【落盘进度】\n");
    for line in lines {
        out.push_str(&format!("- {}\n", line));
    }
    out
}

/// 已落盘的素材缺口在**当前模式下**意味着什么 —— 用户改主意时的影响面。
///
/// 这一行是给"改主意"准备的：换了模式，已经写好的章节不会自动重写，
/// 所以得先把"会多出／少掉哪些要改的地方"摊开。
fn asset_impact_line(mode: AssetMode, missing: &[String]) -> String {
    /// 列全没意义，模型和用户都只需要知道大概规模与前几个。
    const SHOW: usize = 4;

    let mut list = missing
        .iter()
        .take(SHOW)
        .cloned()
        .collect::<Vec<_>>()
        .join("、");
    if missing.len() > SHOW {
        list.push_str(&format!("…（共 {} 处）", missing.len()));
    }
    let n = missing.len();
    match mode {
        AssetMode::OnlyExisting => format!(
            "按「只用已有」有 {} 处要改：{}。若这些其实打算之后补素材，\
             说一句「素材我之后补，先留位置」就能改成「先预留」",
            n, list
        ),
        AssetMode::Reserve => format!(
            "已留空 {} 处：{}。逐条补 / 改 / 接受看 .agent/assets.md；\
             改成「只用已有」它们会变成必须修的错",
            n, list
        ),
        AssetMode::Unspecified => format!(
            "已落盘章节里有 {} 处引用了磁盘上没有的素材：{}。先问用户要「只用已有」还是「先预留」",
            n, list
        ),
    }
}

/// 设计稿里排在「本章」之后的那一章。
fn chapter_after(snap: &StageSnapshot) -> Option<String> {
    let current = snap.next_chapter()?;
    let idx = snap.plan.iter().position(|p| p == current)?;
    snap.plan.get(idx + 1).cloned()
}

/// 队列进度（`.agent/queue.md` 的 `- [ ]` / `- [x]`）。文件不存在返回 `None`。
fn read_queue_progress(dir: &Path) -> Option<String> {
    let text = std::fs::read_to_string(dir.join(QUEUE_REL_PATH)).ok()?;
    let (mut done, mut pending) = (0usize, Vec::new());
    for line in text.lines() {
        let t = line.trim_start();
        if let Some(rest) = t.strip_prefix("- [x]").or_else(|| t.strip_prefix("- [X]")) {
            if !rest.trim().is_empty() {
                done += 1;
            }
        } else if let Some(rest) = t.strip_prefix("- [ ]") {
            if !rest.trim().is_empty() {
                pending.push(rest.trim().to_string());
            }
        }
    }
    let total = done + pending.len();
    if total == 0 {
        return None;
    }
    let mut out = format!("共 {} 项，已完成 {}，还剩 {}", total, done, pending.len());
    if let Some(next) = pending.first() {
        out.push_str(&format!("（下一项：{}）", next));
    }
    Some(out)
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

/// 从剧本包内任意写入路径反推 key（`…/scripts/<key…>/…`）。
///
/// key 的层级不固定（`standalone/x`、`character/角色/x`、扁平 `x`），所以拿已知的剧本包列表
/// 去匹配，而不是猜切几段；嵌套包时优先最长匹配。`keys` 由调用方传入，便于一次扫描多处复用。
pub(crate) fn script_key_of_script_path(path: &str, keys: &[String]) -> Option<String> {
    let normalized = path.replace('\\', "/");
    let mut keys: Vec<&String> = keys.iter().collect();
    keys.sort_by_key(|k| std::cmp::Reverse(k.len()));
    keys.into_iter()
        .find(|k| normalized.contains(&format!("/scripts/{}/", k.trim_matches('/'))))
        .cloned()
}

/// 若这是一次写章节文件的调用，返回章节 id。
fn written_chapter_id(tool: &str, arguments: &str) -> Option<String> {
    if tool != "write_file" {
        return None;
    }
    let args: serde_json::Value = serde_json::from_str(arguments).ok()?;
    chapter_id_of_path(args.get("path")?.as_str()?)
}

/// 素材从哪来。动笔前必须问用户，它决定"引用了不存在的素材"算不算错。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum AssetMode {
    /// 没问过 / 卡片里没写或写得不认识。
    #[default]
    Unspecified,
    /// 只用磁盘上已有的 —— 写了不存在的名字就是错，必须当场换掉。
    OnlyExisting,
    /// 之后补素材，先留位置 —— 缺失只登记，不阻断。
    Reserve,
}

impl AssetMode {
    /// 交接单里那一行：既说模式，也说缺素材在这一模式下算不算错。
    pub fn describe(self) -> &'static str {
        match self {
            AssetMode::Unspecified => "未声明（先问用户：只用已有 / 先预留）",
            AssetMode::OnlyExisting => "只用已有（引用不存在的素材 = 错误，当场改）",
            AssetMode::Reserve => "先预留（缺素材只登记进 .agent/assets.md）",
        }
    }

    /// 未声明时不拦人：没问过就按警告处理，同时催去问。
    fn missing_is_error(self) -> bool {
        self == AssetMode::OnlyExisting
    }

    /// 怎么换一个模式。
    ///
    /// 用户决定没有一次性的：改主意不是"打脸"，是常态，所以要把改法随手说出来，
    /// 而不是让他自己想怎么改。
    pub fn switch_hint(self) -> &'static str {
        match self {
            AssetMode::Unspecified => {
                "素材模式还没定：问用户要「只用已有」还是「先预留」，\
                 写进 .agent/constraints.md（`- 素材模式：只用已有`）。\
                 定下来之后想换，说一句就行。\n"
            },
            AssetMode::OnlyExisting => {
                "素材模式是「只用已有」。若其实打算之后补素材，\
                 说一句「素材我之后补，先留位置」，\
                 我就把 .agent/constraints.md 改成「先预留」，这些就不再拦；已落盘的章节不用重写。\n"
            },
            AssetMode::Reserve => {
                "素材模式是「先预留」。若想改成「只用已有」，说一句就行；\
                 改成后已落盘章节里留空的素材会变成必须修的错。\n"
            },
        }
    }
}

/// 从约束卡片里读素材模式，格式 `- 素材模式：只用已有`（半角全角冒号都认）。
///
/// 只在「仅用已有」「之后补充」两个同义写法上放宽；其余一律当未声明 ——
/// 认错的代价是校验松紧反了，宁可多问一次也不要猜。
pub fn parse_asset_mode(text: &str) -> AssetMode {
    for line in text.lines() {
        let head = line.trim_start_matches(['-', '*', ' ', '\t']);
        let Some(rest) = head.strip_prefix("素材模式") else {
            continue;
        };
        let value = rest.trim_start_matches([':', '：']).trim();
        return match value {
            "只用已有" | "仅用已有" => AssetMode::OnlyExisting,
            "先预留" | "之后补充" => AssetMode::Reserve,
            _ => AssetMode::Unspecified,
        };
    }
    AssetMode::Unspecified
}

fn read_asset_mode(script_dir: &Path) -> AssetMode {
    std::fs::read_to_string(script_dir.join(CONSTRAINTS_REL_PATH))
        .map(|t| parse_asset_mode(&t))
        .unwrap_or_default()
}

/// 一章的自检结果：错误必须当场改完，警告只记录。
#[derive(Debug, Default, PartialEq, Eq)]
pub struct ChapterCheck {
    pub errors: Vec<String>,
    pub warnings: Vec<String>,
    /// 本章有素材缺口（先预留模式下只登记，不阻断）。
    pub assets_missing: bool,
    /// 本轮生效的素材模式；决定回执里怎么告诉用户"还能改"。
    pub mode: AssetMode,
}

impl ChapterCheck {
    pub fn is_empty(&self) -> bool {
        self.errors.is_empty() && self.warnings.is_empty()
    }

    /// 自检回执，按「能不能就此收工」组织。
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
        if self.assets_missing {
            out.push_str("缺的素材登记进 .agent/assets.md，别等交付才发现。\n");
            // 卡在这里的其实是用户当初选的那个模式，所以把换法的句子一并给出
            out.push_str(self.mode.switch_hint());
        }
        out
    }
}

/// 章节写入后的轻量自检；不适用或无问题时返回 `None`。
///
/// 不做整剧本校验：未写完时的断链诊断会误导模型去补后续章节。
/// 但素材是「写完这一章就能定论」的事实，所以这里当场查。
pub fn check_written_chapter(snap: &StageSnapshot, path: &str) -> Option<ChapterCheck> {
    let (dir, id) = chapter_of_write(snap, path)?;
    check_chapter(dir, &id, &crate::api::data_dir())
}

/// 认出「刚写的是本会话这个剧本的哪一章」；认不出就不产生自检。
///
/// 数据目录留到确认是自己剧本的章节之后才取 —— 它跟这一步无关，
/// 提前取会在没有数据目录的场合（测试）无谓地炸掉。
fn chapter_of_write<'a>(snap: &'a StageSnapshot, path: &str) -> Option<(&'a Path, String)> {
    let dir = snap.script_dir.as_deref()?;
    let id = chapter_id_of_path(path)?;
    // 只认本会话绑定剧本自己的章节。写的是相对还是绝对路径不一定，所以用包含判断；
    // 大小写无关是因为 Windows 上实际目录名可能与绑定值不同。
    let normalized = path.replace('\\', "/").to_lowercase();
    let key = snap.script_key.as_deref()?;
    if !normalized.contains(&format!("{}/chapters/", key.to_lowercase())) {
        return None;
    }
    Some((dir, id))
}

/// 判定本体：结构 + 素材；素材的松紧由 `.agent/constraints.md` 里的素材模式决定。
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

    let mut check = ChapterCheck {
        errors: chapter_shape_problems(&value),
        mode: read_asset_mode(dir),
        ..Default::default()
    };

    let findings = validate::check_chapter_assets(data_dir, dir, id, &value);
    for d in findings {
        check.assets_missing = true;
        let at = d.event_index.map(|i| i + 1).unwrap_or_default();
        let line = format!("第 {} 个事件 · {}", at, d.message);
        if check.mode.missing_is_error() {
            check.errors.push(line);
        } else {
            check.warnings.push(line);
        }
    }
    if check.assets_missing && check.mode == AssetMode::Unspecified {
        check.warnings.push(format!(
            "`素材模式`还没声明：这一轮先问用户「只用已有」还是「先预留」，\
             写进 {}（格式 `- 素材模式：只用已有`）。\
             未声明时缺失只按警告算，但这正是用户最在意的那类错。",
            CONSTRAINTS_REL_PATH
        ));
    }

    (!check.is_empty()).then_some(check)
}

/// 已落盘章节里引用了磁盘上不存在的素材，返回 `章节 id + 素材名`。
///
/// 与当前模式无关 —— 它回答的是「换个模式会多出/少掉哪些要改的地方」，
/// 也就是用户改主意时的影响面。改了模式，已写好的章节不会自动重写，得摊开给他看。
fn missing_assets_of_written(snap: &StageSnapshot, dir: &Path, data_dir: &Path) -> Vec<String> {
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
    fn script_key_from_any_package_path() {
        let keys = vec![
            "standalone/我的剧本".to_string(),
            "character/风雪/高塔逆位".to_string(),
            "character/风雪/高塔逆位/嵌套".to_string(),
        ];
        let hit = |p: &str| script_key_of_script_path(p, &keys);
        // 章节 / 设计稿 / 配置，以及 Windows 反斜杠路径都能反推
        assert_eq!(
            hit("D:/d/data/game_data/scripts/character/风雪/高塔逆位/Chapters/01.yaml").as_deref(),
            Some("character/风雪/高塔逆位")
        );
        assert_eq!(
            hit(r"data\game_data\scripts\standalone\我的剧本\.agent\design.md").as_deref(),
            Some("standalone/我的剧本")
        );
        // 嵌套包优先最长匹配
        assert_eq!(
            hit("game_data/scripts/character/风雪/高塔逆位/嵌套/Chapters/01.yaml").as_deref(),
            Some("character/风雪/高塔逆位/嵌套")
        );
        assert_eq!(hit("game_data/scenes.json"), None);
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

    #[test]
    fn real_chapters_pass_shape_check() {
        // 自检每次写章节都会跑，误报会当场拦住作者。拿仓库里真实章节当样本回归。
        // 没带剧本数据的环境（CI）直接跳过。
        let scripts = Path::new(env!("CARGO_MANIFEST_DIR"))
            .parent()
            .map(|p| p.join("data/game_data/scripts"));
        let Some(scripts) = scripts.filter(|p| p.is_dir()) else {
            return;
        };

        let mut checked = 0usize;
        let mut failures: Vec<String> = Vec::new();
        collect_chapter_files(&scripts, &mut |file| {
            let Ok(value) = crate::utils::yaml_file::read_yaml_as_json(file) else {
                return;
            };
            checked += 1;
            let problems = chapter_shape_problems(&value);
            if !problems.is_empty() {
                failures.push(format!("{}：{}", file.display(), problems.join("；")));
            }
        });

        assert!(
            checked > 0,
            "{} 下没扫到章节，样本目录可能不对",
            scripts.display()
        );
        assert!(
            failures.is_empty(),
            "真实章节被自检误报：\n{}",
            failures.join("\n")
        );
    }

    /// 递归收集 `Chapters/` 下的章节文件（`.agent/` 等点号目录不看）。
    fn collect_chapter_files(dir: &Path, f: &mut impl FnMut(&Path)) {
        let Ok(entries) = std::fs::read_dir(dir) else {
            return;
        };
        for entry in entries.flatten() {
            let path = entry.path();
            let name = entry.file_name().to_string_lossy().to_string();
            if name.starts_with('.') {
                continue;
            }
            if path.is_dir() {
                collect_chapter_files(&path, f);
            } else if name.ends_with(".yaml") && path.to_string_lossy().contains("Chapters") {
                f(&path);
            }
        }
    }

    #[test]
    fn artifact_paths_all_live_under_agent_dir() {
        // 「详情」浮窗按目录列产物、按目录取文件，写歪一处就会静默漏掉一份
        for rel in [DESIGN_REL_PATH, CONSTRAINTS_REL_PATH, QUEUE_REL_PATH] {
            assert!(
                rel.starts_with(&format!("{AGENT_DIR}/")),
                "`{rel}` 不在 {AGENT_DIR}/ 下"
            );
        }
    }

    #[test]
    fn asset_mode_reads_canonical_values() {
        let read = |s: &str| parse_asset_mode(&format!("# 用户约束\n\n{}", s));
        assert_eq!(read("- 素材模式：只用已有"), AssetMode::OnlyExisting);
        assert_eq!(read("- 素材模式: 先预留"), AssetMode::Reserve);
        assert_eq!(read("  * 素材模式：仅用已有"), AssetMode::OnlyExisting);
        assert_eq!(read("素材模式：之后补充"), AssetMode::Reserve);
    }

    #[test]
    fn asset_mode_unrecognized_is_unspecified() {
        // 认不出的写法一律当未声明：猜错方向会把校验松紧弄反
        assert_eq!(
            parse_asset_mode("- 素材模式：尽量用已有的"),
            AssetMode::Unspecified
        );
        assert_eq!(parse_asset_mode("- 素材模式："), AssetMode::Unspecified);
        assert_eq!(parse_asset_mode("- 玩家扮演：风雪"), AssetMode::Unspecified);
        assert_eq!(parse_asset_mode(""), AssetMode::Unspecified);
    }

    #[test]
    fn only_existing_mode_turns_missing_asset_into_error() {
        assert!(AssetMode::OnlyExisting.missing_is_error());
        // 没问过就不拦人，但会催去问
        assert!(!AssetMode::Reserve.missing_is_error());
        assert!(!AssetMode::Unspecified.missing_is_error());
    }

    #[test]
    fn chapter_check_render_separates_blocking_from_recorded() {
        let check = ChapterCheck {
            errors: vec!["缺少顶层 `name`".into()],
            warnings: vec!["第 2 个事件 · 找不到素材「夜晚.png」".into()],
            assets_missing: true,
            mode: AssetMode::OnlyExisting,
        };
        let text = check.render();
        assert!(text.contains("还不算写完"), "{text}");
        assert!(text.contains("重新 write_file 覆盖"), "{text}");
        assert!(text.contains("记下来即可"), "{text}");
        assert!(text.contains(".agent/assets.md"), "{text}");
        // 卡人的其实是用户当初选的那个模式，所以回执里必须带上"怎么改"
        assert!(text.contains("改成「先预留」"), "{text}");
    }

    #[test]
    fn every_asset_mode_says_how_to_switch() {
        for mode in [
            AssetMode::Unspecified,
            AssetMode::OnlyExisting,
            AssetMode::Reserve,
        ] {
            let hint = mode.switch_hint();
            assert!(
                hint.contains("素材模式") && hint.contains("说一句"),
                "{mode:?} 没给出改法：{hint}"
            );
        }
    }

    #[test]
    fn impact_line_frames_the_switch_both_ways() {
        let missing = vec!["03 夜晚.png".to_string(), "05 走廊.webp".to_string()];
        // 收紧：告诉他这些会变成必须修的错
        let strict = asset_impact_line(AssetMode::Reserve, &missing);
        assert!(strict.contains("必须修的错"), "{strict}");
        assert!(strict.contains("03 夜晚.png、05 走廊.webp"), "{strict}");
        // 放宽：告诉他一句话就能不再拦
        let loose = asset_impact_line(AssetMode::OnlyExisting, &missing);
        assert!(loose.contains("改成「先预留」"), "{loose}");
        // 未定：先问，不替他决定
        let undecided = asset_impact_line(AssetMode::Unspecified, &missing);
        assert!(undecided.contains("先问用户"), "{undecided}");
    }

    #[test]
    fn impact_line_caps_the_listing() {
        let many: Vec<String> = (0..9).map(|i| format!("0{i} 素材{i}.webp")).collect();
        let line = asset_impact_line(AssetMode::Reserve, &many);
        assert!(line.contains("共 9 处"), "{line}");
        assert!(!line.contains("素材8"), "不该把 9 条全列出来：{line}");
    }

    #[test]
    fn empty_chapter_check_renders_nothing_to_say() {
        assert!(ChapterCheck::default().is_empty());
        assert_eq!(ChapterCheck::default().render(), "");
    }

    #[test]
    fn chapter_after_follows_plan_order() {
        // 本章 = 03 → 下一章 = 04A
        let snap = forge(&["01", "02", "03", "04A"], &["01", "02"]);
        assert_eq!(chapter_after(&snap).as_deref(), Some("04A"));
        // 最后一章之后没有了
        let snap = forge(&["01", "02"], &["01"]);
        assert_eq!(chapter_after(&snap), None);
        // 没有计划就没有"下一章"
        assert_eq!(chapter_after(&forge(&[], &[])), None);
    }

    /// 造一个只属于这条测试的临时剧本包目录。
    fn tmp_script_dir(tag: &str) -> PathBuf {
        let dir =
            std::env::temp_dir().join(format!("lingchat-stage-{}-{}", tag, std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(dir.join(".agent")).unwrap();
        dir
    }

    #[test]
    fn asset_mode_comes_from_constraints_file() {
        let dir = tmp_script_dir("mode");
        assert_eq!(read_asset_mode(&dir), AssetMode::Unspecified);
        std::fs::write(
            dir.join(CONSTRAINTS_REL_PATH),
            "# 用户约束\n\n- 素材模式：只用已有\n- 玩家扮演：风雪\n",
        )
        .unwrap();
        assert_eq!(read_asset_mode(&dir), AssetMode::OnlyExisting);
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn queue_progress_counts_and_names_next() {
        let dir = tmp_script_dir("queue");
        assert_eq!(read_queue_progress(&dir), None);
        std::fs::write(
            dir.join(QUEUE_REL_PATH),
            "# 队列\n- [x] 改章节细节：13–19 章\n- [ ] 写章节：第 09 章\n- [ ] 查素材\n",
        )
        .unwrap();
        let progress = read_queue_progress(&dir).unwrap();
        assert!(progress.contains("共 3 项"), "{progress}");
        assert!(progress.contains("还剩 2"), "{progress}");
        assert!(progress.contains("写章节：第 09 章"), "{progress}");
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn progress_block_reports_mode_and_next_chapter() {
        let dir = tmp_script_dir("progress");
        std::fs::write(dir.join(CONSTRAINTS_REL_PATH), "- 素材模式：先预留\n").unwrap();
        std::fs::write(dir.join(QUEUE_REL_PATH), "- [ ] 写章节：第 09 章\n").unwrap();
        let mut snap = forge(&["01", "02", "03", "04"], &["01", "02"]);
        snap.script_dir = Some(dir.clone());

        let block = progress_block(&snap, &dir, &dir);
        assert!(block.contains("【落盘进度】"), "{block}");
        assert!(block.contains("先预留"), "{block}");
        assert!(block.contains("已落盘：01 02"), "{block}");
        // 本章 = 03，下一章 04 在磁盘上还没有
        assert!(block.contains("下一章 04：尚未落盘"), "{block}");
        assert!(block.contains("队列：共 1 项"), "{block}");
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn progress_block_flags_existing_next_chapter() {
        let dir = tmp_script_dir("progress2");
        // 01 被删了、02 03 还在：本章 = 01，而 02 已有内容
        let mut snap = forge(&["01", "02", "03"], &["02", "03"]);
        snap.script_dir = Some(dir.clone());
        let block = progress_block(&snap, &dir, &dir);
        assert!(block.contains("下一章 02：磁盘上已有内容"), "{block}");
        assert!(block.contains("素材模式：未声明"), "{block}");
        let _ = std::fs::remove_dir_all(&dir);
    }

    /// 造一个完整的剧本包：`<tmp>/scripts/standalone/A/`。
    ///
    /// 里面预置 `Assets/Backgrounds/夜晚.webp` —— 于是「章节里写 夜晚.png」正好是
    /// 「换个扩展名就能找到」那一种，笔误提示与素材模式两条路都能验到。
    fn tmp_script_package(tag: &str, chapter: &str, constraints: &str) -> (PathBuf, PathBuf) {
        let pkg = std::env::temp_dir().join(format!(
            "lingchat-stage-{}-{}/scripts/standalone/A",
            tag,
            std::process::id()
        ));
        let _ = std::fs::remove_dir_all(pkg.parent().unwrap().parent().unwrap());
        std::fs::create_dir_all(pkg.join(".agent")).unwrap();
        std::fs::write(pkg.join(CONSTRAINTS_REL_PATH), constraints).unwrap();
        std::fs::create_dir_all(pkg.join("Chapters")).unwrap();
        std::fs::write(pkg.join("Chapters/01.yaml"), chapter).unwrap();
        let backgrounds = pkg.join("Assets").join("Backgrounds");
        std::fs::create_dir_all(&backgrounds).unwrap();
        std::fs::write(backgrounds.join("夜晚.webp"), b"x").unwrap();
        let data_dir = pkg.parent().unwrap().parent().unwrap().to_path_buf();
        (pkg, data_dir)
    }

    fn snap_of(pkg: &Path) -> StageSnapshot {
        StageSnapshot {
            stage: Stage::Forge,
            script_key: Some("standalone/A".into()),
            script_dir: Some(pkg.to_path_buf()),
            plan: vec!["01".into()],
            written: vec![],
        }
    }

    /// 一个结构错（缺 name）+ 一个素材缺口（磁盘上没有 夜晚.png）的章节。
    const BAD_CHAPTER: &str = "name: ''\nevents:\n  - type: background\n    imagePath: 夜晚.png\n  - type: chapter_end\n    next: end\n";

    #[test]
    fn self_check_flags_shape_and_missing_asset_as_errors_in_only_existing_mode() {
        let (pkg, data_dir) = tmp_script_package("e2e-only", BAD_CHAPTER, "- 素材模式：只用已有\n");
        let check = check_chapter(&pkg, "01", &data_dir).expect("有问题就必须给出自检");

        // 结构错 + 素材缺失 = 两条都必须当场改
        assert_eq!(check.errors.len(), 2, "{:?}", check.errors);
        assert!(check.errors.iter().any(|e| e.contains("name")), "{check:?}");
        let asset = check
            .errors
            .iter()
            .find(|e| e.contains("夜晚.png"))
            .unwrap();
        assert!(asset.contains("疑似写错了扩展名"), "{asset}");
        assert!(check.warnings.is_empty(), "{:?}", check.warnings);
        assert!(check.assets_missing);
        assert!(check.render().contains("还不算写完"));
        let _ = std::fs::remove_dir_all(pkg.parent().unwrap().parent().unwrap());
    }

    #[test]
    fn self_check_downgrades_missing_asset_in_reserve_mode() {
        let (pkg, data_dir) =
            tmp_script_package("e2e-reserve", BAD_CHAPTER, "- 素材模式：先预留\n");
        let check = check_chapter(&pkg, "01", &data_dir).unwrap();

        // 同一份章节：素材缺口从错误降成警告，只剩结构错是非改不可的
        assert_eq!(check.errors.len(), 1, "{:?}", check.errors);
        assert!(
            check.warnings.iter().any(|w| w.contains("夜晚.png")),
            "{check:?}"
        );
        // 已经声明过模式，就不该再催问
        assert!(
            !check.warnings.iter().any(|w| w.contains("还没声明")),
            "{check:?}"
        );
        let _ = std::fs::remove_dir_all(pkg.parent().unwrap().parent().unwrap());
    }

    #[test]
    fn self_check_asks_for_asset_mode_when_undeclared() {
        let (pkg, data_dir) = tmp_script_package("e2e-undeclared", BAD_CHAPTER, "");
        let check = check_chapter(&pkg, "01", &data_dir).unwrap();
        assert!(
            check
                .warnings
                .iter()
                .any(|w| w.contains("素材模式") && w.contains("还没声明")),
            "{:?}",
            check.warnings
        );
        let _ = std::fs::remove_dir_all(pkg.parent().unwrap().parent().unwrap());
    }

    #[test]
    fn self_check_skips_clean_chapter() {
        let clean = "name: 雨夜\nevents:\n  - type: chapter_end\n    next: end\n";
        let (pkg, data_dir) = tmp_script_package("e2e-clean", clean, "- 素材模式：只用已有\n");
        assert_eq!(check_chapter(&pkg, "01", &data_dir), None);
        let _ = std::fs::remove_dir_all(pkg.parent().unwrap().parent().unwrap());
    }

    #[test]
    fn switch_impact_covers_every_written_chapter() {
        let (pkg, data_dir) = tmp_script_package("impact", BAD_CHAPTER, "- 素材模式：先用已有的\n");
        // 02 也留了一个空
        std::fs::write(
            pkg.join("Chapters/02.yaml"),
            "name: 第二章\nevents:\n  - type: music\n    musicPath: 走廊.mp3\n  - type: chapter_end\n    next: end\n",
        )
        .unwrap();

        let mut snap = snap_of(&pkg);
        snap.plan = vec!["01".into(), "02".into()];
        snap.written = vec!["01".into(), "02".into()];

        let missing = missing_assets_of_written(&snap, &pkg, &data_dir);
        assert_eq!(missing, vec!["01 夜晚.png", "02 走廊.mp3"], "{missing:?}");

        // 「先用已有的」不是规范写法 → 当未声明，所以影响面那行是"先问用户"
        let block = progress_block(&snap, &pkg, &data_dir);
        assert!(block.contains("素材模式：未声明"), "{block}");
        assert!(block.contains("随时可改"), "{block}");
        assert!(block.contains("01 夜晚.png"), "{block}");
        let _ = std::fs::remove_dir_all(pkg.parent().unwrap().parent().unwrap());
    }

    #[test]
    fn self_check_ignores_chapters_of_other_scripts() {
        let (pkg, _) = tmp_script_package("e2e-other", BAD_CHAPTER, "- 素材模式：只用已有\n");
        let other = pkg
            .parent()
            .unwrap()
            .join("B")
            .join("Chapters")
            .join("01.yaml");
        assert!(
            chapter_of_write(&snap_of(&pkg), other.to_str().unwrap()).is_none(),
            "别的剧本的章节不该拿来自检"
        );
        // 自己的章节认得出来
        let mine = pkg.join("Chapters").join("01.yaml");
        assert_eq!(
            chapter_of_write(&snap_of(&pkg), mine.to_str().unwrap()).map(|(_, id)| id),
            Some("01".to_string())
        );
        let _ = std::fs::remove_dir_all(pkg.parent().unwrap().parent().unwrap());
    }
}
