//! 插件发起的后台导演任务：一次提示词加一个提交工具，在后台跑工具循环。
//!
//! 任务内容由插件提供：system prompt、数据载荷、提交工具的名称与参数 schema 都随
//! 请求传来，宿主不解释具体领域。宿主只负责运行、限时、存档失效与信号通知。

use std::sync::Arc;
use std::time::Duration;

use async_trait::async_trait;
use chrono::Local;
use serde::Serialize;
use serde_json::{Value, json};
use tauri::AppHandle;
use tokio::sync::{Mutex, watch};

use crate::ai_service::game_system::game_status::GameStatus;
use crate::ai_service::god_agent::core::{GodAgentCore, tail_lines};
use crate::ai_service::llm::{LlmClient, slot_snapshot};
use crate::ai_service::tools::agent::{AgentTask, run_tool_agent};
use crate::ai_service::tools::executor::{Tool, ToolContext, ToolError, ToolResult};
use crate::ai_service::tools::registry::ToolRegistry;
use crate::ai_service::types::{GameLine, ToolDefinition};
use crate::db::entities::line::LineAttribute;

/// 单次导演任务的总时限。超时只通知失败，不自动重试。
pub const TASK_TIMEOUT: Duration = Duration::from_secs(300);

/// 任务说明的字符上限。
const SYSTEM_CHAR_LIMIT: usize = 8_000;
/// 数据载荷的字符上限。
const PAYLOAD_CHAR_LIMIT: usize = 32_000;
/// 最近对话进入载荷前的字符上限。
const RECENT_CHAR_LIMIT: usize = 4_000;
/// 提交工具名的字符上限。
const TOOL_NAME_LIMIT: usize = 64;
/// 提交工具参数 schema 的序列化上限。
const TOOL_PARAMS_BYTE_LIMIT: usize = 16 * 1024;
/// 结果序列化后的字节上限。
const RESULT_BYTE_LIMIT: usize = 2 * 1024 * 1024;
/// 摘要字符上限。
const SUMMARY_CHAR_LIMIT: usize = 240;
/// 导演最多跑几轮工具调用。
const MAX_ROUNDS: usize = 6;

/// 任务状态。序列化为 `running` / `done` / `failed`。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum TaskStatus {
    Running,
    Done,
    Failed,
}

impl TaskStatus {
    fn terminal(self) -> bool {
        !matches!(self, Self::Running)
    }
}

/// 给插件看的任务快照。
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TaskSnapshot {
    pub task_id: String,
    pub kind: String,
    pub status: TaskStatus,
    pub summary: Option<String>,
    pub error: Option<String>,
    pub has_result: bool,
    pub started_at: String,
    pub finished_at: Option<String>,
}

/// 任务成功结束时的产物。`summary` 给插件做短摘要，`result` 是插件要取走的结果。
pub struct TaskOutcome {
    pub summary: Option<String>,
    pub result: Value,
}

/// 插件取走的结果。
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TaskResult {
    pub task_id: String,
    pub kind: String,
    pub summary: Option<String>,
    pub result: Value,
}

/// 校验并规范化任务类型。它是给任务贴的标签，宿主不据此分派逻辑。
pub fn normalize_kind(raw: &str) -> Option<String> {
    let kind = raw.trim().to_ascii_lowercase();
    if kind.is_empty() || kind.len() > 32 {
        return None;
    }
    if !kind
        .chars()
        .all(|ch| ch.is_ascii_alphanumeric() || ch == '_' || ch == '-')
    {
        return None;
    }
    Some(kind)
}

/// 插件带来的任务说明、载荷与提交工具定义。
struct Prepared {
    system_prompt: String,
    payload: String,
    include_recent: bool,
    submit: SubmitSpec,
}

#[derive(Clone)]
struct SubmitSpec {
    tool: String,
    description: String,
    parameters: Value,
    result_field: String,
    summary_field: Option<String>,
}

