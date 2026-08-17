//! 通用市场包安装器。
//!
//! 输入：已下载并通过 sha256 校验的 zip（`manifest.toml` + `payload/` 外壳）。
//! 流程：安全解包到暂存目录 → 解析并校验 manifest → 按 `type` 分流安装：
//!
//! - `plugin`：`payload/` 内容上提，安装到 `data/plugins/<id>/`（工具脚本与
//!   manifest 平级，`ToolSpec.script` 相对插件目录）
//! - `script`：`story_config.yaml` + 章节 → `data/game_data/scripts/character/<角色>/<剧本>/`
//!   （或 `standalone/<剧本>/`），章节落入 `Chapters/`
//! - `character`：`payload/role.txt`（TOML，取 `system_prompt`）→
//!   `data/game_data/characters/<id>/settings.yml`
//! - `voice`：暂不支持
//!
//! 所有函数为同步阻塞实现（zip 解包 + 文件复制），调用方须放
//! `spawn_blocking`。

use std::fs;
use std::io::Read;
use std::path::{Component, Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};

use sha2::{Digest, Sha256};

use crate::ai_service::types::CharacterSettings;

use super::manifest;
use super::types::PluginManifest;

/// 安装结果。
pub struct InstallResult {
    pub manifest: PluginManifest,
    /// 安装到的目标目录。
    pub dir: PathBuf,
}

static STAGING_SEQ: AtomicU64 = AtomicU64::new(0);

/// 计算文件 sha256（64 位小写 hex）。
pub fn sha256_hex(path: &Path) -> Result<String, String> {
    let mut file =
        fs::File::open(path).map_err(|e| format!("打开文件失败: {e}"))?;
    let mut hasher = Sha256::new();
    std::io::copy(&mut file, &mut hasher)
        .map_err(|e| format!("读取文件失败: {e}"))?;
    Ok(hex_encode(&hasher.finalize()))
}

fn hex_encode(bytes: &[u8]) -> String {
    bytes.iter().map(|b| format!("{b:02x}")).collect()
}

/// 安装市场包 zip。
///
/// `data_dir` 是 data 根目录（`game_data` 的父目录），`plugins_root` 是
/// `data/plugins`。返回安装结果（含目标目录）。
pub fn install_package(
    zip_path: &Path,
    data_dir: &Path,
    plugins_root: &Path,
) -> Result<InstallResult, String> {
    // 暂存目录：plugins_root/.install/<seq>-<pid>/（与插件目录同盘，rename 原子）
    let seq = STAGING_SEQ.fetch_add(1, Ordering::Relaxed);
    let staging = plugins_root
        .join(".install")
        .join(format!("{seq}-{}", std::process::id()));
    if staging.exists() {
        fs::remove_dir_all(&staging)
            .map_err(|e| format!("清理旧暂存目录失败: {e}"))?;
    }
    fs::create_dir_all(&staging)
        .map_err(|e| format!("创建暂存目录失败: {e}"))?;

    let result = (|| {
        extract_zip(zip_path, &staging)?;

        let manifest_text = fs::read_to_string(staging.join("manifest.toml"))
            .map_err(|e| format!("包内缺少 manifest.toml: {e}"))?;
        let m = manifest::parse(&manifest_text)?;

        let dir = match m.package_type.as_str() {
            "plugin" => install_plugin(&m, &staging, plugins_root)?,
            "script" => install_script(&m, &staging, data_dir)?,
            "character" => install_character(&m, &staging, data_dir)?,
            "voice" => return Err("语音包（voice）安装暂未支持".to_string()),
            other => return Err(format!("未知包类型: '{other}'")),
        };

        Ok(InstallResult { manifest: m, dir })
    })();

    let _ = fs::remove_dir_all(&staging);
    result
}

/// 安全解包 zip 到 target。
///
/// 逐条目做路径校验：拒绝 `..`、绝对路径、盘符、空段，
/// 防止 zip-slip 把文件写到包外。
fn extract_zip(zip_path: &Path, target: &Path) -> Result<(), String> {
    let file = fs::File::open(zip_path).map_err(|e| format!("打开 zip 失败: {e}"))?;
    let mut archive =
        zip::ZipArchive::new(file).map_err(|e| format!("解析 zip 失败: {e}"))?;
    for i in 0..archive.len() {
        let mut entry = archive
            .by_index(i)
            .map_err(|e| format!("读取 zip 条目 #{i} 失败: {e}"))?;
        let name = entry.name().to_string();
        let rel = safe_relative_path(&name)?;
        let dest = target.join(&rel);

        if entry.is_dir() {
            fs::create_dir_all(&dest).map_err(|e| format!("创建目录失败: {e}"))?;
        } else {
            if let Some(parent) = dest.parent() {
                fs::create_dir_all(parent).map_err(|e| format!("创建目录失败: {e}"))?;
            }
            let mut out = fs::File::create(&dest)
                .map_err(|e| format!("创建文件 '{}' 失败: {e}", rel.display()))?;
            std::io::copy(&mut entry, &mut out)
                .map_err(|e| format!("解包 '{}' 失败: {e}", rel.display()))?;
        }
    }
    Ok(())
}

