pub mod clock;
pub mod executor;
pub mod registry;
pub mod tool_loop;

use std::sync::Arc;

use anyhow::Result;

use clock::CurrentTimeTool;
use registry::ToolRegistry;

/// 创建并注册所有内置聊天工具。
pub fn built_in_registry() -> Result<ToolRegistry> {
    let mut registry = ToolRegistry::new();
    registry.register(Arc::new(CurrentTimeTool))?;
    Ok(registry)
}
