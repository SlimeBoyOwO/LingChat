//! Script-owned external character marker files.
//!
//! Files live beside the portable `data/` directory, under a namespace owned
//! by the active script:
//! `<data parent>/characters/<full script path key>/<declared name>.chr`.
//! A script may only touch names declared in `script_settings.character_files`
//! and may only restore bytes shipped in its own `CharacterFiles/` directory.

use std::fs;
#[cfg(windows)]
use std::os::windows::fs::MetadataExt;
use std::path::{Component, Path, PathBuf};

use anyhow::{anyhow, Context, Result};
use async_trait::async_trait;
use serde_json::Value;

use crate::ai_service::game_system::script_engine::events::{
    parse_duration, register_event, ScriptContext, ScriptEvent,
};
use crate::ai_service::types::ScriptStatus;
use crate::utils::system::open_folder;

const MAX_FILE_NAME_CHARS: usize = 64;
const MAX_TEMPLATE_BYTES: u64 = 64 * 1024;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum CharacterFileAction {
    Ensure,
    Exists,
    Delete,
    OpenFolder,
}

pub struct CharacterFileEvent {
    action: String,
    file: String,
    result_var: String,
    duration: Option<f64>,
}

impl CharacterFileEvent {
    fn from_event_data(data: &Value) -> Self {
        Self {
            action: data
                .get("action")
                .and_then(Value::as_str)
                .unwrap_or("exists")
                .trim()
                .to_ascii_lowercase(),
            file: data
                .get("file")
                .and_then(Value::as_str)
                .unwrap_or_default()
                .trim()
                .to_string(),
            result_var: data
                .get("resultVar")
                .or_else(|| data.get("result_var"))
                .and_then(Value::as_str)
                .unwrap_or("character_file_exists")
                .trim()
                .to_string(),
            duration: parse_duration(data),
        }
    }
}

fn parse_action(action: &str) -> Result<CharacterFileAction> {
    match action {
        "ensure" => Ok(CharacterFileAction::Ensure),
        "exists" => Ok(CharacterFileAction::Exists),
        "delete" => Ok(CharacterFileAction::Delete),
        "open_folder" => Ok(CharacterFileAction::OpenFolder),
        _ => Err(anyhow!(
            "character_file.action 只支持 ensure / exists / delete / open_folder，收到 '{}'",
            action
        )),
    }
}

fn validate_result_var(result_var: &str) -> Result<()> {
    if result_var.is_empty()
        || result_var.chars().count() > 128
        || result_var.chars().any(char::is_control)
    {
        return Err(anyhow!(
            "character_file.resultVar 必须是 1-128 字符的安全变量名"
        ));
    }
    Ok(())
}

fn is_link_like(metadata: &fs::Metadata) -> bool {
    if metadata.file_type().is_symlink() {
        return true;
    }
    #[cfg(windows)]
    {
        // Junctions and other reparse points are not always reported by
        // FileType::is_symlink, but must not become an escape route either.
        const FILE_ATTRIBUTE_REPARSE_POINT: u32 = 0x0400;
        return metadata.file_attributes() & FILE_ATTRIBUTE_REPARSE_POINT != 0;
    }
    #[cfg(not(windows))]
    false
}

fn validate_file_name(name: &str) -> Result<()> {
    if name.is_empty() || name.chars().count() > MAX_FILE_NAME_CHARS {
        return Err(anyhow!("character_file.file 长度必须为 1-64 个字符"));
    }
    let mut components = Path::new(name).components();
    if !matches!(components.next(), Some(Component::Normal(_))) || components.next().is_some() {
        return Err(anyhow!(
            "character_file.file 必须是单个文件名，不能包含路径"
        ));
    }
    if !name.to_ascii_lowercase().ends_with(".chr") {
        return Err(anyhow!("character_file.file 必须使用 .chr 扩展名"));
    }
    if name.starts_with(' ')
        || name.ends_with(' ')
        || name.ends_with('.')
        || name.chars().any(|character| {
            character.is_control()
                || matches!(
                    character,
                    '<' | '>' | ':' | '"' | '/' | '\\' | '|' | '?' | '*'
                )
        })
    {
        return Err(anyhow!("character_file.file 含 Windows 不安全字符"));
    }
    let stem = &name[..name.len() - 4];
    if stem.is_empty() || stem.ends_with(' ') || stem.ends_with('.') {
        return Err(anyhow!(
            "character_file.file 的文件主名不能为空或以点/空格结尾"
        ));
    }
    // Windows reserves device names even when additional extensions follow
    // (for example `CON.backup.chr`).
    let device_stem = stem
        .split('.')
        .next()
        .unwrap_or_default()
        .to_ascii_uppercase();
    const WINDOWS_DEVICES: [&str; 22] = [
        "CON", "PRN", "AUX", "NUL", "COM1", "COM2", "COM3", "COM4", "COM5", "COM6", "COM7", "COM8",
        "COM9", "LPT1", "LPT2", "LPT3", "LPT4", "LPT5", "LPT6", "LPT7", "LPT8", "LPT9",
    ];
    if WINDOWS_DEVICES.contains(&device_stem.as_str()) {
        return Err(anyhow!("character_file.file 不能使用 Windows 保留设备名"));
    }
    Ok(())
}

