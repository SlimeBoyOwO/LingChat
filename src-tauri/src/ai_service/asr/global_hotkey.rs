//! 语音快捷键全局注册（失去焦点可用）。
//!
//! 通过 `tauri-plugin-global-shortcut` 注册 OS 级全局快捷键：任意应用前台
//! 按下快捷键都触发语音输入（与窗口内 keydown 语义一致）。桌面端专属
//! （`mod.rs` 中 `#[cfg(desktop)]` 声明），移动端不编译本模块。
//!
//! 生命周期：`asr_set_settings` 保存后与启动加载设置后调用 [`sync`]；
//! 幂等（记录当前注册串，未变则跳过）。注册失败（键被占用/插件不支持）
//! 返回 Err → 上层 emit `asr:ptt-global-status` 给设置页提示，开关保持开启。

use std::sync::Mutex;

use tauri::{AppHandle, Manager};
use tauri_plugin_global_shortcut::GlobalShortcutExt;

use super::settings::AsrSettings;

/// 当前已注册的快捷键串（幂等判断）：None=未注册，Some=注册中的组合串。
#[derive(Default)]
pub struct GlobalHotkeyState {
    registered: Mutex<Option<String>>,
}

/// 全局快捷键按键事件（emit 到 main 窗口，前端 useAsrInput 监听驱动状态机）。
#[derive(serde::Serialize, Clone)]
pub struct PttGlobalEvent {
    /// "pressed"（按下开始录音/切换） | "released"（松开结束/判定单击）
    pub state: &'static str,
}

/// 全局注册状态事件（仅失败时 emit，设置页显示原因；开关不自动回退）。
#[derive(serde::Serialize, Clone)]
pub struct PttGlobalStatus {
    pub ok: bool,
    pub reason: String,
}

/// 按当前设置同步全局快捷键注册状态（幂等）：
/// ptt_global 开 → 注册 `ptt_key` 映射的组合串；关 → 注销。
/// 注册失败（键被占用/插件不支持该键）返回 Err，内部状态保持"未注册"。
pub fn sync(app: &AppHandle, settings: &AsrSettings) -> Result<(), String> {
    let state = app.state::<GlobalHotkeyState>();
    let want = if settings.ptt_global {
        binding_to_hotkey_str(&settings.ptt_key)
    } else {
        None
    };
    // 开关开但绑定不可映射（手改 settings.json 的 Enter/非法 JSON）：视为注册失败，
    // 走上层错误路径（asr_set_settings emit 状态提示 / 启动路径 warn）——
    // 开关显示开启与实际注册脱节必须可见，不能静默
    if settings.ptt_global && want.is_none() {
        return Err("当前快捷键无法全局注册（Enter 或非法绑定）".into());
    }
    let mut registered = state.registered.lock().unwrap();
    if *registered == want {
        return Ok(()); // 幂等：注册串未变，跳过（避免每次 settings 保存都重复 register）
    }
    if let Some(prev) = registered.take() {
        // 旧键注销失败静默：可能从未真正注册成功（如启动时失败），unregister 幂等无害
        let _ = app.global_shortcut().unregister(prev.as_str());
    }
    if let Some(combo) = &want {
        app.global_shortcut()
            .register(combo.as_str())
            .map_err(|e| format!("全局快捷键注册失败 ({combo}): {e}"))?;
        *registered = Some(combo.clone());
    }
    Ok(())
}

/// 全局快捷键当前注册是否与设置一致（健康检查，设置页启动查询用）。
/// 开关关或绑定不可映射 → false；开关开且已按当前绑定注册 → true。
pub fn is_healthy(app: &AppHandle, settings: &AsrSettings) -> bool {
    let want = if settings.ptt_global {
        binding_to_hotkey_str(&settings.ptt_key)
    } else {
        None
    };
    let registered = app.state::<GlobalHotkeyState>().registered.lock().unwrap().clone();
    want.is_some() && registered == want
}

