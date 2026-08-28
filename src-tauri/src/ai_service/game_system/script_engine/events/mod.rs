//! 剧本事件 trait、注册表与执行上下文。
//!
//! 替代 Python 版的 `BaseEvent` 抽象类 + `EventHandlerLoader` 自动发现。
//! Rust 没有 `importlib`，所以事件处理器通过 `register_event()` 自行注册，
//! 再由 `create_event()` 按 type 查找。

// 事件处理器子模块
pub mod achievement_event;
pub mod ai_dialogue_event;
pub mod ambient_event;
pub mod background_effect_event;
pub mod background_event;
pub mod chapter_end_event;
pub mod character_file_event;
pub mod choice_event;
pub mod console_window_event;
pub mod dialog_event;
pub mod force_choice_event;
pub mod free_dialogue_event;
pub mod glitch_window_event;
pub mod horror_log_event;
pub mod input_event;
pub mod jumpscare_event;
pub mod menu_effect_event;
pub mod modify_character_event;
pub mod music_event;
pub mod narration_event;
pub mod player_event;
pub mod poem_game_event;
pub mod present_pic_event;
pub mod random_var_event;
pub mod set_variable_event;
pub mod sound_event;
pub mod voice_shift_event;
pub mod wait_event;
pub mod watch_file_event;
pub mod window_title_event;

use std::collections::HashMap;
use std::path::Path;
use std::sync::{Arc, RwLock};

use anyhow::Result;
use async_trait::async_trait;
use sea_orm::DatabaseConnection;
use serde_json::Value;
use tauri::AppHandle;
use tokio::sync::Mutex;

use crate::ai_service::config::AIServiceConfig;
use crate::ai_service::game_system::game_status::GameStatus;
use crate::ai_service::llm::LlmClient;

// ============================================================
// 剧本共享通道（剧本运行期间的用户输入/选择）
// ============================================================

/// 当前 `force_choice` 的一次性能力票据：只允许对应主窗口在短时间内移动鼠标，
/// 提交时后端还会强制核对 forced 文本，不能只依赖前端 disabled。
#[derive(Clone, Debug)]
pub struct ForceChoiceGuard {
    pub request_id: String,
    pub forced: String,
    pub warp_enabled: bool,
    pub warp_expires_at: std::time::Instant,
}

/// 剧本运行期间用于用户输入/选择的通道。
/// 存为 `Arc<Mutex<>>`，使后台任务与 Tauri 命令都能访问，而不必持有 `AIService` 的锁。
pub struct ScriptChannels {
    pub input_tx: Option<tokio::sync::oneshot::Sender<String>>,
    pub choice_tx: Option<tokio::sync::oneshot::Sender<String>>,
    /// 当前挂起的 `choices` 事件是否接受自由输入文本。
    ///
    /// 镜像正在执行的 [`choice_event::ChoiceEvent`] 的 `allow_free` 字段。
    /// `script_submit_input` 据此判断：当选项挂起时，输入框里打的字可以转投
    /// `choice_tx` 而不是被拒绝——否则选项永远无法解决，剧本永久阻塞。
    pub choice_allow_free: bool,
    pub force_choice_guard: Option<ForceChoiceGuard>,
    /// 文件监视（watch_file）：目标 .chr 消失时要跳转的章节。
    /// 章节循环在事件之间/事件报错时检查它——监视器会同时丢弃挂起的输入通道，
    /// 让阻塞中的事件立刻以 Err 返回，从而及时让位给目标章节。
    pub watch_jump: Option<String>,
    /// 监视任务句柄；新监视/停止/剧本结束时 abort。
    pub watch_task: Option<tokio::task::JoinHandle<()>>,
}

impl ScriptChannels {
    pub fn new() -> Self {
        Self {
            input_tx: None,
            choice_tx: None,
            choice_allow_free: false,
            force_choice_guard: None,
            watch_jump: None,
            watch_task: None,
        }
    }
}

