use std::collections::HashSet;
use std::fs;
use std::path::{Path, PathBuf};

/// 校验并规范化用户提供的单层目录名。
///
/// 分类名最终会被拼接到资源根目录下，因此这里只允许一个普通路径组件，
/// 同时拒绝 Windows 文件系统中的保留字符、尾随点/空格和设备名。
pub fn validate_directory_name(raw: &str) -> Result<String, String> {
    let name = raw.trim();
    if name.is_empty() {
        return Err("分类名不能为空".to_string());
    }

    let mut components = Path::new(name).components();
    let is_single_normal_component = matches!(
        components.next(),
        Some(std::path::Component::Normal(component)) if component == name
    ) && components.next().is_none();
    if !is_single_normal_component
        || name.chars().any(|c| {
            c.is_control() || matches!(c, '<' | '>' | ':' | '"' | '/' | '\\' | '|' | '?' | '*')
        })
        || name.ends_with('.')
        || name.ends_with(' ')
    {
        return Err("分类名包含非法字符".to_string());
    }

    let stem = name.split('.').next().unwrap_or(name).to_ascii_uppercase();
    let is_reserved_device = matches!(stem.as_str(), "CON" | "PRN" | "AUX" | "NUL")
        || stem
            .strip_prefix("COM")
            .or_else(|| stem.strip_prefix("LPT"))
            .is_some_and(|suffix| {
                matches!(suffix, "1" | "2" | "3" | "4" | "5" | "6" | "7" | "8" | "9")
            });
    if is_reserved_device {
        return Err("分类名不能使用系统保留名称".to_string());
    }

    Ok(name.to_string())
}
fn collect_regular_files(base: &Path, out: &mut Vec<PathBuf>) -> Result<(), String> {
    let entries = fs::read_dir(base)
        .map_err(|e| format!("读取分类目录失败: {} - 路径: {}", e, base.display()))?;
    for entry in entries {
        let entry = entry.map_err(|e| format!("读取分类目录项失败: {e}"))?;
        let path = entry.path();
        let file_type = entry
            .file_type()
            .map_err(|e| format!("读取文件类型失败: {} - 路径: {}", e, path.display()))?;
        if file_type.is_symlink() {
            return Err(format!("分类目录包含不支持的符号链接: {}", path.display()));
        }
        if file_type.is_dir() {
            collect_regular_files(&path, out)?;
        } else if file_type.is_file() {
            out.push(path);
        } else {
            return Err(format!("分类目录包含不支持的文件类型: {}", path.display()));
        }
    }
    Ok(())
}

fn remove_empty_directory_tree(dir: &Path) -> Result<(), String> {
    let entries = fs::read_dir(dir)
        .map_err(|e| format!("检查分类目录失败: {} - 路径: {}", e, dir.display()))?;
    for entry in entries {
        let entry = entry.map_err(|e| format!("读取分类目录项失败: {e}"))?;
        let path = entry.path();
        let file_type = entry
            .file_type()
            .map_err(|e| format!("读取文件类型失败: {} - 路径: {}", e, path.display()))?;
        if !file_type.is_dir() || file_type.is_symlink() {
            return Err(format!("分类目录仍包含未转移的文件: {}", path.display()));
        }
        remove_empty_directory_tree(&path)?;
    }
    fs::remove_dir(dir).map_err(|e| format!("删除空分类目录失败: {} - 路径: {}", e, dir.display()))
}

/// 将目录树中的所有普通文件扁平移动到目标目录。
///
/// 移动前会完整检查同名冲突；只有全部重命名成功后才逐层删除空目录。
/// 任一失败都会直接返回错误，绝不会使用 `remove_dir_all` 清理源目录。
pub fn move_directory_files_to(source_dir: &Path, target_dir: &Path) -> Result<usize, String> {
    let mut files = Vec::new();
    collect_regular_files(source_dir, &mut files)?;

    let mut destination_names = HashSet::new();
    for source in &files {
        let file_name = source
            .file_name()
            .ok_or_else(|| format!("无效的文件路径: {}", source.display()))?;
        let collision_key = file_name.to_string_lossy().to_lowercase();
        if !destination_names.insert(collision_key) {
            return Err(format!(
                "分类中存在重名文件，无法安全移动: {}",
                file_name.to_string_lossy()
            ));
        }
        let destination = target_dir.join(file_name);
        if destination.exists() {
            return Err(format!("目标目录已存在同名文件: {}", destination.display()));
        }
    }

    for source in &files {
        let destination = target_dir.join(source.file_name().expect("file name checked above"));
        fs::rename(source, &destination).map_err(|e| {
            format!(
                "移动文件失败，源分类已保留且不会删除: {} -> {}: {}",
                source.display(),
                destination.display(),
                e
            )
        })?;
    }

    remove_empty_directory_tree(source_dir)?;
    Ok(files.len())
}

