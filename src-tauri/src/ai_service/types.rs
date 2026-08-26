use serde::{Deserialize, Serialize};
use serde_json::Value;
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
    pub fn new(
        name: impl Into<String>,
        description: impl Into<String>,
        parameters: serde_json::Value,
    ) -> Self {
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

/// 归一化工具调用的 `arguments`。有些模型输出非标准形态（嵌套 `{"arguments": {...}}`
/// / `{"params": {...}}`、双编码 JSON 字符串、甚至非法 JSON）。统一成普通对象，
/// 绝不返回 `null` 让 UI 崩溃。
pub(crate) fn parse_tool_args(raw: &str) -> Value {
    let mut args: Value = serde_json::from_str(raw).unwrap_or(Value::Null);

    if let Some(s) = args.as_str() {
        if let Ok(parsed) = serde_json::from_str::<Value>(s) {
            args = parsed;
        }
    }

    if let Value::Object(map) = &args {
        if map.len() == 1 {
            if let Some(inner) = map.get("arguments").or_else(|| map.get("params")) {
                if inner.is_object() {
                    args = inner.clone();
                }
            }
        }
    }

    if !args.is_object() {
        args = serde_json::json!({});
    }
    args
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct LlmMessage {
    pub role: String,
    pub content: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tool_calls: Option<Vec<ToolCall>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tool_call_id: Option<String>,
    /// 本条消息附带的多模态图片（`data:image/...;base64,...` data URL）。
    /// 仅用于**当轮** LLM 请求，绝不写入角色长期记忆（避免每一轮重复携带图片
    /// 导致上下文/缓存占用膨胀）。genai 的 OpenAI 兼容 / Gemini 适配器会把它
    /// 转为 provider 原生的 `image_url` / `inline_data` content part。
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub image_data_url: Option<String>,
}

impl LlmMessage {
    pub fn system(content: impl Into<String>) -> Self {
        Self {
            role: "system".into(),
            content: content.into(),
            tool_calls: None,
            tool_call_id: None,
            image_data_url: None,
        }
    }
    pub fn user(content: impl Into<String>) -> Self {
        Self {
            role: "user".into(),
            content: content.into(),
            tool_calls: None,
            tool_call_id: None,
            image_data_url: None,
        }
    }
    pub fn assistant(content: impl Into<String>) -> Self {
        Self {
            role: "assistant".into(),
            content: content.into(),
            tool_calls: None,
            tool_call_id: None,
            image_data_url: None,
        }
    }
    /// 助手消息携带 tool calls（LLM 请求调用工具）。
    pub fn tool(tool_calls: Vec<ToolCall>) -> Self {
        Self {
            role: "assistant".into(),
            content: String::new(),
            tool_calls: Some(tool_calls),
            tool_call_id: None,
            image_data_url: None,
        }
    }
    /// 工具调用结果（role = "tool"）。
    pub fn tool_result(tool_call_id: impl Into<String>, content: impl Into<String>) -> Self {
        Self {
            role: "tool".into(),
            content: content.into(),
            tool_calls: None,
            tool_call_id: Some(tool_call_id.into()),
            image_data_url: None,
        }
    }
    /// 构建一条携带多模态图片的用户消息。`image_data_url` 为
    /// `data:image/<type>;base64,<data>` 格式的 data URL；`content` 为可选的
    /// 随附文字说明（可为空串）。仅用于当轮请求，不写入记忆。
    pub fn user_with_image(content: impl Into<String>, image_data_url: String) -> Self {
        Self {
            role: "user".into(),
            content: content.into(),
            tool_calls: None,
            tool_call_id: None,
            image_data_url: Some(image_data_url),
        }
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
    /// 该轮生成的思考链（仅挂在每轮最后一条 assistant 行上）。
    pub thinking: Option<String>,
    pub tool_call: Option<String>,
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
    fn from(v: LineAttribute) -> Self {
        Self(v)
    }
}

impl LineAttributeExt {
    pub fn inner(&self) -> &LineAttribute {
        &self.0
    }

    pub fn as_str(&self) -> &'static str {
        match &self.0 {
            LineAttribute::User => "user",
            LineAttribute::System => "system",
            LineAttribute::Assistant => "assistant",
            LineAttribute::Tool => "tool",
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
        Self {
            base,
            perceived_role_ids,
        }
    }

    pub fn content(&self) -> &str {
        &self.base.content
    }
    pub fn sender_role_id(&self) -> Option<i32> {
        self.base.sender_role_id
    }
    pub fn attribute(&self) -> &LineAttribute {
        self.base.attribute.inner()
    }
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

fn default_schema_version() -> u32 {
    1
}

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
            self.data.user_info, self.data.promises, self.data.long_term, self.data.short_term,
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
    /// GSV 六情绪参考语音开关：开启后按情绪分类（吃惊/开心/恐惧/难过/生气/中立）
    /// 实时切换参考音频与文本；关闭时保持原有 gsv_voice_* 行为。
    #[serde(default)]
    pub gsv_emo_enabled: Option<bool>,
    /// 六分类参考文本（分类名 → 文本）
    #[serde(default)]
    pub gsv_emo_texts: Option<HashMap<String, String>>,
    /// 六分类参考音频文件名（分类名 → 文件名，相对角色 voice/ 目录；
    /// 留空时按分类名在 voice/ 下自动查找，复用立绘系统的命名约定）
    #[serde(default)]
    pub gsv_emo_voice_files: Option<HashMap<String, String>>,
    pub aivis_model_uuid: Option<String>,
    pub opentts_voice: Option<String>,
    pub fish_s2_voice: Option<String>,
    pub sbv2_local_voice_id: Option<String>,
    pub sbv2_local_speaker_id: Option<i64>,
    pub sbv2_local_style_id: Option<i32>,
    pub sbv2_local_length_scale: Option<f32>,
    pub sbv2_local_sdp_ratio: Option<f32>,
    pub sbv2_local_cloud_fallback_model: Option<String>,
    pub sbv2_local_cloud_fallback_speaker_id: Option<String>,
    pub cosyvoice_voice_id: Option<String>,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Live2dMotionBinding {
    pub group: String,
    pub index: usize,
    #[serde(default, rename = "loop")]
    pub loop_motion: bool,
    #[serde(flatten)]
    pub extra: HashMap<String, serde_json::Value>,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Live2dParameterBinding {
    pub parameter: String,
    #[serde(default = "default_live2d_gain")]
    pub gain: f64,
    #[serde(flatten)]
    pub extra: HashMap<String, serde_json::Value>,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Live2dEyeBlinkBinding {
    pub left: String,
    pub right: String,
    #[serde(flatten)]
    pub extra: HashMap<String, serde_json::Value>,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Live2dFocusAnchor {
    pub x: f64,
    pub y: f64,
    #[serde(flatten)]
    pub extra: HashMap<String, serde_json::Value>,
}

fn deserialize_live2d_focus_anchor<'de, D>(
    deserializer: D,
) -> Result<Option<Live2dFocusAnchor>, D::Error>
where
    D: serde::Deserializer<'de>,
{
    let anchor = Option::<Live2dFocusAnchor>::deserialize(deserializer)?;
    if let Some(anchor) = &anchor {
        if !anchor.x.is_finite()
            || !anchor.y.is_finite()
            || !(0.0..=1.0).contains(&anchor.x)
            || !(0.0..=1.0).contains(&anchor.y)
        {
            return Err(serde::de::Error::custom(
                "focus_anchor x/y must be finite values between 0 and 1",
            ));
        }
    }
    Ok(anchor)
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, Default)]
pub struct Live2dVariant {
    pub model: String,
    #[serde(default)]
    pub default_expression: Option<String>,
    #[serde(default)]
    pub expressions: HashMap<String, String>,
    #[serde(default)]
    pub motions: HashMap<String, Live2dMotionBinding>,
    #[serde(default)]
    pub idle: Option<Live2dMotionBinding>,
    #[serde(default)]
    pub eye_blink: Option<Live2dEyeBlinkBinding>,
    #[serde(default, deserialize_with = "deserialize_live2d_focus_anchor")]
    pub focus_anchor: Option<Live2dFocusAnchor>,
    #[serde(default)]
    pub lip_sync: Option<Live2dParameterBinding>,
    #[serde(flatten)]
    pub extra: HashMap<String, serde_json::Value>,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Live2dSettings {
    #[serde(default = "default_live2d_version")]
    pub version: u32,
    pub default_variant: String,
    #[serde(default)]
    pub variants: HashMap<String, Live2dVariant>,
    #[serde(default)]
    pub clothes_variants: HashMap<String, String>,
    #[serde(flatten)]
    pub extra: HashMap<String, serde_json::Value>,
}

fn default_live2d_version() -> u32 {
    1
}

fn default_live2d_gain() -> f64 {
    1.0
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
    /// 主对话形象：`live2d` = 有 Live2D 模型则用模型，`image` = 强制静态立绘。
    /// 缺省或非法值等价于 `live2d`（即本功能出现前的行为，老配置无需迁移）。
    /// 用 `Option<String>` 而非 enum：YAML 解析失败时 `read_character_settings`
    /// 会整份回退默认值、`role_repo` 则直接报错导致角色渲染不出来，
    /// 严格 enum 会让一个手写错别字把整份设定打回默认。
    #[serde(default)]
    pub avatar_mode: Option<String>,

    #[serde(default = "default_scale")]
    pub scale_p: f64,
    #[serde(default)]
    pub offset_x_p: f64,
    #[serde(default)]
    pub offset_y_p: f64,
    /// 桌宠形象，语义同 `avatar_mode`（`_p` 后缀沿用 `scale_p` 等桌宠字段惯例）。
    #[serde(default)]
    pub avatar_mode_p: Option<String>,
    /// 桌宠无框模式：true = 隐藏圆形外框、半透明底与粒子，并取消圆形裁剪，
    /// 让角色在本来的方形区域内完整显示。缺省/false = 现有的圆盘外观。
    /// 正语义键名（而非 `pet_frame` 反着写）是为了让 `#[serde(default)]` 的
    /// `None` 直接对应「有框」，不必再写自定义 default 函数。
    /// 用 `Option<bool>` 而非裸 `bool`：`pet_frameless:`（空值）会解析成 YAML 的
    /// null，裸 bool 会硬失败，而失败代价是 `read_character_settings` 把**整份设定**
    /// 打回默认、`role_repo` 则让角色渲染不出来。非布尔值仍然会解析失败，
    /// 这一点与既有的 `scale_p: f64` 同等风险，不额外加固。
    #[serde(default)]
    pub pet_frameless: Option<bool>,

    #[serde(default)]
    pub voice_models: Option<VoiceModel>,
    #[serde(default)]
    pub tts_type: Option<String>,
    #[serde(default)]
    pub voice_lang: Option<String>,
    /// 中文方言（仅 cosyvoice + voice_lang=zh 时生效；如"四川话"，空 = 普通话）
    #[serde(default)]
    pub voice_dialect: Option<String>,

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
    pub live2d: Option<Live2dSettings>,

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

fn default_ai_name() -> String {
    "ai_name未设定".into()
}
fn default_user_name() -> String {
    "user_name未设定".into()
}
fn default_scale() -> f64 {
    1.0
}
fn default_thinking_message() -> String {
    "正在思考中...".into()
}
fn default_bubble_top() -> i32 {
    5
}
fn default_bubble_left() -> i32 {
    20
}

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
            avatar_mode: None,
            scale_p: 1.0,
            offset_x_p: 0.0,
            offset_y_p: 0.0,
            avatar_mode_p: None,
            pet_frameless: None,
            voice_models: None,
            tts_type: None,
            voice_lang: None,
            voice_dialect: None,
            thinking_message: default_thinking_message(),
            bubble_top: default_bubble_top(),
            bubble_left: default_bubble_left(),
            system_prompt: None,
            system_prompt_example: None,
            system_prompt_example_old: None,
            live2d: None,
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
// AffectionVector（六维好感度）
// ==========================================

/// 单个角色对玩家的六维情感状态，各项取值任意整数（允许溢出：
/// >100 为「满溢」、负数为「疏离」）。
///
/// 持久化在存档全局变量 JSON（`GameStatus::global_variables`，键 `affection.{role_id}`，
/// 跟随存档快照回滚）；运行时挂在 `GameRole.affection` 上，由上帝 Agent 定期评估对话后调整。
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct AffectionVector {
    /// 好感：整体喜欢程度，影响语气甜度
    pub fondness: i32,
    /// 信赖：倾诉深度、说真心话的程度
    pub trust: i32,
    /// 亲密：肢体接触与近距离描写的接受度
    pub intimacy: i32,
    /// 默契：接梗、理解言外之意的程度
    pub rapport: i32,
    /// 兴趣：对玩家话题的好奇心、主动提问的倾向
    pub interest: i32,
    /// 思念：久别重逢的反应强度
    pub longing: i32,
}

impl Default for AffectionVector {
    fn default() -> Self {
        Self {
            fondness: 10,
            trust: 5,
            intimacy: 0,
            rapport: 5,
            interest: 15,
            longing: 0,
        }
    }
}

impl AffectionVector {
    /// 图表满刻度的参考上下限；数值本身允许溢出（>100 为满溢、负数为疏离）。
    pub const MIN: i32 = 0;
    pub const MAX: i32 = 100;

    /// 六维的（序列化键名, 中文显示名）。
    pub const DIMENSIONS: [(&'static str, &'static str); 6] = [
        ("fondness", "好感"),
        ("trust", "信赖"),
        ("intimacy", "亲密"),
        ("rapport", "默契"),
        ("interest", "兴趣"),
        ("longing", "思念"),
    ];

    pub fn average(&self) -> i32 {
        (self.fondness + self.trust + self.intimacy + self.rapport + self.interest + self.longing)
            / 6
    }

    pub fn get(&self, dimension: &str) -> Option<i32> {
        match dimension {
            "fondness" => Some(self.fondness),
            "trust" => Some(self.trust),
            "intimacy" => Some(self.intimacy),
            "rapport" => Some(self.rapport),
            "interest" => Some(self.interest),
            "longing" => Some(self.longing),
            _ => None,
        }
    }

    /// 按维度键名增减；数值**允许溢出**（不钳制 0~100，>100 为「满溢」、
    /// 负数为「疏离」），未知维度返回 false。
    pub fn add_delta(&mut self, dimension: &str, delta: i32) -> bool {
        let slot = match dimension {
            "fondness" => &mut self.fondness,
            "trust" => &mut self.trust,
            "intimacy" => &mut self.intimacy,
            "rapport" => &mut self.rapport,
            "interest" => &mut self.interest,
            "longing" => &mut self.longing,
            _ => return false,
        };
        *slot = slot.saturating_add(delta);
        true
    }
}

// ==========================================
// NegativeVector（六维负面情绪）
// ==========================================

/// 单个角色对玩家的六维负面情绪强度，取值任意整数（>100 为「失控」边缘）。
///
/// 与好感度同存于存档全局变量 JSON（键 `affection.{role_id}`）；正面互动会消解、冒犯会积累，
/// 由上帝 Agent 与好感度同一次评估调整。
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct NegativeVector {
    /// 愤怒：被冒犯时的火气
    pub anger: i32,
    /// 受伤：被刺痛、委屈的程度
    pub hurt: i32,
    /// 失望：期待落空的程度
    pub disappointment: i32,
    /// 冷漠：敷衍、不在乎的态度强度
    pub indifference: i32,
    /// 嫉妒：玩家关注别人时的吃醋程度
    pub jealousy: i32,
    /// 疏远：想保持距离的程度
    pub estrangement: i32,
}

impl Default for NegativeVector {
    fn default() -> Self {
        Self {
            anger: 0,
            hurt: 0,
            disappointment: 0,
            indifference: 0,
            jealousy: 0,
            estrangement: 0,
        }
    }
}

impl NegativeVector {
    /// 六维的（序列化键名, 中文显示名）。
    pub const DIMENSIONS: [(&'static str, &'static str); 6] = [
        ("anger", "愤怒"),
        ("hurt", "受伤"),
        ("disappointment", "失望"),
        ("indifference", "冷漠"),
        ("jealousy", "嫉妒"),
        ("estrangement", "疏远"),
    ];

    /// 六维中的最大强度（全 0 表示没有负面情绪）。
    pub fn peak(&self) -> i32 {
        self.anger
            .max(self.hurt)
            .max(self.disappointment)
            .max(self.indifference)
            .max(self.jealousy)
            .max(self.estrangement)
    }

    pub fn get(&self, dimension: &str) -> Option<i32> {
        match dimension {
            "anger" => Some(self.anger),
            "hurt" => Some(self.hurt),
            "disappointment" => Some(self.disappointment),
            "indifference" => Some(self.indifference),
            "jealousy" => Some(self.jealousy),
            "estrangement" => Some(self.estrangement),
            _ => None,
        }
    }

    /// 按维度键名增减；下限 0（负面情绪不会跌成负值），上限不封（允许溢出）。
    pub fn add_delta(&mut self, dimension: &str, delta: i32) -> bool {
        let slot = match dimension {
            "anger" => &mut self.anger,
            "hurt" => &mut self.hurt,
            "disappointment" => &mut self.disappointment,
            "indifference" => &mut self.indifference,
            "jealousy" => &mut self.jealousy,
            "estrangement" => &mut self.estrangement,
            _ => return false,
        };
        *slot = slot.saturating_add(delta).max(0);
        true
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
    /// 对玩家的六维好感度（持久化在存档全局变量 `affection.{role_id}`）。
    pub affection: AffectionVector,
    /// 对玩家的六维负面情绪强度（同源存档全局变量；评估积累、安抚消解）。
    pub negative: NegativeVector,
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

    /// 该剧本来自哪个插件（None = 游戏自有剧本）。用于列表来源角标与插件禁用时移除。
    pub plugin_id: Option<String>,
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
