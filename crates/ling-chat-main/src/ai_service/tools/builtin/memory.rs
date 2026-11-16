use std::collections::{HashMap, HashSet};
use std::path::Path;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Duration;

use sea_orm::DatabaseConnection;
use serde::Serialize;
use serde_json::{Value, json};
use tauri::Manager;

use crate::AppState;
use crate::ai_service::game_system::game_status::GameStatus;
use crate::ai_service::types::ToolDefinition;
use crate::api::character::read_character_settings;
use crate::api::data_dir;
use crate::db::managers::role_repo::RoleRepo;
use crate::db::managers::save_note_repo::SaveNoteRepo;

use ling_chat_notes::Note;
use ling_chat_notes::NotesStore;
use ling_chat_notes::embedding::Embedder;
use ling_chat_notes::retrieval::{DEFAULT_TOP_K, MAX_TOP_K, NotesVectorIndex, note_text};

use crate::ai_service::tools::executor::{Tool, ToolContext, ToolError, ToolResult};
use crate::ai_service::tools::{ensure_no_args, game_status_handle};

// ─── 笔记工具提示词（集中在此，方便查看与调整） ───

const GET_CURRENT_MEMORY_PROMPT: &str =
    "获取当前角色的自动记忆库文本（关于用户的信息、重要约定、长期经历）";

const GET_NOTES_PROMPT: &str = "获取手动记忆笔记（含 id、内容、标签、来源）。默认同时检索当前存档和当前角色的全局笔记，合并去重后按相关度返回；每条笔记的 source 字段标明来源：save=当前存档（同档内所有角色共享），role=角色全局。可用 scope 指定只查 save 或 role；传 role 可指定要查全局笔记的角色。不传 query 时返回全部笔记；传 query 时按语义检索最相关的 top_k 条";

const ADD_NOTE_PROMPT: &str = "添加一条手动记忆笔记。默认写入当前存档（同档内所有角色共享）；只有用户明确要求长期记住、跨存档也必须保留的关键信息，才用 scope=role 写入当前角色的全局笔记";

const UPDATE_NOTE_PROMPT: &str = "按 ID 更新笔记内容或标签，至少提供一项。默认先找当前存档，找不到再找当前角色的全局笔记；也可用 scope 指定";

const DELETE_NOTE_PROMPT: &str =
    "按 ID 删除笔记。默认先找当前存档，找不到再找当前角色的全局笔记；也可用 scope 指定";

// ─── 来源与检索范围 ───

const SOURCE_SAVE: &str = "save";
const SOURCE_ROLE: &str = "role";

/// 尚未存档时的暂存笔记没有持久索引，按批在内存里算相似度。
const SCORE_BATCH: usize = 32;

/// 带来源标记的笔记视图：序列化时与 `Note` 字段平铺，额外带 `source`。
#[derive(Clone, Debug, Serialize)]
struct SourcedNote {
    #[serde(flatten)]
    note: Note,
    source: &'static str,
}

