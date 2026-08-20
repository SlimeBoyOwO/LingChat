use std::time::Duration;

use async_trait::async_trait;
use serde_json::{json, Value};
use tauri::{AppHandle, Emitter, Manager};
use tauri_plugin_store::StoreExt;

use crate::ai_service::game_system::scene_store::{Scene, SceneStore};
use crate::ai_service::image_gen;
use crate::ai_service::types::ToolDefinition;
use crate::api::data_dir;
use crate::AppState;

use super::executor::{Tool, ToolContext, ToolError, ToolResult};
use super::settings::SharedToolSettings;
use super::skill_files::request_user_approval;
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
        activate_scene(&app, &scene).await;

        Ok(json!({"ok": true, "scene_id": scene_id}))
    }
}

/// 把场景设为当前场景：写 GameStatus、持久化、并向主窗口广播。
///
/// `select_scene` 命令由前端自己更新 Pinia；LLM 工具没有这个调用方，必须主动
/// 广播完整场景资料，否则后端 ID 已变化但画面/背景仍停留在旧场景。
/// 任何新增的「切换场景」路径都要走这里，漏掉 emit 的表现是后端切了、画面没动。
pub(crate) async fn activate_scene(app: &AppHandle, scene: &Scene) {
    {
        let gs = game_status_handle(app).await;
        let mut gs = gs.lock().await;
        gs.current_scene_id = Some(scene.id.clone());
    }

    // 持久化到 store，便于下次启动恢复（与 api/scene.rs select_scene 一致）
    if let Ok(store) = app.store(crate::config::STORE_FILE) {
        store.set(
            crate::config::session::LAST_SCENE_ID.to_string(),
            Value::String(scene.id.clone()),
        );
        let _ = store.save();
    }

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
}

/// 读取用户选定的基准场景 ID（`select_scene` 写入，AI 的切换不会动它）。
fn base_scene_id(app: &AppHandle) -> Option<String> {
    app.store(crate::config::STORE_FILE)
        .ok()?
        .get(crate::config::session::BASE_SCENE_ID)?
        .as_str()
        .map(str::to_string)
}

/// 清除当前场景，回到无背景状态。
///
/// 用户从没选过场景时 `scene_return` 走这条路 —— 广播 scene 为 null，
/// 让前端把背景清空，否则画面会一直停在 AI 生成的那张图上。
async fn deactivate_scene(app: &AppHandle) {
    {
        let gs = game_status_handle(app).await;
        let mut gs = gs.lock().await;
        gs.current_scene_id = None;
    }
    if let Ok(store) = app.store(crate::config::STORE_FILE) {
        store.set(
            crate::config::session::LAST_SCENE_ID.to_string(),
            Value::Null,
        );
        let _ = store.save();
    }
    let payload = json!({"type": "scene_switch", "scene": Value::Null});
    if let Err(e) = app.emit("scene:switch", &payload) {
        tracing::warn!("emit scene:switch(null) 失败: {e}");
    }
}

/// scene_return：回到用户原本选定的场景（剧情从外面回来时用）。
pub struct SceneReturn;

