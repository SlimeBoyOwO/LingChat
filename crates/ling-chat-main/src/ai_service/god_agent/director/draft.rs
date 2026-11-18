//! 导演周期小剧本草案（D4）：轮次判定、草案校验、内存状态机。
//!
//! 状态只存活在 DirectorSession 里，不进 DB；切档时草案与轮次进度作废。
//! 草案由玩家预览确认后，才复用 event_tools 的执行路径 emit 事件，绝不自动播放。
//! 本文件的状态转移都是纯函数，便于单测覆盖防重、失败回退与进度推进。

use serde::Serialize;
use serde_json::{Value, json};

use super::event_tools::{
    EXECUTABLE_EVENT_TYPES, event_tool_definition, executable_event_spec,
    validate_event_arguments,
};

/// 默认每 5 轮主对话生成一次草案。
pub const DEFAULT_INTERVAL_ROUNDS: usize = 5;
/// 单次草案最多 12 个事件（D4 约定约 8 到 12 个），防止脏数据一次塞进太多。
pub const MAX_DRAFT_EVENTS: usize = 12;
/// 生成失败时的最大尝试次数；仍失败就落 failed 状态并给出明确错误。
pub const DRAFT_MAX_ATTEMPTS: usize = 2;

/// 草案状态。前端按 pending 展示预览与确认按钮。
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum DraftStatus {
    #[default]
    Idle,
    Generating,
    Pending,
    Applying,
    Failed,
}

impl DraftStatus {
    pub fn as_str(self) -> &'static str {
        match self {
            DraftStatus::Idle => "idle",
            DraftStatus::Generating => "generating",
            DraftStatus::Pending => "pending",
            DraftStatus::Applying => "applying",
            DraftStatus::Failed => "failed",
        }
    }
}

/// 一份草案的内存状态。
#[derive(Debug, Clone, Default)]
pub struct DraftState {
    pub status: DraftStatus,
    /// 已通过白名单与参数校验的事件，形如 {"event":"narration","params":{...}}。
    pub events: Vec<Value>,
    pub error: Option<String>,
    /// 触发时的用户消息序号，用于确认成功后推进 last_generated_seq。
    pub created_seq: Option<u32>,
    /// 生成令牌：每次占住生成位自增，切档 / discard 后旧任务凭此被静默丢弃。
    pub token: Option<u64>,
}

/// 导演模式开关、轮次进度与草案，整体挂在 DirectorSession 上。
#[derive(Debug, Clone)]
pub struct DirectorModeState {
    pub enabled: bool,
    pub interval: usize,
    pub last_generated_seq: Option<u32>,
    pub draft: DraftState,
    /// 单调递增的生成令牌分配器。
    next_token: u64,
}

impl Default for DirectorModeState {
    fn default() -> Self {
        Self {
            enabled: false,
            interval: DEFAULT_INTERVAL_ROUNDS,
            last_generated_seq: None,
            draft: DraftState::default(),
            next_token: 0,
        }
    }
}

/// 导演服务未配置时的默认模式视图（前端只显示关闭态）。
pub fn default_mode_view() -> DirectorModeView {
    DirectorModeState::default().mode_view()
}

/// 导演服务未配置时的默认草案视图（idle 空态）。
pub fn default_draft_view() -> DirectorDraftView {
    DirectorModeState::default().draft_view()
}

/// director_get_mode 的返回，字段按前端约定转 camelCase。
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DirectorModeView {
    pub enabled: bool,
    pub interval: usize,
    pub last_generated_seq: Option<u32>,
    /// 有待确认或处理中的草案时给对象，idle 无草案时给 null。
    pub pending_draft: Option<DirectorDraftView>,
    /// 草案状态字符串，前端据此区分生成中与失败。
    pub status: String,
}

/// director_confirm_draft 的返回：前端结果条据此显示成功/失败与已投递数。
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DirectorConfirmResult {
    pub ok: bool,
    pub applied: usize,
    pub error: Option<String>,
}

/// director_get_draft 的返回。
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DirectorDraftView {
    pub status: String,
    pub events: Vec<Value>,
    pub error: Option<String>,
    pub created_seq: Option<u32>,
}

/// 距上次生成是否已过 interval 轮。从未生成时以 0 为基准；
/// 回溯或切档导致 current 小于 last 时不触发，避免重复生成。
pub fn should_generate(last_generated_seq: Option<u32>, current_seq: u32, interval: usize) -> bool {
    let interval = interval.max(1) as u32;
    let baseline = last_generated_seq.unwrap_or(0);
    current_seq.saturating_sub(baseline) >= interval
}