fn declared_files(script: &ScriptStatus) -> Vec<String> {
    script
        .settings
        .get("character_files")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .filter_map(Value::as_str)
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(ToOwned::to_owned)
        .collect()
}

fn require_declared(script: &ScriptStatus, file: &str) -> Result<()> {
    validate_file_name(file)?;
    if !declared_files(script)
        .iter()
        .any(|declared| declared == file)
    {
        return Err(anyhow!(
            "character_file.file '{}' 未在 script_settings.character_files 中声明",
            file
        ));
    }
    Ok(())
}

pub(crate) fn namespace_from_path_key(path_key: &str) -> Result<PathBuf> {
    let mut namespace = PathBuf::new();
    for component in Path::new(path_key).components() {
        match component {
            Component::Normal(value) if !value.is_empty() => namespace.push(value),
            _ => return Err(anyhow!("剧本 path_key 含不安全的角色文件命名空间组件")),
        }
    }
    if namespace.components().next().is_none() {
        return Err(anyhow!("无法确定剧本角色文件命名空间"));
    }
    Ok(namespace)
}

fn namespace_of(script: &ScriptStatus) -> Result<PathBuf> {
    namespace_from_path_key(&script.path_key())
}

pub fn external_characters_root(data_dir: &Path) -> PathBuf {
    #[cfg(desktop)]
    {
        data_dir.parent().unwrap_or(data_dir).join("characters")
    }
    #[cfg(not(desktop))]
    {
        data_dir.join("characters")
    }
}

#[cfg(test)]
fn script_character_dir(script: &ScriptStatus, data_dir: &Path) -> Result<PathBuf> {
    Ok(external_characters_root(data_dir).join(namespace_of(script)?))
}

fn walk_namespace(root: &Path, namespace: &Path, create: bool) -> Result<Option<PathBuf>> {
    let mut current = root.to_path_buf();
    for component in namespace.components() {
        let Component::Normal(value) = component else {
            return Err(anyhow!("剧本角色文件命名空间含不安全组件"));
        };
        current.push(value);
        match fs::symlink_metadata(&current) {
            Ok(metadata) => {
                if is_link_like(&metadata) {
                    return Err(anyhow!(
                        "剧本角色文件命名空间不能包含符号链接/重解析点: {}",
                        current.display()
                    ));
                }
                if !metadata.is_dir() {
                    return Err(anyhow!(
                        "剧本角色文件命名空间组件不是目录: {}",
                        current.display()
                    ));
                }
            }
            Err(error) if error.kind() == std::io::ErrorKind::NotFound && create => {
                fs::create_dir(&current)
                    .with_context(|| format!("创建剧本角色命名空间失败: {}", current.display()))?;
            }
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(None),
            Err(error) => {
                return Err(error)
                    .with_context(|| format!("读取剧本角色命名空间失败: {}", current.display()))
            }
        }
    }
    Ok(Some(current))
}