fn prepare(request: &Value) -> Result<Prepared, String> {
    if !request.is_object() {
        return Err("导演任务请求必须是 JSON object".to_string());
    }

    let system_prompt = text_field(request, "system_prompt", SYSTEM_CHAR_LIMIT)?;
    let payload = text_field(request, "prompt", PAYLOAD_CHAR_LIMIT)?;
    let include_recent = request
        .get("include_recent")
        .and_then(Value::as_bool)
        .unwrap_or(true);
    let submit = request
        .get("submit")
        .ok_or_else(|| "缺少提交工具 submit".to_string())
        .and_then(prepare_submit)?;

    Ok(Prepared {
        system_prompt,
        payload,
        include_recent,
        submit,
    })
}

fn prepare_submit(raw: &Value) -> Result<SubmitSpec, String> {
    if !raw.is_object() {
        return Err("submit 必须是 JSON object".to_string());
    }

    let tool = text_field(raw, "tool", TOOL_NAME_LIMIT)?;
    if !tool
        .chars()
        .all(|ch| ch.is_ascii_alphanumeric() || ch == '_' || ch == '-')
    {
        return Err("submit.tool 只能包含字母、数字、下划线与连字符".to_string());
    }
    let description = text_field(raw, "description", 2_000)?;
    let parameters = raw
        .get("parameters")
        .filter(|value| value.is_object())
        .cloned()
        .ok_or_else(|| "submit.parameters 必须是 JSON object".to_string())?;
    let size = serde_json::to_vec(&parameters)
        .map(|bytes| bytes.len())
        .unwrap_or(0);
    if size > TOOL_PARAMS_BYTE_LIMIT {
        return Err(format!(
            "submit.parameters 过大（{size} 字节，上限 {TOOL_PARAMS_BYTE_LIMIT} 字节）"
        ));
    }
    let result_field = text_field(raw, "result_field", TOOL_NAME_LIMIT)?;
    let summary_field = raw
        .get("summary_field")
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|field| !field.is_empty())
        .map(str::to_string);

    Ok(SubmitSpec {
        tool,
        description,
        parameters,
        result_field,
        summary_field,
    })
}

fn text_field(raw: &Value, key: &str, limit: usize) -> Result<String, String> {
    let text = raw
        .get(key)
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|text| !text.is_empty())
        .ok_or_else(|| format!("缺少 {key}"))?;
    if text.chars().count() > limit {
        return Err(format!("{key} 过长（上限 {limit} 字）"));
    }
    Ok(text.to_string())
}

struct Entry {
    task_id: String,
    kind: String,
    plugin_id: String,
    status: TaskStatus,
    summary: Option<String>,
    error: Option<String>,
    result: Option<Value>,
    started_at: String,
    finished_at: Option<String>,
    done: watch::Sender<bool>,
    /// 留一个接收端，保证任务在没有任何等待者时也能把终态写进 watch。
    _keepalive: watch::Receiver<bool>,
}

impl Entry {
    fn matches(&self, plugin_id: &str, task_id: Option<&str>) -> bool {
        self.plugin_id == plugin_id && task_id.map_or(true, |id| self.task_id == id)
    }

    fn snapshot(&self) -> TaskSnapshot {
        TaskSnapshot {
            task_id: self.task_id.clone(),
            kind: self.kind.clone(),
            status: self.status,
            summary: self.summary.clone(),
            error: self.error.clone(),
            has_result: self.result.is_some(),
            started_at: self.started_at.clone(),
            finished_at: self.finished_at.clone(),
        }
    }
}

/// 导演任务槽位。同一时间只跑一个任务，完成后结果留在槽位里等插件取走。
#[derive(Default)]
pub struct DirectorTaskManager {
    slot: Mutex<Option<Entry>>,
}

impl DirectorTaskManager {
    pub fn new() -> Self {
        Self::default()
    }

