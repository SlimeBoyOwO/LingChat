//! Skill Agent 的工具定义与分派（OpenAI function-calling 格式）。

use std::collections::HashMap;
use std::path::Path;

use serde_json::json;

use crate::ai_service::game_system::script_engine::validate::{
    self, Diagnostic, Severity, ValidationReport,
};
#[cfg(desktop)]
use crate::ai_service::skill_agent::command_executor;
use crate::ai_service::skill_agent::core::SkillAgentRunContext;
use crate::ai_service::skill_agent::file_tools::FileTools;
use crate::ai_service::skill_agent::role::TaskKind;
use crate::ai_service::skill_agent::skills;
use crate::ai_service::skill_agent::stage;
use crate::ai_service::types::ToolDefinition;

/// LLM 可调用的工具定义，只给 `allowed` 里的那些：藏能力靠这一步，模型看得到全部说明就等于知道还有别的路。
pub fn tool_definitions(allowed: &[&str]) -> Vec<ToolDefinition> {
    all_tool_definitions()
        .into_iter()
        .filter(|t| allowed.contains(&t.function.name.as_str()))
        .collect()
}

/// 工具全集。只有 [`tool_definitions`] 该用这个。
fn all_tool_definitions() -> Vec<ToolDefinition> {
    vec![
        ToolDefinition::new(
            "list_skills",
            "列出所有可用技能的名称、描述与位置。",
            json!({"type": "object", "properties": {}}),
        ),
        ToolDefinition::new(
            "read_skill",
            "加载某个技能的 SKILL.md 指令到上下文。当任务匹配某个可用技能的描述时，在执行任务前调用它。",
            json!({
                "type": "object",
                "properties": {
                    "name": {"type": "string", "description": "要加载的技能名（kebab-case）"}
                },
                "required": ["name"]
            }),
        ),
        ToolDefinition::new(
            "validate_script",
            "用引擎真实的剧本校验器检查剧本（story_config.yaml + Chapters/*.yaml），返回错误/警告/提示诊断。剧本写完、交付之前必须运行本工具，修复所有「错误」后重新校验，直到 error_count == 0。\n\n⚠ 剧本尚未全部写完时（逐章撰写阶段）不要调用本工具：那时它必然报出一批「尚未写完」造成的假错 —— `chapter_end.dangling`（指向还没写的章节）、`graph.unreachable`（后续章节还不可达）、`variable.never_read`（变量已赋值但还没轮到在后面章节消费）、`chapters.empty` 等。这些不是你此刻该修的，照着改会把你引向提前补写后续章节。校验留到全部章节写完后再做。",
            json!({
                "type": "object",
                "properties": {
                    "script_key": {"type": "string", "description": "要校验的剧本 key（如 standalone/我的剧本、character/角色/剧本）。省略时使用当前会话绑定的剧本 key。新建剧本若尚未绑定，必须显式传入。"}
                }
            }),
        ),
        ToolDefinition::new(
            "list_files",
            "列出指定目录下的文件与子目录。",
            json!({
                "type": "object",
                "properties": {
                    "path": {"type": "string", "description": "目录路径，绝对路径或相对于文件沙箱根目录"}
                },
                "required": ["path"]
            }),
        ),
        ToolDefinition::new(
            "read_file",
            "读取文本文件的内容。",
            json!({
                "type": "object",
                "properties": {
                    "path": {"type": "string", "description": "文件路径，绝对路径或相对于文件沙箱根目录"}
                },
                "required": ["path"]
            }),
        ),
        ToolDefinition::new(
            "write_file",
            "向文件写入内容，自动创建父目录。默认覆盖整个文件；append=true 时追加。单次调用写完整内容；仅当一次写入因参数过长而失败（报错会附带 [诊断] 提示）后才用 append=true 分段补齐。",
            json!({
                "type": "object",
                "properties": {
                    "path": {"type": "string", "description": "文件路径，绝对路径或相对于文件沙箱根目录"},
                    "content": {"type": "string", "description": "要写入的内容（append=true 时为要追加的内容）"},
                    "append": {"type": "boolean", "description": "true 表示追加到已有文件末尾，仅用于修复被截断的写入"}
                },
                "required": ["path", "content"]
            }),
        ),
        ToolDefinition::new(
            "edit_file",
            "精确替换文件里的一段文本（改一行、改一个词、删掉整整一节都用它）。\
             `old_string` 必须**唯一命中**，否则不改任何东西并报出有几处；`new_string` 留空 = 删除这段。\
             参数很小，不会因为内容过长被截断 —— **要改文件里的一段就用它，别整份重写**。",
            json!({
                "type": "object",
                "properties": {
                    "path": {"type": "string", "description": "文件路径"},
                    "old_string": {"type": "string", "description": "要被替换掉的原文（必须唯一命中；要删掉一整节就把那一节原文整段放进来）"},
                    "new_string": {"type": "string", "description": "替换成的新文本；留空表示删掉 old_string 那一段"},
                    "replace_all": {"type": "boolean", "description": "确实要替换全部匹配时才设 true"}
                },
                "required": ["path", "old_string"]
            }),
        ),
        ToolDefinition::new(
            "delete_file",
            "删除一个文件（整份）。只能删文件，不能删目录。",
            json!({
                "type": "object",
                "properties": {
                    "path": {"type": "string", "description": "要删除的文件路径"}
                },
                "required": ["path"]
            }),
        ),
        #[cfg(desktop)]
        ToolDefinition::new(
            "execute_command",
            "在本机运行 shell 命令。运行前可能需要用户确认。",
            json!({
                "type": "object",
                "properties": {
                    "command": {"type": "string", "description": "要运行的 shell 命令"},
                    "cwd": {"type": "string", "description": "工作目录，绝对路径或相对于文件沙箱根目录。留空表示沙箱根目录。"}
                },
                "required": ["command"]
            }),
        ),
    ]
}

