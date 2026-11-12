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
        let novelai = self.0.get().image_generation.provider == "novelai";
        let description = if novelai {
            "使用当前 NovelAI 生图模型生成图片并加入背景/场景库。prompt 主要使用英文逗号分隔的正向 tags，按主体、外观、衣着、构图、背景和风格描述，重要内容靠前；可用 negative_prompt 提供不希望出现的负向 tags（Undesired Content）。保留用户给出的 tags 与权重语法。不要把说明性长文直接当作 tags。不能在参数中改写凭据、端点或模型。仅当用户要求切换背景时设置 apply_background。"
        } else {
            "按用户要求生成图片并加入背景/场景库。使用用户在生图设置中选择的供应商和模型，不能在参数中改写凭据、端点或模型。仅当用户希望切换到生成的场景时设置 apply_background。"
        };
        let mut parameters = json!({
            "type": "object",
            "properties": {
                "prompt": {"type": "string", "description": if novelai { "英文逗号分隔的正向 tags；主体和重要特征靠前，可保留权重语法" } else { "要生成的图片的详细描述" }},
                "name": {"type": "string", "description": "图片/场景的显示名称"},
                "apply_background": {"type": "boolean", "description": "是否把当前背景切换到生成的图片，默认 false"}
            },
            "required": ["prompt"],
            "additionalProperties": false
        });
        if novelai {
            parameters["properties"]["negative_prompt"] = json!({"type":"string", "description":"英文逗号分隔的负向 tags（Undesired Content）；仅覆盖本次调用。省略则使用配置中的负向 tags，空字符串表示本次清空。"});
        }
        ToolDefinition::new("generate_image", description, parameters).with_rag_examples([
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ai_service::tools::settings::ToolSettings;

    #[test]
    fn novelai_tool_schema_tracks_provider_changes() {
        let settings = SharedToolSettings::new(ToolSettings::default());
        let tool = GenerateImage(settings.clone());
        assert!(
            tool.definition().function.parameters["properties"]
                .get("negative_prompt")
                .is_none()
        );
        let mut next = settings.get();
        next.image_generation.provider = "novelai".into();
        settings.update(next);
        let definition = tool.definition();
        assert!(
            definition.function.parameters["properties"]
                .get("negative_prompt")
                .is_some()
        );
        assert!(definition.function.description.contains("tags"));
        let mut next = settings.get();
        next.image_generation.provider = "openai-compatible".into();
        settings.update(next);
        assert!(
            tool.definition().function.parameters["properties"]
                .get("negative_prompt")
                .is_none()
        );
    }
}
