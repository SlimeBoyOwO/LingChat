//! Agent 会话/消息持久化：剧本编辑器助手与游戏内导演共用同一套表。
//!
//! `kind` 区分用途，其余读写按会话 id 通用；导演会话额外带 `save_id`。

use chrono::Local;
use sea_orm::{
    ActiveModelTrait, ColumnTrait, DatabaseConnection, EntityTrait, QueryFilter, QueryOrder, Set,
};

use crate::ai_service::skill_agent::events::Usage;
use crate::ai_service::types::{LlmMessage, ToolCall};
use crate::db::entities::agent_conversation::{
    self, Entity as ConvEntity, KIND_DIRECTOR, KIND_SKILL,
};
use crate::db::entities::agent_message::{self, Entity as MsgEntity};

/// 新建剧本编辑器助手会话。只记录创建时的剧本 key，不存剧本内容快照。
pub async fn create_conversation(
    db: &DatabaseConnection,
    title: Option<String>,
    script_key: Option<String>,
) -> Result<i32, String> {
    let now = Local::now().naive_local();
    let model = agent_conversation::ActiveModel {
        id: sea_orm::NotSet,
        title: Set(title),
        script_key: Set(script_key),
        kind: Set(KIND_SKILL.to_string()),
        save_id: Set(None),
        created_at: Set(now),
        updated_at: Set(now),
    };
    let res = model
        .insert(db)
        .await
        .map_err(|e| format!("创建会话失败: {}", e))?;
    Ok(res.id)
}

/// 新建绑定到某个存档的导演会话。
pub async fn create_director_conversation(
    db: &DatabaseConnection,
    save_id: i32,
) -> Result<i32, String> {
    let now = Local::now().naive_local();
    let model = agent_conversation::ActiveModel {
        id: sea_orm::NotSet,
        title: Set(None),
        script_key: Set(None),
        kind: Set(KIND_DIRECTOR.to_string()),
        save_id: Set(Some(save_id)),
        created_at: Set(now),
        updated_at: Set(now),
    };
    let res = model
        .insert(db)
        .await
        .map_err(|e| format!("创建导演会话失败: {}", e))?;
    Ok(res.id)
}

/// 该存档下最近使用的导演会话。没有则返回 None。
pub async fn latest_director_conversation(
    db: &DatabaseConnection,
    save_id: i32,
) -> Result<Option<agent_conversation::Model>, String> {
    ConvEntity::find()
        .filter(agent_conversation::Column::Kind.eq(KIND_DIRECTOR))
        .filter(agent_conversation::Column::SaveId.eq(save_id))
        .order_by_desc(agent_conversation::Column::UpdatedAt)
        .order_by_desc(agent_conversation::Column::Id)
        .one(db)
        .await
        .map_err(|e| format!("查询导演会话失败: {}", e))
}

/// 取该存档的导演会话，没有就建一个。
pub async fn get_or_create_director_conversation(
    db: &DatabaseConnection,
    save_id: i32,
) -> Result<i32, String> {
    if let Some(conversation) = latest_director_conversation(db, save_id).await? {
        return Ok(conversation.id);
    }
    create_director_conversation(db, save_id).await
}

/// 全部剧本编辑器助手会话，按最近更新倒序。导演会话不在这里出现。
pub async fn list_conversations(
    db: &DatabaseConnection,
) -> Result<Vec<agent_conversation::Model>, String> {
    ConvEntity::find()
        .filter(agent_conversation::Column::Kind.eq(KIND_SKILL))
        .order_by_desc(agent_conversation::Column::UpdatedAt)
        .all(db)
        .await
        .map_err(|e| format!("查询会话列表失败: {}", e))
}

pub async fn get_conversation(
    db: &DatabaseConnection,
    id: i32,
) -> Result<Option<agent_conversation::Model>, String> {
    ConvEntity::find_by_id(id)
        .one(db)
        .await
        .map_err(|e| format!("查询会话失败: {}", e))
}