#[async_trait]
impl Tool for SceneReturn {
    fn definition(&self) -> ToolDefinition {
        ToolDefinition::new(
            "scene_return",
            "回到用户原本选定的场景。当剧情从外出的地点回来时调用 —— \
             比如逛完天文馆回到家、散完步回到房间。\
             不需要指定场景，会自动回到用户在设置里选的那个基准场景。",
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
        ensure_no_args(&arguments, "scene_return").map_err(ToolError::Execution)?;
        let app = context.require_app()?;

        let Some(base_id) = base_scene_id(&app) else {
            // 用户从没选过场景：回到无背景，而不是留在 AI 生成的那张图上
            deactivate_scene(&app).await;
            return Ok(json!({
                "ok": true,
                "scene_id": Value::Null,
                "message": "用户没有设定基准场景，已清除背景",
            }));
        };

        {
            let gs = game_status_handle(&app).await;
            let gs = gs.lock().await;
            if gs.current_scene_id.as_deref() == Some(base_id.as_str()) {
                return Ok(json!({
                    "ok": true,
                    "scene_id": base_id,
                    "changed": false,
                    "message": "已经在原本的场景，无需切换",
                }));
            }
        }

        let store = SceneStore::new(&data_dir());
        let scene = store
            .find_by_id(&base_id)
            .map_err(|e| ToolError::Execution(format!("加载场景失败: {e}")))?;
        let Some(scene) = scene else {
            // 基准场景被删掉了：清除背景，别把已删除的 ID 设回去
            deactivate_scene(&app).await;
            return Ok(json!({
                "ok": true,
                "scene_id": Value::Null,
                "message": "原本的场景已被删除，已清除背景",
            }));
        };

        activate_scene(&app, &scene).await;
        Ok(json!({
            "ok": true,
            "scene_id": scene.id,
            "scene_name": scene.name,
            "changed": true,
            "message": "已回到原本的场景",
        }))
    }
}

/// scene_generate：为剧情去到的新地点用 NovelAI 生成背景图，落库成场景并切换过去。
pub struct SceneGenerate {
    settings: SharedToolSettings,
}

impl SceneGenerate {
    pub fn new(settings: SharedToolSettings) -> Self {
        Self { settings }
    }
}

#[async_trait]
impl Tool for SceneGenerate {
    fn definition(&self) -> ToolDefinition {
        ToolDefinition::new(
            "scene_generate",
            "当剧情去到一个现有场景里没有的地点时，用 NovelAI 生成该地点的背景图，\
             并自动创建场景、切换过去。\
             调用前必须先用 scene_list 查看已有场景：只要有合适的就改用 scene_switch，\
             不要为已存在的地点重复生成（生成要花十几秒，也可能消耗额度）。\
             等剧情从这个地点回来时，记得调用 scene_return 回到原本的场景。",
            json!({
                "type": "object",
                "properties": {
                    "scene_name": {
                        "type": "string",
                        "description": "场景名称，简短的中文地名，如「海边」「图书馆」"
                    },
                    "scene_description": {
                        "type": "string",
                        "description": "场景描述（中文），会作为旁白讲给角色听，描述这是什么地方、氛围如何"
                    },
                    "prompt_tags": {
                        "type": "string",
                        "description": "生成图片用的英文 danbooru 标签，逗号分隔，只描述景物不要写人物，\
                                        如 \"beach, ocean, sunset, clouds, summer\"。画风与画质标签会自动追加，不用写。"
                    }
                },
                "required": ["scene_name", "scene_description", "prompt_tags"],
                "additionalProperties": false
            }),
        )
    }

    /// 默认 2 秒远远不够：等用户确认最多 120 秒，之后 NovelAI 出图 10–30 秒
    /// （网络层超时 180 秒）。留到 360 秒，让两段串起来的最坏情况也不会被这里砍掉 ——
    /// 被砍掉的话用户会看到「工具执行超时」，而图其实还在生成。
    fn timeout_hint(&self) -> Option<Duration> {
        Some(Duration::from_secs(360))
    }