    /// 占位并生成任务 id；已有任务在跑时拒绝。
    async fn begin(&self, plugin_id: &str, kind: &str) -> Result<String, String> {
        let mut slot = self.slot.lock().await;
        if let Some(entry) = slot.as_ref()
            && !entry.status.terminal()
        {
            return Err("已有导演任务在生成，请稍后再试".to_string());
        }

        let task_id = format!("{kind}-{}", uuid::Uuid::new_v4().simple());
        let (done, keepalive) = watch::channel(false);
        *slot = Some(Entry {
            task_id: task_id.clone(),
            kind: kind.to_string(),
            plugin_id: plugin_id.to_string(),
            status: TaskStatus::Running,
            summary: None,
            error: None,
            result: None,
            started_at: Local::now().to_rfc3339(),
            finished_at: None,
            done,
            _keepalive: keepalive,
        });
        Ok(task_id)
    }

    /// 等任务进入终态。任务被新任务顶掉或不存在时直接返回。
    pub async fn wait_done(&self, task_id: &str) {
        loop {
            let (finished, mut rx) = {
                let slot = self.slot.lock().await;
                let Some(entry) = slot.as_ref().filter(|entry| entry.task_id == task_id) else {
                    return;
                };
                (entry.status.terminal(), entry.done.subscribe())
            };
            if finished {
                return;
            }
            if rx.changed().await.is_err() {
                return;
            }
        }
    }

    pub async fn snapshot(&self, plugin_id: &str, task_id: Option<&str>) -> Option<TaskSnapshot> {
        let slot = self.slot.lock().await;
        slot.as_ref()
            .filter(|entry| entry.matches(plugin_id, task_id))
            .map(Entry::snapshot)
    }

    /// 取走结果。只有已完成、且结果还在时才有返回值。
    pub async fn take_result(&self, plugin_id: &str, task_id: Option<&str>) -> Option<TaskResult> {
        let mut slot = self.slot.lock().await;
        let entry = slot.as_mut()?;
        if !entry.matches(plugin_id, task_id) || entry.status != TaskStatus::Done {
            return None;
        }
        Some(TaskResult {
            task_id: entry.task_id.clone(),
            kind: entry.kind.clone(),
            summary: entry.summary.clone(),
            result: entry.result.take()?,
        })
    }

    async fn finish(&self, task_id: &str, outcome: Result<TaskOutcome, String>) {
        let mut slot = self.slot.lock().await;
        let Some(entry) = slot.as_mut() else {
            return;
        };
        if entry.task_id != task_id {
            return;
        }
        match outcome {
            Ok(outcome) => {
                entry.status = TaskStatus::Done;
                entry.summary = outcome.summary;
                entry.result = Some(outcome.result);
            },
            Err(error) => {
                entry.status = TaskStatus::Failed;
                entry.error = Some(error);
            },
        }
        entry.finished_at = Some(Local::now().to_rfc3339());
        tracing::info!(task = %task_id, status = ?entry.status, "导演后台任务结束");
        let _ = entry.done.send(true);
    }
}

