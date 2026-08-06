//! 主聊天可用的技能库 / 文件沙箱 / 命令执行工具。
//!
//! 复用 skill_agent 的技能发现（`skills.rs`）、文件沙箱（`file_tools.rs`）与
//! 命令执行（`command_executor.rs`），让主对话角色也能读技能、操作文件、跑命令。
//! 文件工具默认锁定沙箱（`data/`），可通过工具配置「允许访问沙箱外路径」或
//! 「助手设置」的允许任意路径放开。
//! `execute_command` 默认每次都要用户在前端弹窗确认（`chat:command_approval`
//! 事件 + `resolve_command_approval` 回调），可在工具配置开启免确认；
//! `uac=true` 时以管理员权限运行（Windows 弹系统 UAC 框）；耗时任务可选择后台
//! 运行，任务完成后会自动触发一轮仅对模型可见的结果通知。
//! 不含 `validate_script`（剧本编辑器会话专用）。

use std::time::Duration;

use async_trait::async_trait;
use serde_json::{json, Value};
use tauri::{AppHandle, Emitter, Manager};

use crate::ai_service::skill_agent::command_executor::{self, ApprovalMap, ApprovalRequest};
use crate::ai_service::skill_agent::config::SkillAgentConfig;
use crate::ai_service::skill_agent::file_tools::{FileTools, MAX_GREP_RESULTS};
use crate::ai_service::skill_agent::skills;
use crate::ai_service::types::ToolDefinition;
use crate::AppState;

use super::background_command;
use super::executor::{Tool, ToolContext, ToolError, ToolResult};
use super::settings::SharedToolSettings;

const SKILL_TOOL_TIMEOUT: Duration = Duration::from_secs(5);
const FILE_TOOL_TIMEOUT: Duration = Duration::from_secs(15);
const DELETE_FILE_TOOL_TIMEOUT: Duration = Duration::from_secs(135);

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

async fn run_blocking<F>(work: F) -> Result<ToolResult, ToolError>
where
    F: FnOnce() -> Result<ToolResult, ToolError> + Send + 'static,
{
    tokio::task::spawn_blocking(work)
        .await
        .map_err(|error| ToolError::Execution(format!("文件工具后台任务异常: {error}")))?
}

/// 发送主聊天审批事件并等待用户决定。审批请求自身 120 秒超时，调用工具的
/// `timeout_hint` 必须留出额外清理时间。
async fn request_user_approval(
    app: &AppHandle,
    approvals: ApprovalMap,
    event: &str,
    mut payload: Value,
    action: &str,
) -> Result<(), ToolError> {
    let request_id = command_executor::new_request_id();
    let object = payload
        .as_object_mut()
        .ok_or_else(|| ToolError::Execution("审批事件载荷必须是 JSON object".into()))?;
    object.insert("request_id".into(), Value::String(request_id.clone()));

    let (tx, rx) = tokio::sync::oneshot::channel::<bool>();
    approvals
        .lock()
        .await
        .insert(request_id.clone(), ApprovalRequest { tx });
    // 审批框只挂载在主窗口。使用全局广播会让日志/截图等独立窗口也收到事件，
    // 这些窗口没有 AppDialog，回调会一直等待并最终触发 120 秒超时。
    if app.get_webview_window("main").is_none() {
        approvals.lock().await.remove(&request_id);
        return Err(ToolError::Execution(format!(
            "无法发送{action}审批请求: 主窗口不可用"
        )));
    }
    if let Err(error) = app.emit_to("main", event, payload) {
        approvals.lock().await.remove(&request_id);
        return Err(ToolError::Execution(format!(
            "无法发送{action}审批请求: {error}"
        )));
    }
    tracing::info!("[approval] 已向主窗口发送审批事件: event={event} request_id={request_id}");

    let decision = tokio::time::timeout(Duration::from_secs(120), rx).await;
    approvals.lock().await.remove(&request_id);
    match decision {
        Ok(Ok(true)) => {
            tracing::info!("[approval] 用户已批准: request_id={request_id}");
            Ok(())
        }
        Ok(Ok(false)) => Err(ToolError::Execution(format!("{action}已被用户拒绝"))),
        Ok(Err(_)) => Err(ToolError::Execution(format!(
            "审批通道已关闭，{action}未执行"
        ))),
        Err(_) => Err(ToolError::Execution(format!(
            "{action}审批超时（120 秒），已自动拒绝"
        ))),
    }
}

