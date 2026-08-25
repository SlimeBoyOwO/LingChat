//! Persistent main-menu corruption selected by a script.
//!
//! The effect is deliberately preset-only: scripts may select a bounded theme
//! and a short plain-text message, but cannot inject CSS, HTML, URLs, or paths.

use std::fs;
#[cfg(windows)]
use std::os::windows::fs::MetadataExt;
use std::path::{Path, PathBuf};

use anyhow::{anyhow, Context, Result};
use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::ai_service::game_system::script_engine::events::{
    parse_duration, register_event, ScriptContext, ScriptEvent,
};
use crate::ai_service::tools::atomic_replace;

const STATE_FILE: &str = "script_menu_effect.json";
const MAX_MESSAGE_CHARS: usize = 160;
const THEMES: [&str; 3] = ["normal", "blood", "ghost"];

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub struct ScriptMenuEffectState {
    pub version: u8,
    pub owner: String,
    pub theme: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub message: Option<String>,
}

pub struct MenuEffectEvent {
    theme: String,
    message: Option<String>,
    duration: Option<f64>,
}

impl MenuEffectEvent {
    fn from_event_data(data: &Value) -> Self {
        let theme = data
            .get("theme")
            .and_then(Value::as_str)
            .unwrap_or("normal")
            .trim()
            .to_ascii_lowercase();
        let message = data
            .get("message")
            .and_then(Value::as_str)
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .map(ToOwned::to_owned);
        Self {
            theme,
            message,
            duration: parse_duration(data),
        }
    }
}

fn state_path(data_dir: &Path) -> PathBuf {
    data_dir.join(STATE_FILE)
}

fn is_link_like(metadata: &fs::Metadata) -> bool {
    if metadata.file_type().is_symlink() {
        return true;
    }
    #[cfg(windows)]
    {
        const FILE_ATTRIBUTE_REPARSE_POINT: u32 = 0x0400;
        return metadata.file_attributes() & FILE_ATTRIBUTE_REPARSE_POINT != 0;
    }
    #[cfg(not(windows))]
    false
}

#[derive(Debug, Clone)]
pub(crate) struct MenuEffectSnapshot(Option<Vec<u8>>);

pub(crate) fn snapshot_menu_effect_file(data_dir: &Path) -> Result<MenuEffectSnapshot> {
    let path = state_path(data_dir);
    let bytes = match fs::symlink_metadata(&path) {
        Ok(metadata) => {
            if is_link_like(&metadata) || !metadata.is_file() || metadata.len() > 8 * 1024 {
                return Err(anyhow!("主菜单特效状态不是安全的小型普通文件"));
            }
            Some(fs::read(&path).context("读取主菜单特效快照失败")?)
        }
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => None,
        Err(error) => return Err(error).context("读取主菜单特效状态失败"),
    };
    Ok(MenuEffectSnapshot(bytes))
}

pub(crate) fn restore_menu_effect_snapshot(
    data_dir: &Path,
    snapshot: &MenuEffectSnapshot,
) -> Result<()> {
    let path = state_path(data_dir);
    let changed = match &snapshot.0 {
        Some(bytes) => {
            atomic_replace(&path, bytes)
                .map_err(|error| anyhow!("回滚主菜单特效状态失败: {error}"))?;
            true
        }
        None => match fs::remove_file(&path) {
            Ok(()) => true,
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => false,
            Err(error) => return Err(error).context("回滚空主菜单特效状态失败"),
        },
    };
    if changed {
        crate::ai_service::game_system::script_engine::dlc_transaction::sync_directory(data_dir)
            .context("提交菜单特效快照回滚失败")?;
    }
    Ok(())
}

fn validate_message(message: Option<String>) -> Result<Option<String>> {
    let Some(message) = message else {
        return Ok(None);
    };
    if message.chars().count() > MAX_MESSAGE_CHARS {
        return Err(anyhow!(
            "main_menu_effect.message 最多允许 {} 个字符",
            MAX_MESSAGE_CHARS
        ));
    }
    if message
        .chars()
        .any(|character| character.is_control() && !matches!(character, '\n' | '\t'))
    {
        return Err(anyhow!("main_menu_effect.message 包含不允许的控制字符"));
    }
    Ok(Some(message))
}

pub fn read_menu_effect(data_dir: &Path) -> Option<ScriptMenuEffectState> {
    let path = state_path(data_dir);
    let bytes = fs::read(path).ok()?;
    if bytes.len() > 8 * 1024 {
        return None;
    }
    let state: ScriptMenuEffectState = serde_json::from_slice(&bytes).ok()?;
    if state.version != 1
        || state.owner.is_empty()
        || state.owner.chars().count() > 512
        || !THEMES.contains(&state.theme.as_str())
        || state.theme == "normal"
        || validate_message(state.message.clone()).is_err()
    {
        return None;
    }
    Some(state)
}

fn write_menu_effect(data_dir: &Path, state: &ScriptMenuEffectState) -> Result<()> {
    let bytes = serde_json::to_vec_pretty(state).context("序列化主菜单特效状态失败")?;
    atomic_replace(&state_path(data_dir), &bytes)
        .map_err(|error| anyhow!("保存主菜单特效状态失败: {error}"))?;
    crate::ai_service::game_system::script_engine::dlc_transaction::sync_directory(data_dir)
        .context("提交主菜单特效状态失败")
}

pub fn clear_menu_effect(data_dir: &Path) -> Result<bool> {
    let path = state_path(data_dir);
    match fs::remove_file(path) {
        Ok(()) => {
            crate::ai_service::game_system::script_engine::dlc_transaction::sync_directory(
                data_dir,
            )
            .context("提交主菜单特效清除失败")?;
            Ok(true)
        }
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(false),
        Err(error) => Err(error).context("清除主菜单特效状态失败"),
    }
}

