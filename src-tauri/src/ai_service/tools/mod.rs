pub mod clock;
pub mod executor;
pub mod permissions;
pub mod registry;
pub mod tool_loop;

use std::sync::Arc;

use anyhow::Result;

use clock::CurrentTimeTool;
use permissions::ToolPermissionConfig;
use registry::ToolRegistry;

/// 创建并注册所有内置聊天工具。
pub fn built_in_registry(
    role_names: impl IntoIterator<Item = (String, String)>,
) -> Result<ToolRegistry> {
    let mut registry = ToolRegistry::new();
    registry.register(Arc::new(CurrentTimeTool))?;
    let mut permissions = ToolPermissionConfig::load_or_create(
        &crate::api::data_dir(),
        registry
            .definitions()
            .into_iter()
            .map(|definition| definition.function.name),
    )?;
    permissions.initialize_characters(
        &crate::api::data_dir(),
        role_names.into_iter().map(|(_, name)| name),
    )?;
    registry.set_permissions(permissions);
    Ok(registry)
}