fn ensure_safe_script_dir(script: &ScriptStatus, data_dir: &Path) -> Result<PathBuf> {
    let root = external_characters_root(data_dir);
    if let Ok(metadata) = fs::symlink_metadata(&root) {
        if is_link_like(&metadata) {
            return Err(anyhow!("外部 characters 根目录不能是符号链接"));
        }
    }
    fs::create_dir_all(&root)
        .with_context(|| format!("创建外部角色根目录失败: {}", root.display()))?;

    let namespace = namespace_of(script)?;
    let dir = walk_namespace(&root, &namespace, true)?
        .ok_or_else(|| anyhow!("创建剧本角色命名空间失败"))?;

    let canonical_root = root
        .canonicalize()
        .with_context(|| format!("解析外部角色根目录失败: {}", root.display()))?;
    let canonical_dir = dir
        .canonicalize()
        .with_context(|| format!("解析剧本角色目录失败: {}", dir.display()))?;
    if !canonical_dir.starts_with(&canonical_root) || canonical_dir == canonical_root {
        return Err(anyhow!("剧本角色目录越出 characters 根目录"));
    }
    Ok(dir)
}

fn target_path(script: &ScriptStatus, data_dir: &Path, file: &str) -> Result<PathBuf> {
    require_declared(script, file)?;
    let target = ensure_safe_script_dir(script, data_dir)?.join(file);
    if let Ok(metadata) = fs::symlink_metadata(&target) {
        if is_link_like(&metadata) {
            return Err(anyhow!("外部角色标记不能是符号链接"));
        }
    }
    Ok(target)
}

/// 供 `watch_file` 事件复用：与 character_file 完全相同的声明校验与路径解析。
pub(crate) fn resolve_declared_target_path(
    script: &ScriptStatus,
    data_dir: &Path,
    file: &str,
) -> Result<PathBuf> {
    target_path(script, data_dir, file)
}

fn source_path(script: &ScriptStatus, file: &str) -> Result<PathBuf> {
    require_declared(script, file)?;
    let source_dir = script.script_path.join("CharacterFiles");
    if let Ok(metadata) = fs::symlink_metadata(&source_dir) {
        if is_link_like(&metadata) {
            return Err(anyhow!("剧本 CharacterFiles 目录不能是符号链接"));
        }
    }
    let source = source_dir.join(file);
    if let Ok(metadata) = fs::symlink_metadata(&source) {
        if is_link_like(&metadata) {
            return Err(anyhow!("剧本 .chr 模板不能是符号链接"));
        }
    }
    Ok(source)
}

fn validated_template(script: &ScriptStatus, file: &str) -> Result<PathBuf> {
    let source = source_path(script, file)?;
    if !source.is_file() {
        return Err(anyhow!(
            "剧本声明了角色文件 '{}'，但包内 CharacterFiles/ 中不存在",
            file
        ));
    }
    let source_size = fs::metadata(&source)
        .with_context(|| format!("读取角色文件模板信息失败: {}", source.display()))?
        .len();
    if source_size > MAX_TEMPLATE_BYTES {
        return Err(anyhow!("剧本角色文件模板 '{}' 超过 64 KiB 安全上限", file));
    }
    Ok(source)
}

fn ensure_one(script: &ScriptStatus, data_dir: &Path, file: &str) -> Result<bool> {
    let source = validated_template(script, file)?;
    let target = target_path(script, data_dir, file)?;
    if target.is_file() {
        return Ok(false);
    }
    let parent = target
        .parent()
        .ok_or_else(|| anyhow!("角色文件目标无父目录"))?;
    fs::create_dir_all(parent)
        .with_context(|| format!("创建外部角色目录失败: {}", parent.display()))?;
    let bytes =
        fs::read(&source).with_context(|| format!("读取角色文件模板失败: {}", source.display()))?;
    crate::ai_service::tools::atomic_replace(&target, &bytes)
        .map_err(anyhow::Error::msg)
        .with_context(|| {
            format!(
                "恢复角色文件失败: {} -> {}",
                source.display(),
                target.display()
            )
        })?;
    crate::ai_service::game_system::script_engine::dlc_transaction::sync_directory(parent)
        .context("提交角色标记恢复失败")?;
    Ok(true)
}

#[derive(Debug, Clone)]
pub(crate) struct CharacterFilesSnapshot {
    entries: Vec<(String, Option<Vec<u8>>)>,
}

