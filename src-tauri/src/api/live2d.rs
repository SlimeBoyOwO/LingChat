use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

use serde::{Deserialize, Serialize};
use serde_json::Value as JsonValue;
use tauri::{AppHandle, Manager};
use tokio_util::sync::CancellationToken;

use crate::ai_service::types::{
    CharacterSettings, Live2dEyeBlinkBinding, Live2dMotionBinding, Live2dParameterBinding,
    Live2dSettings, Live2dVariant,
};
use crate::db::entities::role::RoleType;
use crate::db::managers::role_repo::RoleRepo;
use crate::utils::archive::extract_zip;
use crate::utils::yaml_file::write_json_as_yaml;
use crate::AppState;

use super::game_data_dir;

#[derive(Debug, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Live2dSourceKind {
    Directory,
    Zip,
}

#[derive(Debug, Serialize)]
pub struct Live2dModelInfo {
    pub variant: String,
    pub model: String,
    pub expressions: Vec<String>,
    pub motions: HashMap<String, Vec<String>>,
}

#[derive(Debug, Serialize)]
pub struct Live2dImportResult {
    pub live2d: Live2dSettings,
    pub models: Vec<Live2dModelInfo>,
}

fn role_dir(
    role_type: &RoleType,
    folder: &str,
    script_key: Option<&str>,
) -> Result<PathBuf, String> {
    match role_type {
        RoleType::Main => Ok(super::resolve_character_dir(folder)),
        RoleType::Npc => script_key
            .map(|key| {
                game_data_dir()
                    .join("scripts")
                    .join(key)
                    .join("characters")
                    .join(folder)
            })
            .ok_or_else(|| "剧本角色缺少 script_key".to_string()),
        RoleType::System | RoleType::User => Err("系统角色不支持 Live2D 资源".to_string()),
    }
}

fn copy_directory(source: &Path, target: &Path) -> Result<(), String> {
    fs::create_dir_all(target).map_err(|e| format!("创建目录失败: {e}"))?;
    for entry in fs::read_dir(source).map_err(|e| format!("读取目录失败: {e}"))? {
        let entry = entry.map_err(|e| format!("读取目录项失败: {e}"))?;
        let destination = target.join(entry.file_name());
        let file_type = entry
            .file_type()
            .map_err(|e| format!("读取文件类型失败: {e}"))?;
        if file_type.is_dir() {
            copy_directory(&entry.path(), &destination)?;
        } else if file_type.is_file() {
            fs::copy(entry.path(), destination).map_err(|e| format!("复制文件失败: {e}"))?;
        }
    }
    Ok(())
}

fn collect_model_files(dir: &Path, files: &mut Vec<PathBuf>) -> Result<(), String> {
    for entry in fs::read_dir(dir).map_err(|e| format!("扫描 Live2D 目录失败: {e}"))? {
        let entry = entry.map_err(|e| format!("读取 Live2D 文件失败: {e}"))?;
        let path = entry.path();
        if entry.file_type().map_err(|e| e.to_string())?.is_dir() {
            collect_model_files(&path, files)?;
        } else if path
            .file_name()
            .and_then(|name| name.to_str())
            .map(|name| name.ends_with(".model3.json"))
            .unwrap_or(false)
        {
            files.push(path);
        }
    }
    Ok(())
}

fn find_import_manifest(dir: &Path) -> Result<Option<PathBuf>, String> {
    for entry in fs::read_dir(dir).map_err(|e| format!("扫描 Live2D 目录失败: {e}"))? {
        let entry = entry.map_err(|e| format!("读取 Live2D 文件失败: {e}"))?;
        let path = entry.path();
        if entry.file_type().map_err(|e| e.to_string())?.is_dir() {
            if let Some(found) = find_import_manifest(&path)? {
                return Ok(Some(found));
            }
        } else if entry.file_name() == "lingchat-live2d.json" {
            return Ok(Some(path));
        }
    }
    Ok(None)
}