/// 本轮工具名清单（供系统提示枚举）。只列真正给出去的那些。
pub fn tool_names(allowed: &[&str]) -> String {
    tool_definitions(allowed)
        .iter()
        .map(|t| t.function.name.clone())
        .collect::<Vec<_>>()
        .join(", ")
}

/// 本项的作用域：是否产出能跑章节、真正执行的任务类型。工具入口的收窄闸门只看这两样。
#[derive(Clone, Copy)]
pub struct ItemScope {
    /// 本项是否产出能跑的章节：决定目录闸门放不放行 `Chapters/`。
    pub allow_runnable: bool,
    /// 本项真正执行的任务类型（补前置时与队列里那一项不同，见 `core::run_chat`）。
    pub kind: TaskKind,
}

/// 白名单闸门（工具入口的第一道）：工具说明已按角色过滤过一遍，但模型可能照历史里的旧印象硬调；
fn name_refusal(allowed: &[&str], name: &str) -> Option<String> {
    if allowed.contains(&name) {
        return None;
    }
    Some(format!(
        "本轮不允许调用 `{name}`（当前这一轮没有这个能力；工具清单里只有 {}）。\
         请用允许的工具，或者如实告诉用户你这一轮做不了什么。",
        allowed.join(" / ")
    ))
}

/// 执行一次工具调用，返回 `(ok, 输出文本或错误信息)`。每个工具一个处理函数，闸门顺序留在各自的函数里：
pub async fn execute_tool(
    ctx: &SkillAgentRunContext,
    allowed: &[&str],
    name: &str,
    args: &serde_json::Value,
    item: ItemScope,
) -> (bool, String) {
    if let Some(refusal) = name_refusal(allowed, name) {
        return (false, refusal);
    }

    let ft = FileTools {
        sandbox_dir: ctx.sandbox_dir.clone(),
        allow_any_path: ctx.config.allow_any_path,
    };

    match name {
        "list_skills" => tool_list_skills(ctx),
        "read_skill" => tool_read_skill(ctx, args),
        "validate_script" => tool_validate_script(ctx, args),
        "list_files" => tool_list_files(&ft, args),
        "read_file" => tool_read_file(&ft, args),
        "write_file" => tool_write_file(ctx, item, &ft, args).await,
        "edit_file" => tool_edit_file(ctx, item, &ft, args).await,
        "delete_file" => tool_delete_file(ctx, item, &ft, args),
        #[cfg(desktop)]
        "execute_command" => tool_execute_command(ctx, args).await,
        other => (false, format!("未知工具: {}", other)),
    }
}

