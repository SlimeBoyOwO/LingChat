use async_trait::async_trait;
use serde_json::{Value, json};

use crate::ai_service::game_system::scene_store::SceneStore;
use crate::ai_service::types::ToolDefinition;
use crate::api::data_dir;

use crate::ai_service::tools::executor::{Tool, ToolContext, ToolError, ToolResult};
use crate::ai_service::tools::{ensure_no_args, game_status_handle};

/// status_get_current：查询当前角色的运行时状态快照。
pub struct CurrentStatus;

#[async_trait]
impl Tool for CurrentStatus {
    fn definition(&self) -> ToolDefinition {
        ToolDefinition::new(
            "status_get_current",
            "查询当前角色的运行时状态：玩家名、当前角色、在场/舞台角色、背景、立绘、音乐、氛围特效、天气特效、当前场景、全局变量等",
            json!({
                "type": "object",
                "properties": {},
                "required": [],
                "additionalProperties": false
            }),
        )
        .with_rag_examples([
            "你现在是什么状态",
            "你观察一下周围的环境",
            "现在背景是什么",
            "现在放的是什么音乐",
        ])
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
        let mut present_role_ids: Vec<i32> = gs.present_role_ids.iter().copied().collect();
        present_role_ids.sort_unstable();
        Ok(json!({
            "ok": true,
            "player": gs.player.user_name,
            "current_role_id": gs.current_role_id,
            "onstage_role_ids": gs.onstage_role_ids,
            "present_role_ids": present_role_ids,
            "main_role_id": gs.main_role_id,
            "background": gs.background,
            "present_pic": gs.present_pic,
            "background_music": gs.background_music,
            "background_effect": gs.background_effect,
            "weather_effect": gs.weather_effect,
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
        .with_rag_examples([
            "我们现在在哪",
            "这是什么地方",
            "周围是什么样子",
            "现在是什么场景",
        ])
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
                "ok": true,
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
