use async_trait::async_trait;
use serde_json::{json, Value};
use tauri::{Emitter, Manager};

use crate::ai_service::types::{LineAttributeExt, LineBase, ToolDefinition};
use crate::config::AppConfig;
use crate::db::entities::line::LineAttribute;
use crate::db::managers::role_repo::RoleRepo;
use crate::utils::prompt::{sys_prompt_builder_by_settings, PromptOptions};
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
/// 这是不清空对话历史的运行时切换，但仍必须完成三件事：加载目标角色、注入其
/// SYSTEM 人设、同步后端在场角色。缺少任一步，下一轮用户消息都会继续落到旧角色
/// 或在没有人设的上下文中生成。
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
        let raw_role_id = obj
            .get("id")
            .and_then(Value::as_i64)
            .ok_or_else(|| ToolError::InvalidArguments("character_switch 需要整数 id".into()))?;
        let role_id = i32::try_from(raw_role_id).map_err(|_| {
            ToolError::InvalidArguments("character_switch 的 id 超出 i32 范围".into())
        })?;

        let app = context.require_app()?;
        let state = app.state::<AppState>();

        // 校验角色存在并取出名称（否则静默切到不存在的 id，前端无感知）
        let roles = RoleRepo::get_all_main_roles(&state.db)
            .await
            .map_err(|e| ToolError::Execution(format!("查询角色列表失败: {e}")))?;
        let Some(role) = roles.iter().find(|r| r.id == role_id) else {
            let available: Vec<String> = roles
                .iter()
                .map(|r| format!("{}={}", r.id, r.name))
                .collect();
            return Err(ToolError::Execution(format!(
                "角色 id {role_id} 不存在，可用角色: {}",
                available.join(", ")
            )));
        };
        let fallback_role_name = role.name.clone();

        let app_config = AppConfig::load(&app).unwrap_or_default();
        let prompt_options = PromptOptions {
            output_sec_lang: app_config.llm_output_sec_lang,
            no_emotion_limit: app_config.no_emotion_limit_prompt,
        };

        let gs = game_status_handle(&app).await;
        let mut gs = gs.lock().await;

        // 先加载并构建目标角色的人设；任何一步失败都不修改 current_role_id，避免
        // 留下“界面已经切换、后端上下文却不可用”的半完成状态。
        gs.get_role(&state.db, role_id)
            .await
            .map_err(|e| ToolError::Execution(format!("加载角色 {role_id} 失败: {e}")))?;
        let (role_name, system_prompt) = {
            let loaded = gs
                .role_manager
                .get_loaded(role_id)
                .ok_or_else(|| ToolError::Execution(format!("角色 {role_id} 加载后不可用")))?;
            let name = loaded.display_name.clone().unwrap_or(fallback_role_name);
            let prompt = sys_prompt_builder_by_settings(&loaded.settings, prompt_options);
            (name, prompt)
        };

        let has_system_prompt = gs.line_list.iter().any(|line| {
            matches!(line.attribute(), LineAttribute::System)
                && line.sender_role_id() == Some(role_id)
        });
        if !has_system_prompt {
            gs.add_line(
                &state.db,
                LineBase {
                    content: system_prompt,
                    attribute: LineAttributeExt(LineAttribute::System),
                    sender_role_id: Some(role_id),
                    display_name: Some(role_name.clone()),
                    ..Default::default()
                },
            )
            .await
            .map_err(|e| ToolError::Execution(format!("初始化角色 {role_id} 人设失败: {e}")))?;
        }

        // 与前端 character:switch 的语义保持一致：目标角色原本不在场时，视为
        // 单角色替换；若已在多人场景中，则只切换当前说话者而保留其他在场角色。
        if !gs.present_role_ids.contains(&role_id) {
            gs.onstage_role_ids.clear();
            gs.present_role_ids.clear();
            gs.onstage_role(role_id);
        }
        gs.current_role_id = Some(role_id);
        gs.refresh_memories(&state.db)
            .await
            .map_err(|e| ToolError::Execution(format!("刷新角色 {role_id} 上下文失败: {e}")))?;
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