fn tool_list_skills(ctx: &SkillAgentRunContext) -> (bool, String) {
    let skills = skills::find_all_skills(&ctx.skills_dir);
    if skills.is_empty() {
        (true, "没有已安装的技能。".into())
    } else {
        let lines = skills
            .iter()
            .map(|s| format!("- {} ({}): {}", s.name, s.location, s.description))
            .collect::<Vec<_>>()
            .join("\n");
        (true, format!("可用技能:\n{}", lines))
    }
}

fn tool_read_skill(ctx: &SkillAgentRunContext, args: &serde_json::Value) -> (bool, String) {
    let name_arg = args.get("name").and_then(|v| v.as_str()).unwrap_or("");
    if name_arg.is_empty() {
        return (false, "缺少 name 参数".into());
    }
    match skills::find_skill(&ctx.skills_dir, name_arg) {
        Some(res) => {
            let msg = format!(
                "Reading: {}\nBase directory: {}\n\n{}\n\nSkill loaded: {}",
                res.name,
                res.base_directory.display(),
                res.content,
                res.name
            );
            (true, msg)
        },
        None => (false, format!("未找到技能: {}", name_arg)),
    }
}

fn tool_validate_script(ctx: &SkillAgentRunContext, args: &serde_json::Value) -> (bool, String) {
    let arg_key = args // 确定剧本 key：显式参数优先，否则回落会话绑定的剧本。
        .get("script_key")
        .and_then(|v| v.as_str())
        .unwrap_or("")
        .trim()
        .to_string();
    let key = if arg_key.is_empty() {
        match &ctx.script_key {
            Some(k) => k.clone(),
            None => {
                return (
                    false,
                    "未指定要校验的剧本 key，且当前会话没有绑定剧本。请传入 script_key 参数（如 standalone/我的剧本）。"
                        .into(),
                );
            },
        }
    } else {
        arg_key
    };

    let dir = match crate::utils::script_paths::resolve_script_dir(&key) {
        Ok(d) => d,
        Err(e) => return (false, format!("无法定位剧本「{}」：{}", key, e)),
    };

    let mut names: HashMap<String, Vec<String>> = HashMap::new();
    for other in crate::utils::script_paths::enumerate_script_keys() {
        if let Ok(d) = crate::utils::script_paths::resolve_script_dir(&other) {
            if let Ok(cfg) = crate::utils::yaml_file::read_story_config(&d) {
                if let Some(n) = cfg.get("script_name").and_then(|v| v.as_str()) {
                    let n = n.trim();
                    if !n.is_empty() {
                        names.entry(n.to_string()).or_default().push(other.clone());
                    }
                }
            }
        }
    }

    let report = validate::validate(crate::data_dir::get_data_dir(), &dir, &key, &names);
    (true, format_validation_report(&key, &report))
}

fn tool_list_files(ft: &FileTools, args: &serde_json::Value) -> (bool, String) {
    let path = args.get("path").and_then(|v| v.as_str()).unwrap_or("");
    if path.trim().is_empty() {
        return (false, "缺少 path 参数".into());
    }
    match ft.list_files(path) {
        Ok(out) => (true, out),
        Err(e) => (false, e.to_string()),
    }
}

fn tool_read_file(ft: &FileTools, args: &serde_json::Value) -> (bool, String) {
    let path = args.get("path").and_then(|v| v.as_str()).unwrap_or("");
    if path.trim().is_empty() {
        return (false, "缺少 path 参数".into());
    }
    match ft.read_file(path) {
        Ok(out) => (true, out),
        Err(e) => (false, e.to_string()),
    }
}