/// 解析模型输出的草案并逐项校验：事件类型必须在 8 类白名单内，
/// 参数必须通过 event_tools 的 validate_event_arguments。任何脏数据整份拒绝。
pub fn parse_draft(raw: &str) -> Result<Vec<Value>, String> {
    let text = strip_code_fence(raw);
    let value: Value =
        serde_json::from_str(text).map_err(|e| format!("草案不是合法 JSON：{e}"))?;
    let events = value
        .get("events")
        .and_then(Value::as_array)
        .ok_or_else(|| "草案缺少 events 数组".to_string())?;
    if events.is_empty() {
        return Err("草案 events 不能为空".to_string());
    }
    let count = events.len();
    if count > MAX_DRAFT_EVENTS {
        return Err(format!("草案事件数 {count} 超过上限 {MAX_DRAFT_EVENTS}"));
    }

    let mut out = Vec::with_capacity(count);
    for (index, item) in events.iter().enumerate() {
        let no = index + 1;
        let type_key = item
            .get("event")
            .and_then(Value::as_str)
            .ok_or_else(|| format!("第 {no} 个事件缺少 event 字段"))?
            .to_string();
        if !EXECUTABLE_EVENT_TYPES.contains(&type_key.as_str()) {
            return Err(format!("第 {no} 个事件类型 {type_key} 不在可生成白名单内"));
        }
        let params = item
            .get("params")
            .ok_or_else(|| format!("第 {no} 个事件 {type_key} 缺少 params"))?
            .clone();
        let spec = executable_event_spec(&type_key)
            .ok_or_else(|| format!("事件 {type_key} 在 schema 里没有可执行定义"))?;
        validate_event_arguments(&spec, &params)
            .map_err(|e| format!("第 {no} 个事件 {type_key} 参数校验失败：{e}"))?;
        out.push(json!({ "event": type_key, "params": params }));
    }
    Ok(out)
}

/// 容忍模型把 JSON 包在代码围栏里。
fn strip_code_fence(raw: &str) -> &str {
    let text = raw.trim();
    let body = text
        .strip_prefix("```json")
        .or_else(|| text.strip_prefix("```"))
        .map(|rest| rest.trim_start())
        .unwrap_or(text);
    body.strip_suffix("```")
        .map(str::trim_end)
        .unwrap_or(body)
}

/// 生成请求里给模型的字段目录：直接复用 schema 生成的工具定义，不另抄字段表。
pub fn draft_event_catalog() -> String {
    let mut out = String::from("可用事件类型（event 取值）及其参数 JSON Schema：\n");
    for &type_key in EXECUTABLE_EVENT_TYPES {
        if let Some(definition) = event_tool_definition(type_key) {
            out.push_str(&format!("- {type_key}: {}\n", definition.function.parameters));
        }
    }
    out
}

/// 生成草案时追加到现场 system 之后的指令。
pub const DRAFT_INSTRUCTION: &str = "【周期小剧本任务】\
你是导演，请依据上面的现场与最近对话，安排接下来约 8 到 12 个舞台事件，\
让氛围、场景、音乐与旁白自然推进剧情。\
只输出一个 JSON 对象，形如 {\"events\":[{\"event\":\"narration\",\"params\":{\"text\":\"...\"}}]}，\
不要输出任何解释、代码围栏或多余文字。\
event 只能取下面列出的 8 类；params 必须满足对应事件的必填字段与类型；\
禁止生成 dialogue、set_variable 以及需要玩家交互的事件。";

/// 生成草案的 user 提示。
pub const DRAFT_USER_PROMPT: &str = "请现在输出这份小剧本草案的 JSON。";

impl DirectorModeState {
    pub fn mode_view(&self) -> DirectorModeView {
        let has_draft = self.draft.status != DraftStatus::Idle;
        DirectorModeView {
            enabled: self.enabled,
            interval: self.interval,
            last_generated_seq: self.last_generated_seq,
            pending_draft: has_draft.then(|| self.draft_view()),
            status: self.draft.status.as_str().to_string(),
        }
    }

    pub fn draft_view(&self) -> DirectorDraftView {
        DirectorDraftView {
            status: self.draft.status.as_str().to_string(),
            events: self.draft.events.clone(),
            error: self.draft.error.clone(),
            created_seq: self.draft.created_seq,
        }
    }

    pub fn set_mode(&mut self, enabled: bool, interval: Option<usize>) {
        self.enabled = enabled;
        if let Some(value) = interval {
            self.interval = value.max(1);
        }
        // 关掉模式时生成中与待确认的草案一并作废；开启时不动现有草案。
        if !enabled {
            self.draft = DraftState::default();
        }
    }

    /// 原子地占住生成位：只有开启、idle 且达到间隔才成功，天然防重复触发。
    /// 返回生成令牌，调用方另起后台生成，结束时凭令牌写回状态。
    pub fn begin_generation(&mut self, current_seq: u32) -> Option<u64> {
        if !self.enabled || self.draft.status != DraftStatus::Idle {
            return None;
        }
        if !should_generate(self.last_generated_seq, current_seq, self.interval) {
            return None;
        }
        self.next_token += 1;
        let token = self.next_token;
        self.draft = DraftState {
            status: DraftStatus::Generating,
            events: Vec::new(),
            error: None,
            created_seq: Some(current_seq),
            token: Some(token),
        };
        Some(token)
    }

