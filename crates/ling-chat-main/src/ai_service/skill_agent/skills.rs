//! SKILL.md 技能库发现与读取。

use serde::Serialize;
use std::collections::HashSet;
use std::fs;
use std::path::{Component, Path, PathBuf};

const MAX_SKILLS: usize = 200;
const MAX_SKILL_BYTES: u64 = 512 * 1024;
/// 技能参考文件单次读取上限。技能目录里的方块状态表接近 512KB，读太多会把导演上下文撑爆。
const MAX_SKILL_READ_CHARS: usize = 64 * 1024;
const MAX_SKILL_READ_BYTES: u64 = 4 * 1024 * 1024;

/// 发现的技能信息。
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SkillInfo {
    pub name: String,
    pub description: String,
    /// "project" 或 "global"。
    pub location: String,
    pub path: PathBuf,
}

/// 读取技能 SKILL.md 的结果。
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SkillLoadResult {
    pub name: String,
    pub base_directory: PathBuf,
    pub content: String,
}

/// 技能搜索目录：技能根 + 两个隐藏子目录（兼容 `.claude`/`.agent` 技能布局）。
pub fn search_dirs(skills_root: &Path) -> Vec<PathBuf> {
    vec![
        skills_root.to_path_buf(),
        skills_root.join(".agent").join("skills"),
        skills_root.join(".claude").join("skills"),
    ]
}

/// 发现全部技能（去重按名优先、项目优先再按名排序）。
pub fn find_all_skills(skills_root: &Path) -> Vec<SkillInfo> {
    let mut skills = Vec::new();
    let mut seen = HashSet::new();

    for dir in search_dirs(skills_root) {
        if !dir.is_dir() {
            continue;
        }
        let Ok(base) = dir.canonicalize() else {
            continue;
        };
        let Ok(entries) = fs::read_dir(&dir) else {
            continue;
        };
        for entry in entries.flatten() {
            if skills.len() >= MAX_SKILLS {
                break;
            }
            let name = entry.file_name().to_string_lossy().to_string();
            if seen.contains(&name) {
                continue;
            }
            let path = entry.path();
            let Ok(file_type) = entry.file_type() else {
                continue;
            };
            if !file_type.is_dir() || file_type.is_symlink() {
                continue;
            }
            let Ok(canonical_skill_dir) = path.canonicalize() else {
                continue;
            };
            if !canonical_skill_dir.starts_with(&base) {
                continue;
            }
            let Some(skill_md) = safe_skill_file(&canonical_skill_dir) else {
                continue;
            };
            let Ok(content) = fs::read_to_string(&skill_md) else {
                continue;
            };
            let description = extract_yaml_field(&content, "description");
            let location = if path.starts_with(skills_root) {
                "project"
            } else {
                "global"
            };
            skills.push(SkillInfo {
                name: name.clone(),
                description,
                location: location.to_string(),
                path: canonical_skill_dir,
            });
            seen.insert(name);
        }
    }

    skills.sort_by(|a, b| {
        let a_proj = a.location == "project";
        let b_proj = b.location == "project";
        b_proj.cmp(&a_proj).then_with(|| a.name.cmp(&b.name))
    });
    skills
}

/// 按名读取技能（SKILL.md 内容 + 所在目录）。
pub fn find_skill(skills_root: &Path, name: &str) -> Option<SkillLoadResult> {
    if !is_safe_skill_name(name) {
        return None;
    }
    for dir in search_dirs(skills_root) {
        let skill_dir = dir.join(name);
        let Ok(base) = dir.canonicalize() else {
            continue;
        };
        let Ok(canonical_skill_dir) = skill_dir.canonicalize() else {
            continue;
        };
        if !canonical_skill_dir.starts_with(&base) {
            continue;
        }
        let Some(skill_md) = safe_skill_file(&canonical_skill_dir) else {
            continue;
        };
        if let Ok(content) = fs::read_to_string(&skill_md) {
            return Some(SkillLoadResult {
                name: name.to_string(),
                base_directory: canonical_skill_dir,
                content,
            });
        }
    }
    None
}

/// Resolve the actual SKILL.md and require it to remain inside its canonical skill directory.
fn safe_skill_file(canonical_skill_dir: &Path) -> Option<PathBuf> {
    let skill_md = canonical_skill_dir.join("SKILL.md").canonicalize().ok()?;
    if !skill_md.starts_with(canonical_skill_dir) {
        return None;
    }
    let metadata = fs::metadata(&skill_md).ok()?;
    (metadata.is_file() && metadata.len() <= MAX_SKILL_BYTES).then_some(skill_md)
}

