use std::collections::HashSet;

use async_trait::async_trait;
use serde_json::Value;
use thiserror::Error;

use crate::ai_service::types::ToolDefinition;

use super::registry::ToolRegistry;

/// 单次工具调用的只读运行上下文。
#[derive(Clone, Debug, Default)]
pub struct ToolContext {
    pub allowed_tools: HashSet<String>,
}

impl ToolContext {
    pub fn new(allowed_tools: HashSet<String>) -> Self {
        Self { allowed_tools }
    }

    pub fn allows(&self, name: &str) -> bool {
        self.allowed_tools.contains(name)
    }
}

/// 工具成功执行后返回的 JSON 数据。
pub type ToolResult = Value;

/// 工具定义或执行失败。
#[derive(Debug, Error)]
pub enum ToolError {
    #[error("工具参数无效: {0}")]
    InvalidArguments(String),
    #[error("工具执行失败: {0}")]
    Execution(String),
}

/// 可注册并执行的聊天工具。
#[async_trait]
pub trait Tool: Send + Sync {
    /// 返回提供给 LLM 的工具定义。
    fn definition(&self) -> ToolDefinition;

    /// 使用解析后的 JSON object 参数执行工具。
    async fn execute(
        &self,
        context: &ToolContext,
        arguments: Value,
    ) -> Result<ToolResult, ToolError>;
}

/// 统一查找、解析、超时并封装工具执行结果。
pub struct ToolExecutor<'a> {
    registry: &'a ToolRegistry,
    timeout: std::time::Duration,
}

impl<'a> ToolExecutor<'a> {
    /// 使用默认两秒超时创建执行器。
    pub fn new(registry: &'a ToolRegistry) -> Self {
        Self {
            registry,
            timeout: std::time::Duration::from_secs(2),
        }
    }

    /// 执行指定工具，并将可恢复错误编码为稳定 JSON。
    pub async fn execute(&self, name: &str, arguments: &str, context: &ToolContext) -> String {
        if !context.allows(name) {
            return error_result("tool_not_allowed", format!("当前调用上下文不允许工具: {name}"));
        }

        let Some(tool) = self.registry.get(name) else {
            return error_result("unknown_tool", format!("未知工具: {name}"));
        };

        let arguments = match serde_json::from_str::<Value>(arguments) {
            Ok(Value::Object(values)) => Value::Object(values),
            Ok(_) => return error_result("invalid_arguments", "工具参数必须是 JSON object"),
            Err(error) => {
                tracing::warn!(tool = name, "工具参数 JSON 解析失败: {error}");
                return error_result("invalid_json", format!("工具参数不是合法 JSON: {error}"));
            }
        };

        match tokio::time::timeout(self.timeout, tool.execute(context, arguments)).await {
            Ok(Ok(result)) => serde_json::to_string(&result).unwrap_or_else(|error| {
                tracing::error!(tool = name, "工具结果序列化失败: {error}");
                error_result("serialization_error", "工具结果无法序列化")
            }),
            Ok(Err(error)) => {
                tracing::warn!(tool = name, "工具执行失败: {error}");
                error_result("tool_error", error.to_string())
            }
            Err(_) => {
                tracing::warn!(tool = name, "工具执行超时");
                error_result("timeout", "工具执行超过 2 秒")
            }
        }
    }

    #[cfg(test)]
    fn with_timeout(registry: &'a ToolRegistry, timeout: std::time::Duration) -> Self {
        Self { registry, timeout }
    }
}

/// 构造稳定的工具错误 JSON。
fn error_result(code: &str, message: impl Into<String>) -> String {
    serde_json::json!({
        "ok": false,
        "error": {
            "code": code,
            "message": message.into(),
        }
    })
    .to_string()
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;

    use super::*;
    use crate::ai_service::tools::registry::ToolRegistry;

    fn test_context() -> ToolContext {
        ToolContext::new(
            ["echo", "error", "slow", "missing"]
                .iter()
                .map(|s| s.to_string())
                .collect(),
        )
    }

    struct EchoTool;

    #[async_trait]
    impl Tool for EchoTool {
        fn definition(&self) -> ToolDefinition {
            ToolDefinition::new("echo", "回显", serde_json::json!({"type": "object"}))
        }

        async fn execute(
            &self,
            _: &ToolContext,
            arguments: Value,
        ) -> Result<ToolResult, ToolError> {
            Ok(arguments)
        }
    }

    struct ErrorTool;

    #[async_trait]
    impl Tool for ErrorTool {
        fn definition(&self) -> ToolDefinition {
            ToolDefinition::new("error", "失败", serde_json::json!({"type": "object"}))
        }

        async fn execute(&self, _: &ToolContext, _: Value) -> Result<ToolResult, ToolError> {
            Err(ToolError::Execution("预期失败".to_string()))
        }
    }

    struct SlowTool;

    #[async_trait]
    impl Tool for SlowTool {
        fn definition(&self) -> ToolDefinition {
            ToolDefinition::new("slow", "慢工具", serde_json::json!({"type": "object"}))
        }

        async fn execute(&self, _: &ToolContext, _: Value) -> Result<ToolResult, ToolError> {
            tokio::time::sleep(std::time::Duration::from_millis(50)).await;
            Ok(serde_json::json!({"done": true}))
        }
    }

    /// 验证执行器可执行合法工具并稳定返回错误。
    #[tokio::test]
    async fn executes_and_encodes_recoverable_errors() {
        let mut registry = ToolRegistry::new();
        registry.register(Arc::new(EchoTool)).unwrap();
        let executor = ToolExecutor::new(&registry);
        let ctx = test_context();

        assert_eq!(executor.execute("echo", "{}", &ctx).await, "{}");
        assert!(executor
            .execute("missing", "{}", &ctx)
            .await
            .contains("unknown_tool"));
        assert!(executor
            .execute("echo", "[", &ctx)
            .await
            .contains("invalid_json"));
        assert!(executor
            .execute("echo", "[]", &ctx)
            .await
            .contains("invalid_arguments"));
    }

    /// 验证工具主动失败会被编码为可回填结果。
    #[tokio::test]
    async fn encodes_tool_errors() {
        let mut registry = ToolRegistry::new();
        registry.register(Arc::new(ErrorTool)).unwrap();
        let executor = ToolExecutor::new(&registry);
        let ctx = test_context();

        let result = executor.execute("error", "{}", &ctx).await;
        assert!(result.contains("tool_error"));
        assert!(result.contains("预期失败"));
    }

    /// 验证超过执行期限的工具会返回超时结果。
    #[tokio::test]
    async fn encodes_timeouts() {
        let mut registry = ToolRegistry::new();
        registry.register(Arc::new(SlowTool)).unwrap();
        let executor = ToolExecutor::with_timeout(&registry, std::time::Duration::from_millis(1));
        let ctx = test_context();

        let result = executor.execute("slow", "{}", &ctx).await;
        assert!(result.contains("timeout"));
    }
}