/// list_skills：列出技能库中全部可用技能。
pub struct ListSkills;

#[async_trait]
impl Tool for ListSkills {
    fn definition(&self) -> ToolDefinition {
        ToolDefinition::new(
            "list_skills",
            "列出所有可用技能的名称、描述与位置。",
            json!({"type": "object", "properties": {}, "additionalProperties": false}),
        )
    }

    fn timeout_hint(&self) -> Option<Duration> {
        Some(SKILL_TOOL_TIMEOUT)
    }

    async fn execute(&self, context: &ToolContext, _: Value) -> Result<ToolResult, ToolError> {
        let config = load_config(context)?;
        let skills_dir = config.resolve_skills_dir();
        let found = tokio::task::spawn_blocking(move || skills::find_all_skills(&skills_dir))
            .await
            .map_err(|error| ToolError::Execution(format!("技能扫描后台任务异常: {error}")))?;
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
                "required": ["name"],
                "additionalProperties": false
            }),
        )
    }

    fn timeout_hint(&self) -> Option<Duration> {
        Some(SKILL_TOOL_TIMEOUT)
    }

    async fn execute(
        &self,
        context: &ToolContext,
        arguments: Value,
    ) -> Result<ToolResult, ToolError> {
        let config = load_config(context)?;
        let name = arg_str(&arguments, "name")?.to_string();
        let skills_dir = config.resolve_skills_dir();
        let found = tokio::task::spawn_blocking(move || skills::find_skill(&skills_dir, &name))
            .await
            .map_err(|error| ToolError::Execution(format!("技能读取后台任务异常: {error}")))?;
        match found {
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
            None => Err(ToolError::Execution(
                "未找到技能，或技能名称/文件不安全".into(),
            )),
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

            fn timeout_hint(&self) -> Option<Duration> {
                Some(FILE_TOOL_TIMEOUT)
            }

            async fn execute(
                &self,
                context: &ToolContext,
                arguments: Value,
            ) -> Result<ToolResult, ToolError> {
                let config = load_config(context)?;
                let ft = file_tools(&config, &self.settings);
                let run: fn(&FileTools, &Value) -> Result<ToolResult, ToolError> = $body;
                run_blocking(move || run(&ft, &arguments)).await
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
        "required": ["path"],
        "additionalProperties": false
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
        "required": ["path"],
        "additionalProperties": false
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
        "required": ["path", "content"],
        "additionalProperties": false
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

/// delete_file：默认在真正删除前弹窗显示解析后的目标路径并等待确认。
pub struct DeleteFile {
    settings: SharedToolSettings,
}

impl DeleteFile {
    pub fn new(settings: SharedToolSettings) -> Self {
        Self { settings }
    }
}

#[async_trait]
impl Tool for DeleteFile {
    fn definition(&self) -> ToolDefinition {
        ToolDefinition::new(
            "delete_file",
            "删除一个文件。默认会先向用户显示目标路径并请求确认；用户拒绝或审批超时则不会删除。",
            json!({
                "type": "object",
                "properties": {
                    "path": {"type": "string", "description": "要删除的文件路径"}
                },
                "required": ["path"],
                "additionalProperties": false
            }),
        )
    }

    fn timeout_hint(&self) -> Option<Duration> {
        Some(DELETE_FILE_TOOL_TIMEOUT)
    }

    async fn execute(
        &self,
        context: &ToolContext,
        arguments: Value,
    ) -> Result<ToolResult, ToolError> {
        let app = context.require_app()?;
        let path = arg_str(&arguments, "path")?.to_string();
        let config = SkillAgentConfig::load(&app);
        let ft = file_tools(&config, &self.settings);

        // 审批前先做同样的沙箱/类型检查，确保弹窗展示真实且允许访问的目标。
        let checked_ft = ft.clone();
        let checked_path = path.clone();
        let display_path = tokio::task::spawn_blocking(move || {
            let target = checked_ft
                .sanitize(&checked_path)
                .map_err(|error| ToolError::Execution(error.to_string()))?;
            let metadata = std::fs::symlink_metadata(&target)
                .map_err(|_| ToolError::Execution(format!("文件不存在: {}", target.display())))?;
            if metadata.file_type().is_dir() {
                return Err(ToolError::Execution(format!(
                    "delete_file 只能删除文件，不能删除目录: {}",
                    target.display()
                )));
            }
            Ok(target.display().to_string())
        })
        .await
        .map_err(|error| ToolError::Execution(format!("删除目标检查异常: {error}")))??;

        if !self.settings.get().file_delete_auto_approve {
            let approvals = app.state::<AppState>().chat_file_delete_approvals.clone();
            request_user_approval(
                &app,
                approvals,
                "chat:file_delete_approval",
                json!({ "path": display_path }),
                "文件删除",
            )
            .await?;
        }

        run_blocking(move || exec(ft.delete_file(&path))).await
    }
}

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
        "required": ["path", "old_string", "new_string"],
        "additionalProperties": false
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
        "required": ["path", "pattern"],
        "additionalProperties": false
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
        "required": ["path", "pattern"],
        "additionalProperties": false
    }),
    |ft: &FileTools, args: &Value| {
        let path = arg_str(args, "path")?;
        let pattern = arg_str(args, "pattern")?;
        let max_results = args
            .get("max_results")
            .and_then(Value::as_u64)
            .map(|n| n.min(MAX_GREP_RESULTS as u64) as usize)
            .unwrap_or(50);
        exec(ft.grep_files(path, pattern, max_results))
    }
);