/// 把 zip 条目名转成安全相对路径。
fn safe_relative_path(name: &str) -> Result<PathBuf, String> {
    let path = Path::new(name);
    let mut out = PathBuf::new();
    for comp in path.components() {
        match comp {
            Component::Normal(seg) => out.push(seg),
            Component::CurDir => {}
            _ => {
                return Err(format!(
                    "zip 条目含非法路径（.. / 绝对路径 / 盘符）: '{name}'"
                ))
            }
        }
    }
    if out.as_os_str().is_empty() {
        return Err(format!("zip 条目为空路径: '{name}'"));
    }
    Ok(out)
}

/// 递归复制目录内容（不含 exclude 文件名集合）。
fn copy_dir_contents(src: &Path, dst: &Path, exclude: &[&str]) -> Result<(), String> {
    if !src.exists() {
        return Ok(());
    }
    fs::create_dir_all(dst).map_err(|e| format!("创建目录失败: {e}"))?;
    for entry in fs::read_dir(src)
        .map_err(|e| format!("读取目录 '{}' 失败: {e}", src.display()))?
    {
        let entry = entry.map_err(|e| format!("读取目录项失败: {e}"))?;
        let name = entry.file_name().to_string_lossy().to_string();
        if exclude.contains(&name.as_str()) {
            continue;
        }
        let from = entry.path();
        let to = dst.join(&name);
        if from.is_dir() {
            copy_dir_contents(&from, &to, &[])?;
        } else {
            if let Some(parent) = to.parent() {
                fs::create_dir_all(parent)
                    .map_err(|e| format!("创建目录失败: {e}"))?;
            }
            fs::copy(&from, &to)
                .map_err(|e| format!("复制 '{}' 失败: {e}", from.display()))?;
        }
    }
    Ok(())
}

// ─── plugin 类型 ────────────────────────────────────────────────

/// 插件：`payload/` 内容上提到 `data/plugins/<id>/`，manifest 放根。
fn install_plugin(
    m: &PluginManifest,
    staging: &Path,
    plugins_root: &Path,
) -> Result<PathBuf, String> {
    let target = plugins_root.join(&m.id);
    if target.exists() {
        fs::remove_dir_all(&target)
            .map_err(|e| format!("移除旧版本插件目录失败: {e}"))?;
    }
    fs::create_dir_all(&target).map_err(|e| format!("创建插件目录失败: {e}"))?;

    fs::copy(staging.join("manifest.toml"), target.join("manifest.toml"))
        .map_err(|e| format!("复制 manifest 失败: {e}"))?;
    copy_dir_contents(&staging.join("payload"), &target, &[])?;
    Ok(target)
}

// ─── script 类型 ────────────────────────────────────────────────

/// 剧本包：`story_config.yaml` → 目标根；其余 `.yaml` → `Chapters/`；
/// 非 yaml 资源按相对路径复制到目标根。
fn install_script(
    m: &PluginManifest,
    staging: &Path,
    data_dir: &Path,
) -> Result<PathBuf, String> {
    let payload = staging.join("payload");
    let story_text = fs::read_to_string(payload.join("story_config.yaml"))
        .map_err(|e| format!("剧本包缺少 story_config.yaml: {e}"))?;
    let story: serde_yaml::Value = serde_yaml::from_str(&story_text)
        .map_err(|e| format!("story_config.yaml 解析失败: {e}"))?;

    let script_name = story
        .get("script_name")
        .and_then(serde_yaml::Value::as_str)
        .ok_or_else(|| "story_config.yaml 缺少 script_name".to_string())?
        .trim()
        .to_string();
    let is_adventure = story
        .get("adventure")
        .and_then(|a| a.get("is_adventure"))
        .and_then(serde_yaml::Value::as_bool)
        .unwrap_or(false);
    let bound = if is_adventure {
        story
            .get("adventure")
            .and_then(|a| a.get("bound_character_folder"))
            .and_then(serde_yaml::Value::as_str)
            .ok_or_else(|| {
                "羁绊冒险剧本缺少 adventure.bound_character_folder".to_string()
            })?
            .trim()
            .to_string()
    } else {
        String::new()
    };

    let scripts_root = data_dir.join("game_data").join("scripts");
    let target = if is_adventure {
        scripts_root
            .join("character")
            .join(&bound)
            .join(&script_name)
    } else {
        scripts_root.join("standalone").join(&script_name)
    };
    if target.exists() {
        fs::remove_dir_all(&target)
            .map_err(|e| format!("移除旧版本剧本目录失败: {e}"))?;
    }
    fs::create_dir_all(&target).map_err(|e| format!("创建剧本目录失败: {e}"))?;

    // story_config.yaml → 根
    fs::copy(
        payload.join("story_config.yaml"),
        target.join("story_config.yaml"),
    )
    .map_err(|e| format!("复制 story_config.yaml 失败: {e}"))?;

    // 其余 .yaml → Chapters/（保留相对路径）；非 yaml → 目标根
    let chapters_dir = target.join("Chapters");
    fs::create_dir_all(&chapters_dir)
        .map_err(|e| format!("创建 Chapters 目录失败: {e}"))?;
    copy_yaml_to_chapters(&payload, &chapters_dir)?;
    copy_non_yaml_contents(&payload, &target)?;

    tracing::info!(
        "剧本包安装完成: id={} script={} bound={} target={}",
        m.id,
        script_name,
        bound,
        target.display()
    );
    Ok(target)
}

