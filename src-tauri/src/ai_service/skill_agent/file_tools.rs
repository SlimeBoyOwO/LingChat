//! LLM 可调用的文件工具。以沙箱根目录为边界，除非显式开启任意路径。

use std::path::{Path, PathBuf};

const MAX_READ_BYTES: u64 = 200 * 1024;

pub struct FileTools {
    pub sandbox_dir: PathBuf,
    pub allow_any_path: bool,
}

impl FileTools {
    /// 解析 LLM 提供的路径：相对路径拼到沙箱根，并强制作用域。
    pub fn sanitize(&self, path: &str) -> anyhow::Result<PathBuf> {
        let p = PathBuf::from(path.trim());
        let abs = if p.is_absolute() {
            p.clone()
        } else {
            self.sandbox_dir.join(&p)
        };

        if self.allow_any_path {
            return Ok(abs);
        }

        let root = canonicalize_deepest(&self.sandbox_dir);
        let target = canonicalize_deepest(&abs);
        if target.starts_with(&root) {
            Ok(abs)
        } else {
            anyhow::bail!(
                "拒绝访问文件沙箱之外的路径: {}（可在「助手设置」中开启允许任意路径）",
                path
            )
        }
    }

    pub fn list_files(&self, path: &str) -> anyhow::Result<String> {
        let dir = self.sanitize(path)?;
        if !dir.is_dir() {
            anyhow::bail!("目录不存在: {}", dir.display());
        }
        let mut entries: Vec<(bool, String)> = Vec::new();
        for entry in std::fs::read_dir(&dir)? {
            let entry = entry?;
            let name = entry.file_name().to_string_lossy().to_string();
            entries.push((entry.file_type()?.is_dir(), name));
        }
        entries.sort();
        let mut lines = format!("📁 {}\n", dir.display());
        for (is_dir, name) in entries {
            let prefix = if is_dir { "📁 " } else { "📄 " };
            lines.push_str(&format!("{}{}\n", prefix, name));
        }
        Ok(lines)
    }

    pub fn read_file(&self, path: &str) -> anyhow::Result<String> {
        let file = self.sanitize(path)?;
        if !file.is_file() {
            anyhow::bail!("文件不存在: {}", file.display());
        }
        let meta = std::fs::metadata(&file)?;
        let truncated = meta.len() > MAX_READ_BYTES;
        let content = if truncated {
            use std::io::Read;
            let mut f = std::fs::File::open(&file)?;
            let mut buf = vec![0u8; MAX_READ_BYTES as usize];
            let n = f.read(&mut buf)?;
            buf.truncate(n);
            String::from_utf8_lossy(&buf).to_string()
        } else {
            std::fs::read_to_string(&file)?
        };
        let mut out = format!("===== {} =====\n{}", file.display(), content);
        if truncated {
            out.push_str("\n...[文件过大，已截断]...");
        }
        Ok(out)
    }

    /// 写入文件；`append=true` 时追加（用于 LLM 分块补写被截断的大文件）。
    pub fn write_file(&self, path: &str, content: &str, append: bool) -> anyhow::Result<String> {
        let file = self.sanitize(path)?;
        if let Some(parent) = file.parent() {
            if !parent.as_os_str().is_empty() {
                std::fs::create_dir_all(parent)?;
            }
        }
        if append && file.exists() {
            use std::io::Write;
            let mut f = std::fs::OpenOptions::new().append(true).open(&file)?;
            f.write_all(content.as_bytes())?;
        } else {
            std::fs::write(&file, content)?;
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
        if !file.exists() {
            anyhow::bail!("文件不存在: {}", file.display());
        }
        std::fs::remove_file(&file)?;
        Ok(format!("已删除 {}", file.display()))
    }

    /// 精确替换文件中的文本：`old_string` 必须唯一匹配（除非 `replace_all`）。
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
            anyhow::bail!("未找到要替换的文本（old_string 无匹配），请先 read_file 确认文件内容");
        }
        if count > 1 && !replace_all {
            anyhow::bail!(
                "old_string 有 {} 处匹配，不唯一；请带上更长的上下文，或确认后设 replace_all=true",
                count
            );
        }
        let replaced = if replace_all {
            content.replace(old_string, new_string)
        } else {
            content.replacen(old_string, new_string, 1)
        };
        std::fs::write(&file, replaced)?;
        Ok(format!(
            "已编辑 {}（替换 {} 处）",
            file.display(),
            if replace_all { count } else { 1 }
        ))
    }

    /// 递归查找文件名匹配通配符（`*` / `?`，大小写不敏感）的文件。
    pub fn search_files(&self, path: &str, pattern: &str) -> anyhow::Result<String> {
        let dir = self.sanitize(path)?;
        if !dir.is_dir() {
            anyhow::bail!("目录不存在: {}", dir.display());
        }
        let mut files = Vec::new();
        walk_files(&dir, 0, &mut files);
        let mut hits: Vec<String> = files
            .iter()
            .filter(|p| {
                p.file_name()
                    .map(|n| wildcard_match(pattern, &n.to_string_lossy()))
                    .unwrap_or(false)
            })
            .map(|p| self.display_path(p))
            .collect();
        hits.sort();
        if hits.is_empty() {
            return Ok(format!("没有文件名匹配「{}」的文件。", pattern));
        }
        Ok(format!("匹配 {} 个文件:\n{}", hits.len(), hits.join("\n")))
    }

    /// 用正则表达式在文本文件中搜索内容，返回 `文件:行号: 内容` 列表。
    pub fn grep_files(&self, path: &str, pattern: &str, max_results: usize) -> anyhow::Result<String> {
        let dir = self.sanitize(path)?;
        if !dir.is_dir() {
            anyhow::bail!("目录不存在: {}", dir.display());
        }
        let re = regex::Regex::new(pattern)
            .map_err(|e| anyhow::anyhow!("正则表达式无效: {}", e))?;
        let cap = max_results.clamp(1, MAX_GREP_RESULTS);
        let mut files = Vec::new();
        walk_files(&dir, 0, &mut files);
        let mut hits: Vec<String> = Vec::new();
        for file in files {
            if hits.len() >= cap {
                break;
            }
            let Ok(meta) = std::fs::metadata(&file) else { continue };
            if meta.len() > MAX_GREP_FILE_BYTES {
                continue;
            }
            let Ok(content) = std::fs::read_to_string(&file) else { continue };
            for (index, line) in content.lines().enumerate() {
                if re.is_match(line) {
                    hits.push(format!("{}:{}: {}", self.display_path(&file), index + 1, line.trim_end()));
                    if hits.len() >= cap {
                        break;
                    }
                }
            }
        }
        if hits.is_empty() {
            return Ok(format!("没有匹配「{}」的内容。", pattern));
        }
        Ok(format!("匹配 {} 行:\n{}", hits.len(), hits.join("\n")))
    }

    /// 展示用路径：能相对沙箱根则用相对路径，否则完整路径。
    fn display_path(&self, path: &Path) -> String {
        path.strip_prefix(&self.sandbox_dir)
            .map(|p| p.display().to_string())
            .unwrap_or_else(|_| path.display().to_string())
    }
}