    /// 生成结束（含重试后仍失败）写回状态。令牌不匹配、已被 discard 或模式已关时
    /// 静默丢弃，防止迟到的旧任务覆盖新状态。
    pub fn finish_generation(&mut self, token: u64, result: Result<Vec<Value>, String>) {
        if self.draft.status != DraftStatus::Generating
            || self.draft.token != Some(token)
            || !self.enabled
        {
            return;
        }
        match result {
            Ok(events) => {
                self.draft.status = DraftStatus::Pending;
                self.draft.events = events;
                self.draft.error = None;
            },
            Err(error) => {
                self.draft.status = DraftStatus::Failed;
                self.draft.events.clear();
                self.draft.error = Some(error);
            },
        }
    }

    /// 玩家确认：取走待执行事件并置 applying；非 pending 返回 None。
    pub fn begin_confirm(&mut self) -> Option<Vec<Value>> {
        if self.draft.status != DraftStatus::Pending {
            return None;
        }
        self.draft.status = DraftStatus::Applying;
        Some(self.draft.events.clone())
    }

    /// 全部事件执行成功才推进 last_generated_seq；失败落 failed。
    /// 已投递的事件不可回滚，因此不允许再次确认，只能 discard 后重新生成。
    pub fn finish_confirm(&mut self, applied: usize, error: Option<String>) {
        if let Some(error) = error {
            self.draft.status = DraftStatus::Failed;
            self.draft.error = Some(format!(
                "第 {} 个事件执行失败：{error}；此前 {applied} 个事件已投递，请丢弃草案后重新生成",
                applied + 1
            ));
            return;
        }
        self.last_generated_seq = self.draft.created_seq;
        self.draft = DraftState::default();
    }

    /// 丢弃草案回到 idle，不清 last_generated_seq，下一轮仍按原进度判定。
    pub fn discard(&mut self) {
        self.draft = DraftState::default();
    }

