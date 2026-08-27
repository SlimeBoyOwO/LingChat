//! SKILL.md 技能库发现与读取。
//!
//! 技能 = 一个含 `SKILL.md`（YAML frontmatter + 指令正文）的目录。选择完全
//! 交给 LLM：系统提示注入 `<available_skills>` 列表，模型用 `read_skill` 把
//! 具体技能的指令加载进上下文后再执行。没有任何规则引擎。

use serde::Serialize;
use std::collections::HashSet;
use std::fs;
use std::path::{Path, PathBuf};

const MAX_SKILLS: usize = 200;
const MAX_SKILL_BYTES: u64 = 512 * 1024;

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

fn is_safe_skill_name(name: &str) -> bool {
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