/// 一路待检索的笔记：存档侧（有存档时走持久索引，无档时内存打分）或角色全局侧。
struct NoteSource {
    source: &'static str,
    /// 向量索引归属键；None 表示内存暂存，需要现场打分。
    owner: Option<String>,
    notes: Vec<Note>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum GetScope {
    Merged,
    Save,
    Role,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum WriteScope {
    Save,
    Role,
}

fn parse_get_scope(obj: &serde_json::Map<String, Value>) -> Result<GetScope, ToolError> {
    match obj.get("scope").and_then(Value::as_str) {
        None | Some("merged") => Ok(GetScope::Merged),
        Some("save") => Ok(GetScope::Save),
        Some("role") => Ok(GetScope::Role),
        Some(other) => Err(ToolError::InvalidArguments(format!(
            "memory_get_notes 的 scope 不支持: {other}"
        ))),
    }
}

/// 写操作的范围：缺省返回 None，由调用方决定默认值（新增默认存档，改删默认自动定位）。
fn parse_write_scope(
    obj: &serde_json::Map<String, Value>,
    tool: &str,
) -> Result<Option<WriteScope>, ToolError> {
    match obj.get("scope").and_then(Value::as_str) {
        None => Ok(None),
        Some("save") => Ok(Some(WriteScope::Save)),
        Some("role") => Ok(Some(WriteScope::Role)),
        Some(other) => Err(ToolError::InvalidArguments(format!(
            "{tool} 的 scope 不支持: {other}"
        ))),
    }
}

// ─── 角色名解析（工具层）：把外部角色名映射到笔记库使用的权威展示名 ───

/// 把外部传入的角色名解析为 LingChat 权威展示名（ai_name）。
///
/// 优先精确匹配 `ai_name`，其次匹配角色 DB 名 `role.name`。
/// 解析成功后才能定位到对应角色的笔记文件，保证"文件名对齐"。
async fn resolve_display_name(db: &DatabaseConnection, given: &str) -> Result<String, ToolError> {
    let given = given.trim();
    if given.is_empty() {
        return Err(ToolError::InvalidArguments("role 不能为空".into()));
    }
    let roles = RoleRepo::get_all_main_roles(db)
        .await
        .map_err(|e| ToolError::Execution(format!("查询角色列表失败: {e}")))?;
    for role in &roles {
        let dn =
            read_character_settings(role.resource_folder.as_deref().unwrap_or_default()).ai_name;
        if dn == given || dn.trim() == given {
            return Ok(dn);
        }
    }
    // 兜底：按 DB 角色名匹配，仍返回该角色的 ai_name
    for role in &roles {
        if role.name == given {
            return Ok(read_character_settings(
                role.resource_folder.as_deref().unwrap_or_default(),
            )
            .ai_name);
        }
    }
    Err(ToolError::Execution(format!("未找到角色: {given}")))
}

/// 取当前对话角色的权威展示名；没有当前角色时返回 None。
async fn current_display_name(gs: &mut GameStatus, db: &DatabaseConnection) -> Option<String> {
    let role_id = gs.current_role_id?;
    let role = gs.get_role(db, role_id).await.ok()?;
    role.display_name.clone()
}

/// 写全局笔记前解析当前角色名；没有当前角色时明确报错。
async fn current_role_name_for_write(context: &ToolContext) -> Result<String, ToolError> {
    let app = context.require_app()?;
    let state = app.state::<AppState>();
    let db = state.db.clone();
    let gs = game_status_handle(&app).await;
    let mut gs = gs.lock().await;
    current_display_name(&mut gs, &db)
        .await
        .ok_or_else(|| ToolError::Execution("当前没有对话角色".into()))
}

fn notes_store() -> NotesStore {
    NotesStore::new(data_dir())
}

/// 存档笔记在向量索引里的归属键。存档索引与角色索引分目录，键只用于文件命名。
pub(crate) fn save_note_owner(save_id: i32) -> String {
    format!("save_{save_id}")
}

// ─── 混合检索：合并、去重、补位 ───

/// 去重键：忽略空白差异与大小写，同一条内容只保留一次。
fn dedup_key(content: &str) -> String {
    content
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
        .to_lowercase()
}

fn source_rank(source: &str) -> u8 {
    if source == SOURCE_SAVE { 0 } else { 1 }
}

/// 无查询时的全量合并：按来源顺序拼接，同内容只保留先出现的一条。
fn merge_full(sources: &[NoteSource]) -> Vec<SourcedNote> {
    let mut seen = HashSet::new();
    let mut result = Vec::new();
    for source in sources {
        for note in &source.notes {
            if seen.insert(dedup_key(&note.content)) {
                result.push(SourcedNote {
                    note: note.clone(),
                    source: source.source,
                });
            }
        }
    }
    result
}

/// 按分数降序合并；同分时存档优先。重复内容跳过，由下一条候选补位。
fn merge_scored(mut candidates: Vec<(SourcedNote, f32)>, top_k: usize) -> Vec<SourcedNote> {
    candidates.sort_by(|a, b| {
        b.1.partial_cmp(&a.1)
            .unwrap_or(std::cmp::Ordering::Equal)
            .then_with(|| source_rank(a.0.source).cmp(&source_rank(b.0.source)))
    });

    let mut seen = HashSet::new();
    let mut result = Vec::with_capacity(top_k.min(candidates.len()));
    for (note, _) in candidates {
        if !seen.insert(dedup_key(&note.note.content)) {
            continue;
        }
        result.push(note);
        if result.len() == top_k {
            break;
        }
    }
    result
}

/// 给尚未落档的暂存笔记现场打分；向量已 L2 归一化，点积即余弦相似度。
fn score_in_memory(
    embedder: &dyn Embedder,
    query: &str,
    notes: &[Note],
) -> anyhow::Result<Vec<(String, f32)>> {
    if notes.is_empty() {
        return Ok(Vec::new());
    }
    let query_vector = embedder.embed(query)?;
    let mut scored = Vec::with_capacity(notes.len());
    for chunk in notes.chunks(SCORE_BATCH) {
        let texts: Vec<String> = chunk.iter().map(note_text).collect();
        let vectors = embedder.embed_batch(&texts)?;
        if vectors.len() != chunk.len() {
            return Err(anyhow::anyhow!("嵌入结果数量与输入不一致"));
        }
        for (note, vector) in chunk.iter().zip(vectors) {
            let score = query_vector
                .iter()
                .zip(vector.iter())
                .map(|(a, b)| a * b)
                .sum::<f32>();
            scored.push((note.id.clone(), score));
        }
    }
    scored.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal));
    Ok(scored)
}

