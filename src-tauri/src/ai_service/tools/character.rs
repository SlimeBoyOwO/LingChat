use async_trait::async_trait;
use serde_json::{Value, json};
use tauri::{AppHandle, Emitter, Manager};

use crate::AppState;
use crate::ai_service::types::{LineAttributeExt, LineBase, ToolDefinition};
use crate::api::character::{ClothesItem, read_character_settings, scan_clothes};
use crate::config::AppConfig;
use crate::db::entities::line::LineAttribute;
use crate::db::managers::role_repo::RoleRepo;
use crate::utils::prompt::{PromptOptions, sys_prompt_builder_by_settings};

use super::executor::{Tool, ToolContext, ToolError, ToolResult};
use super::{ensure_no_args, game_status_handle};

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
                    // name = 对话里的 AI 名称（settings.yml 的 ai_name），
                    // title = 角色标题（DB 的 role.name，即 settings.yml 的 title）。
                    // 两个名字在界面上都出现过：角色列表页的大字用 title，
                    // 详情页与对话里用 name，所以都给出来。
                    let settings =
                        read_character_settings(r.resource_folder.as_deref().unwrap_or_default());
                    json!({"id": r.id, "name": settings.ai_name, "title": r.name})
                })
                .collect::<Vec<_>>()
        }))
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
        // 角色标题（DB 的 role.name，即 settings.yml 的 title）；展示名缺失时拿它兜底
        let title = role.name.clone();
        let fallback_role_name = title.clone();

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

        Ok(json!({"ok": true, "role_id": role_id, "name": role_name, "title": title}))
    }
}

/// 把外部传入的服装名归一到 `scan_clothes` 的列表项上。
///
/// `scan_clothes` 给根目录立绘起的 title 是「默认」，而同一套服装在角色配置里
/// 可能被写成 `default` 或空串（`role_manager` 找不到配置时的兜底值就是
/// `default`），所以这三种写法都归到根目录那一项；其余先精确匹配，再忽略大小写。
/// 都不匹配返回 `None`，由调用方给出带可选列表的错误。
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
///
/// 必须先把角色加载起来：`on_character_change_clothes` 内部走 `get_loaded_mut`，
/// 未加载的角色会直接报「角色 X 未加载」。这里与 `character_switch` 同样处理。
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
    }

    async fn execute(
        &self,
        context: &ToolContext,
        arguments: Value,
    ) -> Result<ToolResult, ToolError> {
        let app = context.require_app()?;
        let role_id = resolve_target_role(&app, &arguments).await?;
        let (name, current, available) = load_clothes_view(&app, role_id).await?;

        // 当前值可能是 "default"/"" 这类别名写法，回给模型前归一到列表里的名字；
        // 归一不了（比如那套服装的目录已被删）就如实回原始值。
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

/// character_set_clothes：更换角色的服装。
///
/// 复用玩家换装的唯一入口 `api::character::select_clothes`（session 持久化 +
/// role_manager override + 旁白台词生成），再补发 `character:clothes-changed`：
/// 玩家换装时前端拿到返回值自己更新 store，LLM 工具没有这个调用方，不广播的话
/// 后端状态变了而前端立绘仍停在旧服装。
pub struct CharacterSetClothes;

#[async_trait]
impl Tool for CharacterSetClothes {
    fn definition(&self) -> ToolDefinition {
        ToolDefinition::new(
            "character_set_clothes",
            "更换服装（默认换你自己，即当前对话角色），立绘会立即更新，并自动生成一句换装旁白。优先省略 role_id；只有明确要更换其他角色的服装时才传。已经是该服装时不重复生成",
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
    }

    async fn execute(
        &self,
        context: &ToolContext,
        arguments: Value,
    ) -> Result<ToolResult, ToolError> {
        let app = context.require_app()?;
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

        // 语义相同但写法不同（当前是 "default"、目标是「默认」）时不能交给
        // select_clothes：它按原始字符串比较，会凭空生成一句换装旁白。
        let switched = canonical_clothes(&available, &current).as_deref() != Some(target.as_str());
        if switched {
            crate::api::character::select_clothes(app.clone(), role_id, target.clone())
                .await
                .map_err(|e| ToolError::Execution(format!("更换服装失败: {e}")))?;
        }

        // 事件无条件发：即使后端判定「已经是这套」，重发一次也能纠正前端可能的陈旧状态
        let payload = json!({"roleId": role_id, "clothesName": target});
        if let Err(e) = app.emit("character:clothes-changed", &payload) {
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
