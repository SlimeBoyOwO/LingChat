use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::PathBuf;

use crate::db::entities::line::LineAttribute;

// ==========================================
// LLM 消息 & Function Calling
// ==========================================

/// Function call 请求参数。
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct FunctionCall {
    pub name: String,
    pub arguments: String, // JSON 字符串
}

/// LLM 返回的 tool call。
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct ToolCall {
    pub id: String,
    #[serde(rename = "type", default = "default_tool_type")]
    pub type_: String,
    pub function: FunctionCall,
}

fn default_tool_type() -> String {
    "function".to_string()
}

/// 注册给 LLM 的 tool / function 定义。
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct ToolDefinition {
    #[serde(rename = "type")]
    pub type_: String,
    pub function: FunctionSchema,
}

impl ToolDefinition {
    pub fn new(name: impl Into<String>, description: impl Into<String>, parameters: serde_json::Value) -> Self {
        Self {
            type_: "function".to_string(),
            function: FunctionSchema {
                name: name.into(),
                description: description.into(),
                parameters,
            },
        }
    }
}

/// Function 的 JSON Schema 描述。
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct FunctionSchema {
    pub name: String,
    pub description: String,
    pub parameters: serde_json::Value,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct LlmMessage {
    pub role: String,
    pub content: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tool_calls: Option<Vec<ToolCall>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tool_call_id: Option<String>,
}

impl LlmMessage {
    pub fn system(content: impl Into<String>) -> Self {
        Self { role: "system".into(), content: content.into(), tool_calls: None, tool_call_id: None }
    }
    pub fn user(content: impl Into<String>) -> Self {
        Self { role: "user".into(), content: content.into(), tool_calls: None, tool_call_id: None }
    }
    pub fn assistant(content: impl Into<String>) -> Self {
        Self { role: "assistant".into(), content: content.into(), tool_calls: None, tool_call_id: None }
    }
    /// 助手消息携带 tool calls（LLM 请求调用工具）。
    pub fn tool(tool_calls: Vec<ToolCall>) -> Self {
        Self { role: "assistant".into(), content: String::new(), tool_calls: Some(tool_calls), tool_call_id: None }
    }
    /// 工具调用结果（role = "tool"）。
    pub fn tool_result(tool_call_id: impl Into<String>, content: impl Into<String>) -> Self {
        Self { role: "tool".into(), content: content.into(), tool_calls: None, tool_call_id: Some(tool_call_id.into()) }
    }
}

// ==========================================
// 台词基础结构
// ==========================================

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, Default)]
pub struct LineBase {
    pub id: Option<i32>,
    pub content: String,
    pub original_emotion: Option<String>,
    pub predicted_emotion: Option<String>,
    pub tts_content: Option<String>,
    pub action_content: Option<String>,
    pub audio_file: Option<String>,
    pub attribute: LineAttributeExt,
    pub sender_role_id: Option<i32>,
    pub display_name: Option<String>,
}

/// 对 `db::entities::line::LineAttribute` 的包装，提供 `Default` 实现
/// 以便 `LineBase::default()` 可用（业务层常用占位构造）。
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(transparent)]
pub struct LineAttributeExt(pub LineAttribute);

impl Default for LineAttributeExt {
    fn default() -> Self {
        Self(LineAttribute::System)
    }
}

impl From<LineAttribute> for LineAttributeExt {
    fn from(v: LineAttribute) -> Self { Self(v) }
}

impl LineAttributeExt {
    pub fn inner(&self) -> &LineAttribute { &self.0 }

    pub fn as_str(&self) -> &'static str {
        match &self.0 {
            LineAttribute::User => "user",
            LineAttribute::System => "system",
            LineAttribute::Assistant => "assistant",
        }
    }
}

/// 运行时对象：在内存中流转的剧本行。
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, Default)]
pub struct GameLine {
    #[serde(flatten)]
    pub base: LineBase,
    pub perceived_role_ids: Vec<i32>,
}

impl GameLine {
    pub fn from_base(base: LineBase, perceived_role_ids: Vec<i32>) -> Self {
        Self { base, perceived_role_ids }
    }

    pub fn content(&self) -> &str { &self.base.content }
    pub fn sender_role_id(&self) -> Option<i32> { self.base.sender_role_id }
    pub fn attribute(&self) -> &LineAttribute { self.base.attribute.inner() }
}