pub type SharedScriptChannels = Arc<Mutex<ScriptChannels>>;

// ============================================================
// ScriptContext —— 事件处理器所需的依赖打包
// ============================================================

/// 事件处理器在执行期间所需的全部依赖。
pub struct ScriptContext<'a> {
    pub db: &'a DatabaseConnection,
    pub data_dir: &'a Path,
    pub app: &'a AppHandle,
    /// 持有的 Arc——事件按需加锁。与 AIService 的锁解耦，
    /// 这样事件能安全地调用 MessageGenerator 而不会死锁。
    pub game_status: Arc<Mutex<GameStatus>>,
    pub config: &'a AIServiceConfig,

    /// `ai_dialogue` / `free_dialogue` / `chapter_end`(ai_judged) 用的 LLM 客户端，可能为空。
    pub llm: Option<&'a Arc<LlmClient>>,

    /// 用户输入/选择事件的共享通道。
    /// 持有的 `Arc` 克隆——处理器在 await 点前后加/解锁。
    pub channels: SharedScriptChannels,

    /// 是否运行在编辑器试玩中。试玩产出的 `ai:reply` 会带 `preview_gen`
    /// 标记，前端据此丢弃中止后迟到的流式回复（见 `ReplyResponse.preview_gen`）。
    /// 正式游玩显式置 `false`。
    pub is_preview: bool,

    /// Auxiliary-window epoch captured when this exact script run starts.
    /// Teardown advances the global epoch, so late events from an old task are rejected.
    pub glitch_window_generation: u64,
}

// ============================================================
// ScriptEvent trait
// ============================================================

/// 所有剧本事件处理器的统一 trait。
///
/// 每个处理器匹配一个 YAML `type:` 字符串，并实现 `execute()`。
/// 对 chapter_end 事件返回 `Ok(Some(下一章名))`；其余返回 `Ok(None)`。
///
/// # 与 Python 版的差异说明
///
/// Python 的 `SetVariableEvent` 重写了 `execute()` 而非 `_execute()`，
/// 导致它静默失效（基类 `process()` 调用的是 `_execute()`）。
/// Rust 用单一 `execute()` 方法——没有这个 bug。
#[async_trait]
pub trait ScriptEvent: Send {
    /// 执行本事件。对 chapter_end 事件返回 `Some(章节名)`。
    async fn execute(&mut self, ctx: &mut ScriptContext<'_>) -> Result<Option<String>>;

    /// 本处理器匹配的 YAML `type:` 字符串（如 `"dialogue"`、`"narration"`）。
    fn event_type() -> &'static str
    where
        Self: Sized;
}

// ============================================================
// 事件注册表
// ============================================================

pub type EventFactory = fn(event_data: Value) -> Box<dyn ScriptEvent>;

static REGISTRY: std::sync::LazyLock<RwLock<HashMap<&'static str, EventFactory>>> =
    std::sync::LazyLock::new(|| RwLock::new(HashMap::new()));

/// 在某个 YAML `type:` 字符串下注册一个事件处理器工厂。
/// 由每个事件模块在启动时调用。
pub fn register_event(event_type: &'static str, factory: EventFactory) {
    let mut registry = REGISTRY.write().expect("event registry poisoned");
    registry.insert(event_type, factory);
}

/// 按 YAML `type:` 字符串创建一个事件处理器实例。
/// `event_data` 是该事件的原始 YAML 字典。
/// 若该类型未注册处理器，返回 `None`。
pub fn create_event(event_type: &str, event_data: Value) -> Option<Box<dyn ScriptEvent>> {
    let registry = REGISTRY.read().expect("event registry poisoned");
    registry.get(event_type).map(|f| f(event_data))
}

/// 从事件 YAML 里读取 `duration`（事件间隔秒数）。
/// 各 handler 在 `from_event_data` 里调用，随后写入给前端的展示 payload。
/// 只接受数字；负数允许（由前端按「等玩家」处理），但 None 表示「没写」。
pub fn parse_duration(data: &Value) -> Option<f64> {
    data.get("duration").and_then(|v| v.as_f64())
}

// ============================================================
// 共享辅助函数
// ============================================================

/// 对剧本变量求值一个条件表达式。
/// 用 JSON 值比较处理简单表达式，如 `flag == true`。
///
/// 这是一个简化的安全求值器（没有 `eval()`）。支持：
/// - 单独 `var_name`（对变量做真值判断）
/// - `var_name == value`（相等）
/// - `var_name != value`（不等）
///
/// 比较按值的字符串形式进行，所以 `flag == true` 既能匹配 `Value::Bool(true)`，
/// 也能匹配字符串 `"true"`。
///
/// # 未定义变量
///
/// 未定义变量视为「不持有任何值」，于是对任意 `v`，`x == v` 为假、`x != v` 为真。
/// 两者相互自洽——不要只改其中一个而不改另一个。
///
/// # 支持的组合
///
/// 支持 `&&`、`||`（`&&` 优先级更高）以及数字的 `>` / `<` / `>=` / `<=`。
/// 仍不解析括号、任意算术表达式或字符串排序；复杂逻辑应拆成多个章节门。
pub(crate) fn split_once_unquoted<'a>(
    input: &'a str,
    operator: &str,
) -> Option<(&'a str, &'a str)> {
    let mut quote = None;
    let mut escaped = false;
    for (index, character) in input.char_indices() {
        if escaped {
            escaped = false;
            continue;
        }
        if character == '\\' {
            escaped = true;
            continue;
        }
        if let Some(active_quote) = quote {
            if character == active_quote {
                quote = None;
            }
            continue;
        }
        if matches!(character, '\'' | '"') {
            quote = Some(character);
            continue;
        }
        if input[index..].starts_with(operator) {
            let right = index + operator.len();
            return Some((&input[..index], &input[right..]));
        }
    }
    None
}

