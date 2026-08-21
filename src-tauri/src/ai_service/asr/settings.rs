//! ASR 配置持久化。
//!
//! 复用 `tauri_plugin_store` 的 `settings.json`，使用单 key `ASR_PROVIDERS` 存
//! `Vec<ProviderConfig>` 与单 key `ASR_ACTIVE_PROVIDER_ID` 存当前激活 provider id。
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
    /// AI 生成中时入队，等 ai:reply 终态后再 flush。
    Queue,
}

/// 单个 provider 的配置：API key + endpoint + 任意额外字段。
#[derive(Serialize, Deserialize, Clone, Debug, Default)]
pub struct ProviderConfig {
    #[serde(default)]
    pub api_key: String,
    #[serde(default)]
    pub endpoint: String,
    #[serde(default)]
    pub extra: HashMap<String, String>,
}

impl ProviderConfig {
    /// 转换为 provider 内部使用的凭据结构。
    pub fn to_credentials(&self) -> ProviderCredentials {
        ProviderCredentials {
            api_key: self.api_key.clone(),
            endpoint: self.endpoint.clone(),
        }
    }
}

/// ASR 全局设置。
#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct AsrSettings {
    pub active_provider: String,
    pub auto_listen: bool,
    pub hotkey_enabled: bool,
    pub hotkey_combination: String,
    pub send_mode: SendMode,
    pub stream_enabled: bool,
    pub hotkey_toggle_auto_listen: bool,
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
            active_provider: "openai-whisper".into(),
            auto_listen: false,
            hotkey_enabled: false,
            hotkey_combination: "Ctrl+Shift+Space".into(),
            send_mode: SendMode::FillOnly,
            stream_enabled: false,
            hotkey_toggle_auto_listen: true,
            provider_configs,
        }
    }
}

const STORE_KEY_PROVIDERS: &str = "ASR_PROVIDERS";
const STORE_KEY_ACTIVE: &str = "ASR_ACTIVE_PROVIDER_ID";
const STORE_KEY_PREFS: &str = "ASR_PREFS";

/// UI 偏好字段（auto_listen / hotkey / send_mode / 流式），与 provider 凭据分开持久化。
#[derive(Serialize, Deserialize, Clone, Debug, Default)]
pub struct AsrPrefs {
    #[serde(default)]
    pub auto_listen: bool,
    #[serde(default)]
    pub hotkey_enabled: bool,
    #[serde(default)]
    pub hotkey_combination: String,
    #[serde(default)]
    pub send_mode: SendMode,
    #[serde(default)]
    pub stream_enabled: bool,
    #[serde(default)]
    pub hotkey_toggle_auto_listen: bool,
}

impl AsrPrefs {
    fn from_settings(s: &AsrSettings) -> Self {
        Self {
            auto_listen: s.auto_listen,
            hotkey_enabled: s.hotkey_enabled,
            hotkey_combination: s.hotkey_combination.clone(),
            send_mode: s.send_mode.clone(),
            stream_enabled: s.stream_enabled,
            hotkey_toggle_auto_listen: s.hotkey_toggle_auto_listen,
        }
    }

    fn apply_to(&self, s: &mut AsrSettings) {
        s.auto_listen = self.auto_listen;
        s.hotkey_enabled = self.hotkey_enabled;
        s.hotkey_combination = self.hotkey_combination.clone();
        s.send_mode = self.send_mode.clone();
        s.stream_enabled = self.stream_enabled;
        s.hotkey_toggle_auto_listen = self.hotkey_toggle_auto_listen;
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
            }
            Err(e) => tracing::warn!("[ASR] ASR_PROVIDERS malformed: {e}"),
        }
    }
    if let Some(v) = store.get(STORE_KEY_ACTIVE) {
        if let Some(id) = v.as_str() {
            s.active_provider = id.to_string();
        }
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
