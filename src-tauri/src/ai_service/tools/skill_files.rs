//! 主聊天可用的技能库 / 文件沙箱 / 命令执行工具。
//!
//! 复用 skill_agent 的技能发现（`skills.rs`）、文件沙箱（`file_tools.rs`）与
//! 命令执行（`command_executor.rs`），让主对话角色也能读技能、操作文件、跑命令。
//! 文件工具默认锁定沙箱（`data/`），可通过工具配置「允许访问沙箱外路径」或
//! 「助手设置」的允许任意路径放开。
//! `execute_command` 默认每次都要用户在前端弹窗确认（`chat:command_approval`
//! 事件 + `resolve_command_approval` 回调），可在工具配置开启免确认；
//! `uac=true` 时以管理员权限运行（Windows 弹系统 UAC 框）。
//! 不含 `validate_script`（剧本编辑器会话专用）。

use std::sync::Arc;
use std::time::Duration;

use async_trait::async_trait;
use serde_json::{json, Value};
use tauri::{Emitter, Manager};

use crate::ai_service::skill_agent::command_executor::{self, ApprovalRequest};
use crate::ai_service::skill_agent::config::SkillAgentConfig;
use crate::ai_service::skill_agent::file_tools::FileTools;
use crate::ai_service::skill_agent::skills;
use crate::ai_service::types::ToolDefinition;
use crate::AppState;

use super::executor::{Tool, ToolContext, ToolError, ToolResult};
use super::settings::SharedToolSettings;

/// 从工具上下文加载 skill agent 配置（沙箱目录 / 任意路径开关）。
fn load_config(context: &ToolContext) -> Result<SkillAgentConfig, ToolError> {
    let app = context.require_app()?;
    Ok(SkillAgentConfig::load(&app))
}