pub(crate) fn snapshot_declared_character_files(
    script: &ScriptStatus,
    data_dir: &Path,
) -> Result<CharacterFilesSnapshot> {
    let files = declared_files(script);
    // Preflight every template before reset can mutate the first marker.
    for file in &files {
        validated_template(script, file)?;
    }

    let mut entries = Vec::with_capacity(files.len());
    for file in files {
        let target = target_path(script, data_dir, &file)?;
        let bytes = match fs::symlink_metadata(&target) {
            Ok(metadata) => {
                if is_link_like(&metadata) || !metadata.is_file() {
                    return Err(anyhow!(
                        "外部角色标记快照目标不是普通文件: {}",
                        target.display()
                    ));
                }
                if metadata.len() > MAX_TEMPLATE_BYTES {
                    return Err(anyhow!("外部角色标记超过 64 KiB，拒绝无界快照"));
                }
                Some(
                    fs::read(&target)
                        .with_context(|| format!("读取角色标记快照失败: {}", target.display()))?,
                )
            }
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => None,
            Err(error) => {
                return Err(error)
                    .with_context(|| format!("读取角色标记状态失败: {}", target.display()))
            }
        };
        entries.push((file, bytes));
    }
    Ok(CharacterFilesSnapshot { entries })
}

pub(crate) fn restore_character_files_snapshot(
    script: &ScriptStatus,
    data_dir: &Path,
    snapshot: &CharacterFilesSnapshot,
) -> Result<()> {
    for (file, bytes) in &snapshot.entries {
        let target = target_path(script, data_dir, file)?;
        let changed = match bytes {
            Some(bytes) => {
                crate::ai_service::tools::atomic_replace(&target, bytes)
                    .map_err(anyhow::Error::msg)
                    .with_context(|| format!("回滚角色标记失败: {}", target.display()))?;
                true
            }
            None => match fs::remove_file(&target) {
                Ok(()) => true,
                Err(error) if error.kind() == std::io::ErrorKind::NotFound => false,
                Err(error) => {
                    return Err(error)
                        .with_context(|| format!("回滚缺失角色标记失败: {}", target.display()))
                }
            },
        };
        if changed {
            let parent = target
                .parent()
                .ok_or_else(|| anyhow!("角色标记回滚目标没有父目录"))?;
            crate::ai_service::game_system::script_engine::dlc_transaction::sync_directory(parent)
                .context("提交角色标记快照回滚失败")?;
        }
    }
    Ok(())
}

pub fn restore_declared_character_files(script: &ScriptStatus, data_dir: &Path) -> Result<usize> {
    let files = declared_files(script);
    for file in &files {
        validated_template(script, file)?;
    }
    let mut restored = 0;
    for file in files {
        if ensure_one(script, data_dir, &file)? {
            restored += 1;
        }
    }
    Ok(restored)
}

pub(crate) fn remove_character_dir_for_owner(path_key: &str, data_dir: &Path) -> Result<bool> {
    let root = external_characters_root(data_dir);
    let root_metadata = match fs::symlink_metadata(&root) {
        Ok(metadata) => metadata,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(false),
        Err(error) => return Err(error).context("读取外部 characters 根目录失败"),
    };
    if is_link_like(&root_metadata) {
        return Err(anyhow!(
            "拒绝通过符号链接/重解析点形式的 characters 根目录删除文件"
        ));
    }

    let namespace = namespace_from_path_key(path_key)?;
    let Some(dir) = walk_namespace(&root, &namespace, false)? else {
        return Ok(false);
    };

    let canonical_root = root
        .canonicalize()
        .with_context(|| format!("解析外部角色根目录失败: {}", root.display()))?;
    let canonical_dir = dir
        .canonicalize()
        .with_context(|| format!("解析剧本角色目录失败: {}", dir.display()))?;
    if !canonical_dir.starts_with(&canonical_root) || canonical_dir == canonical_root {
        return Err(anyhow!("拒绝删除 characters 根目录之外的路径"));
    }

    let parent = dir
        .parent()
        .ok_or_else(|| anyhow!("角色目录没有父目录"))?
        .to_path_buf();
    fs::remove_dir_all(&dir).with_context(|| format!("删除角色目录失败: {}", dir.display()))?;
    crate::ai_service::game_system::script_engine::dlc_transaction::sync_directory(&parent)
        .context("提交角色目录删除失败")?;
    Ok(true)
}