/// 校验请求、占住槽位并在后台跑一次导演任务。返回任务 id，调用方立刻返回。
pub async fn start(
    core: Arc<GodAgentCore>,
    app: AppHandle,
    plugin_id: String,
    kind: String,
    request: Value,
) -> Result<String, String> {
    let kind = normalize_kind(&kind).ok_or_else(|| "任务类型不合法".to_string())?;
    let llm = slot_snapshot(&core.llm)
        .await
        .ok_or_else(|| "上帝Agent 没有可用的模型".to_string())?;
    let prepared = prepare(&request)?;
    let (recent, save_id) = {
        let game_status = crate::ai_service::tools::game_status_handle(&app).await;
        let gs = game_status.lock().await;
        let recent = if prepared.include_recent {
            recent_dialogue(&gs, core.config_snapshot().recent_window)
        } else {
            String::new()
        };
        (recent, gs.active_save_id)
    };

    let task_id = core.tasks.begin(&plugin_id, &kind).await?;
    tracing::info!(task = %task_id, kind = %kind, plugin = %plugin_id, "导演后台任务已受理");
    let core_for_task = core.clone();
    let app_for_task = app.clone();
    let task_id_for_task = task_id.clone();
    tauri::async_runtime::spawn(async move {
        let outcome =
            tokio::time::timeout(TASK_TIMEOUT, run(&llm, &app_for_task, prepared, recent)).await;
        let outcome = match outcome {
            Ok(result) => result,
            Err(_) => Err(format!(
                "导演任务超过 {} 秒没有完成",
                TASK_TIMEOUT.as_secs()
            )),
        };
        let outcome = match outcome {
            Ok(_) if save_changed(&app_for_task, save_id).await => {
                Err("存档已经切换，任务作废".to_string())
            },
            other => other,
        };
        core_for_task.tasks.finish(&task_id_for_task, outcome).await;
    });
    Ok(task_id)
}

/// 发起时的存档与现在不一致就作废。无档开局被自动建档不算切档。
async fn save_changed(app: &AppHandle, origin: Option<i32>) -> bool {
    let Some(origin) = origin else {
        return false;
    };
    let game_status = crate::ai_service::tools::game_status_handle(app).await;
    let current = game_status.lock().await.active_save_id;
    current != Some(origin)
}

async fn run(
    llm: &Arc<LlmClient>,
    app: &AppHandle,
    prepared: Prepared,
    recent: String,
) -> Result<TaskOutcome, String> {
    let slot = Arc::new(Mutex::new(None));
    let tool_name = prepared.submit.tool.clone();
    let payload = user_payload(&prepared, &recent);
    let system_prompt = prepared.system_prompt;
    let spec = prepared.submit;

    let registry = ToolRegistry::new();
    registry
        .register(Arc::new(SubmissionTool {
            spec,
            slot: slot.clone(),
        }))
        .map_err(|error| format!("注册提交工具失败：{error}"))?;

    let task = AgentTask {
        system_prompt,
        payload,
        tool_names: vec![tool_name],
        max_rounds: MAX_ROUNDS,
    };
    let output = run_tool_agent(llm, &registry, &task, Some(app.clone()))
        .await
        .map_err(|error| format!("导演生成失败：{error}"))?;

    let Some((summary, result)) = slot.lock().await.take() else {
        let detail = output.text.trim();
        return Err(if detail.is_empty() {
            "导演没有提交结果".to_string()
        } else {
            format!("导演没有提交结果：{}", truncate_chars(detail, 200))
        });
    };

    Ok(TaskOutcome { summary, result })
}

fn user_payload(prepared: &Prepared, recent: &str) -> String {
    let mut payload = prepared.payload.clone();
    if prepared.include_recent {
        payload.push_str("\n\n【最近对话】\n");
        if recent.trim().is_empty() {
            payload.push_str("（还没有对话）");
        } else {
            payload.push_str(recent);
        }
    }
    payload.push_str("\n\n完成任务时调用 ");
    payload.push_str(&prepared.submit.tool);
    payload.push_str(" 提交结果。");
    payload
}

/// 提交工具：参数 schema 由插件给，宿主只按字段名取出摘要与结果。
struct SubmissionTool {
    spec: SubmitSpec,
    slot: Arc<Mutex<Option<(Option<String>, Value)>>>,
}

#[async_trait]
impl Tool for SubmissionTool {
    fn definition(&self) -> ToolDefinition {
        ToolDefinition::new(
            self.spec.tool.clone(),
            self.spec.description.clone(),
            self.spec.parameters.clone(),
        )
    }

    fn lenient_arguments(&self) -> bool {
        true
    }