fn parse_tags(value: Option<&Value>, tool: &str) -> Result<Option<Vec<String>>, ToolError> {
    let Some(value) = value else {
        return Ok(None);
    };
    let array = value
        .as_array()
        .ok_or_else(|| ToolError::InvalidArguments(format!("{tool} 的 tags 必须是字符串数组")))?;
    let mut tags = Vec::with_capacity(array.len());
    for value in array {
        let tag = value.as_str().ok_or_else(|| {
            ToolError::InvalidArguments(format!("{tool} 的 tags 必须全部是字符串"))
        })?;
        tags.push(tag.to_string());
    }
    Ok(Some(tags))
}

fn require_object<'a>(
    arguments: &'a Value,
    tool: &str,
) -> Result<&'a serde_json::Map<String, Value>, ToolError> {
    arguments
        .as_object()
        .ok_or_else(|| ToolError::InvalidArguments(format!("{tool} 参数必须是 JSON object")))
}

// ─── 工具 ───

/// memory_get_current：获取当前角色的自动记忆库文本。
pub struct GetCurrentMemory;

#[async_trait::async_trait]
impl Tool for GetCurrentMemory {
    fn definition(&self) -> ToolDefinition {
        ToolDefinition::new(
            "memory_get_current",
            GET_CURRENT_MEMORY_PROMPT,
            json!({
                "type": "object",
                "properties": {},
                "required": [],
                "additionalProperties": false
            }),
        )
        .with_rag_examples([
            "你还记得关于我的什么",
            "你记着我哪些事",
            "说说你对我的印象",
            "我们之前聊过什么",
        ])
    }

    async fn execute(
        &self,
        context: &ToolContext,
        arguments: Value,
    ) -> Result<ToolResult, ToolError> {
        ensure_no_args(&arguments, "memory_get_current").map_err(ToolError::Execution)?;
        let app = context.require_app()?;
        let gs = game_status_handle(&app).await;
        let gs = gs.lock().await;
        let Some(role_id) = gs.current_role_id else {
            return Err(ToolError::Execution("当前没有对话角色".into()));
        };
        let memory = gs.role_manager.get_role_memory_text(role_id).await;
        Ok(json!({
            "ok": true,
            "role_id": role_id,
            "memory": memory,
        }))
    }
}

