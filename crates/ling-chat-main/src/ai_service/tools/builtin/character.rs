use async_trait::async_trait;
use serde_json::{Value, json};
use tauri::{AppHandle, Emitter, Manager};

use crate::AppState;
use crate::ai_service::types::ToolDefinition;
use crate::api::character::{ClothesItem, read_character_settings, scan_clothes};
use crate::config::AppConfig;
use crate::db::managers::role_repo::RoleRepo;
use crate::utils::prompt::PromptOptions;

use crate::ai_service::tools::executor::{Tool, ToolContext, ToolError, ToolResult};
use crate::ai_service::tools::{ensure_no_args, game_status_handle};

/// character_list：列出所有可用角色的 ID、名称与角色标题。
pub struct CharacterList;

#[async_trait]
impl Tool for CharacterList {
    fn definition(&self) -> ToolDefinition {
        ToolDefinition::new(
            "character_list",
            "列出所有可用角色的 ID、名称与角色标题",
            json!({
                "type": "object",
                "properties": {},
                "required": [],
                "additionalProperties": false
            }),
        )
        .with_rag_examples([
            "现在有哪些角色",
            "还有别的角色吗",
            "列出所有角色",
            "角色列表给我看看",
        ])
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
        Ok(json!({
            "ok": true,
            "characters": roles
                .iter()
                .map(|r| {
                    let settings =
                        read_character_settings(r.resource_folder.as_deref().unwrap_or_default());
                    json!({"id": r.id, "name": settings.ai_name, "title": r.name})
                })
                .collect::<Vec<_>>()
        }))
    }
}

/// character_switch：切换当前对话角色（不清空对话历史的运行时切换）。
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
        .with_rag_examples([
            "让另一个角色来",
            "我要跟别的角色说话",
            "我想找其他角色",
            "现在让谁来陪我",
        ])
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
        // 角色标题（DB 的 role.name，即 settings.yml 的 title）；展示名缺失时拿它兜底
        let title = role.name.clone();
        let fallback_role_name = title.clone();

        let app_config = AppConfig::load(&app).unwrap_or_default();
        let prompt_options = PromptOptions::from_config(&app_config);

        let gs = game_status_handle(&app).await;
        let mut gs = gs.lock().await;

        // 任何一步失败都不修改 current_role_id，避免半完成状态。
        gs.get_role(&state.db, role_id)
            .await
            .map_err(|e| ToolError::Execution(format!("加载角色 {role_id} 失败: {e}")))?;
        // 人设行不在这里拼：随后的 rebuild_and_refresh 会按当前阵容整行重建
        // （找不到该角色的 SYSTEM 行时会自行插入一条）
        let role_name = gs
            .role_manager
            .get_loaded(role_id)
            .ok_or_else(|| ToolError::Execution(format!("角色 {role_id} 加载后不可用")))?
            .display_name
            .clone()
            .unwrap_or(fallback_role_name);

        // 与前端 character:switch 的语义保持一致：目标角色原本不在场时，视为
        // 单角色替换；若已在多人场景中，则只切换当前说话者而保留其他在场角色。
        if !gs.present_role_ids.contains(&role_id) {
            gs.onstage_role_ids.clear();
            gs.present_role_ids.clear();
            gs.onstage_role(role_id);
        }
        gs.current_role_id = Some(role_id);

        crate::ai_service::game_system::player_identity::persona::rebuild_and_refresh(
            &mut gs,
            &state.db,
            &crate::api::data_dir(),
            prompt_options,
        )
        .await
        .map_err(|e| ToolError::Execution(format!("重建在场角色人设行失败: {e}")))?;
        drop(gs);

        // 通知前端当前对话角色已切换（与 God Agent 切换使用同一事件）
        let payload = json!({
            "type": "character_switch",
            "roleId": role_id,
            "characterName": role_name,
        });
        if let Err(e) = app.emit(
            crate::ai_service::game_system::world_actions::EVENT_CHARACTER_SWITCH,
            &payload,
        ) {
            tracing::warn!("emit character:switch 失败: {e}");
        }

        Ok(json!({"ok": true, "role_id": role_id, "name": role_name, "title": title}))
    }
}