/// 更新会话的更新时间（用于会话列表排序）。
pub async fn touch_conversation(db: &DatabaseConnection, id: i32) -> Result<(), String> {
    let Some(m) = get_conversation(db, id).await? else {
        return Ok(());
    };
    let mut am: agent_conversation::ActiveModel = m.into();
    am.updated_at = Set(Local::now().naive_local());
    am.update(db)
        .await
        .map_err(|e| format!("更新会话失败: {}", e))?;
    Ok(())
}

/// 更新会话标题（用户重命名 / 首轮自动命名共用）。
pub async fn update_conversation_title(
    db: &DatabaseConnection,
    id: i32,
    title: String,
) -> Result<(), String> {
    let Some(m) = get_conversation(db, id).await? else {
        return Ok(());
    };
    let mut am: agent_conversation::ActiveModel = m.into();
    am.title = Set(Some(title));
    am.updated_at = Set(Local::now().naive_local());
    am.update(db)
        .await
        .map_err(|e| format!("更新会话标题失败: {}", e))?;
    Ok(())
}

/// 绑定会话的剧本 key；顺带刷新 updated_at 保持列表排序。
pub async fn update_conversation_script_key(
    db: &DatabaseConnection,
    id: i32,
    script_key: String,
) -> Result<(), String> {
    let Some(m) = get_conversation(db, id).await? else {
        return Ok(());
    };
    let mut am: agent_conversation::ActiveModel = m.into();
    am.script_key = Set(Some(script_key));
    am.updated_at = Set(Local::now().naive_local());
    am.update(db)
        .await
        .map_err(|e| format!("更新会话剧本 key 失败: {}", e))?;
    Ok(())
}

/// 删除会话及其全部消息。
pub async fn delete_conversation(db: &DatabaseConnection, id: i32) -> Result<(), String> {
    MsgEntity::delete_many()
        .filter(agent_message::Column::ConversationId.eq(id))
        .exec(db)
        .await
        .map_err(|e| format!("删除会话消息失败: {}", e))?;
    ConvEntity::delete_by_id(id)
        .exec(db)
        .await
        .map_err(|e| format!("删除会话失败: {}", e))?;
    Ok(())
}

/// 删除剧本编辑器助手会话。导演会话不接受这条入口，避免前端传错 id 删掉导演记录。
pub async fn delete_skill_conversation(db: &DatabaseConnection, id: i32) -> Result<(), String> {
    let Some(conversation) = get_conversation(db, id).await? else {
        return Ok(());
    };
    if conversation.kind != KIND_SKILL {
        return Err("该会话不是剧本编辑器的助手会话".to_string());
    }
    delete_conversation(db, id).await
}

/// 插入一条消息（OpenAI 格式；空 content 存 NULL）。
pub async fn insert_message(
    db: &DatabaseConnection,
    conversation_id: i32,
    msg: &LlmMessage,
    reasoning: Option<&str>,
    usage: Option<&Usage>,
) -> Result<i32, String> {
    let now = Local::now().naive_local();
    let model = agent_message::ActiveModel {
        id: sea_orm::NotSet,
        conversation_id: Set(conversation_id),
        role: Set(msg.role.clone()),
        content: Set(if msg.content.is_empty() {
            None
        } else {
            Some(msg.content.clone())
        }),
        reasoning: Set(reasoning.map(|s| s.to_string())),
        prompt_tokens: Set(usage.map(|u| u.prompt_tokens as i64)),
        completion_tokens: Set(usage.map(|u| u.completion_tokens as i64)),
        cached_tokens: Set(usage.map(|u| u.cached_tokens as i64)),
        tool_calls: Set(msg
            .tool_calls
            .as_ref()
            .map(|tcs| serde_json::to_string(tcs).unwrap_or_default())),
        tool_call_id: Set(msg.tool_call_id.clone()),
        created_at: Set(now),
    };
    let res = model
        .insert(db)
        .await
        .map_err(|e| format!("保存消息失败: {}", e))?;
    Ok(res.id)
}

