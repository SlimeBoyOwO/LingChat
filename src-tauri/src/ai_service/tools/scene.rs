use async_trait::async_trait;
use serde_json::{json, Value};
use tauri::Emitter;
use tauri_plugin_store::StoreExt;

use crate::ai_service::game_system::scene_store::SceneStore;
use crate::ai_service::types::ToolDefinition;
use crate::api::data_dir;

use super::executor::{Tool, ToolContext, ToolError, ToolResult};
use super::{ensure_no_args, game_status_handle};

/// scene_list：列出所有可用场景。
pub struct SceneList;

#[async_trait]
impl Tool for SceneList {
    fn definition(&self) -> ToolDefinition {
        ToolDefinition::new(
            "scene_list",
            "列出所有可用场景的 ID、名称、描述与背景",
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
        ensure_no_args(&arguments, "scene_list").map_err(ToolError::Execution)?;
        let store = SceneStore::new(&data_dir());
        let scenes = store
            .load_all()
            .map_err(|e| ToolError::Execution(format!("加载场景失败: {e}")))?;
        Ok(json!(scenes
            .iter()
            .map(|s| json!({
                "id": s.id,
                "name": s.name,
                "description": s.description,
                "background": s.background,
            }))
            .collect::<Vec<_>>()))
    }
}

/// scene_switch：切换到指定场景（按 id 或 name）。
pub struct SceneSwitch;

#[async_trait]
impl Tool for SceneSwitch {
    fn definition(&self) -> ToolDefinition {
        ToolDefinition::new(
            "scene_switch",
            "切换到指定场景，可按场景 ID 或场景名称指定",
            json!({
                "type": "object",
                "properties": {
                    "id": {"type": "string", "description": "场景 ID"},
                    "name": {"type": "string", "description": "场景名称"}
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
                "scene_switch 参数必须是 JSON object".into(),
            ));
        };
        let id = obj.get("id").and_then(Value::as_str).map(str::to_string);
        let name = obj.get("name").and_then(Value::as_str).map(str::to_string);
        if id.is_none() && name.is_none() {
            return Err(ToolError::InvalidArguments(
                "scene_switch 需要提供 id 或 name".into(),
            ));
        }

        let store = SceneStore::new(&data_dir());
        let scenes = store
            .load_all()
            .map_err(|e| ToolError::Execution(format!("加载场景失败: {e}")))?;
        let scene = match (&id, &name) {
            (Some(i), _) => scenes.iter().find(|s| &s.id == i),
            (_, Some(n)) => scenes.iter().find(|s| &s.name == n),
            _ => None,
        };
        let Some(scene) = scene.cloned() else {
            let what = id.or(name).unwrap_or_default();
            return Err(ToolError::Execution(format!("未找到场景: {what}")));
        };
        let scene_id = scene.id.clone();

        let app = context.require_app()?;
        let gs = game_status_handle(&app).await;
        let mut gs = gs.lock().await;
        gs.current_scene_id = Some(scene_id.clone());

        // 持久化到 store，便于下次启动恢复（与 api/scene.rs select_scene 一致）
        if let Ok(store) = app.store(crate::config::STORE_FILE) {
            store.set(
                crate::config::session::LAST_SCENE_ID.to_string(),
                serde_json::Value::String(scene_id.clone()),
            );
            let _ = store.save();
        }
        drop(gs);

        // select_scene 命令由前端自己更新 Pinia；LLM 工具没有这个调用方，必须主动
        // 广播完整场景资料，否则后端 ID 已变化但画面/背景仍停留在旧场景。
        let background = crate::api::scene::normalize_background(&scene.background);
        let payload = json!({
            "type": "scene_switch",
            "scene": {
                "id": scene.id,
                "scene_name": scene.name,
                "scene_description": scene.description,
                "background": if background.is_empty() { Value::Null } else { json!(background) },
                "lighting": scene.lighting,
                "created_at": scene.created_at,
                "updated_at": scene.updated_at,
            }
        });
        if let Err(e) = app.emit("scene:switch", &payload) {
            tracing::warn!("emit scene:switch 失败: {e}");
        }

        Ok(json!({"ok": true, "scene_id": scene_id}))
    }
}
