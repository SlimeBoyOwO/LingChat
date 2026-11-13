use async_trait::async_trait;
use serde_json::{Value, json};

use crate::ai_service::game_system::scene_store::SceneStore;
use crate::ai_service::types::ToolDefinition;
use crate::api::data_dir;

use crate::ai_service::tools::ensure_no_args;
use crate::ai_service::tools::executor::{Tool, ToolContext, ToolError, ToolResult};

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
        .with_rag_examples([
            "都有哪些能去地方",
            "这里能去什么地方",
            "列出所有地点",
            "有什么场景可以选",
        ])
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
                 找不到明确对应的场景就不要切换。"
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
        .with_rag_examples([
            "我们去别的地方吧",
            "要去别的地方看看吗",
            "走一起去公园吧！",
            "带我去你说的地方",
        ])
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

        // 状态写入、持久化与广播统一走世界动作层（与手动选场景同一实现）
        crate::ai_service::game_system::world_actions::switch_scene(&app, Some(scene_id.clone()))
            .await;

        Ok(json!({"ok": true, "scene_id": scene_id}))
    }
}