const MAX_WALK_DEPTH: usize = 10;
const MAX_WALK_FILES: usize = 500;
const MAX_GREP_FILE_BYTES: u64 = 1024 * 1024;
const MAX_GREP_RESULTS: usize = 100;

/// 递归收集目录下的文件（限深度与总量，防止超大目录拖垮工具）。
fn walk_files(dir: &Path, depth: usize, out: &mut Vec<PathBuf>) {
    if depth > MAX_WALK_DEPTH || out.len() >= MAX_WALK_FILES {
        return;
    }
    let Ok(entries) = std::fs::read_dir(dir) else { return };
    for entry in entries.flatten() {
        if out.len() >= MAX_WALK_FILES {
            return;
        }
        let path = entry.path();
        if path.is_dir() {
            walk_files(&path, depth + 1, out);
        } else {
            out.push(path);
        }
    }
}

/// 大小写不敏感的通配符匹配（`*` 任意序列，`?` 单个字符）。
fn wildcard_match(pattern: &str, name: &str) -> bool {
    let p: Vec<char> = pattern.to_lowercase().chars().collect();
    let n: Vec<char> = name.to_lowercase().chars().collect();
    let (mut pi, mut ni) = (0usize, 0usize);
    let (mut star, mut retry) = (None::<usize>, 0usize);
    while ni < n.len() {
        if pi < p.len() && (p[pi] == '?' || p[pi] == n[ni]) {
            pi += 1;
            ni += 1;
        } else if pi < p.len() && p[pi] == '*' {
            star = Some(pi);
            retry = ni;
            pi += 1;
        } else if let Some(s) = star {
            pi = s + 1;
            retry += 1;
            ni = retry;
        } else {
            return false;
        }
    }
    while pi < p.len() && p[pi] == '*' {
        pi += 1;
    }
    pi == p.len()
}

/// Canonicalize 路径上最深的已存在祖先（目标可能尚不存在）。
fn canonicalize_deepest(path: &Path) -> PathBuf {
    let mut current = path.to_path_buf();
    loop {
        if current.exists() {
            return current.canonicalize().unwrap_or(current);
        }
        if !current.pop() {
            return path.to_path_buf();
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn tmp() -> PathBuf {
        std::env::temp_dir().join(format!("lingchat_ft_test_{}", std::process::id()))
    }

    #[test]
    fn sanitize_rejects_outside_sandbox() {
        let root = tmp();
        std::fs::create_dir_all(&root).unwrap();
        let ft = FileTools {
            sandbox_dir: root.clone(),
            allow_any_path: false,
        };
        assert!(ft.sanitize("../outside").is_err());
        assert!(ft.sanitize("C:/windows").is_err());
        assert!(ft.sanitize("sub/inner").is_ok());
        let _ = std::fs::remove_dir_all(&root);
    }

    #[test]
    fn sanitize_allows_when_any_path() {
        let ft = FileTools {
            sandbox_dir: tmp(),
            allow_any_path: true,
        };
        assert!(ft.sanitize("C:/windows").is_ok());
    }
}