/// memory_get_notes：获取手动记忆笔记。
///
/// 默认合并当前存档与当前角色的全局笔记，按语义相关度去重后返回；
/// 每条结果带 `source` 标明来源。可指定只查存档或角色全局。
pub struct GetNotes {
    index: Arc<NotesVectorIndex>,
    save_index: Arc<NotesVectorIndex>,
    warned_disabled: AtomicBool,
}

impl GetNotes {
    pub fn new(data_dir: impl AsRef<Path>) -> Self {
        Self {
            index: Arc::new(NotesVectorIndex::new(&data_dir)),
            save_index: Arc::new(NotesVectorIndex::for_save_notes(&data_dir)),
            warned_disabled: AtomicBool::new(false),
        }
    }

    fn full_result(notes: Vec<SourcedNote>) -> Value {
        json!({ "ok": true, "notes": notes })
    }

    /// 合并多路来源检索；引擎禁用、检索失败等任何降级都回落到全量返回。
    async fn respond(
        &self,
        engine: Arc<dyn Embedder>,
        sources: Vec<NoteSource>,
        query: Option<String>,
        top_k: usize,
    ) -> Result<ToolResult, ToolError> {
        let full = merge_full(&sources);
        let Some(query) = query else {
            return Ok(Self::full_result(full));
        };
        if full.is_empty() {
            return Ok(Self::full_result(full));
        }
        if !engine.is_enabled() {
            if !self.warned_disabled.swap(true, Ordering::Relaxed) {
                tracing::warn!("文本嵌入后端未启用，memory_get_notes 回退为全量返回");
            }
            return Ok(Self::full_result(full));
        }

        let index = self.index.clone();
        let save_index = self.save_index.clone();
        let search =
            tokio::task::spawn_blocking(move || -> anyhow::Result<Vec<(SourcedNote, f32)>> {
                let mut candidates = Vec::new();
                for source in sources {
                    let scored = match source.owner {
                        Some(owner) => {
                            let target = if source.source == SOURCE_SAVE {
                                &save_index
                            } else {
                                &index
                            };
                            target.search_scored(
                                engine.as_ref(),
                                &owner,
                                &source.notes,
                                &query,
                                MAX_TOP_K,
                            )?
                        },
                        None => score_in_memory(engine.as_ref(), &query, &source.notes)?,
                    };
                    let by_id: HashMap<&str, &Note> = source
                        .notes
                        .iter()
                        .map(|note| (note.id.as_str(), note))
                        .collect();
                    for (id, score) in scored {
                        if let Some(note) = by_id.get(id.as_str()) {
                            candidates.push((
                                SourcedNote {
                                    note: (*note).clone(),
                                    source: source.source,
                                },
                                score,
                            ));
                        }
                    }
                }
                Ok(candidates)
            })
            .await;

        let candidates = match search {
            Ok(Ok(candidates)) => candidates,
            Ok(Err(e)) => {
                tracing::warn!("笔记语义检索失败，回退为全量返回: {e}");
                return Ok(Self::full_result(full));
            },
            Err(e) => {
                tracing::warn!("笔记语义检索任务失败，回退为全量返回: {e}");
                return Ok(Self::full_result(full));
            },
        };

        let selected = merge_scored(candidates, top_k);
        let total = full.len();
        let returned = selected.len();
        Ok(json!({
            "ok": true,
            "notes": selected,
            "total": total,
            "returned": returned,
            "retrieved": true,
        }))
    }
}

/// 非法或缺失的 top_k 一律回落到默认值，整数则限制在合法区间。
fn parse_top_k(value: Option<&Value>) -> usize {
    value
        .and_then(Value::as_i64)
        .map(|value| value.clamp(1, MAX_TOP_K as i64) as usize)
        .unwrap_or(DEFAULT_TOP_K)
}