fn has_url_scheme(value: &str) -> bool {
    let Some((scheme, _)) = value.split_once(':') else {
        return false;
    };
    let mut chars = scheme.chars();
    chars.next().is_some_and(|first| first.is_ascii_alphabetic())
        && chars.all(|character| {
            character.is_ascii_alphanumeric() || matches!(character, '+' | '-' | '.')
        })
}

fn validate_relative_path(value: &str, label: &str) -> Result<(), String> {
    if has_url_scheme(value) || value.starts_with('/') || value.starts_with('\\') {
        return Err(format!("{label} 必须是相对路径: {value}"));
    }
    Ok(())
}

fn referenced_path(
    model_dir: &Path,
    resource_root: &Path,
    value: &JsonValue,
    label: &str,
) -> Result<(), String> {
    let Some(relative) = value.as_str() else {
        return Err(format!("{label} 引用不是字符串"));
    };
    validate_relative_path(relative, &format!("{label} 引用"))?;
    let resolved = model_dir.join(relative);
    if !resolved.is_file() {
        return Err(format!("缺少 {label} 文件: {relative}"));
    }
    let canonical = resolved
        .canonicalize()
        .map_err(|e| format!("解析 {label} 文件失败: {e}"))?;
    let canonical_root = resource_root
        .canonicalize()
        .map_err(|e| format!("解析 Live2D 资源目录失败: {e}"))?;
    canonical
        .strip_prefix(canonical_root)
        .map_err(|_| format!("{label} 文件必须位于本次导入的 Live2D 资源内: {relative}"))?;
    Ok(())
}