/// 递归把 src 下所有 `.yaml`（不含 story_config.yaml）按相对路径复制到 chapters。
fn copy_yaml_to_chapters(src: &Path, chapters: &Path) -> Result<(), String> {
    let entries = match fs::read_dir(src) {
        Ok(e) => e,
        Err(_) => return Ok(()),
    };
    for entry in entries.flatten() {
        let path = entry.path();
        let name = entry.file_name().to_string_lossy().to_string();
        if path.is_dir() {
            copy_yaml_to_chapters(&path, chapters)?;
            continue;
        }
        if !name.ends_with(".yaml") || name == "story_config.yaml" {
            continue;
        }
        let rel = path
            .strip_prefix(src)
            .map_err(|e| format!("路径计算失败: {e}"))?;
        let dst = chapters.join(rel);
        if let Some(parent) = dst.parent() {
            fs::create_dir_all(parent)
                .map_err(|e| format!("创建章节子目录失败: {e}"))?;
        }
        fs::copy(&path, &dst)
            .map_err(|e| format!("复制章节 '{}' 失败: {e}", rel.display()))?;
    }
    Ok(())
}

/// 把 src 下非 yaml 内容（立绘、音频、characters/ 等）复制到 dst 根。
fn copy_non_yaml_contents(src: &Path, dst: &Path) -> Result<(), String> {
    let entries = match fs::read_dir(src) {
        Ok(e) => e,
        Err(_) => return Ok(()),
    };
    for entry in entries.flatten() {
        let path = entry.path();
        let name = entry.file_name().to_string_lossy().to_string();
        if name == "story_config.yaml" {
            continue;
        }
        if path.is_dir() {
            copy_dir_contents(&path, &dst.join(&name), &[])?;
        } else if !name.ends_with(".yaml") {
            fs::copy(&path, dst.join(&name))
                .map_err(|e| format!("复制资源 '{}' 失败: {e}", name))?;
        }
    }
    Ok(())
}

// ─── character 类型 ─────────────────────────────────────────────

/// 角色卡：`payload/role.txt`（TOML，取 `system_prompt`）→
/// `data/game_data/characters/<id>/settings.yml`；其余 payload 内容
/// （avatar 等）原样复制到角色目录。
fn install_character(
    m: &PluginManifest,
    staging: &Path,
    data_dir: &Path,
) -> Result<PathBuf, String> {
    let payload = staging.join("payload");
    let role_txt = payload.join("role.txt");
    let text = fs::read_to_string(&role_txt)
        .map_err(|e| format!("角色包缺少 role.txt: {e}"))?;
    let role: toml::Value = toml::from_str(&text)
        .map_err(|e| format!("role.txt 解析失败（须为 TOML）: {e}"))?;
    let system_prompt = role
        .get("system_prompt")
        .and_then(toml::Value::as_str)
        .unwrap_or("")
        .to_string();

    let folder = &m.id;
    let dir = data_dir
        .join("game_data")
        .join("characters")
        .join(folder);
    if dir.exists() {
        fs::remove_dir_all(&dir)
            .map_err(|e| format!("移除旧版本角色目录失败: {e}"))?;
    }
    fs::create_dir_all(&dir).map_err(|e| format!("创建角色目录失败: {e}"))?;

    let mut settings = CharacterSettings::default();
    settings.ai_name = m.name.clone();
    settings.system_prompt = Some(system_prompt);
    settings.character_folder = folder.clone();
    let yaml = serde_yaml::to_string(&settings)
        .map_err(|e| format!("角色 settings 序列化失败: {e}"))?;
    fs::write(dir.join("settings.yml"), yaml)
        .map_err(|e| format!("写入 settings.yml 失败: {e}"))?;

    copy_dir_contents(&payload, &dir, &["role.txt"])?;
    Ok(dir)
}