#[cfg(test)]
fn remove_script_character_dir(script: &ScriptStatus, data_dir: &Path) -> Result<bool> {
    remove_character_dir_for_owner(&script.path_key(), data_dir)
}

#[async_trait]
impl ScriptEvent for CharacterFileEvent {
    async fn execute(&mut self, ctx: &mut ScriptContext<'_>) -> Result<Option<String>> {
        let action = parse_action(&self.action)?;
        let script = {
            let game_status = ctx.game_status.lock().await;
            game_status
                .script_status
                .as_ref()
                .ok_or_else(|| anyhow!("ScriptStatus 未设置，无法操作角色文件"))?
                .clone()
        };

        if action == CharacterFileAction::OpenFolder {
            if ctx.is_preview {
                tracing::info!("[CharacterFileEvent] 试玩隔离：跳过打开真实角色标记目录");
                return Ok(None);
            }
            let dir = ensure_safe_script_dir(&script, ctx.data_dir)?;
            open_folder(&dir.to_string_lossy()).map_err(|error| anyhow!(error))?;
            return Ok(None);
        }

        require_declared(&script, &self.file)?;
        if ctx.is_preview {
            let virtual_key = format!(
                "__preview_character_file:{}",
                self.file.to_ascii_lowercase()
            );
            if matches!(
                action,
                CharacterFileAction::Ensure | CharacterFileAction::Exists
            ) {
                validated_template(&script, &self.file)?;
            }
            let mut game_status = ctx.game_status.lock().await;
            let status = game_status
                .script_status
                .as_mut()
                .ok_or_else(|| anyhow!("ScriptStatus 未设置，无法模拟角色文件"))?;
            match action {
                CharacterFileAction::Ensure => {
                    status.set_variable(virtual_key, Value::Bool(true));
                }
                CharacterFileAction::Delete => {
                    status.set_variable(virtual_key, Value::Bool(false));
                }
                CharacterFileAction::Exists => {
                    validate_result_var(&self.result_var)?;
                    let exists = status
                        .vars
                        .get(&virtual_key)
                        .and_then(Value::as_bool)
                        .unwrap_or(true);
                    status.set_variable(self.result_var.clone(), Value::Bool(exists));
                }
                CharacterFileAction::OpenFolder => unreachable!(),
            }
            tracing::info!(
                "[CharacterFileEvent] 试玩隔离：仅模拟 {} {}",
                self.action,
                self.file
            );
            return Ok(None);
        }

        let target = target_path(&script, ctx.data_dir, &self.file)?;
        match action {
            CharacterFileAction::Ensure => {
                let created = ensure_one(&script, ctx.data_dir, &self.file)?;
                tracing::info!(
                    "[CharacterFileEvent] ensure {}（created={}）",
                    target.display(),
                    created
                );
            }
            CharacterFileAction::Exists => {
                validate_result_var(&self.result_var)?;
                let exists = target.is_file();
                let mut game_status = ctx.game_status.lock().await;
                let status = game_status
                    .script_status
                    .as_mut()
                    .ok_or_else(|| anyhow!("ScriptStatus 未设置，无法写入角色文件检查结果"))?;
                status.set_variable(self.result_var.clone(), Value::Bool(exists));
                tracing::info!(
                    "[CharacterFileEvent] {}={}（{}）",
                    self.result_var,
                    exists,
                    target.display()
                );
            }
            CharacterFileAction::Delete => {
                // Persist the active script's checkpoint before the destructive
                // filesystem change. A hard process kill after remove_file can
                // then replay the declared transition instead of booting an old
                // act against already-missing markers.
                crate::ai_service::game_system::script_engine::persistent_state::save_playthrough(
                    &script,
                    ctx.data_dir,
                )
                .context("删除角色标记前保存剧本 checkpoint 失败")?;
                match fs::remove_file(&target) {
                    Ok(()) => {
                        let parent = target
                            .parent()
                            .ok_or_else(|| anyhow!("角色标记删除目标没有父目录"))?;
                        crate::ai_service::game_system::script_engine::dlc_transaction::sync_directory(parent)
                            .context("提交角色标记删除失败")?;
                        tracing::info!("[CharacterFileEvent] 已删除 {}", target.display());
                    }
                    Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
                        tracing::info!("[CharacterFileEvent] 文件已不存在 {}", target.display())
                    }
                    Err(error) => {
                        return Err(error)
                            .with_context(|| format!("删除角色文件失败: {}", target.display()))
                    }
                }
            }
            CharacterFileAction::OpenFolder => unreachable!(),
        }
        Ok(None)
    }

    fn event_type() -> &'static str {
        "character_file"
    }

    fn duration(&self) -> Option<f64> {
        self.duration
    }
}

