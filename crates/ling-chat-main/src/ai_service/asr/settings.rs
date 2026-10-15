//! ASR 配置持久化。
//!
//! 复用 `tauri_plugin_store` 的 `settings.json`，三个 key 分开存：
//! `ASR_PROVIDERS`（provider 凭据）、`ASR_ACTIVE_PROVIDER_ID`（当前激活 id）、
//! `ASR_PREFS`（UI 偏好：总开关 / 自动监听 / 发送模式 / 流式 / 静音计时）。
//! 与 [`crate::ai_service::llm::provider_config`] 的持久化模式一致。

use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use tauri::AppHandle;
use tauri_plugin_store::StoreExt;

use super::error::AsrError;
use super::provider::{ProviderCredentials, QwenAsrProvider};
use super::region::DashScopeRegion;

/// 识别后文本如何处理。
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Eq, Default)]
#[serde(rename_all = "snake_case")]
pub enum SendMode {
    /// 填入聊天输入框（默认），用户检查后手动发送。
    #[default]
    FillOnly,
    /// 识别完成后自动 send_chat_message。
    AutoSend,
}

/// 单个 provider 的配置：API key + 端点 + 模型 + 地域。
///
/// **新增字段必须同步加到这里**：设置页把 `config_fields[].key` 写成顶层键
/// （`providerCfgRecord[field.key]`），而本结构没有 `deny_unknown_fields`
/// —— 只加 `AsrConfigField` 而不加字段，用户填的值会被 serde 静默丢弃。
#[derive(Serialize, Deserialize, Clone, Debug, Default)]
pub struct ProviderConfig {
    /// 按地域分存的 API Key，键为地域 id（见 [`DashScopeRegion::id`]）。
    ///
    /// 各地域的 Key 互相独立、不能混用，共用单个字段会让切换地域必须重填。
    /// UI 只显示当前地域的那一个框，读写都落在这里。
    #[serde(default)]
    pub api_keys: HashMap<String, String>,
    /// **历史字段**：引入上方的 `api_keys` 之前的唯一 Key。仍被
    /// [`Self::effective_api_key`] 作为兜底读取（老配置在迁移前、以及
    /// llama-asr 这种无地域概念的 provider 都靠它），但设置页不再写入。
    #[serde(default)]
    pub api_key: String,
    /// 非实时（同步）端点。
    #[serde(default)]
    pub endpoint: String,
    /// 实时（流式）WebSocket 端点。与 `endpoint` 分开：协议不同，且用户可能
    /// 只覆盖其一（业务空间专属域名的 HTTP 与 WS 未必同源）。
    #[serde(default)]
    pub ws_endpoint: String,
    #[serde(default)]
    pub model: String,
    /// DashScope 地域 id（见 [`DashScopeRegion`]）；空/未知 = 默认地域。
    #[serde(default)]
    pub region: String,
}

impl ProviderConfig {
    /// 当前地域生效的 API Key。
    ///
    /// 键用**解析后**的地域 id（[`DashScopeRegion::parse`]）而非 `region` 原值：
    /// 空/拼错的地域在别处一律回退默认地域，这里必须用同一个键，否则会出现
    /// 「端点按北京派生、Key 却按未知 id 查不到」而静默变空。
    ///
    /// 回退链：`api_keys[地域]`（非空白）→ 顶层 `api_key`（迁移前的历史配置，
    /// 以及 llama-asr——它无地域概念，`api_keys` 恒空）。
    pub fn effective_api_key(&self) -> String {
        let region_id = DashScopeRegion::parse(&self.region).id();
        self.api_keys
            .get(region_id)
            .map(String::as_str)
            .filter(|k| !k.trim().is_empty())
            .unwrap_or(self.api_key.as_str())
            .to_string()
    }

    /// 转换为 provider 内部使用的凭据结构。
    ///
    /// **热词不在这里**：热词是逐角色的，按调用经 `AsrOptions::hotwords` 传入
    /// （见 [`super::provider::AsrOptions`]），provider 配置里不再有热词存储。
    pub fn to_credentials(&self) -> ProviderCredentials {
        ProviderCredentials {
            api_key: self.effective_api_key(),
            endpoint: self.endpoint.clone(),
            ws_endpoint: self.ws_endpoint.clone(),
            model: self.model.clone(),
            region: self.region.clone(),
        }
    }
}

