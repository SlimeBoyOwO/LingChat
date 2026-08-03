use async_trait::async_trait;
use serde_json::{json, Value};

use crate::ai_service::game_system::scene_store::SceneStore;
use crate::ai_service::types::ToolDefinition;
use crate::api::data_dir;

use super::executor::{Tool, ToolContext, ToolError, ToolResult};
use super::{ensure_no_args, game_status_handle};

/// status_get_current：查询当前角色的运行时状态快照。
pub struct CurrentStatus;

#[async_trait]
impl Tool for CurrentStatus {
    fn definition(&self) -> ToolDefinition {
        ToolDefinition::new(
            "status_get_current",
            "查询当前角色的运行时状态：玩家名、当前角色、在场/舞台角色、背景、立绘、音乐、特效、当前场景、全局变量等",
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
        ensure_no_args(&arguments, "status_get_current").map_err(ToolError::Execution)?;
        let app = context.require_app()?;
        let gs = game_status_handle(&app).await;
        let gs = gs.lock().await;
        Ok(json!({
            "player": gs.player.user_name,
            "current_role_id": gs.current_role_id,
            "onstage_role_ids": gs.onstage_role_ids,
            "present_role_ids": gs.present_role_ids.iter().collect::<Vec<_>>(),
            "main_role_id": gs.main_role_id,
            "background": gs.background,
            "present_pic": gs.present_pic,
            "background_music": gs.background_music,
            "background_effect": gs.background_effect,
            "current_scene_id": gs.current_scene_id,
            "scene_awareness_enabled": gs.scene_awareness_enabled,
            "global_variables": gs.global_variables,
        }))
    }
}

/// status_get_scene：查询当前场景的描述与背景。
pub struct SceneStatus;

#[async_trait]
impl Tool for SceneStatus {
    fn definition(&self) -> ToolDefinition {
        ToolDefinition::new(
            "status_get_scene",
            "查询当前场景的描述、背景与场景 ID",
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
        ensure_no_args(&arguments, "status_get_scene").map_err(ToolError::Execution)?;
        let app = context.require_app()?;
        let gs = game_status_handle(&app).await;
        let gs = gs.lock().await;
        let Some(id) = gs.current_scene_id.clone() else {
            return Err(ToolError::Execution("当前未选择任何场景".into()));
        };
        let store = SceneStore::new(&data_dir());
        match store.find_by_id(&id) {
            Ok(Some(scene)) => Ok(json!({
                "current_scene_id": id,
                "name": scene.name,
                "description": scene.description,
                "background": scene.background,
            })),
            Ok(None) => Err(ToolError::Execution(format!("当前场景 {id} 不存在"))),
            Err(e) => Err(ToolError::Execution(format!("读取场景失败: {e}"))),
        }
    }
}