#[async_trait::async_trait]
impl Tool for GetNotes {
    fn definition(&self) -> ToolDefinition {
        ToolDefinition::new(
            "memory_get_notes",
            GET_NOTES_PROMPT,
            json!({
                "type": "object",
                "properties": {
                    "scope": {"type": "string", "enum": ["merged", "save", "role"], "description": "检索范围，默认 merged（存档与角色全局合并）"},
                    "role": {"type": "string", "description": "要读取其全局笔记的角色名；不传时用当前角色"},
                    "query": {"type": "string", "description": "检索关键词或简短问句；不传则返回全部笔记。建议用陈述性关键词（如「食物过敏」「睡前约定」）"},
                    "top_k": {"type": "integer", "minimum": 1, "maximum": MAX_TOP_K, "description": format!("返回条数，默认 {DEFAULT_TOP_K}")}
                },
                "required": [],
                "additionalProperties": false
            }),
        )
        .with_rag_examples([
            "还记得我的生日是几号吗？",
            "我想看看你记得我的事情",
            "看看你给我记的笔记",
            "有没有关于睡前的记录",
        ])
    }

    fn timeout_hint(&self) -> Option<Duration> {
        // 首次检索要为整份笔记建索引；缓存命中后只有一次查询嵌入。
        Some(Duration::from_secs(15))
    }

    async fn execute(
        &self,
        context: &ToolContext,
        arguments: Value,
    ) -> Result<ToolResult, ToolError> {
        let obj = require_object(&arguments, "memory_get_notes")?;
        let app = context.require_app()?;
        let state = app.state::<AppState>();
        let db = state.db.clone();

        let scope = parse_get_scope(obj)?;
        let role_param = obj
            .get("role")
            .and_then(Value::as_str)
            .map(str::trim)
            .filter(|role| !role.is_empty());
        let query = obj
            .get("query")
            .and_then(Value::as_str)
            .map(str::trim)
            .filter(|query| !query.is_empty())
            .map(str::to_string);
        let top_k = parse_top_k(obj.get("top_k"));

        let mut sources = Vec::new();

        // 存档侧：有存档读内存副本，无存档读暂存；scope=role 时跳过。
        if scope != GetScope::Role {
            let gs = game_status_handle(&app).await;
            let (save_id, notes) = {
                let gs = gs.lock().await;
                match gs.active_save_id {
                    Some(save_id) => (Some(save_id), gs.save_notes.clone()),
                    None => (None, gs.pending_save_notes.clone()),
                }
            };
            if !notes.is_empty() {
                sources.push(NoteSource {
                    source: SOURCE_SAVE,
                    owner: save_id.map(save_note_owner),
                    notes,
                });
            }
        }

        // 角色全局侧：scope=save 时跳过；未指定角色时用当前角色。
        if scope != GetScope::Save {
            let role = match role_param {
                Some(role_name) => Some(resolve_display_name(&db, role_name).await?),
                None => {
                    let gs = game_status_handle(&app).await;
                    let mut gs = gs.lock().await;
                    current_display_name(&mut gs, &db).await
                },
            };
            match role {
                Some(role) => {
                    let notes = notes_store()
                        .load(&role)
                        .map_err(|e| ToolError::Execution(e.to_string()))?;
                    if !notes.is_empty() {
                        sources.push(NoteSource {
                            source: SOURCE_ROLE,
                            owner: Some(role),
                            notes,
                        });
                    }
                },
                // 显式只要角色全局却没有角色：报错而不是静默返回空
                None if scope == GetScope::Role => {
                    return Err(ToolError::Execution("当前没有对话角色".into()));
                },
                None => {},
            }
        }

        self.respond(state.embedding.clone(), sources, query, top_k)
            .await
    }
}

/// memory_add_note：添加一条手动记忆笔记。
///
/// 默认写当前存档；显式 scope=role 才写当前角色的全局笔记。
pub struct AddNote;