fn inspect_model(
    model_file: &Path,
    resource_root: &Path,
    role_root: &Path,
    variant: String,
) -> Result<(Live2dModelInfo, Live2dVariant), String> {
    let raw = fs::read_to_string(model_file).map_err(|e| format!("读取 model3 失败: {e}"))?;
    let json: JsonValue =
        serde_json::from_str(&raw).map_err(|e| format!("解析 model3 失败: {e}"))?;
    let refs = json
        .get("FileReferences")
        .and_then(JsonValue::as_object)
        .ok_or_else(|| "model3 缺少 FileReferences".to_string())?;
    let model_dir = model_file
        .parent()
        .ok_or_else(|| "model3 路径无效".to_string())?;

    referenced_path(
        model_dir,
        resource_root,
        refs.get("Moc").unwrap_or(&JsonValue::Null),
        "Moc",
    )?;
    if let Some(textures) = refs.get("Textures").and_then(JsonValue::as_array) {
        for texture in textures {
            referenced_path(model_dir, resource_root, texture, "Texture")?;
        }
    }
    for key in ["Physics", "Pose", "UserData", "DisplayInfo"] {
        if let Some(reference) = refs.get(key) {
            referenced_path(model_dir, resource_root, reference, key)?;
        }
    }

    let mut expressions = Vec::new();
    if let Some(items) = refs.get("Expressions").and_then(JsonValue::as_array) {
        for item in items {
            if let Some(file) = item.get("File") {
                referenced_path(model_dir, resource_root, file, "Expression")?;
            }
            if let Some(name) = item.get("Name").and_then(JsonValue::as_str) {
                expressions.push(name.to_string());
            }
        }
    }

    let mut motions = HashMap::new();
    if let Some(groups) = refs.get("Motions").and_then(JsonValue::as_object) {
        for (group, items) in groups {
            let items = items
                .as_array()
                .ok_or_else(|| format!("动作组 {group} 格式无效"))?;
            let mut files = Vec::new();
            for item in items {
                if let Some(file) = item.get("File") {
                    referenced_path(model_dir, resource_root, file, "Motion")?;
                    if let Some(file) = file.as_str() {
                        files.push(file.to_string());
                    }
                }
                if let Some(sound) = item.get("Sound") {
                    if sound.as_str() != Some("") {
                        referenced_path(model_dir, resource_root, sound, "Motion sound")?;
                    }
                }
            }
            motions.insert(group.clone(), files);
        }
    }

    let relative = model_file
        .strip_prefix(role_root)
        .map_err(|_| "model3 不在角色目录内".to_string())?
        .to_string_lossy()
        .replace('\\', "/");
    let idle = motions
        .get("Idle")
        .filter(|items| !items.is_empty())
        .map(|_| Live2dMotionBinding {
            group: "Idle".to_string(),
            index: 0,
            loop_motion: true,
            extra: HashMap::new(),
        });
    let default_expression = expressions
        .iter()
        .find(|name| name.as_str() == "00_Default")
        .cloned()
        .or_else(|| expressions.first().cloned());
    let mut expression_bindings = HashMap::new();
    for (emotion, keywords) in [
        ("正常", &["default", "normal"][..]),
        ("平静", &["default", "normal"]),
        ("高兴", &["happy", "smile"]),
        ("兴奋", &["kira", "waku", "happy"]),
        ("生气", &["angry"]),
        ("害羞", &["shy", "blush"]),
        ("疑惑", &["doubt", "ask"]),
        ("哭泣", &["tear", "sad", "cry"]),
        ("惊讶", &["surpris"]),
        ("厌恶", &["disgust"]),
        ("担心", &["troubled", "worry"]),
        ("无奈", &["speechless"]),
    ] {
        if let Some(name) = expressions.iter().find(|name| {
            let lower = name.to_ascii_lowercase();
            keywords.iter().any(|keyword| lower.contains(keyword))
        }) {
            expression_bindings.insert(emotion.to_string(), name.clone());
        }
    }
    let mut motion_bindings = HashMap::new();
    let motion_keywords = [
        ("高兴", &["waku", "happy"] as &[&str]),
        ("兴奋", &["waku", "happy"]),
        ("生气", &["angry"]),
        ("疑惑", &["doubt"]),
        ("担心", &["troubled"]),
        ("晕", &["dizzy"]),
    ];
    for (emotion, keywords) in motion_keywords {
        'groups: for (group, files) in &motions {
            if group == "Idle" || group == "Background" {
                continue;
            }
            if let Some((index, _)) = files.iter().enumerate().find(|(_, file)| {
                let lower = file.to_ascii_lowercase();
                keywords.iter().any(|keyword| lower.contains(keyword))
            }) {
                motion_bindings.insert(
                    emotion.to_string(),
                    Live2dMotionBinding {
                        group: group.clone(),
                        index,
                        loop_motion: false,
                        extra: HashMap::new(),
                    },
                );
                break 'groups;
            }
        }
    }
    let variant_settings = Live2dVariant {
        model: relative.clone(),
        default_expression,
        expressions: expression_bindings,
        motions: motion_bindings,
        idle,
        eye_blink: Some(Live2dEyeBlinkBinding {
            left: "ParamEyeLOpen".to_string(),
            right: "ParamEyeROpen".to_string(),
            extra: HashMap::new(),
        }),
        focus_anchor: None,
        lip_sync: Some(Live2dParameterBinding {
            parameter: "ParamMouthOpenY".to_string(),
            gain: 1.0,
            extra: HashMap::new(),
        }),
        extra: HashMap::new(),
    };
    Ok((
        Live2dModelInfo {
            variant,
            model: relative,
            expressions,
            motions,
        },
        variant_settings,
    ))
}

fn unique_variant_name(model_file: &Path, existing: &HashMap<String, Live2dVariant>) -> String {
    let base = model_file
        .file_stem()
        .and_then(|name| name.to_str())
        .unwrap_or("model")
        .trim_end_matches(".model3")
        .to_string();
    if !existing.contains_key(&base) {
        return base;
    }
    for index in 2.. {
        let candidate = format!("{base}_{index}");
        if !existing.contains_key(&candidate) {
            return candidate;
        }
    }
    unreachable!()
}

fn validate_motion_binding(
    variant_name: &str,
    label: &str,
    binding: &Live2dMotionBinding,
    info: &Live2dModelInfo,
) -> Result<(), String> {
    let files = info.motions.get(&binding.group).ok_or_else(|| {
        format!(
            "variant {variant_name} 的 {label} 引用了不存在的动作组 {}",
            binding.group
        )
    })?;
    if binding.index >= files.len() {
        return Err(format!(
            "variant {variant_name} 的 {label} 动作索引 {} 越界（组 {} 共 {} 个）",
            binding.index,
            binding.group,
            files.len()
        ));
    }
    Ok(())
}

