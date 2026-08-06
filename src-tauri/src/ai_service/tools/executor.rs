use std::collections::HashSet;
use std::time::Duration;

use async_trait::async_trait;
use serde_json::Value;
use tauri::AppHandle;
use thiserror::Error;

use crate::ai_service::types::ToolDefinition;

use super::registry::ToolRegistry;

/// 单次工具调用的只读运行上下文。
#[derive(Clone, Debug, Default)]
pub struct ToolContext {
    pub allowed_tools: HashSet<String>,
    /// 用于访问 AppState 共享状态的句柄；测试等无宿主环境时为 `None`。
    pub app: Option<AppHandle>,
}

impl ToolContext {
    pub fn new(allowed_tools: HashSet<String>) -> Self {
        Self {
            allowed_tools,
            app: None,
        }
    }

    /// 绑定 AppHandle，供工具访问 db / ai_service / game_status 等共享状态。
    pub fn with_app(mut self, app: AppHandle) -> Self {
        self.app = Some(app);
        self
    }

    pub fn allows(&self, name: &str) -> bool {
        self.allowed_tools.contains(name)
    }

    /// 取 AppHandle，用于访问 AppState 共享状态。无宿主环境（单元测试）时报错。
    pub fn require_app(&self) -> Result<AppHandle, ToolError> {
        self.app
            .clone()
            .ok_or_else(|| ToolError::Execution("当前调用上下文没有宿主环境（AppHandle）".into()))
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

    /// 自定义执行超时；返回 `None` 时使用执行器默认超时（2 秒）。
    fn timeout_hint(&self) -> Option<Duration> {
        None
    }

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
            return error_result(
                "tool_not_allowed",
                format!("当前调用上下文不允许工具: {name}"),
            );
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
        let definition = tool.definition();
        if let Err(error) = validate_value(name, &definition.function.parameters, &arguments) {
            return error_result("invalid_arguments", error);
        }

        let timeout = tool.timeout_hint().unwrap_or(self.timeout);
        match tokio::time::timeout(timeout, tool.execute(context, arguments)).await {
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
                error_result("timeout", "工具执行超时")
            }
        }
    }

    #[cfg(test)]
    fn with_timeout(registry: &'a ToolRegistry, timeout: std::time::Duration) -> Self {
        Self { registry, timeout }
    }
}

/// 校验当前工具定义使用到的 JSON Schema 子集。所有内置工具只依赖 object、
/// string、integer、number、boolean、array/items、required 与
/// additionalProperties；在执行器统一校验，避免每个工具各自静默忽略类型错误。
fn validate_value(path: &str, schema: &Value, value: &Value) -> Result<(), String> {
    let Some(expected) = schema.get("type").and_then(Value::as_str) else {
        return Ok(());
    };
    match expected {
        "object" => {
            let object = value
                .as_object()
                .ok_or_else(|| format!("{path} 必须是 object"))?;
            let properties = schema
                .get("properties")
                .and_then(Value::as_object)
                .cloned()
                .unwrap_or_default();
            if let Some(required) = schema.get("required").and_then(Value::as_array) {
                for key in required.iter().filter_map(Value::as_str) {
                    if !object.contains_key(key) {
                        return Err(format!("{path} 缺少必填字段 {key}"));
                    }
                }
            }
            if schema.get("additionalProperties").and_then(Value::as_bool) == Some(false) {
                if let Some(key) = object.keys().find(|key| !properties.contains_key(*key)) {
                    return Err(format!("{path} 包含未知字段 {key}"));
                }
            }
            for (key, child) in object {
                if let Some(child_schema) = properties.get(key) {
                    validate_value(&format!("{path}.{key}"), child_schema, child)?;
                }
            }
        }
        "array" => {
            let array = value
                .as_array()
                .ok_or_else(|| format!("{path} 必须是 array"))?;
            if let Some(item_schema) = schema.get("items") {
                for (index, item) in array.iter().enumerate() {
                    validate_value(&format!("{path}[{index}]"), item_schema, item)?;
                }
            }
        }
        "string" if !value.is_string() => return Err(format!("{path} 必须是 string")),
        "integer"
            if !value
                .as_number()
                .is_some_and(|number| number.is_i64() || number.is_u64()) =>
        {
            return Err(format!("{path} 必须是 integer"));
        }
        "number" if !value.is_number() => return Err(format!("{path} 必须是 number")),
        "boolean" if !value.is_boolean() => return Err(format!("{path} 必须是 boolean")),
        _ => {}
    }
    Ok(())
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
        let registry = ToolRegistry::new();
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
        let registry = ToolRegistry::new();
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
        let registry = ToolRegistry::new();
        registry.register(Arc::new(SlowTool)).unwrap();
        let executor = ToolExecutor::with_timeout(&registry, std::time::Duration::from_millis(1));
        let ctx = test_context();

        let result = executor.execute("slow", "{}", &ctx).await;
        assert!(result.contains("timeout"));
    }
}