pub fn evaluate_condition(condition: &str, vars: &serde_json::Map<String, Value>) -> bool {
    let condition = condition.trim();
    if condition.is_empty() {
        return true;
    }

    // 最低优先级：OR；随后是 AND。只拆引号之外的运算符，避免破坏
    // 旧剧本里含 `||` / `&&` 字样的普通字符串值。
    if let Some((left, right)) = split_once_unquoted(condition, "||") {
        return evaluate_condition(left, vars) || evaluate_condition(right, vars);
    }
    if let Some((left, right)) = split_once_unquoted(condition, "&&") {
        return evaluate_condition(left, vars) && evaluate_condition(right, vars);
    }

    // 先试 `!=`（更长的模式，优先匹配）
    if let Some((var, val)) = split_once_unquoted(condition, "!=") {
        let var = var.trim();
        let val = val.trim().trim_matches('"').trim_matches('\'');
        if let Some(current) = vars.get(var) {
            let current_str = match current {
                Value::String(s) => s.clone(),
                other => other.to_string(),
            };
            return current_str != val;
        }
        // 未定义变量不持有任何值，故与任何值都不相等。
        // 与下方 `==` 分支自洽——同一情况下那里返回 false。
        return true;
    }

    // 再试 `==`
    if let Some((var, val)) = split_once_unquoted(condition, "==") {
        let var = var.trim();
        let val = val.trim().trim_matches('"').trim_matches('\'');
        if let Some(current) = vars.get(var) {
            let current_str = match current {
                Value::String(s) => s.clone(),
                other => other.to_string(),
            };
            return current_str == val;
        }
        return false;
    }

    // 数字比较。长操作符必须先于短操作符匹配；放在等值比较之后，
    // 避免字符串值本身含有 `<` / `>` 时改变旧有 `==` / `!=` 语义。
    for operator in [">=", "<=", ">", "<"] {
        if let Some((var, raw_value)) = split_once_unquoted(condition, operator) {
            let Some(current) = vars.get(var.trim()).and_then(Value::as_f64) else {
                return false;
            };
            let Ok(expected) = raw_value.trim().parse::<f64>() else {
                return false;
            };
            return match operator {
                ">=" => current >= expected,
                "<=" => current <= expected,
                ">" => current > expected,
                "<" => current < expected,
                _ => unreachable!(),
            };
        }
    }

    // 默认：当作布尔变量查找
    if let Some(current) = vars.get(condition) {
        match current {
            Value::Bool(b) => *b,
            Value::Null => false,
            _ => true, // non-null, non-bool → truthy
        }
    } else {
        false
    }
}