async fn tool_write_file(
    ctx: &SkillAgentRunContext,
    item: ItemScope,
    ft: &FileTools,
    args: &serde_json::Value,
) -> (bool, String) {
    let path = args.get("path").and_then(|v| v.as_str()).unwrap_or("");
    let content = args.get("content").and_then(|v| v.as_str()).unwrap_or("");
    let append = args
        .get("append")
        .and_then(|v| v.as_bool())
        .unwrap_or(false);
    if path.trim().is_empty() {
        return (false, "缺少 path 参数".into());
    }
    if let Some(refusal) = unbound_foreign_package(ctx, path) {
        return (false, refusal);
    }
    if !item.allow_runnable && is_runnable_script_path(path) {
        return (
            false,
            format!(include_str!("prompts/tools_execute_tool.txt"), path = path),
        );
    }
    if let Some(refusal) = draft_scope_refusal(item.kind, "write_file", path, false) {
        return (false, refusal);
    }
    match ft.write_file(path, content, append) {
        Ok(out) => {
            bind_script_key_if_new(ctx, path).await;
            (true, with_chapter_check(ctx, path, out, append).await)
        },
        Err(e) => (false, e.to_string()),
    }
}

async fn tool_edit_file(
    ctx: &SkillAgentRunContext,
    item: ItemScope,
    ft: &FileTools,
    args: &serde_json::Value,
) -> (bool, String) {
    let path = args.get("path").and_then(|v| v.as_str()).unwrap_or("");
    let old = args
        .get("old_string")
        .and_then(|v| v.as_str())
        .unwrap_or("");
    let new = args
        .get("new_string")
        .and_then(|v| v.as_str())
        .unwrap_or("");
    let replace_all = args
        .get("replace_all")
        .and_then(|v| v.as_bool())
        .unwrap_or(false);
    if path.trim().is_empty() {
        return (false, "缺少 path 参数".into());
    }
    if old.trim().is_empty() {
        return (
            false,
            "缺少 old_string（要被替换掉的原文）。要删掉文件里的某一整段（如某一章那一节），\
             就把那一段原文整段放进 `old_string`、`new_string` 留空；\
             一次只改这一处，改不动就一个字都不写、并告诉你命中了几处。"
                .into(),
        );
    }
    if let Some(refusal) = unbound_foreign_package(ctx, path) {
        return (false, refusal);
    }
    if !item.allow_runnable && is_runnable_script_path(path) {
        return (
            false,
            format!(
                "这一轮用户没有要求产出能跑的章节，所以 `{path}` 不能改。\
                 要改 `Chapters/` 目录或 `story_config.yaml` 里的东西，\
                 得用户明说「转成 YAML」/「改 YAML」（或旧说法「落盘」）那一轮才做。"
            ),
        );
    }
    if let Some(refusal) = draft_scope_refusal(item.kind, "edit_file", path, new.trim().is_empty())
    {
        return (false, refusal);
    }
    match ft.edit_text(path, old, new, replace_all) {
        Ok(result) => {
            bind_script_key_if_new(ctx, &result.path.to_string_lossy()).await;
            let out = format!(
                "已改 `{}`（替换 {} 处，未动其余内容）",
                result.path.display(),
                result.replacements
            );
            (true, with_chapter_check(ctx, path, out, false).await)
        },
        Err(e) => (false, e.to_string()),
    }
}

fn tool_delete_file(
    ctx: &SkillAgentRunContext,
    item: ItemScope,
    ft: &FileTools,
    args: &serde_json::Value,
) -> (bool, String) {
    let path = args.get("path").and_then(|v| v.as_str()).unwrap_or("");
    if path.trim().is_empty() {
        return (false, "缺少 path 参数".into());
    }
    if let Some(refusal) = unbound_foreign_package(ctx, path) {
        return (false, refusal);
    }
    if !item.allow_runnable && is_runnable_script_path(path) {
        return (
            false,
            format!(
                "这一轮用户没有要求产出能跑的章节，所以 `{path}` 不能删。\
                 要删 `Chapters/` 里的东西，得用户明说「改 YAML」（或旧说法「落盘」）那一轮才做。"
            ),
        );
    }
    if let Some(conflict) = chapter_delete_conflict(ctx, path) {
        return (false, conflict);
    }
    match ft.delete_file(path) {
        Ok(out) => (true, out),
        Err(e) => (false, e.to_string()),
    }
}

