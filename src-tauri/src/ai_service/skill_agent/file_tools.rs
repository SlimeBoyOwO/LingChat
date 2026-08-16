//! 暴露给 LLM 的文件工具，除非显式关闭，否则一律受沙箱约束。

use std::ffi::OsString;
use std::path::{Component, Path, PathBuf};

const MAX_READ_BYTES: u64 = 200 * 1024;
const MAX_LIST_ENTRIES: usize = 500;
const MAX_WALK_DEPTH: usize = 10;
const MAX_WALK_FILES: usize = 500;
const MAX_GREP_FILE_BYTES: u64 = 1024 * 1024;
pub const MAX_GREP_RESULTS: usize = 100;

/// 先写入目标旁的临时文件，完整刷盘后，再原子地替换目标文件。
/// 这能避免被取消或中断的 LLM 工具调用留下写了一半的文件。
fn atomic_write(path: &Path, content: &[u8]) -> anyhow::Result<()> {
    use std::io::Write;

    let parent = path
        .parent()
        .ok_or_else(|| anyhow::anyhow!("target path has no parent: {}", path.display()))?;
    std::fs::create_dir_all(parent)?;

    let mut temporary = tempfile::NamedTempFile::new_in(parent)?;
    temporary.write_all(content)?;
    temporary.as_file().sync_all()?;
    temporary.persist(path).map_err(|error| {
        anyhow::anyhow!("failed to replace {}: {}", path.display(), error.error)
    })?;
    Ok(())
}

#[derive(Clone)]
pub struct FileTools {
    pub sandbox_dir: PathBuf,
    pub allow_any_path: bool,
}

impl FileTools {
    /// 解析路径：保留不存在的后缀部分，并解析最深一层存在的祖先目录。
    /// 这能防止 `..`、符号链接和 Windows junction 逃出沙箱。
    pub fn sanitize(&self, path: &str) -> anyhow::Result<PathBuf> {
        let requested = PathBuf::from(path.trim());
        let joined = if requested.is_absolute() {
            requested
        } else {
            self.sandbox_dir.join(requested)
        };
        let target = canonicalize_allow_missing(&joined)?;
        if self.allow_any_path {
            return Ok(target);
        }

        let root = canonicalize_allow_missing(&self.sandbox_dir)?;
        if target.starts_with(&root) {
            Ok(target)
        } else {
            anyhow::bail!(
                "拒绝访问文件沙箱之外的路径: {}（可在“助手设置”中开启允许任意路径）",
                path
            )
        }
    }

    pub fn list_files(&self, path: &str) -> anyhow::Result<String> {
        let dir = self.sanitize(path)?;
        if !dir.is_dir() {
            anyhow::bail!("目录不存在: {}", dir.display());
        }
        let mut entries = std::fs::read_dir(&dir)?
            .filter_map(Result::ok)
            .filter_map(|entry| {
                let file_type = entry.file_type().ok()?;
                let kind = if file_type.is_symlink() {
                    2
                } else if file_type.is_dir() {
                    0
                } else {
                    1
                };
                Some((kind, entry.file_name().to_string_lossy().into_owned()))
            })
            .collect::<Vec<_>>();
        entries.sort_by(|a, b| a.1.to_lowercase().cmp(&b.1.to_lowercase()));
        let truncated = entries.len() > MAX_LIST_ENTRIES;

        let mut lines = format!("📂 {}\n", dir.display());
        for (kind, name) in entries.into_iter().take(MAX_LIST_ENTRIES) {
            let prefix = match kind {
                0 => "📂 ",
                2 => "🔗 ",
                _ => "📄 ",
            };
            lines.push_str(&format!("{prefix}{name}\n"));
        }
        if truncated {
            lines.push_str(&format!(
                "...[条目过多，仅显示前 {MAX_LIST_ENTRIES} 项]...\n"
            ));
        }
        Ok(lines)
    }

    pub fn read_file(&self, path: &str) -> anyhow::Result<String> {
        use std::io::Read;

        let file = self.sanitize(path)?;
        if !file.is_file() {
            anyhow::bail!("文件不存在: {}", file.display());
        }
        let mut handle = std::fs::File::open(&file)?;
        let mut bytes = Vec::new();
        handle
            .by_ref()
            .take(MAX_READ_BYTES + 1)
            .read_to_end(&mut bytes)?;
        let truncated = bytes.len() > MAX_READ_BYTES as usize;
        bytes.truncate(MAX_READ_BYTES as usize);

        let content = String::from_utf8_lossy(&bytes);
        let mut out = format!("===== {} =====\n{}", file.display(), content);
        if truncated {
            out.push_str("\n...[文件过大，已截断]...");
        }
        Ok(out)
    }

