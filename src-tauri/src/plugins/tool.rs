//! 插件工具：把 `ToolSpec` 适配为 `ToolRegistry` 可注册的 `Tool`。

use std::time::Duration;

use async_trait::async_trait;
use serde_json::Value;
use tauri::Manager;

use crate::ai_service::tools::executor::{Tool, ToolContext, ToolError, ToolResult};
use crate::ai_service::types::ToolDefinition;
use crate::AppState;

use super::python_backend;
use super::types::ToolSpec;

/// 把一个插件工具包装成可注册的 Tool。
///
/// 执行时经 AppHandle 定位 PluginManager，取该插件当前的 config / env，
/// 在 `spawn_blocking` 线程内跑插件脚本。
pub struct PluginTool {
    /// 工具名（含插件 id 前缀）。
    pub name: String,
    /// 所属插件 id。
    pub plugin_id: String,
    /// 工具 spec（含脚本路径与超时）。
    pub spec: ToolSpec,
}

impl PluginTool {
    pub fn new(plugin_id: String, spec: ToolSpec) -> Self {
        Self {
            name: spec.name.clone(),
            plugin_id,
            spec,
        }
    }

}

#[async_trait]
impl Tool for PluginTool {
    fn definition(&self) -> ToolDefinition {
        let parameters = serde_json::from_str(&self.spec.parameters)
            .unwrap_or_else(|_| serde_json::json!({ "type": "object" }));
        ToolDefinition::new(self.name.clone(), self.spec.description.clone(), parameters)
    }

    fn timeout_hint(&self) -> Option<Duration> {
        Some(Duration::from_millis(self.spec.timeout_ms))
    }

    async fn execute(
        &self,
        context: &ToolContext,
        arguments: Value,
    ) -> Result<ToolResult, ToolError> {
        let app = context.require_app()?;
        let script_rel = self.spec.script.clone();
        let plugin_id = self.plugin_id.clone();
        let name = self.name.clone();

        // 取 config/env、解析脚本路径、跑 Python 都是阻塞操作（PluginManager 内部
        // 用 blocking_lock，RustPython 需要线程局部状态），整体放 spawn_blocking；
        // 外层 timeout_hint 兜底。app 随闭包传入，供脚本内 call_tool 使用。
        let result = tokio::task::spawn_blocking(move || {
            let manager = app.state::<AppState>().data().plugin_manager.clone();
            let (config, env) = manager.plugin_run_env(&plugin_id);
            let script_path = manager
                .plugin_dir(&plugin_id)
                .map(|dir| dir.join(&script_rel))
                .ok_or_else(|| format!("插件 {plugin_id} 目录不存在"))?;
            python_backend::run_plugin_script(&script_path, &name, &arguments, &config, &env, app)
        })
        .await
        .map_err(|join_err| ToolError::Execution(format!("插件线程异常: {join_err}")))?;

        match result {
            Ok(value) => Ok(value),
            Err(e) => Err(ToolError::Execution(e)),
        }
    }
}

/// 供测试使用的轻量插件工具（不依赖 AppHandle）。
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn builds_tool_definition() {
        let spec = ToolSpec {
            name: "tavily_search".into(),
            description: "搜索".into(),
            parameters: "{ \"type\": \"object\" }".into(),
            script: "tavily.py".into(),
            timeout_ms: 30_000,
        };
        let tool = PluginTool::new("tavily".into(), spec);
        let def = tool.definition();
        assert_eq!(def.function.name, "tavily_search");
        assert_eq!(def.type_, "function");
    }
}