#[cfg(desktop)]
async fn tool_execute_command(
    ctx: &SkillAgentRunContext,
    args: &serde_json::Value,
) -> (bool, String) {
    let command = args.get("command").and_then(|v| v.as_str()).unwrap_or("");
    let cwd = args.get("cwd").and_then(|v| v.as_str()).unwrap_or("");
    if command.is_empty() {
        return (false, "缺少 command 参数".into());
    }
    match command_executor::execute_command(
        &ctx.channel,
        &ctx.approvals,
        ctx.config.auto_approve_commands,
        &ctx.sandbox_dir,
        command,
        cwd,
    )
    .await
    {
        Ok(out) => (out.exit_code == 0, out.to_prompt_string()),
        Err(e) => (false, e.to_string()),
    }
}

/// 把校验报告格式化成给 LLM 看的中文文本块。
fn format_validation_report(key: &str, report: &ValidationReport) -> String {
    const MAX_ERROR: usize = 100;
    const MAX_WARN: usize = 40;
    const MAX_INFO: usize = 10;

    fn sev_tag(s: Severity) -> &'static str {
        match s {
            Severity::Error => "错误",
            Severity::Warn => "警告",
            Severity::Info => "提示",
        }
    }

    /// 位置描述：章节「X」 · 第 N 个事件 · 字段「Y」；剧本级诊断无位置则留空。
    fn location(d: &Diagnostic) -> String {
        let mut parts: Vec<String> = Vec::new();
        if let Some(c) = &d.chapter {
            parts.push(format!("章节「{}」", c));
        }
        if let Some(i) = d.event_index {
            parts.push(format!("第 {} 个事件", i + 1));
        }
        if let Some(f) = &d.field {
            parts.push(format!("字段「{}」", f));
        }
        parts.join(" · ")
    }

    let mut out = String::new();
    out.push_str(&format!(
        "[校验报告] 剧本：{}\n错误 {} 条 · 警告 {} 条 · 提示 {} 条\n",
        key, report.error_count, report.warn_count, report.info_count
    ));

    let mut render_group = |sev: Severity, cap: usize| {
        let list: Vec<&Diagnostic> = report
            .diagnostics
            .iter()
            .filter(|d| d.severity == sev)
            .collect();
        if list.is_empty() {
            return;
        }
        out.push_str(&format!("\n【{}】{} 条\n", sev_tag(sev), list.len()));
        for d in list.iter().take(cap) {
            out.push_str(&format!(
                "- [{}][{}] {}：{}\n",
                sev_tag(sev),
                d.code,
                location(d),
                d.message
            ));
        }
        let overflow = list.len().saturating_sub(cap);
        if overflow > 0 {
            out.push_str(&format!(
                "…另有 {} 条{}未显示（共 {} 条）\n",
                overflow,
                sev_tag(sev),
                list.len()
            ));
        }
    };

    render_group(Severity::Error, MAX_ERROR);
    render_group(Severity::Warn, MAX_WARN);
    render_group(Severity::Info, MAX_INFO);

    if report.error_count > 0 {
        out.push_str(
            "\n校验未通过：请按上述诊断修复后重新运行 validate_script，直到 error_count == 0。",
        );
    } else {
        out.push_str("\n校验通过（error_count = 0）。");
        if report.warn_count > 0 {
            out.push_str(" 仍建议按诊断处理以下警告。");
        }
    }

    out
}

