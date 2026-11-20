//! 周期小剧本草案仅在内存中流转，须由玩家确认后才会投递。

use serde::Serialize;
use serde_json::{Value, json};

use super::event_tools::{
    EXECUTABLE_EVENT_TYPES, event_tool_definition, executable_event_spec, validate_event_arguments,
};

pub const DEFAULT_INTERVAL_ROUNDS: usize = 5;
pub const MAX_INTERVAL_ROUNDS: usize = 20;
pub const MAX_DRAFT_EVENTS: usize = 12;
pub const DRAFT_MAX_ATTEMPTS: usize = 2;

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

#[derive(Debug, Clone, Default)]
pub struct DraftState {
    pub status: DraftStatus,
    pub events: Vec<Value>,
    pub error: Option<String>,
    pub created_seq: Option<u32>,
    pub save_id: Option<i32>,
    pub token: Option<u64>,
}

#[derive(Debug, Clone)]
pub struct DirectorModeState {
    pub enabled: bool,
    pub interval: usize,
    pub last_generated_seq: Option<u32>,
    pub draft: DraftState,
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

pub fn default_mode_view() -> DirectorModeView {
    DirectorModeState::default().mode_view()
}

pub fn default_draft_view() -> DirectorDraftView {
    DirectorModeState::default().draft_view()
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DirectorModeView {
    pub enabled: bool,
    pub interval: usize,
    pub last_generated_seq: Option<u32>,
    pub pending_draft: Option<DirectorDraftView>,
    pub status: String,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DirectorConfirmResult {
    pub ok: bool,
    pub applied: usize,
    pub error: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DirectorDraftView {
    pub status: String,
    pub events: Vec<Value>,
    pub error: Option<String>,
    pub created_seq: Option<u32>,
}

/// 序号回退时不触发，间隔最小按一轮计算。
pub fn should_generate(last_generated_seq: Option<u32>, current_seq: u32, interval: usize) -> bool {
    let interval = interval.max(1) as u32;
    let baseline = last_generated_seq.unwrap_or(0);
    current_seq.saturating_sub(baseline) >= interval
}

/// 任一事件不在白名单或参数非法时拒绝整份草案。
pub fn parse_draft(raw: &str) -> Result<Vec<Value>, String> {
    let text = strip_code_fence(raw);
    let value: Value = serde_json::from_str(text).map_err(|e| format!("草案不是合法 JSON：{e}"))?;
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

fn strip_code_fence(raw: &str) -> &str {
    let text = raw.trim();
    let body = text
        .strip_prefix("```json")
        .or_else(|| text.strip_prefix("```"))
        .map(|rest| rest.trim_start())
        .unwrap_or(text);
    body.strip_suffix("```").map(str::trim_end).unwrap_or(body)
}

pub fn draft_event_catalog() -> String {
    let mut out = String::from("可用事件类型（event 取值）及其参数 JSON Schema：\n");
    for &type_key in EXECUTABLE_EVENT_TYPES {
        if let Some(definition) = event_tool_definition(type_key) {
            out.push_str(&format!(
                "- {type_key}: {}\n",
                definition.function.parameters
            ));
        }
    }
    out
}

pub const DRAFT_INSTRUCTION: &str = "【周期小剧本任务】\
你是导演，请依据上面的现场与最近对话，安排接下来约 8 到 12 个舞台事件，\
让氛围、场景、音乐与旁白自然推进剧情。\
只输出一个 JSON 对象，形如 {\"events\":[{\"event\":\"narration\",\"params\":{\"text\":\"...\"}}]}，\
不要输出任何解释、代码围栏或多余文字。\
event 只能取下面列出的 8 类；params 必须满足对应事件的必填字段与类型；\
禁止生成 dialogue、set_variable 以及需要玩家交互的事件。";

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

    pub fn set_mode(&mut self, enabled: bool, interval: Option<usize>) -> Result<(), String> {
        if !enabled && self.draft.status == DraftStatus::Applying {
            return Err("草案正在投递，请等待投递结束".to_string());
        }
        if let Some(value) = interval {
            self.interval = value.clamp(1, MAX_INTERVAL_ROUNDS);
        }
        self.enabled = enabled;
        if !enabled {
            self.draft = DraftState::default();
        }
        Ok(())
    }

    /// 仅空闲且达到间隔时生成，并返回校验迟到结果的令牌。
    pub fn begin_generation(&mut self, current_seq: u32, save_id: Option<i32>) -> Option<u64> {
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
            save_id,
            token: Some(token),
        };
        Some(token)
    }

    /// 只接收当前生成令牌的结果。
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

    /// 仅允许确认当前存档的待确认草案。
    pub fn begin_confirm(&mut self, save_id: Option<i32>) -> Option<(u64, Vec<Value>)> {
        if self.draft.status != DraftStatus::Pending || self.draft.save_id != save_id {
            return None;
        }
        self.draft.status = DraftStatus::Applying;
        Some((self.draft.token?, self.draft.events.clone()))
    }

    /// 全部事件投递成功后才推进轮次基线。
    pub fn finish_confirm(
        &mut self,
        token: u64,
        current_seq: u32,
        applied: usize,
        error: Option<String>,
    ) -> bool {
        if self.draft.status != DraftStatus::Applying || self.draft.token != Some(token) {
            return false;
        }
        if let Some(error) = error {
            self.draft.status = DraftStatus::Failed;
            self.draft.error = Some(format!(
                "{error}；此前 {applied} 个事件已投递，请丢弃草案后重新生成"
            ));
            return true;
        }
        self.last_generated_seq = Some(current_seq.max(self.draft.created_seq.unwrap_or(0)));
        self.draft = DraftState::default();
        true
    }

    /// 投递过程中不能丢弃，防止收尾凭据与进度被抢先改写。
    pub fn discard(&mut self) -> Result<(), String> {
        if self.draft.status == DraftStatus::Applying {
            return Err("草案正在投递，请等待投递结束".to_string());
        }
        self.draft = DraftState::default();
        Ok(())
    }

    /// 切档时清空进度和草案，保留设置，令牌继续单调递增。
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
        assert!(!should_generate(Some(20), 3, DEFAULT_INTERVAL_ROUNDS));
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
        assert!(parse_draft(r#"{"events":[{"event":"narration","params":{}}]}"#).is_err());
        assert!(
            parse_draft(
                r#"{"events":[{"event":"music","params":{"musicPath":"a.mp3","playbackSpeed":"快"}}]}"#
            )
            .is_err()
        );
        assert!(parse_draft(r#"{"events":[]}"#).is_err());
        let many: Vec<Value> = (0..MAX_DRAFT_EVENTS + 1).map(|_| valid_event()).collect();
        assert!(parse_draft(&json!({ "events": many }).to_string()).is_err());
        assert!(parse_draft("不是 JSON").is_err());
        assert!(parse_draft(r#"{"foo":1}"#).is_err());
    }

    #[test]
    fn 状态机防重复触发且失败不推进进度() {
        let mut mode = DirectorModeState::default();
        assert!(mode.begin_generation(10, None).is_none());
        mode.set_mode(true, Some(5)).unwrap();
        assert!(mode.begin_generation(4, None).is_none());
        assert_eq!(mode.draft.status, DraftStatus::Idle);
        let token = mode.begin_generation(5, None).expect("达到间隔应能生成");
        assert_eq!(mode.draft.status, DraftStatus::Generating);
        assert_eq!(mode.draft.created_seq, Some(5));
        assert!(mode.begin_generation(6, None).is_none());
        mode.finish_generation(token + 999, Ok(vec![valid_event()]));
        assert_eq!(mode.draft.status, DraftStatus::Generating);
        mode.finish_generation(token, Err("解析失败".to_string()));
        assert_eq!(mode.draft.status, DraftStatus::Failed);
        assert!(mode.draft.error.as_deref().unwrap().contains("解析失败"));
        assert!(mode.begin_confirm(None).is_none());
        assert!(mode.begin_generation(10, None).is_none());
        mode.discard().unwrap();
        assert_eq!(mode.draft.status, DraftStatus::Idle);
        let token2 = mode
            .begin_generation(10, None)
            .expect("discard 后应能重新生成");
        assert_ne!(token, token2, "新生成的令牌必须不同");
        mode.finish_generation(token2, Ok(vec![valid_event()]));
        assert_eq!(mode.draft.status, DraftStatus::Pending);
        let view = mode.mode_view();
        assert_eq!(view.status, "pending");
        let pending = view.pending_draft.expect("pending 态应带草案对象");
        assert_eq!(pending.events.len(), 1);
        assert!(mode.begin_generation(15, None).is_none());
        let (confirm_token, events) = mode.begin_confirm(None).expect("pending 应能确认");
        assert_eq!(events.len(), 1);
        assert!(mode.begin_confirm(None).is_none());
        mode.finish_confirm(confirm_token, 5, 0, Some("执行报错".to_string()));
        assert_eq!(mode.draft.status, DraftStatus::Failed);
        assert_eq!(mode.last_generated_seq, None);
        assert!(mode.begin_confirm(None).is_none());
    }

    #[test]
    fn 全部事件确认成功才更新已生成序号() {
        let mut mode = DirectorModeState::default();
        mode.set_mode(true, Some(5)).unwrap();
        let token = mode.begin_generation(7, None).expect("应能生成");
        mode.finish_generation(token, Ok(vec![valid_event(), valid_event()]));
        let (confirm_token, _) = mode.begin_confirm(None).expect("pending 应能确认");
        assert!(mode.finish_confirm(confirm_token, 8, 2, None));
        assert_eq!(mode.last_generated_seq, Some(8));
        assert_eq!(mode.draft.status, DraftStatus::Idle);
        assert!(mode.draft.events.is_empty());
        mode.set_mode(false, None).unwrap();
        assert_eq!(mode.last_generated_seq, Some(8));
        assert_eq!(mode.draft.status, DraftStatus::Idle);
    }

    #[test]
    fn 确认中不能丢弃且迟到收尾无效() {
        let mut mode = DirectorModeState::default();
        mode.set_mode(true, Some(5)).unwrap();
        let token = mode.begin_generation(5, Some(7)).expect("应能生成");
        mode.finish_generation(token, Ok(vec![valid_event()]));
        assert!(mode.begin_confirm(Some(8)).is_none());
        let (confirm_token, _) = mode.begin_confirm(Some(7)).expect("原存档应能确认");
        assert!(mode.discard().is_err());
        assert!(mode.set_mode(false, None).is_err());
        assert_eq!(mode.draft.status, DraftStatus::Applying);
        assert!(mode.enabled);
        assert!(!mode.finish_confirm(token.wrapping_add(1), 6, 1, None));
        assert_eq!(mode.last_generated_seq, None);
        assert!(mode.finish_confirm(confirm_token, 6, 1, None));
        assert_eq!(mode.last_generated_seq, Some(6));
        assert_eq!(mode.draft.status, DraftStatus::Idle);
    }

    #[test]
    fn 间隔与界面使用同一上限() {
        let mut mode = DirectorModeState::default();
        mode.set_mode(true, Some(100)).unwrap();
        assert_eq!(mode.interval, MAX_INTERVAL_ROUNDS);
        mode.set_mode(true, Some(0)).unwrap();
        assert_eq!(mode.interval, 1);
    }

    #[test]
    fn 切档重置进度且旧任务令牌失效() {
        let mut mode = DirectorModeState::default();
        mode.set_mode(true, Some(3)).unwrap();
        let token = mode.begin_generation(6, None).expect("应能生成");
        assert_eq!(mode.draft.status, DraftStatus::Generating);
        mode.reset_progress();
        assert_eq!(mode.last_generated_seq, None);
        assert_eq!(mode.draft.status, DraftStatus::Idle);
        assert!(mode.enabled);
        assert_eq!(mode.interval, 3);
        mode.finish_generation(token, Ok(vec![valid_event()]));
        assert_eq!(mode.draft.status, DraftStatus::Idle);
        assert_eq!(mode.last_generated_seq, None);
        assert!(mode.begin_generation(2, None).is_none());
        assert!(mode.begin_generation(3, None).is_some());
    }
}