/// 往导演会话追加一条消息，并刷新会话的更新时间。
pub async fn append_director_message(
    db: &DatabaseConnection,
    conversation_id: i32,
    msg: &LlmMessage,
    reasoning: Option<&str>,
    usage: Option<&Usage>,
) -> Result<i32, String> {
    let id = insert_message(db, conversation_id, msg, reasoning, usage).await?;
    touch_conversation(db, conversation_id).await?;
    Ok(id)
}

/// 某会话的全部消息（按时间升序）。
pub async fn list_messages(
    db: &DatabaseConnection,
    conversation_id: i32,
) -> Result<Vec<agent_message::Model>, String> {
    MsgEntity::find()
        .filter(agent_message::Column::ConversationId.eq(conversation_id))
        .order_by_asc(agent_message::Column::Id)
        .all(db)
        .await
        .map_err(|e| format!("查询消息失败: {}", e))
}

/// 清空某会话的消息。
pub async fn clear_messages(db: &DatabaseConnection, conversation_id: i32) -> Result<(), String> {
    MsgEntity::delete_many()
        .filter(agent_message::Column::ConversationId.eq(conversation_id))
        .exec(db)
        .await
        .map_err(|e| format!("清空消息失败: {}", e))?;
    Ok(())
}

/// 删除某会话中 id >= `from_id` 的全部消息（「回溯」：把对话回退到该消息之前）。
pub async fn delete_messages_from(
    db: &DatabaseConnection,
    conversation_id: i32,
    from_id: i32,
) -> Result<(), String> {
    MsgEntity::delete_many()
        .filter(agent_message::Column::ConversationId.eq(conversation_id))
        .filter(agent_message::Column::Id.gte(from_id))
        .exec(db)
        .await
        .map_err(|e| format!("回溯删除消息失败: {}", e))?;
    Ok(())
}

/// DB 消息模型 → LLM 消息。
pub fn message_to_llm(m: &agent_message::Model) -> LlmMessage {
    LlmMessage {
        role: m.role.clone(),
        content: m.content.clone().unwrap_or_default(),
        tool_calls: m
            .tool_calls
            .as_ref()
            .and_then(|s| serde_json::from_str(s).ok()),
        tool_call_id: m.tool_call_id.clone(),
        image_data_url: None,
    }
}

/// 从会话写入过的路径反推它归属的剧本包，供 `script_key` 为空的会话（老会话，或建包早于绑定逻辑）兜底。
pub async fn derive_script_key(db: &DatabaseConnection, conversation_id: i32) -> Option<String> {
    let rows = MsgEntity::find()
        .filter(agent_message::Column::ConversationId.eq(conversation_id))
        .filter(agent_message::Column::ToolCalls.like("%/scripts/%"))
        .order_by_asc(agent_message::Column::Id)
        .all(db)
        .await
        .ok()?;

    let known = crate::utils::script_paths::enumerate_script_keys();
    let mut votes: Vec<(String, usize)> = Vec::new();
    for row in rows {
        let Some(calls) = row
            .tool_calls
            .as_deref()
            .and_then(|s| serde_json::from_str::<Vec<ToolCall>>(s).ok())
        else {
            continue;
        };
        for call in calls {
            if call.function.name != "write_file" {
                continue;
            }
            let Ok(args) = serde_json::from_str::<serde_json::Value>(&call.function.arguments)
            else {
                continue;
            };
            let Some(path) = args.get("path").and_then(|v| v.as_str()) else {
                continue;
            };
            let derived =
                crate::ai_service::skill_agent::stage::evidence::script_key_of(path, &known);
            let Some(key) = derived else {
                continue;
            };
            match votes.iter_mut().find(|(k, _)| *k == key) {
                Some((_, n)) => *n += 1,
                None => votes.push((key, 1)),
            }
        }
    }

    let mut best: Option<(String, usize)> = None;
    for (key, n) in votes {
        let better = match &best {
            Some((_, bn)) => n > *bn,
            None => true,
        };
        if better {
            best = Some((key, n));
        }
    }
    best.map(|(key, _)| key)
}
