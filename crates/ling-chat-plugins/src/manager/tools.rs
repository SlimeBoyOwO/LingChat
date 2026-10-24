//! 插件工具的注册、注销与角色组授权。

use ling_chat_main::ai_service::tools::permissions::DEFAULT_ROLE_GROUP;
use ling_chat_main::ai_service::tools::registry::ToolRegistry;

use crate::tool::PluginTool;
use crate::types::PluginRecord;

/// 把插件的所有工具注册进 registry、并入 available_tools，并授权给 default 角色组。
///
/// 授权不能省：`available_tools` 只用于展示，真正决定工具是否下发给 LLM
/// （以及能否进 Tool RAG 的候选池）的是权限矩阵，而主聊天取的就是 default
/// 角色组的工具集——不授权的话插件工具等于声明了也没人看得见。
pub(super) fn register_tools(registry: &ToolRegistry, record: &PluginRecord) -> Result<(), String> {
    let mut registered: Vec<String> = Vec::new();
    for spec in &record.manifest.tools {
        let tool = std::sync::Arc::new(PluginTool::new(record.manifest.id.clone(), spec.clone()));
        match registry.register(tool) {
            Ok(()) => registered.push(spec.name.clone()),
            Err(e) => {
                // 精确回滚本次已注册的部分，避免与其他插件同名工具残留
                for name in &registered {
                    registry.unregister(name);
                }
                return Err(format!("{e}（已回滚 {} 个已注册工具）", registered.len()));
            },
        }
    }
    // 授权与展示列表都在全部注册成功之后做：中途失败时已注册的工具刚被注销，
    // 不会留下「注销了却还授着权」的残影。
    grant_tools_to_default_group(registry, &registered);
    registry.add_available_tools(&registered);
    Ok(())
}

/// 注销插件的所有工具（含收回授权）。
pub(super) fn unregister_tools(registry: &ToolRegistry, record: &PluginRecord) {
    let names: Vec<String> = record
        .manifest
        .tools
        .iter()
        .map(|t| t.name.clone())
        .collect();
    for name in &names {
        registry.unregister(name);
    }
    revoke_tools_from_default_group(registry, &names);
    registry.remove_available_tools(&names);
}

/// 把工具名授权给 default 角色组与主动对话场景组。
///
/// 插件工具注册后必须走这一步才可能被下发：权限矩阵里主聊天取的就是 default
/// 角色组的工具集，光进 registry 与 available_tools 展示列表都是看不见的。
fn grant_tools_to_default_group(registry: &ToolRegistry, names: &[String]) {
    if names.is_empty() {
        return;
    }
    registry.update_permissions(|p| {
        for name in names {
            p.set_tool_allowed_for_default_group(name, true);
            p.set_tool_allowed_for_proactive_scene(name, true);
        }
    });
}

/// 收回 default 角色组与主动对话场景组对这批工具名的授权。
fn revoke_tools_from_default_group(registry: &ToolRegistry, names: &[String]) {
    if names.is_empty() {
        return;
    }
    registry.update_permissions(|p| {
        for name in names {
            p.set_tool_allowed_for_proactive_scene(name, false);
        }
        // 组不存在时不动权限：那个入口会顺手建一个 enabled = true 的空组，而空
        // 工具集在 all_tools 场景下等于放行全部工具，一旦被塞进角色就是提权。
        if !p.role_groups.contains_key(DEFAULT_ROLE_GROUP) {
            return;
        }
        for name in names {
            p.set_tool_allowed_for_default_group(name, false);
        }
    });
}

#[cfg(test)]
mod tests {
    use std::path::Path;

    use ling_chat_main::ai_service::message_system::generator::GeneratorSource;
    use ling_chat_main::ai_service::tools::permissions::ToolPermissionConfig;
    use ling_chat_main::ai_service::tools::registry::ToolRegistry;

    use super::*;

    /// 造一个「角色归属 default 组」的注册表，权限取自临时目录里的新配置。
    fn registry_with_role(dir: &Path, role: &str) -> ToolRegistry {
        let mut permissions =
            ToolPermissionConfig::load_or_create(dir, std::iter::empty::<String>()).unwrap();
        permissions
            .initialize_characters(dir, [role.to_string()])
            .unwrap();
        let registry = ToolRegistry::new();
        registry.set_permissions(permissions);
        registry
    }

    /// 插件启停必须同时收放 default 角色组的授权，否则插件工具永远到不了 LLM。
    #[test]
    fn plugin_tool_permission_follows_plugin_state() {
        let dir = tempfile::tempdir().unwrap();
        let role = "测试角色";
        let tool = "test_tool";
        let registry = registry_with_role(dir.path(), role);

        let allowed = registry.allowed_tools(GeneratorSource::UserChat, Some(role));
        assert!(!allowed.contains(tool), "没授权的工具不该下发");

        grant_tools_to_default_group(&registry, &[tool.to_string()]);
        let allowed = registry.allowed_tools(GeneratorSource::UserChat, Some(role));
        assert!(allowed.contains(tool), "插件工具未授权给 default 角色组");

        revoke_tools_from_default_group(&registry, &[tool.to_string()]);
        let allowed = registry.allowed_tools(GeneratorSource::UserChat, Some(role));
        assert!(!allowed.contains(tool), "插件注销后仍留着授权");
    }

    /// 收回授权时不能把不存在的 default 组建出来：空工具集会被当成放行全部工具。
    #[test]
    fn revoke_does_not_create_default_group() {
        let dir = tempfile::tempdir().unwrap();
        let mut permissions =
            ToolPermissionConfig::load_or_create(dir.path(), std::iter::empty::<String>()).unwrap();
        permissions.role_groups.clear();
        let registry = ToolRegistry::new();
        registry.set_permissions(permissions);

        revoke_tools_from_default_group(&registry, &["test_tool".to_string()]);
        assert!(
            !registry
                .permissions()
                .role_groups
                .contains_key(DEFAULT_ROLE_GROUP)
        );
    }
}