/// ShortcutBinding JSON → 插件快捷键字符串（"F8" / "Ctrl+F8"）。
/// JSON 非法/缺 key → None（解析不出则不注册，保守——与前端 resolvePttBinding
/// 回退默认 F8 不同，这里宁可不注册也不猜）；Enter → None（与前端一致拒绝）。
/// 符号键等插件不识别时返回大写原样，由注册失败路径提示用户。
fn binding_to_hotkey_str(raw: &str) -> Option<String> {
    let v: serde_json::Value = serde_json::from_str(raw).ok()?;
    let key = v.get("key")?.as_str()?.to_ascii_lowercase();
    let key_name = map_key(&key)?;
    let mut parts = Vec::new();
    if v.get("ctrl").and_then(|b| b.as_bool()).unwrap_or(false) {
        parts.push("Ctrl");
    }
    if v.get("alt").and_then(|b| b.as_bool()).unwrap_or(false) {
        parts.push("Alt");
    }
    if v.get("shift").and_then(|b| b.as_bool()).unwrap_or(false) {
        parts.push("Shift");
    }
    if v.get("meta").and_then(|b| b.as_bool()).unwrap_or(false) {
        // Windows 下 Super = ⊞ Win 键；meta 语义与前端 e.metaKey 一致
        parts.push("Super");
    }
    parts.push(&key_name);
    Some(parts.join("+"))
}

/// e.key 小写名 → global-hotkey 键名（特殊键显式映射，其余大写原样尝试）。
fn map_key(key: &str) -> Option<String> {
    if key == "enter" {
        return None;
    }
    let name = match key {
        "arrowup" => "ArrowUp",
        "arrowdown" => "ArrowDown",
        "arrowleft" => "ArrowLeft",
        "arrowright" => "ArrowRight",
        " " => "Space",
        "esc" => "Escape",
        "tab" => "Tab",
        "backspace" => "Backspace",
        "delete" => "Delete",
        "home" => "Home",
        "end" => "End",
        "pageup" => "PageUp",
        "pagedown" => "PageDown",
        "insert" => "Insert",
        // F1-F24 / 字母 / 数字 / 标点：大写原样（插件识别则注册成功，否则走失败路径）
        _ => return Some(key.to_uppercase()),
    };
    Some(name.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn maps_bare_f8_default() {
        assert_eq!(binding_to_hotkey_str(r#"{"key":"f8"}"#).as_deref(), Some("F8"));
    }

    #[test]
    fn maps_ctrl_f8_uppercase_key() {
        assert_eq!(
            binding_to_hotkey_str(r#"{"key":"F8","ctrl":true}"#).as_deref(),
            Some("Ctrl+F8")
        );
    }

    #[test]
    fn maps_shift_super_letter() {
        assert_eq!(
            binding_to_hotkey_str(r#"{"key":"a","shift":true,"meta":true}"#).as_deref(),
            Some("Shift+Super+A")
        );
    }

    #[test]
    fn maps_space_and_arrow() {
        assert_eq!(binding_to_hotkey_str(r#"{"key":" "}"#).as_deref(), Some("Space"));
        assert_eq!(
            binding_to_hotkey_str(r#"{"key":"arrowup"}"#).as_deref(),
            Some("ArrowUp")
        );
    }

    #[test]
    fn rejects_enter() {
        assert_eq!(binding_to_hotkey_str(r#"{"key":"Enter"}"#), None);
    }

    #[test]
    fn malformed_json_returns_none() {
        assert_eq!(binding_to_hotkey_str("not json"), None);
        assert_eq!(binding_to_hotkey_str(r#"{"ctrl":true}"#), None);
    }

    #[test]
    fn false_modifiers_are_omitted() {
        assert_eq!(
            binding_to_hotkey_str(r#"{"key":"f8","ctrl":false,"alt":false}"#).as_deref(),
            Some("F8")
        );
    }
}