fn canonical_clothes(available: &[ClothesItem], raw: &str) -> Option<String> {
    let trimmed = raw.trim();
    let is_root_alias =
        trimmed.is_empty() || trimmed == "默认" || trimmed.eq_ignore_ascii_case("default");
    if is_root_alias {
        if let Some(root) = available.iter().find(|item| item.title == "默认") {
            return Some(root.title.clone());
        }
    }
    available
        .iter()
        .find(|item| item.title == trimmed)
        .or_else(|| {
            available
                .iter()
                .find(|item| item.title.eq_ignore_ascii_case(trimmed))
        })
        .map(|item| item.title.clone())
}

/// 解析目标角色：`role_id` 缺省时取当前对话角色。
async fn resolve_target_role(app: &AppHandle, arguments: &Value) -> Result<i32, ToolError> {
    let obj = arguments
        .as_object()
        .ok_or_else(|| ToolError::InvalidArguments("参数必须是 JSON object".into()))?;
    if let Some(raw) = obj.get("role_id") {
        let raw = raw
            .as_i64()
            .ok_or_else(|| ToolError::InvalidArguments("role_id 必须是整数".into()))?;
        return i32::try_from(raw)
            .map_err(|_| ToolError::InvalidArguments("role_id 超出 i32 范围".into()));
    }
    let gs = game_status_handle(app).await;
    let gs = gs.lock().await;
    gs.current_role_id
        .ok_or_else(|| ToolError::Execution("当前没有对话角色".into()))
}

/// 确保目标角色已加载，取出（展示名、当前服装、可换服装列表）。
async fn load_clothes_view(
    app: &AppHandle,
    role_id: i32,
) -> Result<(String, String, Vec<ClothesItem>), ToolError> {
    let state = app.state::<AppState>();
    let exists = RoleRepo::get_role_by_id(&state.db, role_id)
        .await
        .map_err(|e| ToolError::Execution(format!("查询角色 {role_id} 失败: {e}")))?
        .is_some();
    if !exists {
        return Err(ToolError::Execution(format!("角色 id {role_id} 不存在")));
    }

    let gs = game_status_handle(app).await;
    let mut gs = gs.lock().await;
    let role = gs
        .get_role(&state.db, role_id)
        .await
        .map_err(|e| ToolError::Execution(format!("加载角色 {role_id} 失败: {e}")))?;
    let name = role.display_name.clone().unwrap_or_default();
    let current = role.current_clothes.clone();
    let folder = role
        .resource_path
        .clone()
        .ok_or_else(|| ToolError::Execution(format!("角色 {role_id} 缺少资源目录")))?;
    // 扫描要读目录，别占着 game_status 锁
    drop(gs);

    Ok((name, current, scan_clothes(&folder)))
}

/// character_get_clothes：查询角色当前服装与可更换的服装列表。
pub struct CharacterGetClothes;

#[async_trait]
impl Tool for CharacterGetClothes {
    fn definition(&self) -> ToolDefinition {
        ToolDefinition::new(
            "character_get_clothes",
            "查询角色当前穿着的服装，以及该角色可更换的全部服装名称。优先省略 role_id（查你自己）",
            json!({
                "type": "object",
                "properties": {
                    "role_id": {"type": "integer", "description": "角色 ID；省略（推荐）时查询你自己当前的服装，仅在明确要看其他角色时才传"}
                },
                "required": [],
                "additionalProperties": false
            }),
        )
        .with_rag_examples([
            "你现在穿的哪一身",
            "都有哪些衣服可以换",
            "看看你的衣柜",
            "今天穿的是什么装扮",
        ])
    }

