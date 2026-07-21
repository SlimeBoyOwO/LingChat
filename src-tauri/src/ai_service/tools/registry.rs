use std::collections::HashMap;
use std::sync::Arc;

use thiserror::Error;

use crate::ai_service::types::ToolDefinition;

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
}

impl ToolRegistry {
    /// 创建空注册表。
    pub fn new() -> Self {
        Self::default()
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
