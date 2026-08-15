//! 敏感凭据加密存储（对标 NORP 的 keyring/DPAPI 实践）。
//!
//! 桌面端（Windows/macOS/Linux）使用系统 keyring 存储敏感凭据：
//! - Windows：Credential Manager（DPAPI 加密）
//! - macOS：Keychain
//! - Linux：Secret Service / kernel keyutils
//!
//! `settings.json` / `tool_settings.toml` 只保留非敏感配置；敏感值一律
//! 迁移到 keyring 并清空明文。移动端（Android/iOS）未接入 keyring 时，
//! 保持原有明文行为（降级），并在日志中告警。
//!
//! keyring 条目命名：service = `com.noiq.ling-chat`，user = 配置键。

use tauri::AppHandle;
use tauri_plugin_store::StoreExt;

/// settings.json 中的敏感凭据键（明文迁移到 keyring）。
pub const SECRET_SETTINGS_KEYS: &[&str] = &[
    "llm.api_key",
    "translate.api_key",
    "tts.aivis_api_key",
    "tts.opentts_api_key",
    "workshop.github_token",
    "VD_API_KEY",
    "lan_sync.auth_token",
];

/// 网页搜索 API Key 的 keyring 键（原存于 data/tool_settings.toml）。
pub const WEB_SEARCH_SECRET_KEY: &str = "tools.web_search.api_key";

/// 多供应商模式下单个 provider 的 keyring 键。
pub fn provider_secret_key(provider_id: &str) -> String {
    format!("llm.provider.{provider_id}.api_key")
}

#[cfg(not(any(target_os = "android", target_os = "ios")))]
mod backend {
    use super::*;

    const SERVICE: &str = "com.noiq.ling-chat";

    pub fn set(key: &str, value: &str) -> Result<(), String> {
        let entry = keyring::Entry::new(SERVICE, key).map_err(|e| e.to_string())?;
        entry.set_password(value).map_err(|e| e.to_string())
    }

    pub fn get(key: &str) -> Result<Option<String>, String> {
        let entry = keyring::Entry::new(SERVICE, key).map_err(|e| e.to_string())?;
        match entry.get_password() {
            Ok(value) => Ok(Some(value)),
            Err(keyring::Error::NoEntry) => Ok(None),
            Err(e) => Err(e.to_string()),
        }
    }

    pub fn delete(key: &str) -> Result<(), String> {
        let entry = keyring::Entry::new(SERVICE, key).map_err(|e| e.to_string())?;
        match entry.delete_credential() {
            Ok(()) | Err(keyring::Error::NoEntry) => Ok(()),
            Err(e) => Err(e.to_string()),
        }
    }
}

#[cfg(any(target_os = "android", target_os = "ios"))]
mod backend {
    // 移动端降级：不接入系统 keyring，凭据保持原有存储位置。
    pub fn set(_key: &str, _value: &str) -> Result<(), String> {
        Ok(())
    }

    pub fn get(_key: &str) -> Result<Option<String>, String> {
        Ok(None)
    }

    pub fn delete(_key: &str) -> Result<(), String> {
        Ok(())
    }
}

/// 当前平台是否具备可用的凭据加密存储（桌面端 keyring；移动端无）。
pub fn secret_storage_available() -> bool {
    cfg!(not(any(target_os = "android", target_os = "ios")))
}

/// 写入凭据。空值视为删除。
pub fn set_secret(key: &str, value: &str) -> Result<(), String> {
    if value.is_empty() {
        delete_secret(key)
    } else {
        backend::set(key, value)
    }
}

/// 读取凭据；不存在返回 `Ok(None)`，keyring 不可用返回 `Err`（调用方回退明文）。
pub fn get_secret(key: &str) -> Result<Option<String>, String> {
    backend::get(key)
}

/// 删除凭据（不存在视为成功）。
pub fn delete_secret(key: &str) -> Result<(), String> {
    backend::delete(key)
}

/// 读取凭据，keyring 缺失/失败时回退到给定明文。
pub fn get_secret_or(key: &str, plaintext: Option<String>) -> Option<String> {
    match get_secret(key) {
        Ok(Some(value)) => Some(value),
        Ok(None) => plaintext.filter(|v| !v.is_empty()),
        Err(e) => {
            tracing::warn!("读取 keyring 凭据失败（回退明文）: {key}: {e}");
            plaintext.filter(|v| !v.is_empty())
        }
    }
}

