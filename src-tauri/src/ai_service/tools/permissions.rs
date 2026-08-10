//! 工具调用权限控制。
//!
//! 权限模型为"场景组 × 角色组"二维矩阵：
//! - [`scene_mapping`] 将调用来源映射到场景组
//! - [`scene_groups`] 定义各场景下可用工具集
//! - [`role_groups`] 将角色分组，角色必须归属于某个组才能获得工具权限
//!
//! # 快速参考
//!
//! | API | 说明 |
//! | --- | --- |
//! | [`load_or_create`](ToolPermissionConfig::load_or_create) | 加载/创建配置 |
//! | [`initialize_characters`](ToolPermissionConfig::initialize_characters) | 新角色自动归入 `default` 角色组 |
//! | [`allowed_tools`](ToolPermissionConfig::allowed_tools) | 计算角色实际可用工具集 |
//! | 场景组管理 | `get_scene_group` / `set_scene_group` / `delete_scene_group` |
//! | 角色组管理 | `create_role_group` / `delete_role_group` / `add_role_to_group` / `remove_role_from_group` / `find_role_group` |

use std::collections::{HashMap, HashSet};
use std::fs;
use std::path::Path;

use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};

use crate::ai_service::message_system::generator::GeneratorSource;

pub const CONFIG_FILE_NAME: &str = "tool_permissions.toml";

/// 默认角色组名称，新角色自动加入此组。
pub const DEFAULT_ROLE_GROUP: &str = "default";

/// 调用来源枚举，用作 scene_mapping 的键。
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq, Deserialize, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum GeneratorSourceKey {
    UserChat,
    Proactive,
    ScriptAiDialogue,
    ScriptFreeDialogue,
    EntryGreeting,
}

impl From<GeneratorSource> for GeneratorSourceKey {
    fn from(source: GeneratorSource) -> Self {
        match source {
            GeneratorSource::UserChat => Self::UserChat,
            GeneratorSource::Proactive => Self::Proactive,
            GeneratorSource::ScriptAiDialogue => Self::ScriptAiDialogue,
            GeneratorSource::ScriptFreeDialogue => Self::ScriptFreeDialogue,
            GeneratorSource::EntryGreeting => Self::EntryGreeting,
        }
    }
}

/// 权限配置根。
#[derive(Clone, Debug, Default, Deserialize, Serialize)]
pub struct ToolPermissionConfig {
    /// 软件当前可用的全部工具名。**仅用于展示**，运行时不被本模块读取、
    /// 也不参与权限计算；每次初始化直接覆盖为最新列表。
    #[serde(default)]
    pub available_tools: Vec<String>,
    /// 调用来源 → 场景组 的映射，可自定义。
    #[serde(default)]
    pub scene_mapping: HashMap<GeneratorSourceKey, String>,
    /// 场景组权限。
    #[serde(default)]
    pub scene_groups: HashMap<String, ToolPermission>,
    /// 角色组权限。角色归属到组中统一控制。
    #[serde(default)]
    pub role_groups: HashMap<String, GroupPermission>,
}

/// 场景组/条目级的工具权限。
#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct ToolPermission {
    #[serde(default = "default_enabled")]
    pub enabled: bool,
    #[serde(default)]
    pub tools: HashSet<String>,
    /// 为 true 时自动允许所有工具（跳过 tools 列表过滤）。
    #[serde(default, skip_serializing_if = "is_false")]
    pub all_tools: bool,
}

impl Default for ToolPermission {
    fn default() -> Self {
        Self {
            enabled: true,
            tools: HashSet::new(),
            all_tools: false,
        }
    }
}

/// 角色组权限。角色通过归属于组来继承权限。
#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct GroupPermission {
    #[serde(default = "default_enabled")]
    pub enabled: bool,
    #[serde(default)]
    pub tools: HashSet<String>,
    /// 为 true 时该组自动允许所有工具。
    #[serde(default, skip_serializing_if = "is_false")]
    pub all_tools: bool,
    /// 归属于该组的角色名称列表。
    #[serde(default)]
    pub roles: HashSet<String>,
}