/// 全部一次性迁移的**唯一入口**，调用方只该调它。
///
/// 内部顺序固定且不可交换：先补地域与端点，再按地域搬 API Key —— 反过来会
/// 把 Key 搬到（尚未补齐的）错误地域上。合成一个函数就是为了让这个顺序
/// 无法被调用方弄错。
///
/// 返回是否有变更（两段各自判断，任一有变更即为 true），调用方据此决定要不要落盘。
pub fn migrate_provider_cfg(provider_id: &str, cfg: &mut ProviderConfig) -> bool {
    let region_fixed = migrate_region_and_endpoints(provider_id, cfg);
    let key_moved = migrate_api_key_to_region(provider_id, cfg);
    region_fixed || key_moved
}

/// 一次性迁移引入地域/双端点之前的 qwen 老配置。
///
/// **只对 [`QwenAsrProvider`] 生效**：llama-asr 的本地地址没有地域语义，
/// 套用下面的规则会把它污染成 DashScope 域名。
///
/// 以 `region` 是否为空作为「是否已迁移」的判据 —— 迁移过就不再干预，
/// 尊重用户后续的手改与清空（否则每次启动都会把用户清掉的端点填回去）。
/// 返回是否有变更，调用方据此决定要不要落盘。
fn migrate_region_and_endpoints(provider_id: &str, cfg: &mut ProviderConfig) -> bool {
    if provider_id != QwenAsrProvider::ID || !cfg.region.trim().is_empty() {
        return false;
    }

    // 地域推断顺序：端点 host 反推（含业务空间专属域名）→ 默认地域
    let region = DashScopeRegion::from_endpoint_hint(&cfg.endpoint)
        .or_else(|| DashScopeRegion::from_endpoint_hint(&cfg.ws_endpoint))
        .unwrap_or(DashScopeRegion::DEFAULT);
    cfg.region = region.id().to_string();

    // 老版本的 `ModelInfo.endpoint` 预设会在选中流式模型时把唯一那个 endpoint
    // 改写成 `wss://...`。不搬走的话，设置页的「非实时端点」输入框里会躺着一个
    // WebSocket 地址，用户无法理解。
    let trimmed = cfg.endpoint.trim();
    if trimmed.starts_with("ws://") || trimmed.starts_with("wss://") {
        if cfg.ws_endpoint.trim().is_empty() {
            cfg.ws_endpoint = trimmed.to_string();
        }
        cfg.endpoint.clear();
    }

    // 空端点填该地域默认值：让设置页显示真实生效的地址而非空白
    // （后端在空值时也会派生同样的默认，这里只是把它显式化）
    if cfg.endpoint.trim().is_empty() {
        cfg.endpoint = region.http_endpoint();
    }
    if cfg.ws_endpoint.trim().is_empty() {
        cfg.ws_endpoint = region.ws_endpoint();
    }
    true
}

/// 一次性迁移引入「按地域分存 API Key」之前的那个唯一的 `api_key`。
///
/// **只对 [`QwenAsrProvider`] 生效**：llama-asr 的 Key 与地域无关（本地服务
/// 的 `--api-key`），搬进映射反而会随地域切换而"消失"。
///
/// 以 `api_keys` 是否为空作为「是否已迁移」的判据 —— 不能用 `region`（那条
/// 判据属于上一段迁移，凡是启动过一次的配置都已置位，这里会被永远跳过）。
/// 已迁移过的配置不再干预，尊重用户后来的清空。
///
/// 搬到**解析后**的地域（与 [`ProviderConfig::effective_api_key`] 同键）：迁移时 `region`
/// 可能为空或拼错，而读取侧一律按默认地域兜底，两边必须一致。
/// 迁移成功后清空顶层 `api_key`，不留两份真相。
fn migrate_api_key_to_region(provider_id: &str, cfg: &mut ProviderConfig) -> bool {
    if provider_id != QwenAsrProvider::ID || !cfg.api_keys.is_empty() {
        return false;
    }
    let legacy = cfg.api_key.trim();
    if legacy.is_empty() {
        return false;
    }
    let region_id = DashScopeRegion::parse(&cfg.region).id();
    cfg.api_keys
        .insert(region_id.to_string(), legacy.to_string());
    cfg.api_key.clear();
    true
}