    async fn execute(
        &self,
        context: &ToolContext,
        arguments: Value,
    ) -> Result<ToolResult, ToolError> {
        let app = context.require_app()?;
        let role_id = resolve_target_role(&app, &arguments).await?;
        let (name, current, available) = load_clothes_view(&app, role_id).await?;

        let clothes_name =
            canonical_clothes(&available, &current).unwrap_or_else(|| current.clone());

        Ok(json!({
            "ok": true,
            "role_id": role_id,
            "name": name,
            "clothes_name": clothes_name,
            "clothes": available
                .iter()
                .map(|item| item.title.clone())
                .collect::<Vec<_>>(),
        }))
    }
}

pub struct CharacterSetClothes;

#[async_trait]
impl Tool for CharacterSetClothes {
    fn definition(&self) -> ToolDefinition {
        ToolDefinition::new(
            "character_set_clothes",
            "更换服装（默认换你自己，即当前对话角色），立绘会立即更新，并自动生成一句换装旁白。优先省略 role_id；只有明确要更换其他角色的服装时才传。已经是该服装时不重复生成。若工具返回 locked（用户手动锁定过服装），任何调用都不会执行——不要再尝试，可提示用户在设置中重新打开自动换装或手动选择",
            json!({
                "type": "object",
                "properties": {
                    "name": {"type": "string", "description": "目标服装名称，须为 character_get_clothes 返回的服装之一"},
                    "role_id": {"type": "integer", "description": "角色 ID；省略（推荐）时更换你自己当前的服装，仅在明确要换其他角色时才传"}
                },
                "required": ["name"],
                "additionalProperties": false
            }),
        )
        .with_rag_examples([
            "换一身衣服",
            "穿上那套校服",
            "换成另一套衣服",
            "把衣服换一下",
        ])
    }

    async fn execute(
        &self,
        context: &ToolContext,
        arguments: Value,
    ) -> Result<ToolResult, ToolError> {
        let app = context.require_app()?;

        let auto_allowed = crate::config::AppConfig::load(&app)
            .map(|c| c.ai_auto_clothes)
            .unwrap_or(true);
        if !auto_allowed {
            return Ok(json!({
                "success": false,
                "locked": true,
                "message": "用户已手动选择当前服装并关闭了自动更换，本次及后续的换装调用都不会执行。请自然接受，不要反复尝试；若用户想换装，可提示其在设置中重新打开自动换装，或在角色卡中手动选择",
            }));
        }

        let raw = arguments
            .as_object()
            .and_then(|obj| obj.get("name"))
            .and_then(Value::as_str)
            .ok_or_else(|| {
                ToolError::InvalidArguments("character_set_clothes 需要字符串 name".into())
            })?;

        let role_id = resolve_target_role(&app, &arguments).await?;
        let (name, current, available) = load_clothes_view(&app, role_id).await?;
        let target = canonical_clothes(&available, raw).ok_or_else(|| {
            let names: Vec<&str> = available.iter().map(|item| item.title.as_str()).collect();
            ToolError::Execution(format!(
                "角色 {role_id} 没有服装「{raw}」，可选：{}",
                names.join("、")
            ))
        })?;

        let switched = canonical_clothes(&available, &current).as_deref() != Some(target.as_str());
        if switched {
            // 走不落锁的内部实现：落锁只发生在"用户手动"的 select_clothes 命令里
            crate::api::character::select_clothes_inner(app.clone(), role_id, target.clone())
                .await
                .map_err(|e| ToolError::Execution(format!("更换服装失败: {e}")))?;
        }

        // 事件无条件发：即使后端判定「已经是这套」，重发一次也能纠正前端可能的陈旧状态
        let payload = json!({"roleId": role_id, "clothesName": target});
        if let Err(e) = app.emit(
            crate::ai_service::game_system::world_actions::EVENT_CLOTHES_CHANGED,
            &payload,
        ) {
            tracing::warn!("emit character:clothes-changed 失败: {e}");
        }

        Ok(json!({
            "ok": true,
            "role_id": role_id,
            "name": name,
            "clothes_name": target,
            "switched": switched,
        }))
    }
}