    /// 切档：旧存档的轮次进度与草案全部作废，用户开关与间隔偏好保留。
    /// next_token 不重置，确保切档前起飞的后台任务令牌永不匹配新状态。
    pub fn reset_progress(&mut self) {
        self.last_generated_seq = None;
        self.draft = DraftState::default();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn valid_event() -> Value {
        json!({ "event": "narration", "params": { "text": "夜色渐深" } })
    }

    #[test]
    fn 轮次差值达到间隔才触发() {
        assert!(!should_generate(None, 4, DEFAULT_INTERVAL_ROUNDS));
        assert!(should_generate(None, 5, DEFAULT_INTERVAL_ROUNDS));
        assert!(!should_generate(Some(5), 9, DEFAULT_INTERVAL_ROUNDS));
        assert!(should_generate(Some(5), 10, DEFAULT_INTERVAL_ROUNDS));
        // 回溯后当前序号小于上次生成序号：不得触发
        assert!(!should_generate(Some(20), 3, DEFAULT_INTERVAL_ROUNDS));
        // interval 为 0 兜底成 1，避免每轮立即刷草案
        assert!(should_generate(None, 1, 0));
        assert!(!should_generate(Some(7), 7, 0));
    }

    #[test]
    fn 合法草案解析后保留事件与参数() {
        let raw = json!({"events":[
            {"event":"narration","params":{"text":"风起了"}},
            {"event":"music","params":{"musicPath":"a.mp3","playbackSpeed":1.0}}
        ]})
        .to_string();
        let events = parse_draft(&raw).expect("合法草案应通过");
        assert_eq!(events.len(), 2);
        assert_eq!(events[0]["event"], "narration");
        assert_eq!(events[1]["params"]["musicPath"], "a.mp3");
        // 代码围栏同样能解析
        let fenced = format!("```json\n{raw}\n```");
        assert_eq!(parse_draft(&fenced).expect("围栏 JSON 应通过").len(), 2);
    }

    #[test]
    fn 白名单外事件与非法参数整份拒绝() {
        for bad in [
            "dialogue",
            "set_variable",
            "choices",
            "director_no_such_event",
        ] {
            let raw = json!({"events":[{"event":bad,"params":{}}]}).to_string();
            assert!(parse_draft(&raw).is_err(), "{bad} 不应通过");
        }
        // 缺必填字段
        assert!(parse_draft(r#"{"events":[{"event":"narration","params":{}}]}"#).is_err());
        // 字段类型错误
        assert!(
            parse_draft(
                r#"{"events":[{"event":"music","params":{"musicPath":"a.mp3","playbackSpeed":"快"}}]}"#
            )
            .is_err()
        );
        // 空数组、超上限、非 JSON、缺 events
        assert!(parse_draft(r#"{"events":[]}"#).is_err());
        let many: Vec<Value> = (0..MAX_DRAFT_EVENTS + 1).map(|_| valid_event()).collect();
        assert!(parse_draft(&json!({ "events": many }).to_string()).is_err());
        assert!(parse_draft("不是 JSON").is_err());
        assert!(parse_draft(r#"{"foo":1}"#).is_err());
    }

    #[test]
    fn 状态机防重复触发且失败不推进进度() {
        let mut mode = DirectorModeState::default();
        // 未开启不触发
        assert!(mode.begin_generation(10).is_none());
        mode.set_mode(true, Some(5));
        // 未达间隔不触发
        assert!(mode.begin_generation(4).is_none());
        assert_eq!(mode.draft.status, DraftStatus::Idle);
        // 达到间隔：占住生成位并拿到令牌
        let token = mode.begin_generation(5).expect("达到间隔应能生成");
        assert_eq!(mode.draft.status, DraftStatus::Generating);
        assert_eq!(mode.draft.created_seq, Some(5));
        // 生成中重复触发被挡
        assert!(mode.begin_generation(6).is_none());
        // 令牌不匹配的迟到结果不得覆盖
        mode.finish_generation(token + 999, Ok(vec![valid_event()]));
        assert_eq!(mode.draft.status, DraftStatus::Generating);
        // 生成失败落 failed 并带明确错误
        mode.finish_generation(token, Err("解析失败".to_string()));
        assert_eq!(mode.draft.status, DraftStatus::Failed);
        assert!(mode.draft.error.as_deref().unwrap().contains("解析失败"));
        // failed 不能确认，也不能自动重触发
        assert!(mode.begin_confirm().is_none());
        assert!(mode.begin_generation(10).is_none());
        // discard 回 idle 后可重新生成
        mode.discard();
        assert_eq!(mode.draft.status, DraftStatus::Idle);
        let token2 = mode.begin_generation(10).expect("discard 后应能重新生成");
        assert_ne!(token, token2, "新生成的令牌必须不同");
        mode.finish_generation(token2, Ok(vec![valid_event()]));
        assert_eq!(mode.draft.status, DraftStatus::Pending);
        let view = mode.mode_view();
        assert_eq!(view.status, "pending");
        let pending = view.pending_draft.expect("pending 态应带草案对象");
        assert_eq!(pending.events.len(), 1);
        // pending 期间重复触发仍被挡
        assert!(mode.begin_generation(15).is_none());
        // 确认：部分失败不推进进度
        let events = mode.begin_confirm().expect("pending 应能确认");
        assert_eq!(events.len(), 1);
        mode.finish_confirm(0, Some("执行报错".to_string()));
        assert_eq!(mode.draft.status, DraftStatus::Failed);
        assert_eq!(mode.last_generated_seq, None);
        assert!(mode.begin_confirm().is_none());
    }

    #[test]
    fn 全部事件确认成功才更新已生成序号() {
        let mut mode = DirectorModeState::default();
        mode.set_mode(true, Some(5));
        let token = mode.begin_generation(7).expect("应能生成");
        mode.finish_generation(token, Ok(vec![valid_event(), valid_event()]));
        assert!(mode.begin_confirm().is_some());
        mode.finish_confirm(2, None);
        assert_eq!(mode.last_generated_seq, Some(7));
        assert_eq!(mode.draft.status, DraftStatus::Idle);
        assert!(mode.draft.events.is_empty());
        // 关模式作废草案但不丢轮次进度
        mode.set_mode(false, None);
        assert_eq!(mode.last_generated_seq, Some(7));
        assert_eq!(mode.draft.status, DraftStatus::Idle);
    }

    #[test]
    fn 切档重置进度且旧任务令牌失效() {
        let mut mode = DirectorModeState::default();
        mode.set_mode(true, Some(3));
        let token = mode.begin_generation(6).expect("应能生成");
        assert_eq!(mode.draft.status, DraftStatus::Generating);
        // 切档：进度与草案作废，偏好保留
        mode.reset_progress();
        assert_eq!(mode.last_generated_seq, None);
        assert_eq!(mode.draft.status, DraftStatus::Idle);
        assert!(mode.enabled);
        assert_eq!(mode.interval, 3);
        // 旧任务迟到结果不得写回
        mode.finish_generation(token, Ok(vec![valid_event()]));
        assert_eq!(mode.draft.status, DraftStatus::Idle);
        assert_eq!(mode.last_generated_seq, None);
        // 新档从 0 起算，达到间隔后仍可生成
        assert!(mode.begin_generation(2).is_none());
        assert!(mode.begin_generation(3).is_some());
    }
}