#[async_trait::async_trait]
impl Tool for AddNote {
    fn definition(&self) -> ToolDefinition {
        ToolDefinition::new(
            "memory_add_note",
            ADD_NOTE_PROMPT,
            json!({
                "type": "object",
                "properties": {
                    "content": {"type": "string", "description": "笔记内容"},
                    "tags": {"type": "array", "items": {"type": "string"}, "description": "标签，可选"},
                    "scope": {"type": "string", "enum": ["save", "role"], "description": "写入范围，默认 save（当前存档）；role=当前角色的全局笔记，仅用于必须跨存档保留的关键信息"}
                },
                "required": ["content"],
                "additionalProperties": false
            }),
        )
        .with_rag_examples([
            "记住我喜欢吃草莓",
            "要记住我的生日哦",
            "别忘了我不吃香菜",
            "把这个记到小本本上",
        ])
    }

    async fn execute(
        &self,
        context: &ToolContext,
        arguments: Value,
    ) -> Result<ToolResult, ToolError> {
        let obj = require_object(&arguments, "memory_add_note")?;
        let content = obj
            .get("content")
            .and_then(Value::as_str)
            .map(str::to_string)
            .ok_or_else(|| ToolError::InvalidArguments("memory_add_note 需要 content".into()))?;
        if content.trim().is_empty() {
            return Err(ToolError::InvalidArguments(
                "memory_add_note 的 content 不能为空".into(),
            ));
        }
        let tags = parse_tags(obj.get("tags"), "memory_add_note")?.unwrap_or_default();
        let scope = parse_write_scope(obj, "memory_add_note")?.unwrap_or(WriteScope::Save);

        if scope == WriteScope::Role {
            let role = current_role_name_for_write(context).await?;
            let id = notes_store()
                .add(&role, content, tags)
                .map_err(|e| ToolError::Execution(e.to_string()))?;
            return Ok(json!({"ok": true, "id": id, "source": SOURCE_ROLE}));
        }

        let app = context.require_app()?;
        let state = app.state::<AppState>();
        let db = state.db.clone();
        let gs = game_status_handle(&app).await;
        let mut gs = gs.lock().await;
        let note = Note::new(content, tags);
        let id = note.id.clone();
        match gs.active_save_id {
            Some(save_id) => {
                SaveNoteRepo::insert(&db, save_id, &note)
                    .await
                    .map_err(|e| ToolError::Execution(e.to_string()))?;
                gs.save_notes.push(note);
            },
            None => gs.pending_save_notes.push(note),
        }
        Ok(json!({"ok": true, "id": id, "source": SOURCE_SAVE}))
    }
}

/// memory_update_note：更新手动记忆笔记。
pub struct UpdateNote;

#[async_trait::async_trait]
impl Tool for UpdateNote {
    fn definition(&self) -> ToolDefinition {
        ToolDefinition::new(
            "memory_update_note",
            UPDATE_NOTE_PROMPT,
            json!({
                "type": "object",
                "properties": {
                    "id": {"type": "string", "description": "笔记 ID"},
                    "content": {"type": "string", "description": "新的笔记内容，可选"},
                    "tags": {"type": "array", "items": {"type": "string"}, "description": "新的标签，可选"},
                    "scope": {"type": "string", "enum": ["save", "role"], "description": "指定笔记所在库；不传时先找当前存档，找不到再找当前角色全局笔记"}
                },
                "required": ["id"],
                "additionalProperties": false
            }),
        )
        .with_rag_examples([
            "把那条笔记改一下",
            "更正一下你记的内容",
            "我搬家了，更新一下地址",
            "那件事记得不对，改改",
        ])
    }