fn validate_variant_bindings(
    variant_name: &str,
    variant: &Live2dVariant,
    info: &Live2dModelInfo,
) -> Result<(), String> {
    if let Some(anchor) = &variant.focus_anchor {
        if !anchor.x.is_finite()
            || !anchor.y.is_finite()
            || !(0.0..=1.0).contains(&anchor.x)
            || !(0.0..=1.0).contains(&anchor.y)
        {
            return Err(format!(
                "variant {variant_name} 的 focus_anchor x/y 必须是 0 到 1 之间的有限数值"
            ));
        }
    }
    if let Some(expression) = &variant.default_expression {
        if !info.expressions.contains(expression) {
            return Err(format!(
                "variant {variant_name} 的默认表情不存在: {expression}"
            ));
        }
    }
    for (emotion, expression) in &variant.expressions {
        if !info.expressions.contains(expression) {
            return Err(format!(
                "variant {variant_name} 的情绪 {emotion} 引用了不存在的表情: {expression}"
            ));
        }
    }
    if let Some(idle) = &variant.idle {
        validate_motion_binding(variant_name, "idle", idle, info)?;
    }
    for (emotion, motion) in &variant.motions {
        validate_motion_binding(variant_name, &format!("情绪 {emotion}"), motion, info)?;
    }
    Ok(())
}