/// 保守识别常见的文件删除命令。任意 shell/程序都可能间接删除文件，因此这里
/// 优先避免漏报；误报只会多要求一次用户确认，不会改变命令内容。
fn command_may_delete_files(command: &str) -> bool {
    let normalized = command.to_ascii_lowercase();
    let tokens = normalized
        .split(|ch: char| !(ch.is_ascii_alphanumeric() || matches!(ch, '-' | '_' | '.')))
        .filter(|token| !token.is_empty())
        .collect::<Vec<_>>();

    if tokens.iter().any(|token| {
        matches!(
            *token,
            "del"
                | "del.exe"
                | "erase"
                | "erase.exe"
                | "rd"
                | "rd.exe"
                | "rmdir"
                | "rmdir.exe"
                | "rm"
                | "rm.exe"
                | "ri"
                | "unlink"
                | "unlink.exe"
                | "shred"
                | "shred.exe"
                | "sdelete"
                | "sdelete.exe"
                | "rimraf"
                | "rimraf.cmd"
                | "truncate"
                | "truncate.exe"
                | "remove-item"
                | "clear-content"
                | "-delete"
                | "--delete"
        )
    }) {
        return true;
    }

    (tokens.contains(&"git") || tokens.contains(&"git.exe"))
        && (tokens.contains(&"rm") || tokens.contains(&"clean"))
        || (tokens.contains(&"robocopy") || tokens.contains(&"robocopy.exe"))
            && tokens.contains(&"mir")
        || normalized.contains("os.remove(")
        || normalized.contains("os.unlink(")
        || normalized.contains("os.rmdir(")
        || normalized.contains("shutil.rmtree(")
        || normalized.contains(".unlink(")
        || normalized.contains("file.delete(")
        || normalized.contains("directory.delete(")
}