// ==========================================
// MemoryBank
// ==========================================

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, Default)]
pub struct GameMemoryBankMeta {
    #[serde(default)]
    pub last_processed_global_idx: i64,
    #[serde(default)]
    pub updated_at: String,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct GameMemoryBankData {
    pub short_term: String,
    pub long_term: String,
    pub user_info: String,
    pub promises: String,
}

impl Default for GameMemoryBankData {
    fn default() -> Self {
        Self {
            short_term: "暂无近期对话摘要。".into(),
            long_term: "暂无长期关键经历。".into(),
            user_info: "暂无用户特征记录。".into(),
            promises: "暂无未完成的约定。".into(),
        }
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct GameMemoryBank {
    #[serde(default = "default_schema_version")]
    pub schema_version: u32,
    #[serde(default)]
    pub meta: GameMemoryBankMeta,
    #[serde(default)]
    pub data: GameMemoryBankData,
}

fn default_schema_version() -> u32 { 1 }

impl Default for GameMemoryBank {
    fn default() -> Self {
        Self {
            schema_version: 1,
            meta: GameMemoryBankMeta::default(),
            data: GameMemoryBankData::default(),
        }
    }
}

impl GameMemoryBank {
    pub fn to_prompt_text(&self) -> String {
        format!(
            "\n\n====== 核心记忆库 (Memory Bank) ======\n\
             【用户信息】：{}\n\
             【重要约定】：{}\n\
             【长期经历】：{}\n\
             【近期回顾】：{}\n\
             ====================================\n",
            self.data.user_info,
            self.data.promises,
            self.data.long_term,
            self.data.short_term,
        )
    }
}

// ==========================================
// 角色设定 (CharacterSettings)
// ==========================================

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, Default)]
pub struct VoiceModel {
    pub sva_speaker_id: Option<String>,
    pub sbv2_name: Option<String>,
    pub sbv2_speaker_id: Option<String>,
    pub bv2_speaker_id: Option<String>,
    pub sbv2api_name: Option<String>,
    pub sbv2api_speaker_id: Option<String>,
    pub gsv_voice_text: Option<String>,
    pub gsv_voice_filename: Option<String>,
    pub gsv_gpt_model_name: Option<String>,
    pub gsv_sovits_model_name: Option<String>,
    pub aivis_model_uuid: Option<String>,
}

/// 角色设定模型，对应 Python `CharacterSettings`。
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct CharacterSettings {
    #[serde(default = "default_ai_name")]
    pub ai_name: String,
    #[serde(default)]
    pub ai_subtitle: Option<String>,
    #[serde(default = "default_user_name")]
    pub user_name: String,
    #[serde(default)]
    pub user_subtitle: Option<String>,
    #[serde(default)]
    pub title: Option<String>,
    #[serde(default)]
    pub info: Option<String>,

    #[serde(default)]
    pub body_part: Option<HashMap<String, serde_json::Value>>,
    #[serde(default = "default_scale")]
    pub scale: f64,
    #[serde(default)]
    pub offset_x: f64,
    #[serde(default)]
    pub offset_y: f64,
    #[serde(default)]
    pub clothes_name: Option<String>,
    #[serde(default)]
    pub clothes: Option<Vec<HashMap<String, String>>>,

    #[serde(default = "default_scale")]
    pub scale_p: f64,
    #[serde(default)]
    pub offset_x_p: f64,
    #[serde(default)]
    pub offset_y_p: f64,

    #[serde(default)]
    pub voice_models: Option<VoiceModel>,
    #[serde(default)]
    pub tts_type: Option<String>,

    #[serde(default = "default_thinking_message")]
    pub thinking_message: String,
    #[serde(default = "default_bubble_top")]
    pub bubble_top: i32,
    #[serde(default = "default_bubble_left")]
    pub bubble_left: i32,

    #[serde(default)]
    pub system_prompt: Option<String>,
    #[serde(default)]
    pub system_prompt_example: Option<String>,
    #[serde(default)]
    pub system_prompt_example_old: Option<String>,

    #[serde(default)]
    pub character_folder: String,
    #[serde(default)]
    pub resource_path: Option<String>,
    #[serde(default)]
    pub script_role_key: Option<String>,
    #[serde(default)]
    pub script_key: Option<String>,
    #[serde(default)]
    pub character_id: Option<i32>,

    // Pydantic `extra="allow"` 允许任意扩展字段
    #[serde(flatten)]
    pub extra: HashMap<String, serde_json::Value>,
}

fn default_ai_name() -> String { "ai_name未设定".into() }
fn default_user_name() -> String { "user_name未设定".into() }
fn default_scale() -> f64 { 1.0 }
fn default_thinking_message() -> String { "正在思考中...".into() }
fn default_bubble_top() -> i32 { 5 }
fn default_bubble_left() -> i32 { 20 }

impl Default for CharacterSettings {
    fn default() -> Self {
        Self {
            ai_name: default_ai_name(),
            ai_subtitle: Some(String::new()),
            user_name: default_user_name(),
            user_subtitle: Some(String::new()),
            title: None,
            info: None,
            body_part: None,
            scale: 1.0,
            offset_x: 0.0,
            offset_y: 0.0,
            clothes_name: None,
            clothes: None,
            scale_p: 1.0,
            offset_x_p: 0.0,
            offset_y_p: 0.0,
            voice_models: None,
            tts_type: None,
            thinking_message: default_thinking_message(),
            bubble_top: default_bubble_top(),
            bubble_left: default_bubble_left(),
            system_prompt: None,
            system_prompt_example: None,
            system_prompt_example_old: None,
            character_folder: String::new(),
            resource_path: None,
            script_role_key: None,
            script_key: None,
            character_id: None,
            extra: HashMap::new(),
        }
    }
}

// ==========================================
// GameRole
// ==========================================

#[derive(Clone, Debug, Default)]
pub struct GameRole {
    pub role_id: Option<i32>,
    pub memory: Vec<LlmMessage>,

