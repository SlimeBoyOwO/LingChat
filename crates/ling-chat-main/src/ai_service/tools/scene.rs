use async_trait::async_trait;
use serde_json::{Value, json};
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
        Ok(json!({
            "ok": true,
            "scenes": scenes
                .iter()
                .map(|s| json!({
                    "id": s.id,
                    "name": s.name,
                    "description": s.description,
                    "background": s.background,
                }))
                .collect::<Vec<_>>()
        }))
    }
}

/// scene_switch：切换到指定场景（按 id 或 name）。
pub struct SceneSwitch;

#[async_trait]
impl Tool for SceneSwitch {
    fn definition(&self) -> ToolDefinition {
        // 把当前可用场景名直接写进描述，模型无需先调 scene_list 就知道有哪些
        // 场景——这是"用户提到地点 → 后台匹配切景"主动行为的前提。
        let scene_names = SceneStore::new(&data_dir())
            .load_all()
            .map(|scenes| {
                scenes
                    .iter()
                    .map(|s| s.name.as_str())
                    .collect::<Vec<_>>()
                    .join("、")
            })
            .unwrap_or_default();
        ToolDefinition::new(
            "scene_switch",
            format!(
                "切换到指定场景。当前可用场景：{scene_names}。\
                 当用户提到自己所在的地点（如「我现在在占卜摊」）、或时间氛围发生变化\
                 （入夜、清晨、下雨）时，可主动切换到名称最匹配的场景，无需用户指示；\
                 找不到明确对应的场景就不要切换。若工具返回 locked（用户手动锁定过场景），\
                 任何调用都不会执行——不要再尝试，可提示用户在设置中重新打开自动切换"
            ),
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

        let app = context.require_app()?;

        // 手动优先（硬边界）：用户手动选过场景后，AI 的切景请求一律不执行；
        // 恢复自动切景的唯一途径是用户在设置页重新打开开关
        let auto_allowed = crate::config::AppConfig::load(&app)
            .map(|c| c.ai_auto_scene)
            .unwrap_or(true);
        if !auto_allowed {
            return Ok(json!({
                "success": false,
                "locked": true,
                "message": "用户已手动选择当前场景并关闭了自动切换，本次及后续的切景调用都不会执行。请自然接受，不要反复尝试；若用户想恢复自动切景，可提示其在设置中重新打开",
            }));
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
        let background = crate::api::scene::resolved_background_of(&scene);

        let gs = game_status_handle(&app).await;
        let mut gs = gs.lock().await;
        gs.current_scene_id = Some(scene_id.clone());
        gs.clear_background_override();
        gs.sync_background_with_scene(background.clone());

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
        let payload = json!({
            "type": "scene_switch",
            "scene": {
                "id": scene.id,
                "scene_name": scene.name,
                "scene_description": scene.description,
                "background": background.map_or(Value::Null, |bg| json!(bg)),
                "lighting": scene.lighting,
                "location_type": scene.location_type.map(|l| l.as_str()),
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