/// 直接从一个已知目录读取技能。插件携带的技能不走全局技能库，由插件系统解析出目录后交给这里。
pub fn load_skill_at(base_dir: &Path) -> Option<SkillLoadResult> {
    let canonical = base_dir.canonicalize().ok()?;
    let skill_md = safe_skill_file(&canonical)?;
    let content = fs::read_to_string(&skill_md).ok()?;
    let name = canonical
        .file_name()
        .map(|name| name.to_string_lossy().to_string())?;
    Some(SkillLoadResult {
        name,
        base_directory: canonical,
        content,
    })
}

/// 读取技能目录内的参考文件。只接受技能目录内的相对路径，返回截断后的文本。
pub fn read_skill_file(base_dir: &Path, relative: &str) -> Result<(String, bool), String> {
    let base = base_dir
        .canonicalize()
        .map_err(|error| format!("技能目录无效: {error}"))?;
    let relative = relative.trim();
    if relative.is_empty() {
        return Err("缺少文件路径".to_string());
    }
    let rel_path = Path::new(relative);
    if rel_path.is_absolute()
        || rel_path.components().any(|component| {
            matches!(
                component,
                Component::ParentDir | Component::RootDir | Component::Prefix(_)
            )
        })
    {
        return Err("只允许技能目录内的相对路径".to_string());
    }

    let path = base.join(rel_path);
    let canonical = path
        .canonicalize()
        .map_err(|error| format!("读取技能文件失败: {error}"))?;
    if !canonical.starts_with(&base) {
        return Err("路径越出技能目录".to_string());
    }
    let metadata =
        fs::metadata(&canonical).map_err(|error| format!("读取技能文件失败: {error}"))?;
    if !metadata.is_file() {
        return Err("不是普通文件".to_string());
    }
    if metadata.len() > MAX_SKILL_READ_BYTES {
        return Err(format!(
            "文件太大（{} 字节，上限 {MAX_SKILL_READ_BYTES}）",
            metadata.len()
        ));
    }

    let bytes = fs::read(&canonical).map_err(|error| format!("读取技能文件失败: {error}"))?;
    let text = String::from_utf8(bytes).map_err(|_| "技能参考文件不是文本".to_string())?;
    let truncated = text.chars().count() > MAX_SKILL_READ_CHARS;
    let content = if truncated {
        let mut out: String = text.chars().take(MAX_SKILL_READ_CHARS).collect();
        out.push_str("\n\n……（文件过长，已截断）");
        out
    } else {
        text
    };
    Ok((content, truncated))
}

pub fn is_safe_skill_name(name: &str) -> bool {
    let mut components = Path::new(name).components();
    matches!(components.next(), Some(std::path::Component::Normal(_)))
        && components.next().is_none()
}

/// 从 YAML frontmatter 提取字段（最小正则，够用即可）。
pub fn extract_yaml_field(content: &str, field: &str) -> String {
    if !content.trim_start().starts_with("---") {
        return String::new();
    }
    let prefix = format!("{}:", field);
    for line in content.lines().skip(1) {
        let trimmed = line.trim();
        if trimmed == "---" {
            break;
        }
        if let Some(rest) = trimmed.strip_prefix(&prefix) {
            return rest.trim().trim_matches('"').trim_matches('\'').to_string();
        }
    }
    String::new()
}

/// 构建 `<available_skills>` 块注入系统提示；无技能时返回空串。
pub fn build_skills_xml(skills: &[SkillInfo]) -> String {
    if skills.is_empty() {
        return String::new();
    }
    let tags = skills
        .iter()
        .map(|s| {
            format!(
                "<skill>\n<name>{}</name>\n<description>{}</description>\n<location>{}</location>\n</skill>",
                escape_xml(&s.name),
                escape_xml(&s.description),
                escape_xml(&s.location)
            )
        })
        .collect::<Vec<_>>()
        .join("\n\n");
    format!(
        "\n\n<skills_system priority=\"1\">\n<available_skills>\n{}\n</available_skills>\n</skills_system>",
        tags
    )
}

fn escape_xml(value: &str) -> String {
    value
        .replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
        .replace('\'', "&apos;")
}
