//! Skill Agent 的工具定义与分派（OpenAI function-calling 格式）。

use std::collections::HashMap;

use serde_json::json;

use crate::ai_service::skill_agent::command_executor;
use crate::ai_service::skill_agent::core::SkillAgentRunContext;
use crate::ai_service::skill_agent::file_tools::FileTools;
use crate::ai_service::skill_agent::skills;
use crate::api::script_editor::validate::{self, Diagnostic, Severity, ValidationReport};
use crate::ai_service::types::ToolDefinition;

/// LLM 可调用的工具定义。
pub fn tool_definitions() -> Vec<ToolDefinition> {
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
            "用引擎真实的剧本校验器检查剧本（story_config.yaml + Chapters/*.yaml），返回错误/警告/提示诊断。剧本写完、交付之前必须运行本工具，修复所有「错误」后重新校验，直到 error_count == 0。",
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
            "delete_file",
            "删除一个文件。",
            json!({
                "type": "object",
                "properties": {
                    "path": {"type": "string", "description": "要删除的文件路径"}
                },
                "required": ["path"]
            }),
        ),
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

/// 全部工具名（供系统提示枚举）。
pub fn tool_names() -> String {
    tool_definitions()
        .iter()
        .map(|t| t.function.name.clone())
        .collect::<Vec<_>>()
        .join(", ")
}

/// 执行工具。返回 `(ok, 输出文本或错误信息)`。
pub async fn execute_tool(
    ctx: &SkillAgentRunContext,
    name: &str,
    args: &serde_json::Value,
) -> (bool, String) {
    let ft = || FileTools {
        sandbox_dir: ctx.sandbox_dir.clone(),
        allow_any_path: ctx.config.allow_any_path,
    };

    match name {
        "list_skills" => {
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
        "read_skill" => {
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
                }
                None => (false, format!("未找到技能: {}", name_arg)),
            }
        }
        "validate_script" => {
            // 确定剧本 key：显式参数优先，否则回落会话绑定的剧本。
            let arg_key = args
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
                    }
                }
            } else {
                arg_key
            };

            // 解析剧本目录（目录须已存在；写剧本流程先 write_file 建目录，交付前必然存在）。
            let dir = match crate::utils::script_paths::resolve_script_dir(&key) {
                Ok(d) => d,
                Err(e) => return (false, format!("无法定位剧本「{}」：{}", key, e)),
            };

            // 收集其他剧本的 script_name 用于查重（与 editor_validate_script 相同）。
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

            // 运行引擎级校验（只读，无副作用）。诊断本身是工具的合法结果 —— 有错误也要返回 ok。
            let report = validate::validate(&crate::api::data_dir(), &dir, &key, &names);
            (true, format_validation_report(&key, &report))
        }
        "list_files" => {
            let path = args.get("path").and_then(|v| v.as_str()).unwrap_or("");
            if path.trim().is_empty() {
                return (false, "缺少 path 参数".into());
            }
            match ft().list_files(path) {
                Ok(out) => (true, out),
                Err(e) => (false, e.to_string()),
            }
        }
        "read_file" => {
            let path = args.get("path").and_then(|v| v.as_str()).unwrap_or("");
            if path.trim().is_empty() {
                return (false, "缺少 path 参数".into());
            }
            match ft().read_file(path) {
                Ok(out) => (true, out),
                Err(e) => (false, e.to_string()),
            }
        }
        "write_file" => {
            let path = args.get("path").and_then(|v| v.as_str()).unwrap_or("");
            let content = args
                .get("content")
                .and_then(|v| v.as_str())
                .unwrap_or("");
            let append = args.get("append").and_then(|v| v.as_bool()).unwrap_or(false);
            if path.trim().is_empty() {
                return (false, "缺少 path 参数".into());
            }
            match ft().write_file(path, content, append) {
                Ok(out) => (true, out),
                Err(e) => (false, e.to_string()),
            }
        }
        "delete_file" => {
            let path = args.get("path").and_then(|v| v.as_str()).unwrap_or("");
            if path.trim().is_empty() {
                return (false, "缺少 path 参数".into());
            }
            match ft().delete_file(path) {
                Ok(out) => (true, out),
                Err(e) => (false, e.to_string()),
            }
        }
        "execute_command" => {
            let command = args
                .get("command")
                .and_then(|v| v.as_str())
                .unwrap_or("");
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
        other => (false, format!("未知工具: {}", other)),
    }
}

/// 把校验报告格式化成给 LLM 看的中文文本块。
///
/// `report.diagnostics` 已按 severity（error → warn → info）排好序，这里按组渲染并做截断，
/// 保证工具结果（会落库、显示在 UI）长度可控，同时让 LLM 始终看到真实总数。
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
        out.push_str("\n校验未通过：请按上述诊断修复后重新运行 validate_script，直到 error_count == 0。");
    } else {
        out.push_str("\n校验通过（error_count = 0）。");
        if report.warn_count > 0 {
            out.push_str(" 仍建议按诊断处理以下警告。");
        }
    }

    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn format_validation_report_renders_counts_and_lines() {
        let report = ValidationReport {
            diagnostics: vec![
                Diagnostic {
                    severity: Severity::Error,
                    code: "config.duplicate_name",
                    message: "剧本名与另一个剧本重复，引擎按名索引会互相覆盖。".into(),
                    chapter: None,
                    event_index: None,
                    field: None,
                },
                Diagnostic {
                    severity: Severity::Warn,
                    code: "graph.unreachable",
                    message: "章节「03」从开场章节不可达。".into(),
                    chapter: Some("03".into()),
                    event_index: Some(2),
                    field: None,
                },
            ],
            error_count: 1,
            warn_count: 1,
            info_count: 0,
            variables: Vec::new(),
            edges: Vec::new(),
        };
        let s = format_validation_report("standalone/x", &report);
        assert!(s.contains("错误 1 条 · 警告 1 条 · 提示 0 条"));
        assert!(s.contains("[校验报告] 剧本：standalone/x"));
        assert!(s.contains("[错误][config.duplicate_name]"));
        assert!(s.contains("[警告][graph.unreachable]"));
        assert!(s.contains("章节「03」 · 第 3 个事件"));
        assert!(s.contains("校验未通过"));
    }

    #[test]
    fn format_validation_report_clean_report() {
        let report = ValidationReport {
            diagnostics: Vec::new(),
            error_count: 0,
            warn_count: 2,
            info_count: 0,
            variables: Vec::new(),
            edges: Vec::new(),
        };
        let s = format_validation_report("standalone/ok", &report);
        assert!(s.contains("校验通过（error_count = 0）"));
        assert!(s.contains("仍建议按诊断处理以下警告"));
        assert!(!s.contains("校验未通过"));
    }

    #[test]
    fn format_validation_report_truncates_warns() {
        let report = ValidationReport {
            diagnostics: (0..50)
                .map(|i| Diagnostic {
                    severity: Severity::Warn,
                    code: "graph.unreachable",
                    message: format!("警告 {}", i),
                    chapter: None,
                    event_index: None,
                    field: None,
                })
                .collect(),
            error_count: 0,
            warn_count: 50,
            info_count: 0,
            variables: Vec::new(),
            edges: Vec::new(),
        };
        let s = format_validation_report("standalone/x", &report);
        assert!(s.contains("…另有 10 条警告未显示（共 50 条）"));
    }
}