/// 删角色文件彩蛋（DDLC ghost menu 的对应物）：剧本至少真正进过一次（有运行状态
/// 记录）、声明了 character_files 管理，但其角色标记目录里一个 .chr 都不剩——
/// 不管第几幕，主菜单变 ghost；玩家把 .chr 放回后下次读取自动恢复正常。
///
/// 目录本身不存在不算"删"——刚进剧本就退出、标记尚未创建的情形不能误伤；
/// 只有"目录在、文件没了"才是玩家故意删文件的强信号。
pub(crate) fn ghost_owner_for_deleted_markers(
    data_dir: &Path,
    declared_keys: &std::collections::HashSet<String>,
) -> Option<String> {
    for path_key in
        crate::ai_service::game_system::script_engine::persistent_state::played_script_keys(
            data_dir,
        )
    {
        if !declared_keys.contains(&path_key) {
            continue;
        }
        let Ok(namespace) = super::character_file_event::namespace_from_path_key(&path_key)
        else {
            continue;
        };
        let dir = super::character_file_event::external_characters_root(data_dir).join(namespace);
        let Ok(entries) = fs::read_dir(&dir) else {
            continue;
        };
        let chr_count = entries
            .flatten()
            .filter(|entry| entry.path().extension().is_some_and(|ext| ext == "chr"))
            .count();
        if chr_count == 0 {
            tracing::info!(
                "[MenuEffect] 剧本 '{}' 的角色标记被全部删除，主菜单切换为 ghost 主题",
                path_key
            );
            return Some(path_key);
        }
    }
    None
}

pub fn clear_menu_effect_for_owner(data_dir: &Path, owner: &str) -> Result<bool> {
    let Some(state) = read_menu_effect(data_dir) else {
        return Ok(false);
    };
    if state.owner != owner {
        return Ok(false);
    }
    clear_menu_effect(data_dir)
}

#[async_trait]
impl ScriptEvent for MenuEffectEvent {
    async fn execute(&mut self, ctx: &mut ScriptContext<'_>) -> Result<Option<String>> {
        if !THEMES.contains(&self.theme.as_str()) {
            return Err(anyhow!(
                "main_menu_effect.theme 只支持 normal / blood / ghost，收到 '{}'",
                self.theme
            ));
        }

        if ctx.is_preview {
            if self.theme != "normal" {
                validate_message(self.message.clone())?;
            }
            tracing::info!(
                "[MenuEffectEvent] 试玩隔离：仅预览主题配置 {}，不改持久菜单",
                self.theme
            );
            return Ok(None);
        }

        if self.theme == "normal" {
            clear_menu_effect(ctx.data_dir)?;
            tracing::info!("[MenuEffectEvent] 已恢复普通主菜单");
            return Ok(None);
        }

        let owner = {
            let game_status = ctx.game_status.lock().await;
            game_status
                .script_status
                .as_ref()
                .ok_or_else(|| anyhow!("ScriptStatus 未设置，无法保存主菜单特效"))?
                .path_key()
        };
        let state = ScriptMenuEffectState {
            version: 1,
            owner,
            theme: self.theme.clone(),
            message: validate_message(self.message.clone())?,
        };
        write_menu_effect(ctx.data_dir, &state)?;
        tracing::info!(
            "[MenuEffectEvent] 主菜单主题设为 {}（owner={}）",
            state.theme,
            state.owner
        );
        Ok(None)
    }

    fn event_type() -> &'static str {
        "main_menu_effect"
    }

    fn duration(&self) -> Option<f64> {
        self.duration
    }
}

pub fn register() {
    register_event(MenuEffectEvent::event_type(), |data| {
        Box::new(MenuEffectEvent::from_event_data(&data))
    });
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rejects_long_message() {
        assert!(validate_message(Some("坏".repeat(MAX_MESSAGE_CHARS + 1))).is_err());
    }

    #[test]
    fn accepts_bounded_message() {
        assert_eq!(
            validate_message(Some("CHARACTER DATA CORRUPTED".to_string())).unwrap(),
            Some("CHARACTER DATA CORRUPTED".to_string())
        );
    }

    #[test]
    fn persistence_is_owner_scoped_and_recoverable() {
        let unique = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let dir = std::env::temp_dir().join(format!(
            "lingchat-menu-effect-{}-{unique}",
            std::process::id()
        ));
        std::fs::create_dir_all(&dir).unwrap();

        let state = ScriptMenuEffectState {
            version: 1,
            owner: "standalone/seventh".to_string(),
            theme: "ghost".to_string(),
            message: Some("STATE PERSISTS".to_string()),
        };
        write_menu_effect(&dir, &state).unwrap();
        let loaded = read_menu_effect(&dir).unwrap();
        assert_eq!(loaded.owner, state.owner);
        assert_eq!(loaded.theme, state.theme);
        assert_eq!(loaded.message, state.message);
        assert!(!clear_menu_effect_for_owner(&dir, "standalone/other").unwrap());
        assert!(read_menu_effect(&dir).is_some());
        let snapshot = snapshot_menu_effect_file(&dir).unwrap();
        assert!(clear_menu_effect_for_owner(&dir, &state.owner).unwrap());
        assert!(read_menu_effect(&dir).is_none());
        restore_menu_effect_snapshot(&dir, &snapshot).unwrap();
        assert_eq!(read_menu_effect(&dir).unwrap().theme, "ghost");
        assert!(clear_menu_effect_for_owner(&dir, &state.owner).unwrap());

        let _ = std::fs::remove_dir_all(dir);
    }
}