pub fn register() {
    register_event(CharacterFileEvent::event_type(), |data| {
        Box::new(CharacterFileEvent::from_event_data(&data))
    });
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn only_accepts_safe_chr_basenames() {
        for valid in ["MAIN.chr", "钦灵.chr", "seventh-main.chr"] {
            assert!(validate_file_name(valid).is_ok(), "{valid}");
        }
        for invalid in [
            "../MAIN.chr",
            "folder/MAIN.chr",
            "folder\\MAIN.chr",
            "MAIN.txt",
            "CON.chr",
            "CON.backup.chr",
            ".chr",
            "bad:name.chr",
            " MAIN.chr",
            "MAIN.chr.",
        ] {
            assert!(validate_file_name(invalid).is_err(), "{invalid}");
        }
    }

    #[test]
    fn restores_and_removes_only_the_script_namespace() {
        let unique = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let root = std::env::temp_dir().join(format!(
            "lingchat-character-file-{}-{unique}",
            std::process::id()
        ));
        let script_path = root.join("scripts").join("standalone").join("seventh-test");
        let templates = script_path.join("CharacterFiles");
        let data_dir = root.join("data");
        std::fs::create_dir_all(&templates).unwrap();
        std::fs::create_dir_all(&data_dir).unwrap();
        std::fs::write(templates.join("MAIN.chr"), b"marker-main").unwrap();
        std::fs::write(templates.join("ql.chr"), b"marker-ql").unwrap();

        let mut settings = serde_json::Map::new();
        settings.insert(
            "character_files".to_string(),
            serde_json::json!(["MAIN.chr", "ql.chr"]),
        );
        let script = ScriptStatus {
            folder_key: "seventh-test".to_string(),
            name: "Seventh test".to_string(),
            description: String::new(),
            intro_chapter: "a1_boot".to_string(),
            settings,
            script_path,
            recommand_start: String::new(),
            adventure: Default::default(),
            content_warning: Some("horror".to_string()),
            running_client_id: None,
            current_chapter_key: String::new(),
            current_event_process: 0,
            vars: serde_json::Map::new(),
        };

        assert_eq!(
            restore_declared_character_files(&script, &data_dir).unwrap(),
            2
        );
        let namespace = script_character_dir(&script, &data_dir).unwrap();
        let mut same_leaf_other_layout = script.clone();
        same_leaf_other_layout.script_path = root
            .join("scripts")
            .join("character")
            .join("role")
            .join("seventh-test");
        assert_ne!(
            namespace,
            script_character_dir(&same_leaf_other_layout, &data_dir).unwrap()
        );
        assert_eq!(
            std::fs::read(namespace.join("MAIN.chr")).unwrap(),
            b"marker-main"
        );
        assert_eq!(
            std::fs::read(namespace.join("ql.chr")).unwrap(),
            b"marker-ql"
        );
        assert_eq!(
            restore_declared_character_files(&script, &data_dir).unwrap(),
            0
        );

        let snapshot = snapshot_declared_character_files(&script, &data_dir).unwrap();
        std::fs::remove_file(namespace.join("MAIN.chr")).unwrap();
        std::fs::write(namespace.join("ql.chr"), b"mutated").unwrap();
        restore_character_files_snapshot(&script, &data_dir, &snapshot).unwrap();
        assert_eq!(
            std::fs::read(namespace.join("MAIN.chr")).unwrap(),
            b"marker-main"
        );
        assert_eq!(
            std::fs::read(namespace.join("ql.chr")).unwrap(),
            b"marker-ql"
        );

        assert!(remove_script_character_dir(&script, &data_dir).unwrap());
        assert!(!namespace.exists());
        assert!(!remove_script_character_dir(&script, &data_dir).unwrap());

        let _ = std::fs::remove_dir_all(root);
    }
}