#[tauri::command]
pub async fn import_live2d(
    app: AppHandle,
    role_id: i32,
    source_path: String,
    source_kind: Live2dSourceKind,
) -> Result<Live2dImportResult, String> {
    let state = app.state::<AppState>();
    let role = RoleRepo::get_role_by_id(&state.db, role_id)
        .await
        .map_err(|e| format!("查询角色失败: {e}"))?
        .ok_or_else(|| format!("角色 {role_id} 不存在"))?;
    let folder = role
        .resource_folder
        .as_deref()
        .ok_or_else(|| "角色资源目录不存在".to_string())?;
    let root = role_dir(&role.role_type, folder, role.script_key.as_deref())?;
    let source = PathBuf::from(source_path);
    if !source.exists() {
        return Err("Live2D 来源不存在".to_string());
    }
    let canonical_source = source
        .canonicalize()
        .map_err(|e| format!("解析 Live2D 来源失败: {e}"))?;
    let canonical_root = root
        .canonicalize()
        .map_err(|e| format!("解析角色目录失败: {e}"))?;
    if canonical_root.starts_with(&canonical_source) {
        return Err("不能从包含当前角色目录的上级目录导入 Live2D".to_string());
    }

    let nonce = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_err(|e| e.to_string())?
        .as_millis();
    let staging = root.join(format!(".live2d-staging-{nonce}"));
    fs::create_dir_all(&staging).map_err(|e| format!("创建临时目录失败: {e}"))?;

    let import_result = (|| -> Result<(), String> {
        match source_kind {
            Live2dSourceKind::Directory => {
                if !source.is_dir() {
                    return Err("选择的 Live2D 来源不是目录".to_string());
                }
                copy_directory(&source, &staging)
            }
            Live2dSourceKind::Zip => {
                if !source.is_file() {
                    return Err("选择的 Live2D 来源不是 ZIP 文件".to_string());
                }
                extract_zip(&source, &staging, &CancellationToken::new(), &|_| {})
                    .map(|_| ())
                    .map_err(|e| e.to_string())
            }
        }
    })();
    if let Err(error) = import_result {
        let _ = fs::remove_dir_all(&staging);
        return Err(error);
    }

    let mut model_files = Vec::new();
    if let Err(error) = collect_model_files(&staging, &mut model_files) {
        let _ = fs::remove_dir_all(&staging);
        return Err(error);
    }
    model_files.sort();
    if model_files.is_empty() {
        let _ = fs::remove_dir_all(&staging);
        return Err("未找到 .model3.json".to_string());
    }

    let manifest_relative = match find_import_manifest(&staging) {
        Ok(manifest) => manifest
            .map(|path| path.strip_prefix(&staging).map(Path::to_path_buf))
            .transpose()
            .map_err(|e| e.to_string())?,
        Err(error) => {
            let _ = fs::remove_dir_all(&staging);
            return Err(error);
        }
    };
    let target = root.join("live2d").join(format!("import-{nonce}"));
    if let Err(error) = fs::create_dir_all(target.parent().unwrap()) {
        let _ = fs::remove_dir_all(&staging);
        return Err(error.to_string());
    }
    if let Err(error) = fs::rename(&staging, &target) {
        let _ = fs::remove_dir_all(&staging);
        return Err(format!("保存 Live2D 资源失败: {error}"));
    }
    let canonical_target = match target.canonicalize() {
        Ok(path) => path,
        Err(error) => {
            let _ = fs::remove_dir_all(&target);
            return Err(format!("解析 Live2D 导入目录失败: {error}"));
        }
    };

    let (live2d, models) = if let Some(manifest_relative) = manifest_relative {
        let manifest_path = target.join(manifest_relative);
        let manifest_dir = manifest_path
            .parent()
            .ok_or_else(|| "Live2D 导入清单路径无效".to_string())?;
        let configuration_result =
            (|| -> Result<(Live2dSettings, Vec<Live2dModelInfo>), String> {
                let raw = fs::read_to_string(&manifest_path)
                    .map_err(|e| format!("读取 lingchat-live2d.json 失败: {e}"))?;
                let mut configured: Live2dSettings = serde_json::from_str(&raw)
                    .map_err(|e| format!("解析 lingchat-live2d.json 失败: {e}"))?;
                if configured.version != 1
                    || !configured
                        .variants
                        .contains_key(&configured.default_variant)
                {
                    return Err(
                        "lingchat-live2d.json 必须是 version 1 且包含 default_variant".to_string(),
                    );
                }
                let mut inspected = Vec::new();
                for (variant_name, variant) in &mut configured.variants {
                    validate_relative_path(
                        &variant.model,
                        &format!("variant {variant_name} 的模型路径"),
                    )?;
                    let source_model = manifest_dir
                        .join(&variant.model)
                        .canonicalize()
                        .map_err(|e| format!("解析导入清单模型路径失败: {e}"))?;
                    source_model
                        .strip_prefix(&canonical_target)
                        .map_err(|_| "导入清单中的模型必须位于本次导入目录内".to_string())?;
                    let relative = source_model
                        .strip_prefix(&canonical_root)
                        .map_err(|_| "导入清单中的模型路径无效".to_string())?
                        .to_string_lossy()
                        .replace('\\', "/");
                    variant.model = relative;
                    let (info, _) = inspect_model(
                        &source_model,
                        &canonical_target,
                        &root,
                        variant_name.clone(),
                    )?;
                    validate_variant_bindings(variant_name, variant, &info)?;
                    inspected.push(info);
                }
                for (clothes, variant_name) in &configured.clothes_variants {
                    if !configured.variants.contains_key(variant_name) {
                        return Err(format!(
                            "服装 {clothes} 映射到不存在的 variant: {variant_name}"
                        ));
                    }
                }
                inspected.sort_by(|left, right| left.variant.cmp(&right.variant));
                Ok((configured, inspected))
            })();
        match configuration_result {
            Ok(result) => result,
            Err(error) => {
                let _ = fs::remove_dir_all(&target);
                return Err(error);
            }
        }
    } else {
        let generation_result = (|| -> Result<(Live2dSettings, Vec<Live2dModelInfo>), String> {
            let mut variants = HashMap::new();
            let mut inspected = Vec::new();
            for staged_model in model_files {
                let relative_in_staging = staged_model
                    .strip_prefix(&staging)
                    .map_err(|e| e.to_string())?;
                let model_file = target.join(relative_in_staging);
                let variant_name = unique_variant_name(&model_file, &variants);
                let (info, variant) = inspect_model(
                    &model_file,
                    &canonical_target,
                    &root,
                    variant_name.clone(),
                )?;
                variants.insert(variant_name, variant);
                inspected.push(info);
            }
            let default_variant = inspected[0].variant.clone();
            Ok((
                Live2dSettings {
                    version: 1,
                    default_variant: default_variant.clone(),
                    variants,
                    clothes_variants: HashMap::from([("default".to_string(), default_variant)]),
                    extra: HashMap::new(),
                },
                inspected,
            ))
        })();
        match generation_result {
            Ok(result) => result,
            Err(error) => {
                let _ = fs::remove_dir_all(&target);
                return Err(error);
            }
        }
    };

    let mut settings =
        match RoleRepo::get_role_settings_by_id(&state.db, &super::data_dir(), role_id).await {
            Ok(settings) => settings.unwrap_or_else(CharacterSettings::default),
            Err(error) => {
                let _ = fs::remove_dir_all(&target);
                return Err(format!("读取角色配置失败: {error}"));
            }
        };
    settings.live2d = Some(live2d.clone());
    let mut value = match serde_json::to_value(&settings) {
        Ok(value) => value,
        Err(error) => {
            let _ = fs::remove_dir_all(&target);
            return Err(error.to_string());
        }
    };
    if let Some(object) = value.as_object_mut() {
        for transient in [
            "character_id",
            "resource_path",
            "character_folder",
            "script_key",
            "script_role_key",
        ] {
            object.remove(transient);
        }
    }
    if let Err(error) = write_json_as_yaml(&root.join("settings.yml"), &value) {
        let _ = fs::remove_dir_all(&target);
        return Err(format!("保存 Live2D 配置失败: {error}"));
    }

    {
        let service = state.ai_service.lock().await;
        let mut game_status = service.game_status.lock().await;
        game_status
            .role_manager
            .update_role_live2d_settings(role_id, &settings);
    }

    Ok(Live2dImportResult { live2d, models })
}