/// ASR 全局设置。
#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct AsrSettings {
    pub active_provider: String,
    pub auto_listen: bool,
    pub send_mode: SendMode,
    pub stream_enabled: bool,
    pub voice_input_enabled: bool,
    /// VAD 静音计时（毫秒）：停止说话后静音该时长才结束一轮录音（默认 800ms）。
    pub vad_silence_ms: u32,
    /// 能量监测启动缓冲期（毫秒）：TTS 播完恢复监听后该时长内不触发录音
    /// （默认 100，0=无缓冲）。历史上前端私有字段，schema 归后端统一存储。
    pub energy_warmup_ms: u32,
    /// 是否输出逐帧 VAD 能量检测日志（frame/prob/len）。默认关：录音期间每秒
    /// 一条，只在排查「语音识别为什么不触发」时打开（见 [`super::debug_log`]）。
    pub vad_debug_log: bool,
    /// 语音输入快捷键（按住说话 PTT / 单击 toggle / auto_listen 模式开时切换自动监听）。
    /// 存储 ShortcutBinding 序列化 JSON（如 {"key":"f8"} / {"key":"f8","ctrl":true}），默认裸 F8。
    pub ptt_key: String,
    /// 失去焦点快捷键可用（全局快捷键）：开启后窗口不在前台时语音快捷键仍可用。
    /// 仅桌面端注册 OS 级全局快捷键；默认关（全局注册是 OS 级抢占）。
    pub ptt_global: bool,
    pub provider_configs: HashMap<String, ProviderConfig>,
}

impl AsrSettings {
    /// 返回默认值，按 [`super::provider::list_provider_info`] 注册表填齐每个 provider 的占位配置。
    pub fn defaults() -> Self {
        let mut provider_configs = HashMap::new();
        for info in super::provider::list_provider_info() {
            provider_configs.insert(info.id.to_string(), ProviderConfig::default());
        }
        Self {
            active_provider: "qwen-asr".into(),
            auto_listen: false,
            send_mode: SendMode::FillOnly,
            stream_enabled: false,
            // 语音输入默认关闭：仅影响全新用户（无持久化数据时用 defaults）；
            // 老用户 settings.json ASR_PREFS 的持久化值会覆盖此默认
            voice_input_enabled: false,
            vad_silence_ms: 800,
            energy_warmup_ms: 100,
            // 逐帧 VAD 日志默认关（新增字段缺省即为关，与前端 DEFAULT_SETTINGS 一致）
            vad_debug_log: false,
            ptt_key: default_ptt_key(),
            // 全局快捷键默认关：OS 级抢占，仅用户显式开启
            ptt_global: false,
            provider_configs,
        }
    }
}

const STORE_KEY_PROVIDERS: &str = "ASR_PROVIDERS";
const STORE_KEY_ACTIVE: &str = "ASR_ACTIVE_PROVIDER_ID";
const STORE_KEY_PREFS: &str = "ASR_PREFS";

/// UI 偏好字段（auto_listen / send_mode / 流式 / 总开关 / 静音计时），与 provider 凭据分开持久化。
#[derive(Serialize, Deserialize, Clone, Debug, Default)]
pub struct AsrPrefs {
    #[serde(default)]
    pub auto_listen: bool,
    #[serde(default)]
    pub send_mode: SendMode,
    #[serde(default)]
    pub stream_enabled: bool,
    // 新字段缺省必须为 true（其余字段 #[serde(default)] 缺省 false，
    // 直接 default 会让旧持久化数据反序列化后语音输入被意外禁用）。
    #[serde(default = "default_true")]
    pub voice_input_enabled: bool,
    // 同理：缺省为 0 会让老数据的 VAD 静音计时变成 0（一静音立即切段）。
    #[serde(default = "default_vad_silence_ms")]
    pub vad_silence_ms: u32,
    // 同理：缺省为 0 会让老数据恢复后能量监测无缓冲（TTS 残响立即误触发）。
    #[serde(default = "default_energy_warmup_ms")]
    pub energy_warmup_ms: u32,
    // 逐帧 VAD 日志开关：bool 缺省 false 即正确（默认关闭），无需 default 函数
    #[serde(default)]
    pub vad_debug_log: bool,
    // PTT 快捷键：新字段缺省必须给显式默认（serde default 函数），
    // 否则旧持久化数据反序列化后 ptt_key 为空串 → 前端解析失败虽会回退 F8，
    // 但显式默认让默认值唯一真相在后端
    #[serde(default = "default_ptt_key")]
    pub ptt_key: String,
    // 失去焦点快捷键可用：默认关——全局注册会拦截系统级按键，默认开启风险不可接受。
    // bool 缺省 false 即正确，无需 default 函数（与 voice_input_enabled 的 true 兜底相反）。
    #[serde(default)]
    pub ptt_global: bool,
}

/// AsrPrefs 的 voice_input_enabled 兜底：默认开启。
fn default_true() -> bool {
    true
}

/// AsrPrefs 的 vad_silence_ms 兜底：默认 800ms。
fn default_vad_silence_ms() -> u32 {
    800
}

