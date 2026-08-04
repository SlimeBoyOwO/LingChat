use async_trait::async_trait;
use serde_json::{json, Value};
use tauri::{Emitter, Manager};

use crate::ai_service::types::ToolDefinition;
use crate::db::managers::role_repo::RoleRepo;
use crate::AppState;

use super::executor::{Tool, ToolContext, ToolError, ToolResult};
use super::{ensure_no_args, game_status_handle};

/// character_list：列出所有可用角色的 ID 与名称。
pub struct CharacterList;

#[async_trait]
impl Tool for CharacterList {
    fn definition(&self) -> ToolDefinition {
        ToolDefinition::new(
            "character_list",
            "列出所有可用角色的 ID 与名称",
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
        ensure_no_args(&arguments, "character_list").map_err(ToolError::Execution)?;
        let app = context.require_app()?;
        let state = app.state::<AppState>();
        let roles = RoleRepo::get_all_main_roles(&state.db)
            .await
            .map_err(|e| ToolError::Execution(format!("查询角色列表失败: {e}")))?;
        Ok(json!(roles
            .iter()
            .map(|r| json!({"id": r.id, "name": r.name}))
            .collect::<Vec<_>>()))
    }
}

/// character_switch：切换当前对话角色。
///
/// 注意：这是**轻量切换**——只更新 `current_role_id`，不清空对话历史、
/// 不重建 TTS、不返回 WebInitData。若需完整切换（含对话重置）请走前端命令
/// `select_character`。
pub struct CharacterSwitch;

#[async_trait]
impl Tool for CharacterSwitch {
    fn definition(&self) -> ToolDefinition {
        ToolDefinition::new(
            "character_switch",
            "切换到指定角色作为当前对话角色（仅切换，不重置对话历史）",
            json!({
                "type": "object",
                "properties": {
                    "id": {"type": "integer", "description": "角色 ID"}
                },
                "required": ["id"],
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
                "character_switch 参数必须是 JSON object".into(),
            ));
        };
        let role_id = obj
            .get("id")
            .and_then(Value::as_i64)
            .ok_or_else(|| ToolError::InvalidArguments("character_switch 需要整数 id".into()))?
            as i32;

        let app = context.require_app()?;
        let state = app.state::<AppState>();

        // 校验角色存在并取出名称（否则静默切到不存在的 id，前端无感知）
        let roles = RoleRepo::get_all_main_roles(&state.db)
            .await
            .map_err(|e| ToolError::Execution(format!("查询角色列表失败: {e}")))?;
        let Some(role) = roles.iter().find(|r| r.id == role_id) else {
            let available: Vec<String> = roles.iter().map(|r| format!("{}={}", r.id, r.name)).collect();
            return Err(ToolError::Execution(format!(
                "角色 id {role_id} 不存在，可用角色: {}",
                available.join(", ")
            )));
        };
        let role_name = role.name.clone();

        let gs = game_status_handle(&app).await;
        let mut gs = gs.lock().await;
        gs.current_role_id = Some(role_id);
        // 预加载新角色到 role_manager（人设/记忆/回复构建都依赖 get_loaded），
        // 否则下一轮回复取不到角色信息，会被前端当作未初始化角色丢弃
        if let Err(e) = gs.get_role(&state.db, role_id).await {
            tracing::warn!("预加载切换后的角色失败: {e}");
        }
        drop(gs);

        // 通知前端当前对话角色已切换（与 God Agent 切换使用同一事件）
        let payload = json!({
            "type": "character_switch",
            "roleId": role_id,
            "characterName": role_name,
        });
        if let Err(e) = app.emit("character:switch", &payload) {
            tracing::warn!("emit character:switch 失败: {e}");
        }

        Ok(json!({"ok": true, "role_id": role_id, "name": role_name}))
    }
}