#[tauri::command]
pub async fn get_live2d_file(
    app: AppHandle,
    role_id: i32,
    file_path: String,
) -> Result<String, String> {
    let state = app.state::<AppState>();
    let role = RoleRepo::get_role_by_id(&state.db, role_id)
        .await
        .map_err(|e| format!("查询角色失败: {e}"))?
        .ok_or_else(|| format!("角色 {role_id} 不存在"))?;
    let folder = role
        .resource_folder
        .as_deref()
        .ok_or_else(|| "角色资源目录不存在".to_string())?;
    let root = role_dir(&role.role_type, folder, role.script_key.as_deref())?;
    let resolved = root.join(file_path);
    crate::utils::path::validate_path_in_base(&resolved, &root)?;
    if !resolved.is_file() {
        return Err("Live2D 文件不存在".to_string());
    }
    resolved
        .canonicalize()
        .map(|path| path.to_string_lossy().into_owned())
        .map_err(|e| format!("解析 Live2D 文件路径失败: {e}"))
}

#[tauri::command]
pub async fn inspect_live2d(app: AppHandle, role_id: i32) -> Result<Live2dImportResult, String> {
    let state = app.state::<AppState>();
    let role = RoleRepo::get_role_by_id(&state.db, role_id)
        .await
        .map_err(|e| format!("查询角色失败: {e}"))?
        .ok_or_else(|| format!("角色 {role_id} 不存在"))?;
    let folder = role
        .resource_folder
        .as_deref()
        .ok_or_else(|| "角色资源目录不存在".to_string())?;
    let root = role_dir(&role.role_type, folder, role.script_key.as_deref())?;
    let settings = RoleRepo::get_role_settings_by_id(&state.db, &super::data_dir(), role_id)
        .await
        .map_err(|e| format!("读取角色配置失败: {e}"))?
        .ok_or_else(|| "角色配置不存在".to_string())?;
    let live2d = settings
        .live2d
        .ok_or_else(|| "角色未配置 Live2D".to_string())?;
    let mut models = Vec::new();
    for (variant_name, variant) in &live2d.variants {
        let model_file = root.join(&variant.model);
        let (info, _) = inspect_model(&model_file, &root, &root, variant_name.clone())?;
        models.push(info);
    }
    models.sort_by(|left, right| left.variant.cmp(&right.variant));
    Ok(Live2dImportResult { live2d, models })
}