/// execute_command：在本机运行 shell 命令（默认需用户弹窗确认，可后台运行或 UAC 提权）。
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
        let shell_hint = if cfg!(windows) {
            "当前运行环境是 Windows，命令由 cmd.exe /D /C 执行且没有交互输入；需要 PowerShell 语法时请显式调用 powershell -NoProfile -Command，延时请使用 PowerShell Start-Sleep 而不是依赖控制台输入的 timeout。"
        } else {
            "当前命令由 sh -c 执行。"
        };
        ToolDefinition::new(
            "execute_command",
            format!(
                "在本机运行 shell 命令。{shell_hint}执行前通常会弹窗请用户确认；uac=true 时以管理员权限运行（仅 Windows，会再弹系统 UAC 确认框）。耗时任务可设 run_in_background=true 并提供 description：工具会立即返回任务 ID，完成后自动通知，无需轮询。"
            ),
            json!({
                "type": "object",
                "properties": {
                    "command": {"type": "string", "description": "要运行的 shell 命令"},
                    "cwd": {"type": "string", "description": "工作目录，绝对路径或相对于文件沙箱根目录。留空表示沙箱根目录。"},
                    "uac": {"type": "boolean", "description": "true 时请求管理员权限运行（仅 Windows，弹 UAC 确认框）"},
                    "timeout_seconds": {"type": "integer", "description": "命令最长运行秒数（前台默认 60/最大 300；后台默认 600/最大 3600；最小 1）"},
                    "run_in_background": {"type": "boolean", "description": "true 时在后台运行并立即返回任务 ID；完成后会自动通知模型，无需轮询。不能与 uac=true 同时使用"},
                    "description": {"type": "string", "description": "后台任务的简短说明；run_in_background=true 时必填"}
                },
                "required": ["command"],
                "additionalProperties": false
            }),
        )
    }

    fn timeout_hint(&self) -> Option<Duration> {
        // 覆盖审批等待（120s）+ 最大命令时间（300s）+ 清理余量。
        Some(Duration::from_secs(430))
    }

    async fn execute(
        &self,
        context: &ToolContext,
        arguments: Value,
    ) -> Result<ToolResult, ToolError> {
        let app = context.require_app()?;
        let command = arg_str(&arguments, "command")?;
        let cwd = arguments.get("cwd").and_then(Value::as_str).unwrap_or("");
        let uac = arguments
            .get("uac")
            .and_then(Value::as_bool)
            .unwrap_or(false);
        let run_in_background = arguments
            .get("run_in_background")
            .and_then(Value::as_bool)
            .unwrap_or(false);
        let description = arguments
            .get("description")
            .and_then(Value::as_str)
            .unwrap_or("")
            .trim();
        if run_in_background && description.is_empty() {
            return Err(ToolError::InvalidArguments(
                "run_in_background=true 时必须提供非空 description".into(),
            ));
        }
        if run_in_background && uac {
            return Err(ToolError::InvalidArguments(
                "后台命令不支持 uac=true；需要提权时请以前台方式执行".into(),
            ));
        }
        let (default_timeout, max_timeout) = if run_in_background {
            (
                background_command::DEFAULT_BACKGROUND_COMMAND_TIMEOUT,
                background_command::MAX_BACKGROUND_COMMAND_TIMEOUT,
            )
        } else {
            (
                command_executor::DEFAULT_COMMAND_TIMEOUT,
                command_executor::MAX_COMMAND_TIMEOUT,
            )
        };
        let timeout = Duration::from_secs(
            arguments
                .get("timeout_seconds")
                .and_then(Value::as_u64)
                .unwrap_or(default_timeout.as_secs())
                .clamp(1, max_timeout.as_secs()),
        );
        let config = SkillAgentConfig::load(&app);
        let sandbox_dir = config.resolve_sandbox_dir();
        let settings = self.settings.get();
        let is_delete_command = command_may_delete_files(command);

        if is_delete_command && !settings.command_delete_auto_approve {
            let approvals = app.state::<AppState>().chat_file_delete_approvals.clone();
            request_user_approval(
                &app,
                approvals,
                "chat:command_delete_approval",
                json!({
                    "command": command,
                    "cwd": cwd,
                    "uac": uac,
                    "run_in_background": run_in_background,
                    "description": description,
                }),
                "删除命令",
            )
            .await?;
        } else if !settings.command_auto_approve {
            let approvals = app.state::<AppState>().chat_command_approvals.clone();
            request_user_approval(
                &app,
                approvals,
                "chat:command_approval",
                json!({
                    "command": command,
                    "cwd": cwd,
                    "uac": uac,
                    "run_in_background": run_in_background,
                    "description": description,
                }),
                "命令",
            )
            .await?;
        }

        if run_in_background {
            return background_command::start_background_command(
                app,
                sandbox_dir,
                command.to_string(),
                cwd.to_string(),
                description.to_string(),
                timeout,
            )
            .await;
        }

        let result = if uac {
            command_executor::run_shell_command_elevated_with_timeout(
                &sandbox_dir,
                command,
                cwd,
                timeout,
            )
            .await
        } else {
            command_executor::run_shell_command_with_timeout(&sandbox_dir, command, cwd, timeout)
                .await
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ai_service::tools::settings::ToolSettings;

    #[test]
    fn code_tool_schemas_are_strict_openai_objects() {
        let settings = SharedToolSettings::new(ToolSettings::default());
        let tools: Vec<Box<dyn Tool>> = vec![
            Box::new(ListSkills),
            Box::new(ReadSkill),
            Box::new(ListFiles::new(settings.clone())),
            Box::new(ReadFile::new(settings.clone())),
            Box::new(WriteFile::new(settings.clone())),
            Box::new(DeleteFile::new(settings.clone())),
            Box::new(EditFile::new(settings.clone())),
            Box::new(SearchFiles::new(settings.clone())),
            Box::new(GrepFiles::new(settings.clone())),
            Box::new(ExecuteCommand::new(settings)),
        ];

        for tool in tools {
            let definition = tool.definition();
            assert_eq!(definition.type_, "function");
            assert_eq!(definition.function.parameters["type"], "object");
            assert_eq!(
                definition.function.parameters["additionalProperties"], false,
                "{} must reject unknown arguments",
                definition.function.name
            );
            assert!(definition.function.parameters["properties"].is_object());
            let expected_timeout = match definition.function.name.as_str() {
                "list_skills" | "read_skill" => SKILL_TOOL_TIMEOUT,
                "delete_file" => DELETE_FILE_TOOL_TIMEOUT,
                "execute_command" => Duration::from_secs(430),
                _ => FILE_TOOL_TIMEOUT,
            };
            assert_eq!(tool.timeout_hint(), Some(expected_timeout));
        }
    }

    #[test]
    fn command_schema_exposes_bounded_timeout() {
        let tool = ExecuteCommand::new(SharedToolSettings::new(ToolSettings::default()));
        let definition = tool.definition();
        assert_eq!(
            definition.function.parameters["properties"]["timeout_seconds"]["type"],
            "integer"
        );
        assert_eq!(
            definition.function.parameters["properties"]["run_in_background"]["type"],
            "boolean"
        );
        assert_eq!(
            definition.function.parameters["properties"]["description"]["type"],
            "string"
        );
        assert_eq!(tool.timeout_hint(), Some(Duration::from_secs(430)));
    }

    #[test]
    fn delete_file_schema_reserves_time_for_user_approval() {
        let tool = DeleteFile::new(SharedToolSettings::new(ToolSettings::default()));
        let definition = tool.definition();
        assert!(definition.function.description.contains("请求确认"));
        assert_eq!(tool.timeout_hint(), Some(DELETE_FILE_TOOL_TIMEOUT));
    }

    #[test]
    fn detects_common_file_deletion_commands() {
        for command in [
            r#"cmd /c del /q "C:\temp\old.txt""#,
            r#"powershell -NoProfile -Command "Remove-Item -LiteralPath 'C:\temp\old.txt'""#,
            "rm -rf ./build",
            "git clean -fdx",
            "find . -name '*.tmp' -delete",
            "python -c \"import shutil; shutil.rmtree('build')\"",
            "robocopy empty target /MIR",
        ] {
            assert!(command_may_delete_files(command), "not detected: {command}");
        }

        for command in [
            "Get-ChildItem -Force",
            "cargo test --lib",
            "git status --short",
            "python -c \"print('hello')\"",
        ] {
            assert!(
                !command_may_delete_files(command),
                "false positive: {command}"
            );
        }
    }
}