/// AsrPrefs 的 energy_warmup_ms 兜底：默认 100ms（与前端 DEFAULT_SETTINGS 一致）。
fn default_energy_warmup_ms() -> u32 {
    100
}

/// AsrPrefs 的 ptt_key 兜底：默认裸 F8（ShortcutBinding JSON 格式，与前端 DEFAULT_SETTINGS 一致）。
fn default_ptt_key() -> String {
    "{\"key\":\"f8\"}".into()
}

impl AsrPrefs {
    fn from_settings(s: &AsrSettings) -> Self {
        Self {
            auto_listen: s.auto_listen,
            send_mode: s.send_mode.clone(),
            stream_enabled: s.stream_enabled,
            voice_input_enabled: s.voice_input_enabled,
            vad_silence_ms: s.vad_silence_ms,
            energy_warmup_ms: s.energy_warmup_ms,
            vad_debug_log: s.vad_debug_log,
            ptt_key: s.ptt_key.clone(),
            ptt_global: s.ptt_global,
        }
    }

    fn apply_to(&self, s: &mut AsrSettings) {
        s.auto_listen = self.auto_listen;
        s.send_mode = self.send_mode.clone();
        s.stream_enabled = self.stream_enabled;
        s.voice_input_enabled = self.voice_input_enabled;
        s.vad_silence_ms = self.vad_silence_ms;
        s.energy_warmup_ms = self.energy_warmup_ms;
        s.vad_debug_log = self.vad_debug_log;
        s.ptt_key = self.ptt_key.clone();
        s.ptt_global = self.ptt_global;
    }
}

/// 从 `settings.json` 加载 ASR 设置。缺失字段用 defaults；malformed JSON 走 fallback + warn。
pub fn load(app: &AppHandle) -> Result<AsrSettings, AsrError> {
    let store = app
        .store("settings.json")
        .map_err(|e| AsrError::EngineLoadFailed(format!("store: {e}")))?;
    let mut s = AsrSettings::defaults();
    if let Some(v) = store.get(STORE_KEY_PROVIDERS) {
        match serde_json::from_value::<HashMap<String, ProviderConfig>>(v) {
            Ok(map) => {
                // 仅覆盖已存在的 provider，未注册的 provider 跳过（避免脏数据）
                for (k, v) in map {
                    s.provider_configs.insert(k, v);
                }
            },
            Err(e) => tracing::warn!("[ASR] ASR_PROVIDERS malformed: {e}"),
        }
    }
    if let Some(v) = store.get(STORE_KEY_ACTIVE) {
        if let Some(id) = v.as_str() {
            s.active_provider = id.to_string();
        }
    }
    // 兼容：active_provider 指向已删除的 provider（旧版本数据）→ 回退默认
    if !super::provider::list_provider_info()
        .iter()
        .any(|p| p.id == s.active_provider)
    {
        tracing::warn!(
            "[ASR] active_provider '{}' 不在注册表中，回退默认",
            s.active_provider
        );
        s.active_provider = "qwen-asr".into();
    }
    // UI 偏好：独立 key 读取，缺省保持 defaults
    if let Some(v) = store.get(STORE_KEY_PREFS) {
        match serde_json::from_value::<AsrPrefs>(v) {
            Ok(prefs) => prefs.apply_to(&mut s),
            Err(e) => tracing::warn!("[ASR] ASR_PREFS malformed: {e}"),
        }
    }
    Ok(s)
}

/// 把 ASR 设置写回 `settings.json`（全量：providers + active + UI 偏好）。
pub fn save(app: &AppHandle, s: &AsrSettings) -> Result<(), AsrError> {
    let store = app
        .store("settings.json")
        .map_err(|e| AsrError::EngineLoadFailed(format!("store: {e}")))?;
    let providers_json = serde_json::to_value(&s.provider_configs)
        .map_err(|e| AsrError::EngineLoadFailed(format!("serialize providers: {e}")))?;
    let prefs_json = serde_json::to_value(AsrPrefs::from_settings(s))
        .map_err(|e| AsrError::EngineLoadFailed(format!("serialize prefs: {e}")))?;
    store.set(STORE_KEY_PROVIDERS, providers_json);
    store.set(
        STORE_KEY_ACTIVE,
        serde_json::Value::String(s.active_provider.clone()),
    );
    store.set(STORE_KEY_PREFS, prefs_json);
    store
        .save()
        .map_err(|e| AsrError::EngineLoadFailed(format!("store save: {e}")))?;
    Ok(())
}