    /// 写入完整文件；仅在显式要求时才追加。
    pub fn write_file(&self, path: &str, content: &str, append: bool) -> anyhow::Result<String> {
        let file = self.sanitize(path)?;
        if let Some(parent) = file.parent() {
            if !parent.as_os_str().is_empty() {
                std::fs::create_dir_all(parent)?;
            }
        }
        if append && file.exists() {
            use std::io::Write;

            let mut handle = std::fs::OpenOptions::new().append(true).open(&file)?;
            handle.write_all(content.as_bytes())?;
        } else {
            atomic_write(&file, content.as_bytes())?;
        }
        Ok(format!(
            "{} {}（{} 字节）",
            if append { "已追加到" } else { "已写入" },
            file.display(),
            content.len()
        ))
    }

    pub fn delete_file(&self, path: &str) -> anyhow::Result<String> {
        let file = self.sanitize(path)?;
        let metadata = std::fs::symlink_metadata(&file)
            .map_err(|_| anyhow::anyhow!("文件不存在: {}", file.display()))?;
        if metadata.file_type().is_dir() {
            anyhow::bail!("delete_file 只能删除文件，不能删除目录: {}", file.display());
        }
        std::fs::remove_file(&file)?;
        Ok(format!("已删除 {}", file.display()))
    }

    /// 精确替换文本；除非 `replace_all=true`，否则 `old_string` 必须唯一。
    pub fn edit_file(
        &self,
        path: &str,
        old_string: &str,
        new_string: &str,
        replace_all: bool,
    ) -> anyhow::Result<String> {
        let file = self.sanitize(path)?;
        if !file.is_file() {
            anyhow::bail!("文件不存在: {}", file.display());
        }
        if old_string.is_empty() {
            anyhow::bail!("old_string 不能为空");
        }
        let content = std::fs::read_to_string(&file)?;
        let count = content.matches(old_string).count();
        if count == 0 {
            anyhow::bail!("未找到要替换的文本（old_string 无匹配），请先用 read_file 确认文件内容");
        }
        if count > 1 && !replace_all {
            anyhow::bail!(
                "old_string 有 {count} 处匹配，不唯一；请提供更长的上下文，或确认后设置 replace_all=true"
            );
        }
        let replaced = if replace_all {
            content.replace(old_string, new_string)
        } else {
            content.replacen(old_string, new_string, 1)
        };
        atomic_write(&file, replaced.as_bytes())?;
        Ok(format!(
            "已编辑 {}（替换 {} 处）",
            file.display(),
            if replace_all { count } else { 1 }
        ))
    }

    /// 以大小写不敏感的 `*` / `?` 通配符递归搜索文件名。
    pub fn search_files(&self, path: &str, pattern: &str) -> anyhow::Result<String> {
        let dir = self.sanitize(path)?;
        if !dir.is_dir() {
            anyhow::bail!("目录不存在: {}", dir.display());
        }
        let mut files = Vec::new();
        let truncated = walk_files(&dir, 0, &mut files);
        let mut hits = files
            .iter()
            .filter(|path| {
                path.file_name()
                    .map(|name| wildcard_match(pattern, &name.to_string_lossy()))
                    .unwrap_or(false)
            })
            .map(|path| self.display_path(path))
            .collect::<Vec<_>>();
        hits.sort_by_key(|path| path.to_lowercase());
        if hits.is_empty() {
            return Ok(format!("没有文件名匹配“{pattern}”的文件。"));
        }
        let suffix = if truncated {
            format!("\n...[搜索已达到 {MAX_WALK_FILES} 个文件或 {MAX_WALK_DEPTH} 层限制]...")
        } else {
            String::new()
        };
        Ok(format!(
            "匹配 {} 个文件:\n{}{suffix}",
            hits.len(),
            hits.join("\n")
        ))
    }

