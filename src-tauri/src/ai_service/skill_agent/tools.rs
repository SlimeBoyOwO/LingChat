//! Skill Agent 的工具定义与分派（OpenAI function-calling 格式）。

use serde_json::json;

use crate::ai_service::skill_agent::command_executor;
use crate::ai_service::skill_agent::core::SkillAgentRunContext;
use crate::ai_service::skill_agent::file_tools::FileTools;
use crate::ai_service::skill_agent::skills;
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