/// 授权/收回 default 角色组对某个工具的访问（供工具设置页开关使用）。
impl ToolPermissionConfig {
    /// `allowed = true`：确保 default 角色组启用且包含该工具；
    /// `allowed = false`：仅从 default 组工具列表移除，不动组的启用状态与其他工具。
    pub fn set_tool_allowed_for_default_group(&mut self, tool: &str, allowed: bool) {
        let group = self
            .role_groups
            .entry(DEFAULT_ROLE_GROUP.to_string())
            .or_default();
        if allowed {
            group.enabled = true;
            group.tools.insert(tool.to_string());
        } else {
            group.tools.remove(tool);
        }
    }
}

impl Default for GroupPermission {
    fn default() -> Self {
        Self {
            enabled: true,
            tools: HashSet::new(),
            all_tools: false,
            roles: HashSet::new(),
        }
    }
}

// ─── ToolPermissionConfig ───

impl ToolPermissionConfig {
    /// 覆盖当前可用工具展示列表（仅写入配置，不影响权限计算）。
    pub fn set_available_tools(&mut self, tools: Vec<String>) {
        self.available_tools = tools;
    }

    pub fn load_or_create(data_dir: &Path, tool_names: impl IntoIterator<Item = String>) -> Result<Self> {
        let path = data_dir.join(CONFIG_FILE_NAME);
        if path.exists() {
            return Self::load(&path);
        }

        let config = Self::with_default_tools(tool_names);
        config.save(&path)?;
        Ok(config)
    }

    /// 确保已知角色都归属于某个角色组，未归属的自动加入 default 组。
    /// default 组默认没有任何工具权限（enabled = false）。
    pub fn initialize_characters(
        &mut self,
        data_dir: &Path,
        role_names: impl IntoIterator<Item = String>,
    ) -> Result<()> {
        let mut changed = false;
        for role_name in role_names {
            let in_group = self.role_groups.values().any(|g| g.roles.contains(&role_name));
            if !in_group {
                self.role_groups
                    .entry(DEFAULT_ROLE_GROUP.to_string())
                    .or_insert_with(|| GroupPermission {
                        enabled: false,
                        tools: HashSet::new(),
                        all_tools: false,
                        roles: HashSet::new(),
                    })
                    .roles
                    .insert(role_name);
                changed = true;
            }
        }
        if changed {
            self.save(&data_dir.join(CONFIG_FILE_NAME))?;
        }
        Ok(())
    }

    /// 根据调用来源和角色名，计算实际可用的工具集。
    /// 先通过 scene_mapping 找到场景组，再查角色组，取交集。
    /// `all_names` 是全量工具名集合，当双方均 `all_tools = true` 时返回全集。
    pub fn allowed_tools(
        &self,
        source: GeneratorSource,
        role_name: Option<&str>,
        all_names: &HashSet<String>,
    ) -> HashSet<String> {
        // 1. 查映射找到场景组名（映射不存在时回退到 scene_default）
        let key: GeneratorSourceKey = source.into();
        let scene_group_name = self.scene_mapping.get(&key).map(|s| s.as_str()).unwrap_or("scene_default");
        let Some(scene) = self.scene_groups.get(scene_group_name) else {
            return HashSet::new();
        };
        if !scene.enabled {
            return HashSet::new();
        }

        // 2. 角色归属的角色组权限
        // 没有角色名时直接返回空集 —— 未登录/未指定角色的调用方没有工具权限，
        // 必须通过角色归属到某个角色组才能获取工具访问权。
        let Some(role_name) = role_name else {
            return HashSet::new();
        };
        let Some(group) = self.role_groups.values().find(|g| g.roles.contains(role_name)) else {
            return HashSet::new();
        };
        if !group.enabled {
            return HashSet::new();
        }

        // 3. 交集 / all_tools
        // all_tools = true 且对方的工具集为空时，回退到全量名集合，
        // 使得"双方都 all_tools"时能返回所有工具。
        if group.all_tools {
            if scene.all_tools || scene.tools.is_empty() {
                all_names.clone()
            } else {
                scene.tools.clone()
            }
        } else if scene.all_tools {
            if group.all_tools || group.tools.is_empty() {
                all_names.clone()
            } else {
                group.tools.clone()
            }
        } else {
            let mut allowed = scene.tools.clone();
            allowed.retain(|name| group.tools.contains(name));
            allowed
        }
    }

