use async_trait::async_trait;
use serde_json::{Value, json};
use std::time::Duration;
use tauri::{Emitter, Manager};
use tauri_plugin_store::StoreExt;

use crate::AppState;
use crate::ai_service::types::ToolDefinition;

use super::executor::{Tool, ToolContext, ToolError, ToolResult};
use super::game_status_handle;

/// 列出指定已加载角色的可用服装名。
fn available_clothes(
    gs: &mut crate::ai_service::game_system::game_status::GameStatus,
    role_id: i32,
) -> Vec<String> {
    gs.role_manager
        .get_loaded(role_id)
        .map(|r| {
            r.settings
                .clothes
                .as_ref()
                .map(|list| list.iter().filter_map(|c| c.get("name").cloned()).collect())
                .unwrap_or_default()
        })
        .unwrap_or_default()
}

/// character_outfits：列出当前角色可用的服装名称。
pub struct CharacterOutfits;

#[async_trait]
impl Tool for CharacterOutfits {
    fn definition(&self) -> ToolDefinition {
        ToolDefinition::new(
            "character_outfits",
            "列出当前角色可用的服装名称与当前穿着（配合 change_clothes 使用）",
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
        let Some(obj) = arguments.as_object() else {
            return Err(ToolError::InvalidArguments("参数必须是 JSON object".into()));
        };
        if !obj.is_empty() {
            return Err(ToolError::InvalidArguments(
                "character_outfits 不接受参数".into(),
            ));
        }
        let app = context.require_app()?;
        let gs = game_status_handle(&app).await;
        let mut gs = gs.lock().await;
        let Some(role_id) = gs.current_role_id else {
            return Err(ToolError::Execution("当前没有对话中的角色".into()));
        };
        let current_clothes = gs
            .role_manager
            .get_loaded(role_id)
            .map(|r| r.current_clothes.clone())
            .unwrap_or_default();
        let clothes = available_clothes(&mut gs, role_id);
        Ok(json!({
            "role_id": role_id,
            "current_clothes": current_clothes,
            "clothes": clothes,
        }))
    }
}

/// change_clothes：让当前角色换上指定服装。
///
/// 与前端 select_clothes 命令走同一条链路：持久化选择 → 记录 override →
/// GameStatus 统一换装（去重 + 换装旁白生成）。
pub struct ChangeClothes;

#[async_trait]
impl Tool for ChangeClothes {
    /// 换装涉及 DB 写入与旁白行生成，放宽默认 2 秒超时作为保险。
    fn timeout_hint(&self) -> Option<Duration> {
        Some(Duration::from_secs(10))
    }

    fn definition(&self) -> ToolDefinition {
        ToolDefinition::new(
            "change_clothes",
            "让当前角色换上指定服装（会生成换装旁白）。天气变化、用户要求或剧情合适时可主动使用；服装名必须来自 character_outfits 返回的列表",
            json!({
                "type": "object",
                "properties": {
                    "clothes_name": {"type": "string", "description": "服装名称"}
                },
                "required": ["clothes_name"],
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
            return Err(ToolError::InvalidArguments("参数必须是 JSON object".into()));
        };
        let clothes_name = obj
            .get("clothes_name")
            .and_then(Value::as_str)
            .map(str::trim)
            .filter(|s| !s.is_empty())
            .ok_or_else(|| ToolError::InvalidArguments("change_clothes 需要 clothes_name".into()))?
            .to_string();

        let app = context.require_app()?;
        let state = app.state::<AppState>();

        let gs = game_status_handle(&app).await;
        let role_id = gs
            .lock()
            .await
            .current_role_id
            .ok_or_else(|| ToolError::Execution("当前没有对话中的角色".into()))?;

        // 持久化该角色的服装选择（与前端 select_clothes 相同）
        if let Ok(store) = app.store(crate::config::STORE_FILE) {
            let key = crate::config::session::last_clothes_key(role_id);
            store.set(key, Value::String(clothes_name.clone()));
            let _ = store.save();
        }

        let switched = {
            let mut gs = gs.lock().await;
            let available = available_clothes(&mut gs, role_id);
            if !available.is_empty() && !available.iter().any(|n| *n == clothes_name) {
                return Err(ToolError::Execution(format!(
                    "服装「{clothes_name}」不存在，可用：{}",
                    available.join(", ")
                )));
            }
            gs.role_manager
                .set_character_clothes_override(role_id, clothes_name.clone());
            gs.on_character_change_clothes(&state.db, role_id, &clothes_name)
                .await
                .map_err(|e| ToolError::Execution(format!("切换服装失败: {e}")))?
        };

        if switched {
            // 通知前端即时切换立绘/Live2D（select_clothes 命令路径由前端在成功回调里本地更新，不发此事件）
            let payload = json!({
                "type": "clothes_changed",
                "roleId": role_id,
                "clothesName": clothes_name.clone(),
            });
            if let Err(e) = app.emit("clothes:changed", &payload) {
                tracing::warn!("emit clothes:changed 失败: {e}");
            }
        }

        Ok(json!({
            "success": true,
            "switched": switched,
            "message": if switched {
                format!("已换上「{clothes_name}」")
            } else {
                "当前已经是这套衣服".to_string()
            },
        }))
    }
}
