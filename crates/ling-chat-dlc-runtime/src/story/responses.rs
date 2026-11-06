pub use ling_chat_main::ai_service::game_system::script_engine::responses::*;
use serde::Serialize;

/// 突脸惊吓：全屏图片闪现 + 音效。图片为空串表示解析失败（前端应跳过）。
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct JumpscarePayload {
    pub image_path: String,
    /// 可选音效；空串表示无
    pub sound_path: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub duration: Option<f64>,
}

/// 显式时间轴停顿。必须进入前端事件队列，才能相对玩家点击后的真实画面计时。
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct WaitPayload {
    pub duration: f64,
}

/// 队列有序的显式窗口标题意图；空串表示恢复应用默认标题。
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct WindowTitlePayload {
    pub title: String,
}

/// 已在 Rust 端完成安全校验的辅助故障窗口一次性票据。
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct GlitchWindowTicketPayload {
    pub request_id: u64,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub duration: Option<f64>,
}

/// 真实系统窗口的一次性票据：完整内容仅保存在 Rust，前端无法自由构造、
/// 重放或跨剧本运行消费。
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ConsoleWindowTicketPayload {
    pub request_id: u64,
}

/// 强制选择：前端用"鼠标被拖向 forced 选项"的演出，最终只能提交 forced。
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ForceChoicePayload {
    /// 绑定当前剧本运行与 force_choice 实例的一次性能力票据。
    pub request_id: String,
    pub choices: Vec<ChoiceItem>,
    /// 必然被选中的选项文本（必须在 choices 里存在）
    pub forced: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub duration: Option<f64>,
}

/// 选词写诗小游戏中的一个候选词。三个分值只用于客户端即时反馈，
/// 最终结果会回传并由后端再次校验范围。
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PoemWordPayload {
    pub text: String,
    pub warm_points: i64,
    pub script_points: i64,
    pub void_points: i64,
    #[serde(default)]
    pub glitch: bool,
}

/// DDLC 式选词写诗互动，但使用本剧本原创的「她 / 剧本 / 空白」三种倾向。
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PoemGamePayload {
    pub request_id: String,
    pub background_path: String,
    pub music_path: String,
    pub glitch_music_path: String,
    pub warm_sticker_path: String,
    pub script_sticker_path: String,
    pub void_sticker_path: String,
    /// Explicit presentation mode; the client must not infer an act from playthrough.
    pub mode: String,
    pub rounds: Vec<Vec<PoemWordPayload>>,
    pub normal_loop_start: f64,
    pub glitch_loop_start: f64,
}

/// Voice shift：角色语音（TTS）播放倍率 + 音调偏移。rate <1 时因
/// preservesPitch=false 同时降调；pitch 为纯音调偏移（半音数，负数=低沉，
/// 由前端 Web Audio detune 实现，不改变语速），两者可叠加。
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct VoiceShiftPayload {
    pub rate: f64,
    pub pitch: f64,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub duration: Option<f64>,
}

pub mod event_names {
    pub use ling_chat_main::ai_service::game_system::script_engine::responses::event_names::*;
    pub const SCRIPT_JUMPSCARE: &str = "script:jumpscare";
    pub const SCRIPT_FORCE_CHOICE: &str = "script:force-choice";
    pub const SCRIPT_POEM_GAME: &str = "script:poem-game";
    pub const SCRIPT_VOICE_SHIFT: &str = "script:voice-shift";
    pub const SCRIPT_WAIT: &str = "script:wait";
    pub const SCRIPT_GLITCH_WINDOW: &str = "script:glitch-window";
    pub const SCRIPT_CONSOLE_WINDOW: &str = "script:console-window";
    pub const SCRIPT_WINDOW_TITLE: &str = "script:window-title";
    pub const SCRIPT_WINDOW_TITLE_RESET: &str = "script:window-title-reset";
}

/// Only authored presentation fields cross the host event boundary. Other keys
/// (including core identity/action fields) cannot be overwritten by extension data.
pub fn event_fields(
    kind: &str,
    data: &serde_json::Value,
) -> serde_json::Map<String, serde_json::Value> {
    use serde_json::{Map, Value};
    let mut fields = Map::new();
    match kind {
        "modify_character" => {
            let flash = data.get("flash").is_some_and(|v| {
                v.as_bool().unwrap_or_else(|| {
                    v.as_str()
                        .is_some_and(|s| s.trim().eq_ignore_ascii_case("true"))
                })
            });
            if flash {
                fields.insert("flash".into(), Value::Bool(true));
            }
            if let Some(noise) = data.get("noise").and_then(Value::as_str) {
                fields.insert("noise".into(), Value::String(noise.into()));
            }
            if let Some(number) = data
                .get("noise_fade_in")
                .and_then(|v| v.as_f64().or_else(|| v.as_str()?.trim().parse().ok()))
                .and_then(serde_json::Number::from_f64)
            {
                fields.insert("noiseFadeIn".into(), Value::Number(number));
            }
        },
        "background_effect" => {
            for key in ["text", "echo"] {
                if let Some(text) = data.get(key).and_then(Value::as_str) {
                    fields.insert(key.into(), Value::String(text.into()));
                }
            }
        },
        _ => {},
    }
    fields
}
