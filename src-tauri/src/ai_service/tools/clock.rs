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