    pub display_name: Option<String>,
    pub settings: CharacterSettings,
    pub resource_path: Option<String>,
    pub prompt: Option<String>,
    pub current_clothes: String,
    pub memory_bank: GameMemoryBank,
    pub voice_maker: Option<crate::ai_service::tts::VoiceMaker>,
}

impl GameRole {
    pub fn new_empty(role_id: i32) -> Self {
        Self {
            role_id: Some(role_id),
            current_clothes: "default".into(),
            ..Default::default()
        }
    }
}

impl PartialEq for GameRole {
    fn eq(&self, other: &Self) -> bool {
        match (self.role_id, other.role_id) {
            (Some(a), Some(b)) => a == b,
            _ => std::ptr::eq(self, other),
        }
    }
}

impl Eq for GameRole {}

impl std::hash::Hash for GameRole {
    fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
        match self.role_id {
            Some(id) => id.hash(state),
            None => (self as *const _ as usize).hash(state),
        }
    }
}

// ==========================================
// Player
// ==========================================

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct Player {
    pub user_name: String,
    pub user_subtitle: String,
    pub user_prompt: String,
}

// ==========================================
// AdventureConfig
// ==========================================

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct AdventureConfig {
    #[serde(default)]
    pub is_adventure: bool,
    #[serde(default)]
    pub bound_character_folder: String,
    #[serde(default)]
    pub order: i32,
    #[serde(default)]
    pub unlock_conditions: Vec<serde_json::Value>,
    #[serde(default)]
    pub trigger: serde_json::Map<String, serde_json::Value>,
    #[serde(default)]
    pub completion_achievements: Vec<serde_json::Value>,
}

// ==========================================
// ScriptStatus
// ==========================================

#[derive(Clone, Debug)]
pub struct ScriptStatus {
    pub folder_key: String,
    pub name: String,
    pub description: String,
    pub intro_chapter: String,
    pub settings: serde_json::Map<String, serde_json::Value>,
    pub script_path: PathBuf,

    pub recommand_start: String,
    pub adventure: AdventureConfig,
    pub running_client_id: Option<String>,

    pub current_chapter_key: String,
    pub current_event_process: i32,

    pub vars: serde_json::Map<String, serde_json::Value>,
}

impl ScriptStatus {
    /// 返回 `scripts/` 之后的相对路径字符串。
    pub fn path_key(&self) -> String {
        let parts: Vec<_> = self
            .script_path
            .components()
            .map(|c| c.as_os_str().to_string_lossy().to_string())
            .collect();
        if let Some(idx) = parts.iter().position(|p| p == "scripts") {
            if idx + 1 < parts.len() {
                return parts[idx + 1..].join(std::path::MAIN_SEPARATOR_STR);
            }
            return String::new();
        }
        self.script_path.to_string_lossy().to_string()
    }

    pub fn set_variable(&mut self, key: impl Into<String>, value: serde_json::Value) {
        self.vars.insert(key.into(), value);
    }

    pub fn get_variable<'a>(&'a self, key: &str) -> Option<&'a serde_json::Value> {
        self.vars.get(key)
    }
}