    async fn execute(
        &self,
        _context: &ToolContext,
        arguments: Value,
    ) -> Result<ToolResult, ToolError> {
        let summary = match self.spec.summary_field.as_deref() {
            Some(field) => {
                let text = arguments
                    .get(field)
                    .and_then(Value::as_str)
                    .map(str::trim)
                    .filter(|text| !text.is_empty())
                    .ok_or_else(|| ToolError::InvalidArguments(format!("缺少 {field}")))?;
                if text.chars().count() > SUMMARY_CHAR_LIMIT {
                    return Err(ToolError::InvalidArguments(format!(
                        "{field} 过长（上限 {SUMMARY_CHAR_LIMIT} 字）"
                    )));
                }
                Some(text.to_string())
            },
            None => None,
        };

        let result = coerce_field(arguments.get(&self.spec.result_field)).ok_or_else(|| {
            ToolError::InvalidArguments(format!(
                "缺少 {}，或它不是合法 JSON",
                self.spec.result_field
            ))
        })?;
        let size = serde_json::to_vec(&result)
            .map_err(|error| ToolError::Execution(format!("结果序列化失败：{error}")))?
            .len();
        if size > RESULT_BYTE_LIMIT {
            return Err(ToolError::Execution(format!(
                "结果过大（{size} 字节，上限 {RESULT_BYTE_LIMIT} 字节）"
            )));
        }

        let mut slot = self.slot.lock().await;
        if slot.is_some() {
            return Ok(json!({"ok": false, "error": "结果已经提交过了"}));
        }
        *slot = Some((summary, result));
        Ok(json!({"ok": true, "message": "已提交"}))
    }
}

/// 模型偶尔会把结果再编码成字符串，这里接住；其余类型原样保留。
fn coerce_field(value: Option<&Value>) -> Option<Value> {
    match value? {
        Value::Null => None,
        Value::String(text) => serde_json::from_str(text)
            .ok()
            .or_else(|| Some(Value::String(text.clone()))),
        value => Some(value.clone()),
    }
}

/// 取最近对话，由旧到新，超出预算时从最旧的一条开始丢。
fn recent_dialogue(gs: &GameStatus, window: usize) -> String {
    let lines = tail_lines(gs, window);
    let mut kept: Vec<String> = Vec::new();
    let mut used = 0usize;
    for line in lines.iter().rev() {
        let text = format_line(gs, line);
        if text.is_empty() {
            continue;
        }
        let len = text.chars().count() + 1;
        if used + len > RECENT_CHAR_LIMIT {
            break;
        }
        used += len;
        kept.push(text);
    }
    kept.reverse();
    if kept.is_empty() {
        "（还没有对话）".to_string()
    } else {
        kept.join("\n")
    }
}

fn format_line(gs: &GameStatus, line: &GameLine) -> String {
    let content = truncate_chars(&line.base.content, 200);
    match line.attribute() {
        LineAttribute::Assistant => {
            let role_id = line.sender_role_id().unwrap_or(0);
            format!("[{}] {content}", role_name(gs, role_id))
        },
        LineAttribute::User => {
            let role_id = line.sender_role_id().unwrap_or(0);
            if role_id == 0 {
                format!("[旁白] {content}")
            } else {
                let name = line
                    .base
                    .display_name
                    .clone()
                    .unwrap_or_else(|| gs.player.user_name.clone());
                format!("[{name}] {content}")
            }
        },
        LineAttribute::System | LineAttribute::Tool => String::new(),
    }
}

fn role_name(gs: &GameStatus, role_id: i32) -> String {
    if role_id == 0 {
        return gs.player.user_name.clone();
    }
    gs.role_manager
        .get_loaded(role_id)
        .map(|role| {
            role.display_name
                .clone()
                .unwrap_or_else(|| role.settings.ai_name.clone())
        })
        .unwrap_or_else(|| format!("角色{role_id}"))
}

fn truncate_chars(text: &str, limit: usize) -> String {
    if text.chars().count() <= limit {
        return text.to_string();
    }
    let mut out: String = text.chars().take(limit.saturating_sub(1)).collect();
    out.push('…');
    out
}