    /// 用正则表达式搜索文本文件，返回 `文件:行号: 内容` 条目。
    pub fn grep_files(
        &self,
        path: &str,
        pattern: &str,
        max_results: usize,
    ) -> anyhow::Result<String> {
        let dir = self.sanitize(path)?;
        if !dir.is_dir() {
            anyhow::bail!("目录不存在: {}", dir.display());
        }
        let regex =
            regex::Regex::new(pattern).map_err(|e| anyhow::anyhow!("正则表达式无效: {e}"))?;
        let cap = max_results.clamp(1, MAX_GREP_RESULTS);
        let mut files = Vec::new();
        let walk_truncated = walk_files(&dir, 0, &mut files);
        let mut hits = Vec::new();
        for file in files {
            if hits.len() >= cap {
                break;
            }
            let Ok(metadata) = std::fs::metadata(&file) else {
                continue;
            };
            if metadata.len() > MAX_GREP_FILE_BYTES {
                continue;
            }
            let Ok(bytes) = std::fs::read(&file) else {
                continue;
            };
            if bytes.contains(&0) {
                continue;
            }
            let content = String::from_utf8_lossy(&bytes);
            for (index, line) in content.lines().enumerate() {
                if regex.is_match(line) {
                    hits.push(format!(
                        "{}:{}: {}",
                        self.display_path(&file),
                        index + 1,
                        line.trim_end()
                    ));
                    if hits.len() >= cap {
                        break;
                    }
                }
            }
        }
        if hits.is_empty() {
            return Ok(format!("没有匹配“{pattern}”的内容。"));
        }
        let truncated = hits.len() >= cap || walk_truncated;
        let suffix = if truncated {
            "\n...[结果已达到限制]..."
        } else {
            ""
        };
        Ok(format!(
            "匹配 {} 行:\n{}{suffix}",
            hits.len(),
            hits.join("\n")
        ))
    }

    fn display_path(&self, path: &Path) -> String {
        let root = canonicalize_allow_missing(&self.sandbox_dir)
            .unwrap_or_else(|_| self.sandbox_dir.clone());
        path.strip_prefix(root)
            .map(|relative| relative.display().to_string())
            .unwrap_or_else(|_| path.display().to_string())
    }
}

/// 确定性地收集普通文件。绝不跟随符号链接/junction。
fn walk_files(dir: &Path, depth: usize, out: &mut Vec<PathBuf>) -> bool {
    if depth > MAX_WALK_DEPTH || out.len() >= MAX_WALK_FILES {
        return true;
    }
    let Ok(read_dir) = std::fs::read_dir(dir) else {
        return false;
    };
    let mut entries = read_dir.filter_map(Result::ok).collect::<Vec<_>>();
    entries.sort_by_key(|entry| entry.file_name().to_string_lossy().to_lowercase());

    let mut truncated = false;
    for entry in entries {
        if out.len() >= MAX_WALK_FILES {
            return true;
        }
        let Ok(file_type) = entry.file_type() else {
            continue;
        };
        if file_type.is_symlink() {
            continue;
        }
        if file_type.is_dir() {
            if depth >= MAX_WALK_DEPTH {
                truncated = true;
            } else {
                truncated |= walk_files(&entry.path(), depth + 1, out);
            }
        } else if file_type.is_file() {
            out.push(entry.path());
        }
    }
    truncated
}

fn wildcard_match(pattern: &str, name: &str) -> bool {
    let pattern = pattern.to_lowercase().chars().collect::<Vec<_>>();
    let name = name.to_lowercase().chars().collect::<Vec<_>>();
    let (mut pattern_index, mut name_index) = (0usize, 0usize);
    let (mut star, mut retry) = (None::<usize>, 0usize);
    while name_index < name.len() {
        if pattern_index < pattern.len()
            && (pattern[pattern_index] == '?' || pattern[pattern_index] == name[name_index])
        {
            pattern_index += 1;
            name_index += 1;
        } else if pattern_index < pattern.len() && pattern[pattern_index] == '*' {
            star = Some(pattern_index);
            retry = name_index;
            pattern_index += 1;
        } else if let Some(star_index) = star {
            pattern_index = star_index + 1;
            retry += 1;
            name_index = retry;
        } else {
            return false;
        }
    }
    while pattern_index < pattern.len() && pattern[pattern_index] == '*' {
        pattern_index += 1;
    }
    pattern_index == pattern.len()
}

/// 规范化最深存在的祖先目录，拼接缺失的后缀，再做归一化。
fn canonicalize_allow_missing(path: &Path) -> anyhow::Result<PathBuf> {
    let mut ancestor = path.to_path_buf();
    let mut suffix = Vec::<OsString>::new();
    loop {
        match ancestor.canonicalize() {
            Ok(base) => {
                let resolved = suffix
                    .into_iter()
                    .rev()
                    .fold(base, |path, component| path.join(component));
                return Ok(lexical_normalize(&resolved));
            }
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
                let Some(component) = ancestor.file_name().map(OsString::from) else {
                    return Err(error.into());
                };
                suffix.push(component);
                if !ancestor.pop() {
                    return Err(error.into());
                }
            }
            Err(error) => return Err(error.into()),
        }
    }
}