    // ─── 场景组管理 ───

    /// 获取场景组权限配置（不存在则返回 None）。
    pub fn get_scene_group(&self, name: &str) -> Option<&ToolPermission> {
        self.scene_groups.get(name)
    }

    /// 创建或更新场景组。
    pub fn set_scene_group(&mut self, name: &str, permission: ToolPermission) {
        self.scene_groups.insert(name.to_string(), permission);
    }

    /// 删除场景组。
    pub fn delete_scene_group(&mut self, name: &str) -> Result<(), String> {
        if self.scene_groups.remove(name).is_none() {
            return Err(format!("场景组 '{name}' 不存在"));
        }
        Ok(())
    }

    // ─── 角色组管理 ───

    /// 创建新角色组。名称重复会返回错误。
    pub fn create_role_group(&mut self, name: &str, permission: GroupPermission) -> Result<(), String> {
        if self.role_groups.contains_key(name) {
            return Err(format!("角色组 '{name}' 已存在"));
        }
        self.role_groups.insert(name.to_string(), permission);
        Ok(())
    }

    /// 删除角色组。default 组不可删除。
    pub fn delete_role_group(&mut self, name: &str) -> Result<(), String> {
        if name == DEFAULT_ROLE_GROUP {
            return Err(format!("不能删除默认角色组 '{DEFAULT_ROLE_GROUP}'"));
        }
        if self.role_groups.remove(name).is_none() {
            return Err(format!("角色组 '{name}' 不存在"));
        }
        Ok(())
    }

    /// 获取角色组中的角色列表。
    pub fn get_role_group_roles(&self, name: &str) -> Option<&HashSet<String>> {
        self.role_groups.get(name).map(|g| &g.roles)
    }

    /// 获取所有角色组名称。
    pub fn get_all_role_groups(&self) -> Vec<&str> {
        self.role_groups.keys().map(|s| s.as_str()).collect()
    }

    /// 添加角色到指定组（角色会自动从其他组移除，确保一对一组归属）。
    pub fn add_role_to_group(&mut self, group: &str, role: &str) -> Result<(), String> {
        if !self.role_groups.contains_key(group) {
            return Err(format!("角色组 '{group}' 不存在"));
        }
        // 从所有其他组移除
        for g in self.role_groups.values_mut() {
            g.roles.remove(role);
        }
        self.role_groups.get_mut(group).unwrap().roles.insert(role.to_string());
        Ok(())
    }

    /// 从组中移除角色。移除后角色不归属任何组，下次初始化会回到 default。
    pub fn remove_role_from_group(&mut self, group: &str, role: &str) -> Result<(), String> {
        let g = self.role_groups.get_mut(group).ok_or_else(|| format!("角色组 '{group}' 不存在"))?;
        if !g.roles.remove(role) {
            return Err(format!("角色 '{role}' 不在组 '{group}' 中"));
        }
        Ok(())
    }

    /// 获取角色所在的角色组名。
    pub fn find_role_group(&self, role: &str) -> Option<&str> {
        self.role_groups
            .iter()
            .find(|(_, g)| g.roles.contains(role))
            .map(|(name, _)| name.as_str())
    }

