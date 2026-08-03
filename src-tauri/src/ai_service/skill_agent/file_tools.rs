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
