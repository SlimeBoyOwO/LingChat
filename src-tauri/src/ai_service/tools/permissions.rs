use std::collections::{HashMap, HashSet};
use std::fs;
use std::path::Path;

use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};

use crate::ai_service::message_system::generator::GeneratorSource;

pub const CONFIG_FILE_NAME: &str = "tool_permissions.toml";

#[derive(Clone, Debug, Default, Deserialize, Serialize)]
pub struct ToolPermissionConfig {
    #[serde(default)]
    pub modules: HashMap<GeneratorSourceKey, ToolPermission>,
    #[serde(default)]
    pub characters: HashMap<String, ToolPermission>,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct ToolPermission {
    #[serde(default = "default_enabled")]
    pub enabled: bool,
    #[serde(default)]
    pub tools: HashSet<String>,
}

impl Default for ToolPermission {
    fn default() -> Self {
        Self {
            enabled: true,
            tools: HashSet::new(),
        }
    }
}

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

impl ToolPermissionConfig {
    pub fn load_or_create(data_dir: &Path, tool_names: impl IntoIterator<Item = String>) -> Result<Self> {
        let path = data_dir.join(CONFIG_FILE_NAME);
        if path.exists() {
            return Self::load(&path);
        }

        let config = Self::with_default_tools(tool_names);
        config.save(&path)?;
        Ok(config)
    }

    /// 为新发现的角色写入启用状态和空工具集合，并将旧数据库名称的配置复制到运行时名称。
    pub fn initialize_characters(
        &mut self,
        data_dir: &Path,
        role_names: impl IntoIterator<Item = (String, String)>,
    ) -> Result<()> {
        let mut changed = false;
        for (database_name, role_name) in role_names {
            if self.characters.contains_key(&role_name) {
                continue;
            }

            if let Some(permission) = self.characters.get(&database_name).cloned() {
                self.characters.insert(role_name, permission);
            } else {
                self.characters
                    .insert(role_name, ToolPermission::default());
            }
            changed = true;
        }
        if changed {
            self.save(&data_dir.join(CONFIG_FILE_NAME))?;
        }
        Ok(())
    }

    pub fn allowed_tools(
        &self,
        source: GeneratorSource,
        role_name: Option<&str>,
    ) -> HashSet<String> {
        let Some(module) = self.modules.get(&source.into()) else {
            return HashSet::new();
        };
        if !module.enabled {
            return HashSet::new();
        }

        let Some(role_name) = role_name else {
            return HashSet::new();
        };
        let Some(role_permission) = self.characters.get(role_name) else {
            return HashSet::new();
        };
        if !role_permission.enabled {
            return HashSet::new();
        }

        let mut allowed = module.tools.clone();
        allowed.retain(|name| role_permission.tools.contains(name));
        allowed
    }

    fn with_default_tools(tool_names: impl IntoIterator<Item = String>) -> Self {
        let tools = tool_names.into_iter().collect::<HashSet<_>>();
        let mut modules = HashMap::new();
        for source in [
            GeneratorSourceKey::UserChat,
            GeneratorSourceKey::Proactive,
            GeneratorSourceKey::ScriptAiDialogue,
            GeneratorSourceKey::ScriptFreeDialogue,
            GeneratorSourceKey::EntryGreeting,
        ] {
            modules.insert(
                source,
                ToolPermission {
                    enabled: true,
                    tools: tools.clone(),
                },
            );
        }
        Self {
            modules,
            characters: HashMap::new(),
        }
    }

    fn load(path: &Path) -> Result<Self> {
        let text = fs::read_to_string(path)
            .with_context(|| format!("读取工具权限配置失败: {}", path.display()))?;
        toml::from_str(&text)
            .with_context(|| format!("解析工具权限配置失败: {}", path.display()))
    }

    fn save(&self, path: &Path) -> Result<()> {
        let tmp = path.with_extension("tmp");
        let text = toml::to_string_pretty(self).context("序列化工具权限配置失败")?;
        fs::write(&tmp, text)
            .with_context(|| format!("写入工具权限临时配置失败: {}", tmp.display()))?;
        fs::rename(&tmp, path)
            .with_context(|| format!("保存工具权限配置失败: {}", path.display()))?;
        Ok(())
    }
}

const fn default_enabled() -> bool {
    true
}