    // ─── 内部方法 ───

    fn with_default_tools(tool_names: impl IntoIterator<Item = String>) -> Self {
        let all_tools: HashSet<_> = tool_names.into_iter().collect();

        // 默认映射
        let mut scene_mapping = HashMap::new();
        scene_mapping.insert(GeneratorSourceKey::UserChat, "scene_admin".into());
        scene_mapping.insert(GeneratorSourceKey::Proactive, "scene_normal".into());
        scene_mapping.insert(GeneratorSourceKey::ScriptFreeDialogue, "scene_normal".into());
        scene_mapping.insert(GeneratorSourceKey::ScriptAiDialogue, "scene_default".into());
        scene_mapping.insert(GeneratorSourceKey::EntryGreeting, "scene_default".into());

        // 默认场景组
        let mut scene_groups = HashMap::new();
        scene_groups.insert(
            "scene_admin".into(),
            ToolPermission {
                enabled: true,
                tools: all_tools.clone(),
                all_tools: true,
            },
        );
        scene_groups.insert(
            "scene_normal".into(),
            ToolPermission {
                enabled: true,
                tools: all_tools.clone(),
                all_tools: false,
            },
        );
        scene_groups.insert(
            "scene_default".into(),
            ToolPermission {
                enabled: false,
                tools: HashSet::new(),
                all_tools: false,
            },
        );

        // 默认角色组
        let mut role_groups = HashMap::new();
        role_groups.insert(
            DEFAULT_ROLE_GROUP.to_string(),
            GroupPermission {
                enabled: false,
                tools: HashSet::new(),
                all_tools: false,
                roles: HashSet::new(),
            },
        );

        Self {
            available_tools: Vec::new(),
            scene_mapping,
            scene_groups,
            role_groups,
        }
    }

    fn load(path: &Path) -> Result<Self> {
        let text = fs::read_to_string(path)
            .with_context(|| format!("读取工具权限配置失败: {}", path.display()))?;
        toml::from_str(&text)
            .with_context(|| format!("解析工具权限配置失败: {}", path.display()))
    }

    pub fn save(&self, path: &Path) -> Result<()> {
        let text = toml::to_string_pretty(self).context("序列化工具权限配置失败")?;
        super::atomic_replace(path, text.as_bytes())
            .map_err(anyhow::Error::msg)
            .with_context(|| format!("保存工具权限配置失败: {}", path.display()))?;
        Ok(())
    }
}

const fn default_enabled() -> bool {
    true
}

fn is_false(b: &bool) -> bool {
    !b
}

#[cfg(test)]
mod tests {
    use super::*;

    /// available_tools 仅作展示，必须序列化到 TOML 最前面。
    #[test]
    fn available_tools_serialized_first() {
        let config = ToolPermissionConfig {
            available_tools: vec!["get_current_time".into(), "schedule_add_todo".into()],
            ..Default::default()
        };
        let toml_str = toml::to_string(&config).unwrap();
        assert!(toml_str.starts_with("available_tools = [\"get_current_time\", \"schedule_add_todo\"]\n"));
    }

    /// 旧配置无 available_tools 字段时也能正常反序列化（serde default）。
    #[test]
    fn deserializes_legacy_config_without_available_tools() {
        let legacy = r#"
[scene_mapping]
user_chat = "scene_admin"
"#;
        let config: ToolPermissionConfig = toml::from_str(legacy).unwrap();
        assert!(config.available_tools.is_empty());
    }

    #[test]
    fn save_can_replace_existing_permission_file() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("permissions.toml");
        let mut config = ToolPermissionConfig::default();
        config.save(&path).unwrap();
        config.available_tools = vec!["get_current_time".into()];
        config.save(&path).unwrap();

        let loaded = ToolPermissionConfig::load(&path).unwrap();
        assert_eq!(loaded.available_tools, vec!["get_current_time"]);
    }
}