/// 写完章节后附上自检回执（写一章和查一章是同一个动作，不攒到最后）；分段追加（`append = true`）时跳过，那时文件还没写完。
async fn with_chapter_check(
    ctx: &SkillAgentRunContext,
    path: &str,
    out: String,
    append: bool,
) -> String {
    if append {
        return out;
    }
    let Some(check) =
        stage::evidence::check_written_chapter(&ctx.stage_snapshot, path, &ctx.data_dir)
    else {
        return out;
    };
    let chapter = stage::evidence::chapter_id_of_path(path).unwrap_or_default();

    if !check.missing_assets.is_empty() {
        if let Some(dir) = ctx.stage_snapshot.script_dir.as_deref() {
            if let Err(e) = stage::evidence::update_assets_gap(dir, &chapter, &check.missing_assets)
            {
                tracing::warn!("[skill_agent] 素材缺口表写入失败: {e}");
            }
        }
    }

    tracing::info!(
        // 这一章的结局只进日志（原先那条事件流水已删）：改完才算落盘，记下当场有没有必须修的
        "[skill_agent] 章节自检 剧本={} 章={chapter} 错误={} 警告={} 缺口={}",
        ctx.script_key.as_deref().unwrap_or("-"),
        check.errors.len(),
        check.warnings.len(),
        check.missing_assets.len()
    );

    format!("{}{}", out, check.render())
}

/// 未绑定会话想往已经存在的剧本包里写就拦住：模型会自己扫盘挑一个"正好缺这一章"的包当成用户的剧本，
fn unbound_foreign_package(ctx: &SkillAgentRunContext, path: &str) -> Option<String> {
    foreign_package_refusal(
        ctx.stage_snapshot.script_key.as_deref(),
        &ctx.existing_script_keys,
        path,
    )
}

/// 判定本体。抽出来只为测得动 —— 为它造一个完整的 run context 不值得。
fn foreign_package_refusal(bound: Option<&str>, existing: &[String], path: &str) -> Option<String> {
    if bound.is_some() {
        return None;
    }
    let key = stage::evidence::script_key_of(path, &[])?;
    if !existing.iter().any(|k| k == &key) {
        return None;
    }
    Some(format!(
        include_str!("prompts/tools_foreign_package_refusal_info.txt"),
        key = key
    ))
}

/// `story_config.yaml` 落盘即剧本包诞生，从写入路径反推 key 绑到会话上；已有绑定不覆盖，绑定下一轮生效。
async fn bind_script_key_if_new(ctx: &SkillAgentRunContext, path: &str) {
    if ctx.stage_snapshot.script_key.is_some() {
        return;
    }
    let Some(key) = stage::evidence::script_key_of(path, &[]) else {
        return;
    };
    let Ok(dir) = crate::utils::script_paths::resolve_script_dir(&key) else {
        return;
    };
    ctx.remember_bound_key(&key);
    match stage::ensure_package_skeleton(&dir, &key) {
        Ok(true) => tracing::info!("[skill_agent] 已为新剧本包补最小骨架: {}", key),
        Ok(false) => {},
        Err(e) => tracing::warn!("[skill_agent] 补剧本骨架失败: {e}"),
    }
    match crate::ai_service::skill_agent::db::update_conversation_script_key(
        &ctx.db,
        ctx.conversation_id,
        key.clone(),
    )
    .await
    {
        Ok(()) => tracing::info!("[skill_agent] 会话已绑定剧本 key: {}", key),
        Err(e) => tracing::warn!("[skill_agent] 绑定剧本 key 失败: {}", e),
    }
}

/// 这个路径是不是能跑的剧本文件：`Chapters/` 下的任何文件，或 `story_config.yaml`；只按路径字符串判断（不碰文件系统）。
pub(crate) fn is_runnable_script_path(path: &str) -> bool {
    let p = format!("/{}", path.replace('\\', "/").to_lowercase());
    let file = p.rsplit('/').next().unwrap_or("");
    file == "story_config.yaml" || p.contains("/chapters/")
}

