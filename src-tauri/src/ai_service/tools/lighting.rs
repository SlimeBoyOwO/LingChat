//! 光影工具：让 LLM 与插件能改舞台灯光。
//!
//! 插件沙箱只能 `call_tool`，摸不到渲染层，所以「光影插件」的实际控光能力
//! 必须由这三个内置工具提供；插件负责在上面叠语义（心情 → 预设的映射、
//! 一键氛围、按剧情推荐）。
//!
//! 写入统一走 [`crate::api::lighting::apply_lighting`]，与设置面板、剧本事件
//! 共用同一条广播，避免三份状态各说各话。

use async_trait::async_trait;
use serde_json::{Value, json};
use tauri::AppHandle;

use crate::ai_service::game_system::lighting_store;
use crate::ai_service::game_system::scene_store::LightingOverride;
use crate::ai_service::types::ToolDefinition;

use super::executor::{Tool, ToolContext, ToolError, ToolResult};
use super::{ensure_no_args, game_status_handle};

/// lighting_list_presets：列出可用光影预设。
pub struct LightingListPresets;

#[async_trait]
impl Tool for LightingListPresets {
    fn definition(&self) -> ToolDefinition {
        ToolDefinition::new(
            "lighting_list_presets",
            "列出所有光影预设（id、名称、说明、适用心情关键词）。切换灯光前先调用它确认 id。",
            json!({
                "type": "object",
                "properties": {},
                "required": [],
                "additionalProperties": false
            }),
        )
    }

    async fn execute(
        &self,
        _context: &ToolContext,
        arguments: Value,
    ) -> Result<ToolResult, ToolError> {
        ensure_no_args(&arguments, "lighting_list_presets").map_err(ToolError::Execution)?;
        Ok(json!(lighting_store::summaries()))
    }
}

/// lighting_apply：切换舞台光影，或清回「跟随场景」。
pub struct LightingApply;

#[async_trait]
impl Tool for LightingApply {
    fn definition(&self) -> ToolDefinition {
        ToolDefinition::new(
            "lighting_apply",
            "切换舞台光影氛围（暖窗光 / 冷月夜 / 逆光剪影等）。传 preset 应用预设；\
             传 clear=true 回到「跟随场景」的默认灯光。只影响画面光照，不改背景图。",
            json!({
                "type": "object",
                "properties": {
                    "preset": {"type": "string", "description": "预设 id，见 lighting_list_presets"},
                    "clear": {"type": "boolean", "description": "true = 清除覆盖，回到跟随场景"}
                },
                "required": [],
                "additionalProperties": false
            }),
        )
    }

    async fn execute(
        &self,
        context: &ToolContext,
        arguments: Value,
    ) -> Result<ToolResult, ToolError> {
        let Some(obj) = arguments.as_object() else {
            return Err(ToolError::InvalidArguments(
                "lighting_apply 参数必须是 JSON object".into(),
            ));
        };
        let clear = obj.get("clear").and_then(Value::as_bool).unwrap_or(false);
        let preset = obj
            .get("preset")
            .and_then(Value::as_str)
            .map(str::trim)
            .filter(|s| !s.is_empty() && *s != "follow_scene")
            .map(str::to_string);

        if !clear && preset.is_none() {
            return Err(ToolError::InvalidArguments(
                "lighting_apply 需要 preset，或 clear=true".into(),
            ));
        }

        let app: AppHandle = context.require_app()?;
        let override_ = if clear {
            None
        } else {
            Some(LightingOverride::from_preset(&preset.unwrap_or_default()))
        };

        let payload = crate::api::lighting::apply_lighting(&app, override_, "tool")
            .await
            .map_err(ToolError::Execution)?;

        Ok(json!({
            "ok": true,
            "preset": payload.preset,
            "cleared": payload.params.is_none(),
        }))
    }
}

/// lighting_get：读回当前光影状态。
pub struct LightingGet;

#[async_trait]
impl Tool for LightingGet {
    fn definition(&self) -> ToolDefinition {
        ToolDefinition::new(
            "lighting_get",
            "查询当前舞台光影。以 active_* 为准判断屏幕上正在打的灯（含用户手动选的预设与跟随场景的默认灯）；\
             override_* 只是剧本/工具留下的运行时覆盖记录，单独看它会漏掉用户手动设定的灯光。",
            json!({
                "type": "object",
                "properties": {},
                "required": [],
                "additionalProperties": false
            }),
        )
    }

    async fn execute(
        &self,
        context: &ToolContext,
        arguments: Value,
    ) -> Result<ToolResult, ToolError> {
        ensure_no_args(&arguments, "lighting_get").map_err(ToolError::Execution)?;
        let app = context.require_app()?;
        let gs = game_status_handle(&app).await;
        let gs = gs.lock().await;
        let active_preset = gs.lighting_active_preset.clone();
        let active_name = active_preset
            .as_deref()
            .and_then(lighting_store::preset_name);
        Ok(json!({
            // 屏幕上真正在渲染的灯，前端算好后回传
            "active_preset": active_preset,
            "active_preset_name": active_name,
            "active_source": gs.lighting_active_source,
            // 后端侧的运行时覆盖记录，不含设置面板的全局预设
            "override_preset": gs.lighting_override.as_ref().and_then(|o| o.preset.clone()),
            "override_source": gs.lighting_override_source,
            "current_scene_id": gs.current_scene_id,
        }))
    }
}
