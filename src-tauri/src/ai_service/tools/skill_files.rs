//! 主聊天可用的技能库 / 文件沙箱工具。
//!
//! 复用 skill_agent 的技能发现（`skills.rs`）与文件沙箱（`file_tools.rs`），
//! 让主对话角色也能读取技能、操作沙箱内文件（默认 `data/`，受「助手设置」的
//! 沙箱目录 / 允许任意路径配置约束）。
//! 不含 `execute_command`（主聊天缺少命令审批通道，无法向用户请求确认）
//! 与 `validate_script`（剧本编辑器会话专用）。

use async_trait::async_trait;
use serde_json::{json, Value};

use crate::ai_service::skill_agent::config::SkillAgentConfig;
use crate::ai_service::skill_agent::file_tools::FileTools;
use crate::ai_service::skill_agent::skills;
use crate::ai_service::types::ToolDefinition;

use super::executor::{Tool, ToolContext, ToolError, ToolResult};

/// 从工具上下文加载 skill agent 配置（沙箱目录 / 任意路径开关）。
fn load_config(context: &ToolContext) -> Result<SkillAgentConfig, ToolError> {
    let app = context.require_app()?;
    Ok(SkillAgentConfig::load(&app))
}

/// 由配置构造文件沙箱工具。
fn file_tools(config: &SkillAgentConfig) -> FileTools {
    FileTools {
        sandbox_dir: config.resolve_sandbox_dir(),
        allow_any_path: config.allow_any_path,
    }
}

fn arg_str<'a>(arguments: &'a Value, key: &str) -> Result<&'a str, ToolError> {
    let value = arguments.get(key).and_then(Value::as_str).unwrap_or("");
    if value.trim().is_empty() {
        return Err(ToolError::InvalidArguments(format!("缺少 {key} 参数")));
    }
    Ok(value)
}

fn exec(result: anyhow::Result<String>) -> Result<ToolResult, ToolError> {
    result
        .map(|out| json!({ "ok": true, "output": out }))
        .map_err(|e| ToolError::Execution(e.to_string()))
}

/// list_skills：列出技能库中全部可用技能。
pub struct ListSkills;

#[async_trait]
impl Tool for ListSkills {
    fn definition(&self) -> ToolDefinition {
        ToolDefinition::new(
            "list_skills",
            "列出所有可用技能的名称、描述与位置。",
            json!({"type": "object", "properties": {}}),
        )
    }

    async fn execute(&self, context: &ToolContext, _: Value) -> Result<ToolResult, ToolError> {
        let config = load_config(context)?;
        let found = skills::find_all_skills(&config.resolve_skills_dir());
        if found.is_empty() {
            return Ok(json!({ "ok": true, "output": "没有已安装的技能。" }));
        }
        let lines = found
            .iter()
            .map(|s| format!("- {} ({}): {}", s.name, s.location, s.description))
            .collect::<Vec<_>>()
            .join("\n");
        Ok(json!({ "ok": true, "output": format!("可用技能:\n{lines}") }))
    }
}

/// read_skill：把某个技能的 SKILL.md 指令加载进上下文。
pub struct ReadSkill;

#[async_trait]
impl Tool for ReadSkill {
    fn definition(&self) -> ToolDefinition {
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
        )
    }

    async fn execute(&self, context: &ToolContext, arguments: Value) -> Result<ToolResult, ToolError> {
        let config = load_config(context)?;
        let name = arg_str(&arguments, "name")?;
        match skills::find_skill(&config.resolve_skills_dir(), name) {
            Some(res) => Ok(json!({
                "ok": true,
                "output": format!(
                    "Reading: {}\nBase directory: {}\n\n{}\n\nSkill loaded: {}",
                    res.name,
                    res.base_directory.display(),
                    res.content,
                    res.name
                ),
            })),
            None => Err(ToolError::Execution(format!("未找到技能: {name}"))),
        }
    }
}

/// list_files：列出沙箱内指定目录的文件与子目录。
pub struct ListFiles;

#[async_trait]
impl Tool for ListFiles {
    fn definition(&self) -> ToolDefinition {
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
        )
    }

    async fn execute(&self, context: &ToolContext, arguments: Value) -> Result<ToolResult, ToolError> {
        let config = load_config(context)?;
        exec(file_tools(&config).list_files(arg_str(&arguments, "path")?))
    }
}

/// read_file：读取沙箱内文本文件内容（超大文件截断）。
pub struct ReadFile;

#[async_trait]
impl Tool for ReadFile {
    fn definition(&self) -> ToolDefinition {
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
        )
    }

    async fn execute(&self, context: &ToolContext, arguments: Value) -> Result<ToolResult, ToolError> {
        let config = load_config(context)?;
        exec(file_tools(&config).read_file(arg_str(&arguments, "path")?))
    }
}

/// write_file：写入沙箱内文件（自动建父目录，append 可追加）。
pub struct WriteFile;

#[async_trait]
impl Tool for WriteFile {
    fn definition(&self) -> ToolDefinition {
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
        )
    }

    async fn execute(&self, context: &ToolContext, arguments: Value) -> Result<ToolResult, ToolError> {
        let config = load_config(context)?;
        let path = arg_str(&arguments, "path")?;
        let content = arguments
            .get("content")
            .and_then(Value::as_str)
            .ok_or_else(|| ToolError::InvalidArguments("缺少 content 参数".into()))?;
        let append = arguments
            .get("append")
            .and_then(Value::as_bool)
            .unwrap_or(false);
        exec(file_tools(&config).write_file(path, content, append))
    }
}

/// delete_file：删除沙箱内一个文件。
pub struct DeleteFile;

#[async_trait]
impl Tool for DeleteFile {
    fn definition(&self) -> ToolDefinition {
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
        )
    }

    async fn execute(&self, context: &ToolContext, arguments: Value) -> Result<ToolResult, ToolError> {
        let config = load_config(context)?;
        exec(file_tools(&config).delete_file(arg_str(&arguments, "path")?))
    }
}