/// 大纲类任务不许往细节稿里写正文（三层套着：大纲 → 正文 → 能跑的章节，动下面那层可往上对齐、反过来只能靠删除连带）。
fn draft_scope_refusal(kind: TaskKind, tool: &str, path: &str, blank_new: bool) -> Option<String> {
    if !matches!(kind, TaskKind::Outline | TaskKind::ReviseOutline) {
        return None;
    }
    let probe = format!("/{}", path.replace('\\', "/").to_lowercase());
    if !probe.ends_with(&format!("/{}", stage::CHAPTER_DETAILS_REL_PATH)) {
        return None;
    }
    if tool == "edit_file" && blank_new {
        return None;
    }
    Some(format!(
        include_str!("prompts/tools_draft_scope_refusal_info.txt"),
        kind.label(),
        path = path
    ))
}

/// 删一份章节文件之前先看有没有别处指着它：删了链子就断，玩家走到那里会卡住，而且只有跑起来才发现。
fn chapter_delete_conflict(ctx: &SkillAgentRunContext, path: &str) -> Option<String> {
    let dir = ctx.stage_snapshot.script_dir.as_deref()?;
    let id = stage::evidence::chapter_id_of_path(path)?;
    let refs = chapter_referrers(dir, &id);
    if refs.is_empty() {
        return None;
    }
    Some(format!(
        "先别删 `{id}.yaml`：还有别处指着它 —— {}。\
         删了链子就断（玩家走到那里会卡住）。先把这些引用改到别的章节（或改成 `\"end\"`），再回来删它。",
        refs.join("、")
    ))
}

/// 这个剧本包里有哪些东西指着第 `id` 章。纯读盘，不碰引擎 —— 逻辑测得动。
fn chapter_referrers(dir: &Path, id: &str) -> Vec<String> {
    let mut refs: Vec<String> = Vec::new();

    let entries = std::fs::read_dir(dir.join("Chapters"))
        .into_iter()
        .flatten()
        .flatten();
    for entry in entries {
        let file = entry.path();
        if file.extension().and_then(|e| e.to_str()) != Some("yaml") {
            continue;
        }
        let name = file
            .file_stem()
            .and_then(|s| s.to_str())
            .unwrap_or_default()
            .to_string();
        if name == id {
            continue;
        }
        let Ok(value) = crate::utils::yaml_file::read_yaml_as_json(&file) else {
            continue;
        };
        let points_here = value
            .get("events")
            .and_then(|e| e.as_array())
            .map(|events| events.iter().any(|ev| event_points_at(ev, id)))
            .unwrap_or(false);
        if points_here {
            refs.push(format!("`Chapters/{name}.yaml` 的 `chapter_end`"));
        }
    }

    if let Ok(config) = crate::utils::yaml_file::read_yaml_as_json(&dir.join("story_config.yaml")) {
        if points_at(config.get("intro_chapter"), id) {
            refs.push("`story_config.yaml` 的 `intro_chapter`".into());
        }
    }

    refs
}

/// 这一条事件里有没有指向第 `id` 章的跳转，只看 `chapter_end`：事件顶层的 `next_chapter` / `next`，
fn event_points_at(ev: &serde_json::Value, id: &str) -> bool {
    if ev.get("type").and_then(|t| t.as_str()) != Some("chapter_end") {
        return false;
    }
    if ["next_chapter", "next"]
        .iter()
        .any(|k| points_at(ev.get(*k), id))
    {
        return true;
    }
    ev.get("options")
        .and_then(|o| o.as_array())
        .is_some_and(|options| options.iter().any(|opt| points_at(opt.get("next"), id)))
}

/// 这个 YAML 字段是不是指着第 `id` 章：章节号写法很杂（`02`、`"02"`、`2`、`第2章`），统一经 [`stage::evidence::chapter_ids_in`] 归一化后再比。
fn points_at(value: Option<&serde_json::Value>, id: &str) -> bool {
    let raw = match value {
        Some(serde_json::Value::String(s)) => s.clone(),
        Some(serde_json::Value::Number(n)) => n.to_string(),
        _ => return false,
    };
    !raw.trim().is_empty()
        && stage::evidence::chapter_ids_in(&raw) == stage::evidence::chapter_ids_in(id)
}
