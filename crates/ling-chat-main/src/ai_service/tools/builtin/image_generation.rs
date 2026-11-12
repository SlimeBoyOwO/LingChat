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
        ToolDefinition::new(
            "generate_image",
            "按用户要求生成图片并加入背景/场景库。使用用户在生图设置中选择的供应商和模型，不能在参数中改写凭据、端点或模型。仅当用户希望切换到生成的场景时设置 apply_background。",
            json!({
                "type": "object",
                "properties": {
                    "prompt": {"type": "string", "description": "要生成的图片的详细描述"},
                    "name": {"type": "string", "description": "图片/场景的显示名称"},
                    "apply_background": {"type": "boolean", "description": "是否把当前背景切换到生成的图片，默认 false"}
                },
                "required": ["prompt"],
                "additionalProperties": false
            }),
        ).with_rag_examples(["帮我画一张图", "生成一张背景", "画一个新的场景"])
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
