use crate::ai_service::image_generation::{self, GenerateImageRequest};
use crate::ai_service::tools::executor::{Tool, ToolContext, ToolError, ToolResult};
use crate::ai_service::tools::settings::SharedToolSettings;
use crate::ai_service::types::ToolDefinition;
use async_trait::async_trait;
use serde_json::{Value, json};
use std::time::Duration;

pub struct GenerateImage(pub SharedToolSettings);

#[async_trait]
impl Tool for GenerateImage {
    fn definition(&self) -> ToolDefinition {
        let hints = image_generation::prompt_hints(&self.0.get().image_generation);
        let mut parameters = json!({
            "type": "object",
            "properties": {
                "prompt": {"type": "string", "description": hints.prompt},
                "name": {"type": "string", "description": "图片/场景的显示名称"},
                "apply_background": {"type": "boolean", "description": "是否把当前背景切换到生成的图片，默认 false"}
            },
            "required": ["prompt"],
            "additionalProperties": false
        });
        if let Some(description) = hints.negative_prompt {
            parameters["properties"]["negative_prompt"] =
                json!({"type":"string", "description":description});
        }
        ToolDefinition::new("generate_image", hints.description, parameters).with_rag_examples([
            "帮我画一张图",
            "生成一张背景",
            "画一个新的场景",
        ])
    }

    fn timeout_hint(&self) -> Option<Duration> {
        Some(Duration::from_secs(
            self.0.get().image_generation.timeout_secs.clamp(30, 900) + 60,
        ))
    }

    async fn execute(
        &self,
        context: &ToolContext,
        arguments: Value,
    ) -> Result<ToolResult, ToolError> {
        let request: GenerateImageRequest = serde_json::from_value(arguments)
            .map_err(|e| ToolError::InvalidArguments(e.to_string()))?;
        let result = image_generation::generate(&context.require_app()?, request)
            .await
            .map_err(|e| ToolError::Execution(e.to_string()))?;
        serde_json::to_value(result).map_err(|e| ToolError::Execution(e.to_string()))
    }
}
