use std::collections::HashMap;
use std::path::Path;
use std::sync::{Arc, RwLock};

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
///
/// 内部使用 `RwLock`，支持运行期热注册/热注销（插件启用/禁用），
/// 对外方法全部为 `&self`。
#[derive(Default)]
pub struct ToolRegistry {
    tools: RwLock<HashMap<String, Arc<dyn Tool>>>,
    order: RwLock<Vec<String>>,
    permissions: RwLock<ToolPermissionConfig>,
}

impl ToolRegistry {
    /// 创建空注册表。
    pub fn new() -> Self {
        Self::default()
    }

    /// 用持久化权限配置覆盖默认权限。
    pub fn set_permissions(&self, permissions: ToolPermissionConfig) {
        *self.permissions.write().unwrap() = permissions;
    }

    /// 返回当前权限配置的克隆（供权限页读取）。
    pub fn permissions(&self) -> ToolPermissionConfig {
        self.permissions.read().unwrap().clone()
    }

    /// 注册工具；重复名称会失败。
    pub fn register(&self, tool: Arc<dyn Tool>) -> Result<(), RegistryError> {
        let name = tool.definition().function.name;
        let mut tools = self.tools.write().unwrap();
        if tools.contains_key(&name) {
            return Err(RegistryError::DuplicateName(name));
        }
        self.order.write().unwrap().push(name.clone());
        tools.insert(name, tool);
        Ok(())
    }

    /// 注销工具；不存在的工具忽略。
    pub fn unregister(&self, name: &str) {
        let mut tools = self.tools.write().unwrap();
        if tools.remove(name).is_some() {
            let mut order = self.order.write().unwrap();
            order.retain(|n| n != name);
        }
    }

    /// 按名称查找工具。
    pub fn get(&self, name: &str) -> Option<Arc<dyn Tool>> {
        self.tools.read().unwrap().get(name).cloned()
    }

    /// 按注册顺序返回工具定义快照。
    pub fn definitions(&self) -> Vec<ToolDefinition> {
        let tools = self.tools.read().unwrap();
        self.order
            .read()
            .unwrap()
            .iter()
            .filter_map(|name| tools.get(name))
            .map(|tool| tool.definition())
            .collect()
    }

    /// 根据预计算的允许工具集合过滤定义，避免在调用方重复计算权限。
    pub fn definitions_for_allowed(
        &self,
        allowed: &std::collections::HashSet<String>,
    ) -> Vec<ToolDefinition> {
        let tools = self.tools.read().unwrap();
        self.order
            .read()
            .unwrap()
            .iter()
            .filter(|name| allowed.contains(*name))
            .filter_map(|name| tools.get(name))
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
        let all_names: std::collections::HashSet<String> = self
            .definitions()
            .into_iter()
            .map(|d| d.function.name)
            .collect();
        self.permissions
            .read()
            .unwrap()
            .allowed_tools(source, role_name, &all_names)
    }

    /// 把插件工具名并入 available_tools 展示列表（不落盘）。
    pub fn add_available_tools(&self, names: &[String]) {
        let mut perms = self.permissions.write().unwrap();
        let mut merged: std::collections::HashSet<String> =
            perms.available_tools.iter().cloned().collect();
        merged.extend(names.iter().cloned());
        perms.available_tools = merged.into_iter().collect();
    }

    /// 从 available_tools 展示列表移除插件工具名（插件禁用时调用，不落盘）。
    pub fn remove_available_tools(&self, names: &[String]) {
        let mut perms = self.permissions.write().unwrap();
        let mut set: std::collections::HashSet<String> =
            perms.available_tools.iter().cloned().collect();
        for name in names {
            set.remove(name);
        }
        perms.available_tools = set.into_iter().collect();
    }

    /// 把当前权限配置落盘到 data_dir/tool_permissions.toml。
    ///
    /// 用原子 tmp+rename 模式（与 permissions.rs 内部一致）。
    pub fn save_permissions(&self, data_dir: &Path) -> anyhow::Result<()> {
        let perms = self.permissions.read().unwrap();
        let path = data_dir.join(super::permissions::CONFIG_FILE_NAME);
        perms.save(&path)
    }

    /// 返回 available_tools 展示列表（供前端权限页/插件页读取）。
    pub fn available_tools(&self) -> Vec<String> {
        self.permissions.read().unwrap().available_tools.clone()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ai_service::tools::clock::CurrentTimeTool;

    /// 验证注册、发现与重复名称保护。
    #[test]
    fn registers_tools_in_stable_order() {
        let registry = ToolRegistry::new();
        registry.register(Arc::new(CurrentTimeTool)).unwrap();
        assert!(registry.get("get_current_time").is_some());
        assert_eq!(registry.definitions()[0].function.name, "get_current_time");
        assert!(registry.register(Arc::new(CurrentTimeTool)).is_err());
        assert!(registry.get("missing").is_none());
    }

    /// 验证注销后不可见且不破坏其余顺序。
    #[test]
    fn unregisters_tools() {
        let registry = ToolRegistry::new();
        registry.register(Arc::new(CurrentTimeTool)).unwrap();
        registry.unregister("get_current_time");
        assert!(registry.get("get_current_time").is_none());
        assert!(registry.definitions().is_empty());
        // 重新注册应成功（名称已释放）
        registry.register(Arc::new(CurrentTimeTool)).unwrap();
        assert!(registry.get("get_current_time").is_some());
    }
}