/// 由配置构造文件沙箱工具。「助手设置」或工具配置任一方放开任意路径即生效。
fn file_tools(config: &SkillAgentConfig, settings: &SharedToolSettings) -> FileTools {
    FileTools {
        sandbox_dir: config.resolve_sandbox_dir(),
        allow_any_path: config.allow_any_path || settings.get().file_ops_allow_any_path,
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

/// 文件类工具共用：持有工具配置句柄（沙箱外开关热更新）。
macro_rules! file_tool {
    ($name:ident, $tool_name:literal, $desc:literal, $schema:expr, $body:expr) => {
        pub struct $name {
            settings: SharedToolSettings,
        }

        impl $name {
            pub fn new(settings: SharedToolSettings) -> Self {
                Self { settings }
            }
        }

        #[async_trait]
        impl Tool for $name {
            fn definition(&self) -> ToolDefinition {
                ToolDefinition::new($tool_name, $desc, $schema)
            }

            async fn execute(&self, context: &ToolContext, arguments: Value) -> Result<ToolResult, ToolError> {
                let config = load_config(context)?;
                let ft = file_tools(&config, &self.settings);
                let run: fn(&FileTools, &Value) -> Result<ToolResult, ToolError> = $body;
                run(&ft, &arguments)
            }
        }
    };
}

file_tool!(
    ListFiles,
    "list_files",
    "列出指定目录下的文件与子目录。",
    json!({
        "type": "object",
        "properties": {
            "path": {"type": "string", "description": "目录路径，绝对路径或相对于文件沙箱根目录"}
        },
        "required": ["path"]
    }),
    |ft: &FileTools, args: &Value| {
        let path = arg_str(args, "path")?;
        exec(ft.list_files(path))
    }
);

file_tool!(
    ReadFile,
    "read_file",
    "读取文本文件的内容。",
    json!({
        "type": "object",
        "properties": {
            "path": {"type": "string", "description": "文件路径，绝对路径或相对于文件沙箱根目录"}
        },
        "required": ["path"]
    }),
    |ft: &FileTools, args: &Value| {
        let path = arg_str(args, "path")?;
        exec(ft.read_file(path))
    }
);

file_tool!(
    WriteFile,
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
    |ft: &FileTools, args: &Value| {
        let path = arg_str(args, "path")?;
        let content = args
            .get("content")
            .and_then(Value::as_str)
            .ok_or_else(|| ToolError::InvalidArguments("缺少 content 参数".into()))?;
        let append = args.get("append").and_then(Value::as_bool).unwrap_or(false);
        exec(ft.write_file(path, content, append))
    }
);

file_tool!(
    DeleteFile,
    "delete_file",
    "删除一个文件。",
    json!({
        "type": "object",
        "properties": {
            "path": {"type": "string", "description": "要删除的文件路径"}
        },
        "required": ["path"]
    }),
    |ft: &FileTools, args: &Value| {
        let path = arg_str(args, "path")?;
        exec(ft.delete_file(path))
    }
);

file_tool!(
    EditFile,
    "edit_file",
    "精确替换文件中的文本：old_string 必须唯一匹配（除非 replace_all=true）。修改前先用 read_file 确认内容；替换失败会说明原因（无匹配/多处匹配）。",
    json!({
        "type": "object",
        "properties": {
            "path": {"type": "string", "description": "文件路径，绝对路径或相对于文件沙箱根目录"},
            "old_string": {"type": "string", "description": "要被替换的原文（须唯一匹配）"},
            "new_string": {"type": "string", "description": "替换成的新文本"},
            "replace_all": {"type": "boolean", "description": "true 时替换全部匹配处"}
        },
        "required": ["path", "old_string", "new_string"]
    }),
    |ft: &FileTools, args: &Value| {
        let path = arg_str(args, "path")?;
        let old_string = args
            .get("old_string")
            .and_then(Value::as_str)
            .ok_or_else(|| ToolError::InvalidArguments("缺少 old_string 参数".into()))?;
        let new_string = args
            .get("new_string")
            .and_then(Value::as_str)
            .ok_or_else(|| ToolError::InvalidArguments("缺少 new_string 参数".into()))?;
        let replace_all = args.get("replace_all").and_then(Value::as_bool).unwrap_or(false);
        exec(ft.edit_file(path, old_string, new_string, replace_all))
    }
);

file_tool!(
    SearchFiles,
    "search_files",
    "按文件名通配符（* 匹配任意序列、? 匹配单字符，大小写不敏感）在目录中递归查找文件，返回路径列表。",
    json!({
        "type": "object",
        "properties": {
            "path": {"type": "string", "description": "要搜索的目录，绝对路径或相对于文件沙箱根目录"},
            "pattern": {"type": "string", "description": "文件名通配符，如 *.txt、report_????.csv"}
        },
        "required": ["path", "pattern"]
    }),
    |ft: &FileTools, args: &Value| {
        let path = arg_str(args, "path")?;
        let pattern = arg_str(args, "pattern")?;
        exec(ft.search_files(path, pattern))
    }
);

file_tool!(
    GrepFiles,
    "grep_files",
    "用正则表达式在目录的文本文件中搜索内容，返回 文件:行号: 内容 列表（大文件与二进制自动跳过）。",
    json!({
        "type": "object",
        "properties": {
            "path": {"type": "string", "description": "要搜索的目录，绝对路径或相对于文件沙箱根目录"},
            "pattern": {"type": "string", "description": "正则表达式"},
            "max_results": {"type": "integer", "description": "最多返回多少条匹配（默认 50，上限 100）"}
        },
        "required": ["path", "pattern"]
    }),
    |ft: &FileTools, args: &Value| {
        let path = arg_str(args, "path")?;
        let pattern = arg_str(args, "pattern")?;
        let max_results = args
            .get("max_results")
            .and_then(Value::as_u64)
            .map(|n| n as usize)
            .unwrap_or(50);
        exec(ft.grep_files(path, pattern, max_results))
    }
);

/// execute_command：在本机运行 shell 命令（默认需用户弹窗确认，可 UAC 提权）。
pub struct ExecuteCommand {
    settings: SharedToolSettings,
}

impl ExecuteCommand {
    pub fn new(settings: SharedToolSettings) -> Self {
        Self { settings }
    }
}

#[async_trait]
impl Tool for ExecuteCommand {
    fn definition(&self) -> ToolDefinition {
        ToolDefinition::new(
            "execute_command",
            "在本机运行 shell 命令。执行前通常会弹窗请用户确认；uac=true 时以管理员权限运行（仅 Windows，会再弹系统 UAC 确认框）。看到输出后据此继续回答。",
            json!({
                "type": "object",
                "properties": {
                    "command": {"type": "string", "description": "要运行的 shell 命令"},
                    "cwd": {"type": "string", "description": "工作目录，绝对路径或相对于文件沙箱根目录。留空表示沙箱根目录。"},
                    "uac": {"type": "boolean", "description": "true 时请求管理员权限运行（仅 Windows，弹 UAC 确认框）"}
                },
                "required": ["command"]
            }),
        )
    }

    fn timeout_hint(&self) -> Option<Duration> {
        // 覆盖审批等待（120s）+ 命令运行时间
        Some(Duration::from_secs(300))
    }

    async fn execute(&self, context: &ToolContext, arguments: Value) -> Result<ToolResult, ToolError> {
        let app = context.require_app()?;
        let command = arg_str(&arguments, "command")?;
        let cwd = arguments.get("cwd").and_then(Value::as_str).unwrap_or("");
        let uac = arguments.get("uac").and_then(Value::as_bool).unwrap_or(false);
        let config = SkillAgentConfig::load(&app);
        let sandbox_dir = config.resolve_sandbox_dir();

        if !self.settings.get().command_auto_approve {
            let state = app.state::<AppState>();
            let request_id = command_executor::new_request_id();
            let (tx, rx) = tokio::sync::oneshot::channel::<bool>();
            state
                .chat_command_approvals
                .lock()
                .await
                .insert(request_id.clone(), ApprovalRequest { tx });
            let _ = app.emit(
                "chat:command_approval",
                json!({
                    "request_id": request_id,
                    "command": command,
                    "cwd": cwd,
                    "uac": uac,
                }),
            );
            let decision = tokio::time::timeout(Duration::from_secs(120), rx).await;
            state.chat_command_approvals.lock().await.remove(&request_id);
            match decision {
                Ok(Ok(true)) => {}
                Ok(Ok(false)) => return Err(ToolError::Execution("命令已被用户拒绝".into())),
                Ok(Err(_)) => return Err(ToolError::Execution("审批通道已关闭，命令未执行".into())),
                Err(_) => {
                    return Err(ToolError::Execution(
                        "命令审批超时（120 秒），已自动拒绝".into(),
                    ))
                }
            }
        }

        let result = if uac {
            command_executor::run_shell_command_elevated(&sandbox_dir, command, cwd).await
        } else {
            command_executor::run_shell_command(&sandbox_dir, command, cwd).await
        };
        match result {
            Ok(out) => Ok(json!({
                "ok": out.exit_code == 0,
                "exit_code": out.exit_code,
                "output": out.to_prompt_string(),
            })),
            Err(e) => Err(ToolError::Execution(e.to_string())),
        }
    }
}