    async fn execute(
        &self,
        context: &ToolContext,
        arguments: Value,
    ) -> Result<ToolResult, ToolError> {
        let cfg = self.settings.get().image_gen;
        if !cfg.enabled {
            return Err(ToolError::Execution(
                "场景背景生成未启用，请在设置 - 工具配置中开启".into(),
            ));
        }
        if cfg.api_token.trim().is_empty() {
            return Err(ToolError::Execution(
                "未配置 NovelAI Token，请在设置 - 工具配置中填写".into(),
            ));
        }

        let arg_str = |key: &str| -> Result<String, ToolError> {
            arguments
                .get(key)
                .and_then(Value::as_str)
                .map(str::trim)
                .filter(|s| !s.is_empty())
                .map(str::to_string)
                .ok_or_else(|| ToolError::InvalidArguments(format!("缺少 {key} 参数")))
        };
        let scene_name = arg_str("scene_name")?;
        let scene_description = arg_str("scene_description")?;
        let prompt_tags = arg_str("prompt_tags")?;

        // 同名场景已存在就直接切过去 —— 模型没先查 scene_list 时的兜底，省掉一次生成。
        let store = SceneStore::new(&data_dir());
        let existing = store
            .load_all()
            .map_err(|e| ToolError::Execution(format!("加载场景失败: {e}")))?
            .into_iter()
            .find(|s| s.name == scene_name);
        if let Some(scene) = existing {
            let app = context.require_app()?;
            activate_scene(&app, &scene).await;
            return Ok(json!({
                "ok": true,
                "generated": false,
                "scene_id": scene.id,
                "message": "同名场景已存在，已直接切换，未重复生成",
            }));
        }

        // 参数越界要在弹确认框之前就拦下来，别让用户确认一个注定被拒的请求。
        cfg.check_free_tier().map_err(ToolError::Execution)?;

        let app = context.require_app()?;
        let prompt = image_gen::build_prompt(&cfg, &prompt_tags);

        if cfg.require_confirm {
            let approvals = app.state::<AppState>().chat_scene_generate_approvals.clone();
            request_user_approval(
                &app,
                approvals,
                "chat:scene_generate_approval",
                json!({
                    "scene_name": scene_name,
                    "scene_description": scene_description,
                    "prompt": prompt,
                    "model": cfg.model,
                    "width": cfg.width,
                    "height": cfg.height,
                    "steps": cfg.steps,
                    // 这次实际是否免费，而不是「有没有开限制」——
                    // 关掉限制后参数仍可能落在额度内，不该一律警告要扣费。
                    "free_tier": cfg.width.saturating_mul(cfg.height)
                        <= super::settings::NAI_FREE_MAX_PIXELS
                        && cfg.steps <= super::settings::NAI_FREE_MAX_STEPS,
                }),
                "背景生成",
            )
            .await?;
        }

        // NovelAI 有并发锁，同时打两个请求会 429。占不到锁就直接回报正忙，
        // 不排队 —— 排队会让对话卡在这里等上一张出完。
        let _guard = image_gen::try_acquire_generation_lock().ok_or_else(|| {
            ToolError::Execution("已有一张背景正在生成，请等它完成后再试".into())
        })?;

        let image = image_gen::generate_image(&cfg, &prompt_tags)
            .await
            .map_err(|e| ToolError::Execution(e.to_string()))?;

        let scene = save_generated_scene(
            &store,
            &scene_name,
            &scene_description,
            &image,
        )
        .map_err(ToolError::Execution)?;

        activate_scene(&app, &scene).await;

        Ok(json!({
            "ok": true,
            "generated": true,
            "scene_id": scene.id,
            "scene_name": scene.name,
            "seed": image.seed,
            "message": "背景已生成并切换过去",
        }))
    }
}

/// 落盘生成结果：先写场景记录、再写图片文件。
///
/// 顺序是刻意的 —— `list_scenes` 会把背景目录里「没有任何场景引用」的图片自动注册成
/// 一个无描述场景。反过来先写文件的话，中间这一瞬如果前端刷新了场景列表，就会多出
/// 一个同图但没有描述的重复场景。写文件失败时回滚记录，避免留下指向空文件的场景。
fn save_generated_scene(
    store: &SceneStore,
    scene_name: &str,
    scene_description: &str,
    image: &image_gen::GeneratedImage,
) -> Result<Scene, String> {
    // 文件名只用时间戳与种子：场景名是中文，进文件名会在跨平台同步时出问题。
    let file_name = format!(
        "nai_{}_{}.png",
        chrono::Utc::now().format("%Y%m%d%H%M%S"),
        image.seed
    );
    let now = chrono::Utc::now().to_rfc3339();
    let scene = Scene {
        id: uuid::Uuid::new_v4().to_string(),
        name: scene_name.to_string(),
        description: scene_description.to_string(),
        background: file_name.clone(),
        lighting: None,
        created_at: now.clone(),
        updated_at: now,
    };

    let mut scenes = store
        .load_all()
        .map_err(|e| format!("加载场景失败: {e}"))?;
    scenes.push(scene.clone());
    store
        .save_all(&scenes)
        .map_err(|e| format!("保存场景失败: {e}"))?;

    let dir = crate::api::backgrounds_dir();
    let write_result = std::fs::create_dir_all(&dir)
        .and_then(|_| std::fs::write(dir.join(&file_name), &image.bytes));

    if let Err(e) = write_result {
        // 回滚刚写进去的场景记录，否则会留下一个指向不存在文件的场景。
        scenes.retain(|s| s.id != scene.id);
        let _ = store.save_all(&scenes);
        return Err(format!("写入背景图片失败: {e}"));
    }

    Ok(scene)
}