    async fn execute(
        &self,
        context: &ToolContext,
        arguments: Value,
    ) -> Result<ToolResult, ToolError> {
        let obj = require_object(&arguments, "memory_update_note")?;
        let id = obj
            .get("id")
            .and_then(Value::as_str)
            .map(str::to_string)
            .ok_or_else(|| ToolError::InvalidArguments("memory_update_note 需要 id".into()))?;
        let has_content = obj.get("content").is_some();
        let has_tags = obj.get("tags").is_some();
        if !has_content && !has_tags {
            return Err(ToolError::InvalidArguments(
                "memory_update_note 至少需要 content/tags 中的一项".into(),
            ));
        }
        let content = match obj.get("content") {
            Some(value) => {
                let content = value.as_str().ok_or_else(|| {
                    ToolError::InvalidArguments("memory_update_note 的 content 必须是字符串".into())
                })?;
                if content.trim().is_empty() {
                    return Err(ToolError::InvalidArguments(
                        "memory_update_note 的 content 不能为空".into(),
                    ));
                }
                Some(content.to_string())
            },
            None => None,
        };
        let tags = parse_tags(obj.get("tags"), "memory_update_note")?;
        let scope = parse_write_scope(obj, "memory_update_note")?;

        let app = context.require_app()?;
        let state = app.state::<AppState>();
        let db = state.db.clone();

        if scope != Some(WriteScope::Role) {
            let gs = game_status_handle(&app).await;
            let mut gs = gs.lock().await;
            let found = match gs.active_save_id {
                Some(_) => gs.save_notes.iter().any(|note| note.id == id),
                None => gs.pending_save_notes.iter().any(|note| note.id == id),
            };
            if found {
                match gs.active_save_id {
                    Some(save_id) => {
                        SaveNoteRepo::update(&db, save_id, &id, content.clone(), tags.clone())
                            .await
                            .map_err(|e| ToolError::Execution(e.to_string()))?;
                        if let Some(note) = gs.save_notes.iter_mut().find(|note| note.id == id) {
                            if let Some(content) = &content {
                                note.content = content.clone();
                            }
                            if let Some(tags) = &tags {
                                note.tags = tags.clone();
                            }
                        }
                    },
                    None => gs
                        .update_pending_note(&id, content.clone(), tags.clone())
                        .map_err(|e| ToolError::Execution(e.to_string()))?,
                }
                return Ok(json!({"ok": true, "id": id, "source": SOURCE_SAVE}));
            }
            if scope == Some(WriteScope::Save) {
                return Err(ToolError::Execution(format!("笔记 {id} 不存在")));
            }
        }

        let role = current_role_name_for_write(context).await?;
        notes_store()
            .update(&role, &id, content, tags)
            .map_err(|e| ToolError::Execution(e.to_string()))?;
        Ok(json!({"ok": true, "id": id, "source": SOURCE_ROLE}))
    }
}

/// memory_delete_note：删除手动记忆笔记。
pub struct DeleteNote;

#[async_trait::async_trait]
impl Tool for DeleteNote {
    fn definition(&self) -> ToolDefinition {
        ToolDefinition::new(
            "memory_delete_note",
            DELETE_NOTE_PROMPT,
            json!({
                "type": "object",
                "properties": {
                    "id": {"type": "string", "description": "笔记 ID"},
                    "scope": {"type": "string", "enum": ["save", "role"], "description": "指定笔记所在库；不传时先找当前存档，找不到再找当前角色全局笔记"}
                },
                "required": ["id"],
                "additionalProperties": false
            }),
        )
        .with_rag_examples([
            "把那条笔记删了",
            "这件事不用记了",
            "忘掉你记的那个",
            "删掉关于我的那条记录",
        ])
    }