/// 将角色资源路径解析为绝对路径。
///
/// 游戏自有角色：相对路径统一放在 `data/game_data/characters` 下，绝对路径保持不变。
/// 插件角色：`resource_folder` 编码为 `plugin:<id>/<folder>`，解析到
/// `data/plugins/<id>/characters/<folder>`（运行时直读，不复制）。
pub fn resolve_character_path(data_dir: &Path, resource_path: &str) -> PathBuf {
    if let Some(rest) = resource_path.strip_prefix("plugin:") {
        if let Some((plugin_id, folder)) = rest.split_once('/') {
            if !plugin_id.is_empty() && !folder.is_empty() {
                return data_dir
                    .join("plugins")
                    .join(plugin_id)
                    .join("characters")
                    .join(folder);
            }
        }
    }
    let path = PathBuf::from(resource_path);
    if path.is_absolute() {
        path
    } else {
        data_dir.join("game_data").join("characters").join(path)
    }
}

/// 批量创建目录（幂等）。任一失败立即返回错误。
pub fn ensure_dirs(dirs: &[&Path]) -> Result<(), String> {
    for d in dirs {
        std::fs::create_dir_all(d).map_err(|e| format!("create_dir_all {}: {e}", d.display()))?;
    }
    Ok(())
}

/// 路径穿越防护：验证 canonical 路径是否以预期的基础目录开头。
///
/// 原为 `api/mod.rs` 下的共享辅助，迁到 utils 后各域（编辑器路径解析、
/// 局域网同步、字体/素材校验等）都能复用。
pub fn validate_path_in_base(resolved: &Path, base: &Path) -> Result<(), String> {
    let canon_resolved = resolved
        .canonicalize()
        .map_err(|e| format!("路径解析失败: {} - 路径: {:?}", e, resolved))?;

    let canon_base = base
        .canonicalize()
        .map_err(|e| format!("基础目录解析失败: {} - 路径: {:?}", e, base))?;

    if !canon_resolved.starts_with(&canon_base) {
        return Err(format!(
            "非法路径：试图访问基础目录之外的文件\n\
             请求路径: {:?}\n\
             规范路径: {:?}\n\
             基础目录: {:?}\n\
             规范基础目录: {:?}",
            resolved, canon_resolved, base, canon_base
        ));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use std::fs;

    use super::{move_directory_files_to, validate_directory_name};

    #[test]
    fn accepts_a_single_unicode_directory_name() {
        assert_eq!(validate_directory_name("  日常音乐  ").unwrap(), "日常音乐");
    }

    #[test]
    fn rejects_traversal_and_windows_reserved_names() {
        for invalid in [
            "../outside",
            r"foo\bar",
            "a/b",
            "NUL",
            "COM1.txt",
            "trailing.",
        ] {
            assert!(validate_directory_name(invalid).is_err(), "{invalid}");
        }
    }

    #[test]
    fn refuses_move_when_destination_name_already_exists() {
        let temp = tempfile::tempdir().unwrap();
        let source = temp.path().join("category");
        let target = temp.path().join("root");
        fs::create_dir_all(&source).unwrap();
        fs::create_dir_all(&target).unwrap();
        fs::write(source.join("same.mp3"), b"source").unwrap();
        fs::write(target.join("same.mp3"), b"target").unwrap();

        assert!(move_directory_files_to(&source, &target).is_err());
        assert_eq!(fs::read(source.join("same.mp3")).unwrap(), b"source");
        assert_eq!(fs::read(target.join("same.mp3")).unwrap(), b"target");
    }

    #[test]
    fn removes_source_only_after_every_file_moves() {
        let temp = tempfile::tempdir().unwrap();
        let source = temp.path().join("category");
        let nested = source.join("nested");
        let target = temp.path().join("root");
        fs::create_dir_all(&nested).unwrap();
        fs::create_dir_all(&target).unwrap();
        fs::write(source.join("one.mp3"), b"one").unwrap();
        fs::write(nested.join("two.mp3"), b"two").unwrap();

        assert_eq!(move_directory_files_to(&source, &target).unwrap(), 2);
        assert!(!source.exists());
        assert_eq!(fs::read(target.join("one.mp3")).unwrap(), b"one");
        assert_eq!(fs::read(target.join("two.mp3")).unwrap(), b"two");
    }
}
