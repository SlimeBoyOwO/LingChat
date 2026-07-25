use async_trait::async_trait;
use chrono::Local;
use serde_json::Value;

use crate::ai_service::types::ToolDefinition;

use super::executor::{Tool, ToolContext, ToolError, ToolResult};

/// 查询运行设备本地时间的内置工具。
pub struct CurrentTimeTool;

#[async_trait]
impl Tool for CurrentTimeTool {
    /// 返回无参数的时间查询工具定义。
    fn definition(&self) -> ToolDefinition {
        ToolDefinition::new(
            "get_current_time",
            "查询当前设备的本地日期和时间",
            serde_json::json!({
                "type": "object",
                "properties": {},
                "required": [],
                "additionalProperties": false
            }),
        )
    }

    /// 返回设备本地 RFC3339 时间与 Unix 秒级时间戳。
    async fn execute(&self, _: &ToolContext, arguments: Value) -> Result<ToolResult, ToolError> {
        let Some(arguments) = arguments.as_object() else {
            return Err(ToolError::InvalidArguments("参数必须是 JSON object".into()));
        };
        if !arguments.is_empty() {
            return Err(ToolError::InvalidArguments(
                "get_current_time 不接受参数".into(),
            ));
        }

        let now = Local::now();
        Ok(serde_json::json!({
            "local_time": now.to_rfc3339(),
            "timezone": "local",
            "unix_timestamp": now.timestamp(),
        }))
    }
}

#[cfg(test)]
mod tests {
    use chrono::DateTime;

    use super::*;

    /// 验证工具定义严格禁止额外参数。
    #[test]
    fn exposes_empty_object_schema() {
        let definition = CurrentTimeTool.definition();
        assert_eq!(definition.function.name, "get_current_time");
        assert_eq!(
            definition.function.parameters["additionalProperties"],
            false
        );
    }

    /// 验证时间结果字段完整且可解析。
    #[tokio::test]
    async fn returns_parseable_local_time() {
        let before = Local::now().timestamp();
        let result = CurrentTimeTool
            .execute(&ToolContext::default(), serde_json::json!({}))
            .await
            .unwrap();
        let after = Local::now().timestamp();
        DateTime::parse_from_rfc3339(result["local_time"].as_str().unwrap()).unwrap();
        let timestamp = result["unix_timestamp"].as_i64().unwrap();
        assert!((before..=after).contains(&timestamp));
        assert_eq!(result["timezone"], "local");
        assert!(CurrentTimeTool
            .execute(&ToolContext::default(), serde_json::json!({"x": 1}))
            .await
            .is_err());
    }
}

