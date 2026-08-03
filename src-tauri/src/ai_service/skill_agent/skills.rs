//! SKILL.md 技能库发现与读取。
//!
//! 技能 = 一个含 `SKILL.md`（YAML frontmatter + 指令正文）的目录。选择完全
//! 交给 LLM：系统提示注入 `<available_skills>` 列表，模型用 `read_skill` 把
//! 具体技能的指令加载进上下文后再执行。没有任何规则引擎。

use serde::Serialize;
use std::collections::HashSet;
use std::fs;
use std::path::{Path, PathBuf};

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
        let Ok(entries) = fs::read_dir(&dir) else { continue };
        for entry in entries.flatten() {
            let name = entry.file_name().to_string_lossy().to_string();
            if seen.contains(&name) {
                continue;
            }
            let path = entry.path();
            if !path.is_dir() {
                continue;
            }
            let skill_md = path.join("SKILL.md");
            if !skill_md.exists() {
                continue;
            }
            let Ok(content) = fs::read_to_string(&skill_md) else { continue };
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
                path,
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
    for dir in search_dirs(skills_root) {
        let skill_dir = dir.join(name);
        let skill_md = skill_dir.join("SKILL.md");
        if skill_md.is_file() {
            if let Ok(content) = fs::read_to_string(&skill_md) {
                return Some(SkillLoadResult {
                    name: name.to_string(),
                    base_directory: skill_dir,
                    content,
                });
            }
        }
    }
    None
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
                s.name, s.description, s.location
            )
        })
        .collect::<Vec<_>>()
        .join("\n\n");
    format!(
        "\n\n<skills_system priority=\"1\">\n<available_skills>\n{}\n</available_skills>\n</skills_system>",
        tags
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn extracts_yaml_fields() {
        let content = "---\nname: my-skill\ndescription: Use when foo\n---\n\n# Body";
        assert_eq!(extract_yaml_field(content, "name"), "my-skill");
        assert_eq!(extract_yaml_field(content, "description"), "Use when foo");
        assert_eq!(extract_yaml_field(content, "missing"), "");
    }

    #[test]
    fn handles_quoted_and_missing_frontmatter() {
        assert_eq!(
            extract_yaml_field("---\ndescription: \"quoted value\"\n---", "description"),
            "quoted value"
        );
        assert_eq!(extract_yaml_field("# no frontmatter", "name"), "");
    }

    #[test]
    fn xml_empty_when_no_skills() {
        assert_eq!(build_skills_xml(&[]), "");
    }
}
