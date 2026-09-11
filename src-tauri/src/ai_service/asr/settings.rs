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
use super::provider::ProviderCredentials;

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

/// 单个 provider 的配置：API key + endpoint + 模型 + 任意额外字段。
#[derive(Serialize, Deserialize, Clone, Debug, Default)]
pub struct ProviderConfig {
    #[serde(default)]
    pub api_key: String,
    #[serde(default)]
    pub endpoint: String,
    #[serde(default)]
    pub model: String,
    #[serde(default)]
    pub extra: HashMap<String, String>,
}

impl ProviderConfig {
    /// 转换为 provider 内部使用的凭据结构。
    pub fn to_credentials(&self) -> ProviderCredentials {
        ProviderCredentials {
            api_key: self.api_key.clone(),
            endpoint: self.endpoint.clone(),
            model: self.model.clone(),
            // 热词接口（llama-asr 的 prompt 偏置）：从 extra["hotwords"] 读
            // 逗号/分号/空白分隔的列表。设置页暂不做输入 UI（先不做热词输入），
            // 此处保留接口——后续要加 UI 时只动前端，后端已就绪。
            hotwords: self
                .extra
                .get("hotwords")
                .map(|s| {
                    s.split(|c: char| c == ',' || c == ';' || c.is_whitespace())
                        .map(str::trim)
                        .filter(|s| !s.is_empty())
                        .map(str::to_string)
                        .collect()
                })
                .unwrap_or_default(),
        }
    }
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

impl AsrPrefs {
    fn from_settings(s: &AsrSettings) -> Self {
        Self {
            auto_listen: s.auto_listen,
            send_mode: s.send_mode.clone(),
            stream_enabled: s.stream_enabled,
            voice_input_enabled: s.voice_input_enabled,
            vad_silence_ms: s.vad_silence_ms,
            energy_warmup_ms: s.energy_warmup_ms,
        }
    }

    fn apply_to(&self, s: &mut AsrSettings) {
        s.auto_listen = self.auto_listen;
        s.send_mode = self.send_mode.clone();
        s.stream_enabled = self.stream_enabled;
        s.voice_input_enabled = self.voice_input_enabled;
        s.vad_silence_ms = self.vad_silence_ms;
        s.energy_warmup_ms = self.energy_warmup_ms;
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn to_credentials_parses_hotwords_from_extra() {
        let cfg = ProviderConfig {
            api_key: "k".into(),
            endpoint: "http://127.0.0.1:8080".into(),
            model: "models/Qwen3-ASR-1.7B-Q8_0.gguf".into(),
            extra: [(
                "hotwords".to_string(),
                "Quantinuum, Anthropic, 量子计算".to_string(),
            )]
            .into_iter()
            .collect(),
        };
        let cred = cfg.to_credentials();
        assert_eq!(cred.hotwords, vec!["Quantinuum", "Anthropic", "量子计算"]);
    }

    #[test]
    fn to_credentials_empty_hotwords_without_extra() {
        let cfg = ProviderConfig::default();
        assert!(cfg.to_credentials().hotwords.is_empty());
    }

    #[test]
    fn to_credentials_tolerates_semicolon_and_whitespace() {
        let cfg = ProviderConfig {
            extra: [("hotwords".to_string(), "A;B  C, D\nE".to_string())]
                .into_iter()
                .collect(),
            ..Default::default()
        };
        let cred = cfg.to_credentials();
        assert_eq!(cred.hotwords, vec!["A", "B", "C", "D", "E"]);
    }
}