/// 启动时执行一次明文 → keyring 迁移（幂等）。
///
/// 覆盖 settings.json 的扁平敏感键与 `llm.providers` 数组内的 api_key。
/// 迁移后明文字段清空。移动端降级：不迁移，保留明文并告警。
pub fn migrate_settings_secrets(app: &AppHandle) -> Result<(), String> {
    #[cfg(any(target_os = "android", target_os = "ios"))]
    {
        let _ = app;
        tracing::warn!(
            "当前平台未接入系统 keyring，敏感凭据保持明文存储（建议使用桌面端）"
        );
        return Ok(());
    }

    #[cfg(not(any(target_os = "android", target_os = "ios")))]
    {
        let store = app
            .store(crate::config::STORE_FILE)
            .map_err(|e| format!("打开设置存储失败: {e}"))?;
        let mut changed = false;

        // 1) 扁平敏感键
        for key in SECRET_SETTINGS_KEYS {
            let plain = store
                .get(*key)
                .and_then(|v| v.as_str().map(|s| s.to_string()))
                .unwrap_or_default();
            if !plain.is_empty() {
                backend::set(key, &plain).map_err(|e| format!("迁移凭据 {key} 失败: {e}"))?;
                store.set(key.to_string(), serde_json::Value::String(String::new()));
                changed = true;
                tracing::info!("已迁移敏感凭据到 keyring: {key}");
            }
        }

        // 2) 多供应商 api_key
        if let Some(serde_json::Value::Array(arr)) = store.get(crate::config::keys::LLM_PROVIDERS)
        {
            let mut providers: Vec<serde_json::Value> = Vec::new();
            for value in arr {
                let mut entry = value.clone();
                let migrated = entry
                    .as_object_mut()
                    .and_then(|obj| {
                        let id = obj.get("id")?.as_str()?;
                        let key = obj.get("api_key")?.as_str()?;
                        Some((id.to_string(), key.to_string()))
                    })
                    .and_then(|(id, key)| {
                        if key.is_empty() {
                            None
                        } else {
                            Some((id, key))
                        }
                    });
                if let Some((id, plain)) = migrated {
                    if let Err(e) = backend::set(&provider_secret_key(&id), &plain) {
                        tracing::warn!("迁移 provider 凭据失败（保留明文）: {id}: {e}");
                        providers.push(entry);
                        continue;
                    }
                    if let Some(obj) = entry.as_object_mut() {
                        obj.insert("api_key".to_string(), serde_json::json!(""));
                        changed = true;
                        tracing::info!("已迁移 provider 凭据到 keyring: {id}");
                    }
                }
                providers.push(entry);
            }
            if changed {
                store.set(
                    crate::config::keys::LLM_PROVIDERS.to_string(),
                    serde_json::Value::Array(providers),
                );
            }
        }

        if changed {
            store.save().map_err(|e| format!("保存设置存储失败: {e}"))?;
        }
        Ok(())
    }
}

/// 启动时执行一次 `data/tool_settings.toml` 中网页搜索 API Key 的迁移（幂等）。
pub fn migrate_tool_settings_secret(data_dir: &std::path::Path) -> Result<(), String> {
    #[cfg(any(target_os = "android", target_os = "ios"))]
    {
        let _ = data_dir;
        return Ok(());
    }

    #[cfg(not(any(target_os = "android", target_os = "ios")))]
    {
        let path = data_dir.join(crate::ai_service::tools::settings::SETTINGS_FILE_NAME);
        if !path.exists() {
            return Ok(());
        }
        let text = std::fs::read_to_string(&path).map_err(|e| format!("读取工具配置失败: {e}"))?;
        let mut doc: toml::Value =
            toml::from_str(&text).map_err(|e| format!("解析工具配置失败: {e}"))?;
        let plain = doc
            .get("web_search")
            .and_then(|w| w.get("api_key"))
            .and_then(|v| v.as_str())
            .unwrap_or("")
            .to_string();
        if plain.is_empty() {
            return Ok(());
        }
        backend::set(WEB_SEARCH_SECRET_KEY, &plain)
            .map_err(|e| format!("迁移网页搜索 API Key 失败: {e}"))?;
        if let Some(table) = doc
            .get_mut("web_search")
            .and_then(|w| w.as_table_mut())
        {
            table.insert(
                "api_key".to_string(),
                toml::Value::String(String::new()),
            );
        }
        let text = toml::to_string_pretty(&doc).map_err(|e| format!("序列化工具配置失败: {e}"))?;
        std::fs::write(&path, text).map_err(|e| format!("保存工具配置失败: {e}"))?;
        tracing::info!("已迁移网页搜索 API Key 到 keyring");
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn provider_key_naming_is_stable() {
        assert_eq!(
            provider_secret_key("abc-123"),
            "llm.provider.abc-123.api_key"
        );
    }

    #[test]
    fn secret_settings_keys_include_main_credentials() {
        assert!(SECRET_SETTINGS_KEYS.contains(&"llm.api_key"));
        assert!(SECRET_SETTINGS_KEYS.contains(&"translate.api_key"));
        assert!(SECRET_SETTINGS_KEYS.contains(&"tts.aivis_api_key"));
        assert!(SECRET_SETTINGS_KEYS.contains(&"tts.opentts_api_key"));
        assert!(SECRET_SETTINGS_KEYS.contains(&"workshop.github_token"));
    }
}