#[cfg(test)]
mod tests {
    use super::evaluate_condition;
    use serde_json::{Map, Value, json};

    fn vars(pairs: &[(&str, Value)]) -> Map<String, Value> {
        pairs
            .iter()
            .map(|(k, v)| (k.to_string(), v.clone()))
            .collect()
    }

    #[test]
    fn empty_condition_always_passes() {
        assert!(evaluate_condition("", &Map::new()));
        assert!(evaluate_condition("   ", &Map::new()));
    }

    #[test]
    fn equality_compares_string_form() {
        let v = vars(&[
            ("flag", json!(true)),
            ("name", json!("钦灵")),
            ("count", json!(2)),
        ]);
        assert!(evaluate_condition("flag == true", &v));
        assert!(evaluate_condition("name == 钦灵", &v));
        assert!(evaluate_condition("name == \"钦灵\"", &v));
        assert!(evaluate_condition("count == 2", &v));
        assert!(!evaluate_condition("count == 3", &v));
    }

    #[test]
    fn inequality_is_the_complement_of_equality() {
        let v = vars(&[("route", json!("shop"))]);
        assert!(evaluate_condition("route != home", &v));
        assert!(!evaluate_condition("route != shop", &v));
    }

    /// Undefined variables must behave consistently across `==` and `!=`.
    /// Regression guard for the contradictory comment removed in PR1.
    #[test]
    fn undefined_variable_is_unequal_to_everything() {
        let v = Map::new();
        assert!(!evaluate_condition("missing == 1", &v));
        assert!(evaluate_condition("missing != 1", &v));
        assert!(!evaluate_condition("missing", &v));
    }

    #[test]
    fn bare_variable_is_a_truthiness_check() {
        let v = vars(&[
            ("t", json!(true)),
            ("f", json!(false)),
            ("n", Value::Null),
            ("s", json!("x")),
            ("zero", json!(0)),
        ]);
        assert!(evaluate_condition("t", &v));
        assert!(!evaluate_condition("f", &v));
        assert!(!evaluate_condition("n", &v));
        assert!(evaluate_condition("s", &v));
        // 说明：0 属于「非空、非布尔」，因此为真。这是有意为之。
        assert!(evaluate_condition("zero", &v));
    }

    #[test]
    fn supports_numeric_comparisons_and_boolean_composition() {
        let v = vars(&[
            ("hp", json!(10)),
            ("act", json!(4)),
            ("done", json!(true)),
            ("label", json!("a>b")),
            ("pipes", json!("a||b&&c")),
        ]);
        assert!(evaluate_condition("hp >= 5", &v));
        assert!(evaluate_condition("hp > 5", &v));
        assert!(evaluate_condition("hp <= 10", &v));
        assert!(!evaluate_condition("hp < 10", &v));
        assert!(evaluate_condition("act == 4 && done == true", &v));
        assert!(evaluate_condition("act == 3 || done == true", &v));
        assert!(!evaluate_condition("act == 3 || hp < 2", &v));
        assert!(evaluate_condition("label == a>b", &v));
        assert!(evaluate_condition("pipes == \"a||b&&c\"", &v));
    }
}
