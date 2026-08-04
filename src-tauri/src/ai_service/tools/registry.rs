use std::collections::HashMap;
use std::sync::Arc;

use thiserror::Error;

use crate::ai_service::message_system::generator::GeneratorSource;
use crate::ai_service::types::ToolDefinition;

use super::permissions::ToolPermissionConfig;

use super::executor::Tool;

/// 工具注册失败。
#[derive(Debug, Error)]
pub enum RegistryError {
    #[error("工具名称重复: {0}")]
    DuplicateName(String),
}

/// 应用级聊天工具注册表。
#[derive(Default)]
pub struct ToolRegistry {
    tools: HashMap<String, Arc<dyn Tool>>,
    order: Vec<String>,
    /// 权限矩阵；RwLock 支持运行时热更新（如设置页开关工具）。
    permissions: std::sync::RwLock<ToolPermissionConfig>,
}

impl ToolRegistry {
    /// 创建空注册表。
    pub fn new() -> Self {
        Self::default()
    }

    /// 用持久化权限配置覆盖默认权限。
    pub fn set_permissions(&mut self, permissions: ToolPermissionConfig) {
        *self.permissions.get_mut().expect("权限锁已中毒") = permissions;
    }

    /// 运行时修改权限配置（调用方负责持久化）。
    pub fn update_permissions(&self, f: impl FnOnce(&mut ToolPermissionConfig)) {
        let mut guard = self.permissions.write().expect("权限锁已中毒");
        f(&mut guard);
    }

    /// 注册工具；重复名称会失败。
    pub fn register(&mut self, tool: Arc<dyn Tool>) -> Result<(), RegistryError> {
        let name = tool.definition().function.name;
        if self.tools.contains_key(&name) {
            return Err(RegistryError::DuplicateName(name));
        }
        self.order.push(name.clone());
        self.tools.insert(name, tool);
        Ok(())
    }

    /// 按名称查找工具。
    pub fn get(&self, name: &str) -> Option<Arc<dyn Tool>> {
        self.tools.get(name).cloned()
    }

    /// 按注册顺序返回工具定义快照。
    pub fn definitions(&self) -> Vec<ToolDefinition> {
        self.order
            .iter()
            .filter_map(|name| self.tools.get(name))
            .map(|tool| tool.definition())
            .collect()
    }

    /// 根据预计算的允许工具集合过滤定义，避免在调用方重复计算权限。
    pub fn definitions_for_allowed(
        &self,
        allowed: &std::collections::HashSet<String>,
    ) -> Vec<ToolDefinition> {
        self.order
            .iter()
            .filter(|name| allowed.contains(*name))
            .filter_map(|name| self.tools.get(name))
            .map(|tool| tool.definition())
            .collect()
    }

    /// 根据调用模块和角色限制返回本轮可下发给 LLM 的工具定义。
    pub fn definitions_for(
        &self,
        source: GeneratorSource,
        role_name: Option<&str>,
    ) -> Vec<ToolDefinition> {
        let allowed = self.allowed_tools(source, role_name);
        self.definitions_for_allowed(&allowed)
    }

    /// 返回本轮可执行的工具名称集合，供执行层二次校验。
    pub fn allowed_tools(
        &self,
        source: GeneratorSource,
        role_name: Option<&str>,
    ) -> std::collections::HashSet<String> {
        let all_names: std::collections::HashSet<String> = self.definitions()
            .into_iter()
            .map(|d| d.function.name)
            .collect();
        self.permissions
            .read()
            .expect("权限锁已中毒")
            .allowed_tools(source, role_name, &all_names)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ai_service::tools::clock::CurrentTimeTool;

    /// 验证注册、发现与重复名称保护。
    #[test]
    fn registers_tools_in_stable_order() {
        let mut registry = ToolRegistry::new();
        registry.register(Arc::new(CurrentTimeTool)).unwrap();
        assert!(registry.get("get_current_time").is_some());
        assert_eq!(registry.definitions()[0].function.name, "get_current_time");
        assert!(registry.register(Arc::new(CurrentTimeTool)).is_err());
        assert!(registry.get("missing").is_none());
    }
}

