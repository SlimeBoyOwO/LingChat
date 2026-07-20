//! 集成测试共享 helper：递归遍历目录 + 构造最小可导入的角色结构。
//!
//! 与 archive.rs 内的原 #[cfg(test)] helper 保持一致，迁移到集成测试
//! crate 后改为 pub 形式以便子模块访问。

use std::path::{Path, PathBuf};

/// 递归遍历目录，返回所有子文件与目录的路径列表（深度优先）。
pub fn walkdir(dir: &Path) -> Vec<PathBuf> {
    let mut out = Vec::new();
    if dir.is_file() {
        out.push(dir.to_path_buf());
        return out;
    }
    for entry in std::fs::read_dir(dir).unwrap() {
        let entry = entry.unwrap();
        out.push(entry.path());
        if entry.path().is_dir() {
            out.extend(walkdir(&entry.path()));
        }
    }
    out
}

/// 在 `tmp/<sample>/` 下创建一个最小角色目录：
/// `avatar/a.png`, `avatar/b.txt`, `settings.yml`。
/// 返回 sample 目录路径。
pub fn _build_sample_role(tmp: &Path) -> PathBuf {
    let role = tmp.join("sample");
    std::fs::create_dir_all(role.join("avatar")).unwrap();
    std::fs::write(role.join("settings.yml"), "title: Sample
").unwrap();
    std::fs::write(role.join("avatar/a.png"), b"AAAA").unwrap();
    std::fs::write(role.join("avatar/b.txt"), b"BBBB").unwrap();
    role
}