fn lexical_normalize(path: &Path) -> PathBuf {
    let mut normalized = PathBuf::new();
    for component in path.components() {
        match component {
            Component::Prefix(prefix) => normalized.push(prefix.as_os_str()),
            Component::RootDir => normalized.push(component.as_os_str()),
            Component::CurDir => {}
            Component::ParentDir => {
                let _ = normalized.pop();
            }
            Component::Normal(part) => normalized.push(part),
        }
    }
    normalized
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicU64, Ordering};

    static TEST_COUNTER: AtomicU64 = AtomicU64::new(0);

    struct TempDir(PathBuf);

    impl TempDir {
        fn new(label: &str) -> Self {
            let id = TEST_COUNTER.fetch_add(1, Ordering::Relaxed);
            let path = std::env::temp_dir().join(format!(
                "lingchat_file_tools_{label}_{}_{}",
                std::process::id(),
                id
            ));
            std::fs::create_dir_all(&path).unwrap();
            Self(path)
        }
    }

    impl Drop for TempDir {
        fn drop(&mut self) {
            let _ = std::fs::remove_dir_all(&self.0);
        }
    }

    fn tools(root: &Path) -> FileTools {
        FileTools {
            sandbox_dir: root.to_path_buf(),
            allow_any_path: false,
        }
    }

    #[test]
    fn sanitize_rejects_outside_and_normalizes_missing_paths() {
        let root = TempDir::new("sandbox");
        let file_tools = tools(&root.0);
        assert!(file_tools.sanitize("../outside").is_err());
        assert!(file_tools
            .sanitize("missing/../../outside/new.txt")
            .is_err());
        assert_eq!(
            file_tools.sanitize("sub/../inner.txt").unwrap(),
            root.0.canonicalize().unwrap().join("inner.txt")
        );
    }

    #[test]
    fn allow_any_path_still_returns_a_normalized_path() {
        let root = TempDir::new("any");
        let file_tools = FileTools {
            sandbox_dir: root.0.clone(),
            allow_any_path: true,
        };
        assert_eq!(
            file_tools.sanitize("sub/../file.txt").unwrap(),
            root.0.canonicalize().unwrap().join("file.txt")
        );
    }

    #[test]
    fn complete_file_tool_lifecycle_works() {
        let root = TempDir::new("lifecycle");
        let file_tools = tools(&root.0);
        file_tools
            .write_file("notes/example.txt", "alpha\nbeta\n", false)
            .unwrap();
        file_tools
            .write_file("notes/example.txt", "gamma\n", true)
            .unwrap();
        file_tools
            .edit_file("notes/example.txt", "beta", "BETA", false)
            .unwrap();

        let read = file_tools.read_file("notes/example.txt").unwrap();
        assert!(read.contains("alpha\nBETA\ngamma"));
        let list = file_tools.list_files("notes").unwrap();
        assert!(list.contains("example.txt"));
        let search = file_tools.search_files(".", "*.TXT").unwrap();
        assert!(search.contains("notes\\example.txt") || search.contains("notes/example.txt"));
        let grep = file_tools.grep_files(".", "B.TA", 10).unwrap();
        assert!(grep.contains("example.txt:2: BETA"));
        file_tools.delete_file("notes/example.txt").unwrap();
        assert!(!root.0.join("notes/example.txt").exists());
    }

    #[test]
    fn read_file_handles_non_utf8_lossily() {
        let root = TempDir::new("binary");
        std::fs::write(root.0.join("mixed.bin"), [0xff, b'A']).unwrap();
        let output = tools(&root.0).read_file("mixed.bin").unwrap();
        assert!(output.contains('A'));
    }

    #[cfg(windows)]
    #[test]
    fn recursive_search_does_not_follow_directory_symlinks() {
        use std::os::windows::fs::symlink_dir;

        let root = TempDir::new("links");
        let outside = TempDir::new("outside");
        std::fs::write(outside.0.join("secret.txt"), "secret").unwrap();
        if symlink_dir(&outside.0, root.0.join("outside-link")).is_err() {
            // 创建符号链接可能需要开发者模式或提权的测试权限。
            return;
        }
        let file_tools = tools(&root.0);
        assert!(file_tools.sanitize("outside-link/secret.txt").is_err());
        assert!(!file_tools
            .search_files(".", "*.txt")
            .unwrap()
            .contains("secret"));
    }

    #[test]
    fn wildcard_is_case_insensitive() {
        assert!(wildcard_match("report_????.CSV", "Report_2026.csv"));
        assert!(!wildcard_match("*.txt", "image.png"));
    }
}