    async fn execute(
        &self,
        context: &ToolContext,
        arguments: Value,
    ) -> Result<ToolResult, ToolError> {
        let obj = require_object(&arguments, "memory_delete_note")?;
        let id = obj
            .get("id")
            .and_then(Value::as_str)
            .map(str::to_string)
            .ok_or_else(|| ToolError::InvalidArguments("memory_delete_note 需要 id".into()))?;
        let scope = parse_write_scope(obj, "memory_delete_note")?;

        let app = context.require_app()?;
        let state = app.state::<AppState>();
        let db = state.db.clone();

        if scope != Some(WriteScope::Role) {
            let gs = game_status_handle(&app).await;
            let mut gs = gs.lock().await;
            let found = match gs.active_save_id {
                Some(_) => gs.save_notes.iter().any(|note| note.id == id),
                None => gs.pending_save_notes.iter().any(|note| note.id == id),
            };
            if found {
                match gs.active_save_id {
                    Some(save_id) => {
                        SaveNoteRepo::delete(&db, save_id, &id)
                            .await
                            .map_err(|e| ToolError::Execution(e.to_string()))?;
                        gs.save_notes.retain(|note| note.id != id);
                    },
                    None => gs
                        .delete_pending_note(&id)
                        .map_err(|e| ToolError::Execution(e.to_string()))?,
                }
                return Ok(json!({"ok": true, "id": id, "source": SOURCE_SAVE}));
            }
            if scope == Some(WriteScope::Save) {
                return Err(ToolError::Execution(format!("笔记 {id} 不存在")));
            }
        }

        let role = current_role_name_for_write(context).await?;
        notes_store()
            .delete(&role, &id)
            .map_err(|e| ToolError::Execution(e.to_string()))?;
        Ok(json!({"ok": true, "id": id, "source": SOURCE_ROLE}))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn note_with(id: &str, content: &str) -> Note {
        Note {
            id: id.to_string(),
            content: content.to_string(),
            tags: Vec::new(),
            created_at: "2026-01-01T00:00:00Z".to_string(),
        }
    }

    fn sourced(id: &str, content: &str, source: &'static str) -> SourcedNote {
        SourcedNote {
            note: note_with(id, content),
            source,
        }
    }

    #[test]
    fn argument_parsing() {
        assert_eq!(parse_top_k(None), DEFAULT_TOP_K);
        assert_eq!(parse_top_k(Some(&json!("8"))), DEFAULT_TOP_K);
        assert_eq!(parse_top_k(Some(&json!(0))), 1);
        assert_eq!(parse_top_k(Some(&json!(30))), MAX_TOP_K);
        assert_eq!(parse_top_k(Some(&json!(5))), 5);

        let get = json!({"scope": "role"});
        assert_eq!(
            parse_get_scope(get.as_object().unwrap()).unwrap(),
            GetScope::Role
        );
        let get = json!({});
        assert_eq!(
            parse_get_scope(get.as_object().unwrap()).unwrap(),
            GetScope::Merged
        );

        let write = json!({"scope": "role"});
        assert_eq!(
            parse_write_scope(write.as_object().unwrap(), "t").unwrap(),
            Some(WriteScope::Role)
        );
        let write = json!({});
        assert_eq!(
            parse_write_scope(write.as_object().unwrap(), "t").unwrap(),
            None
        );
    }

    #[test]
    fn merge_dedups_and_backfills() {
        let candidates = vec![
            (sourced("a", "喜欢猫", SOURCE_SAVE), 0.9),
            (sourced("c", "喜欢猫", SOURCE_ROLE), 0.9),
            (sourced("b", "怕冷", SOURCE_SAVE), 0.8),
            (sourced("d", "对花生过敏", SOURCE_ROLE), 0.7),
        ];
        let merged = merge_scored(candidates, 3);
        assert_eq!(merged.len(), 3);
        // 同分优先存档
        assert_eq!(merged[0].note.id, "a");
        assert_eq!(merged[1].note.id, "b");
        // 重复内容被跳过后，由下一条较高分候选补位
        assert_eq!(merged[2].note.id, "d");
    }

    #[test]
    fn dedup_key_normalizes_whitespace_and_case() {
        assert_eq!(dedup_key("  Hello   World "), "hello world");
    }
}
