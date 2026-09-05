//! Skill Agent 会话/消息持久化。

use chrono::Local;
use sea_orm::{
    ActiveModelTrait, ColumnTrait, DatabaseConnection, EntityTrait, QueryFilter, QueryOrder, Set,
};

use crate::ai_service::skill_agent::events::Usage;
use crate::ai_service::types::LlmMessage;
use crate::db::entities::skill_agent_conversation::{self, Entity as ConvEntity};
use crate::db::entities::skill_agent_message::{self, Entity as MsgEntity};

// ==================== 会话 ====================

/// 新建会话。只记录创建时的剧本 key，不存剧本内容快照。
pub async fn create_conversation(
    db: &DatabaseConnection,
    title: Option<String>,
    script_key: Option<String>,
) -> Result<i32, String> {
    let now = Local::now().naive_local();
    let model = skill_agent_conversation::ActiveModel {
        id: sea_orm::NotSet,
        title: Set(title),
        script_key: Set(script_key),
        created_at: Set(now),
        updated_at: Set(now),
    };
    let res = model
        .insert(db)
        .await
        .map_err(|e| format!("创建会话失败: {}", e))?;
    Ok(res.id)
}

/// 全部会话，按最近更新倒序。
pub async fn list_conversations(
    db: &DatabaseConnection,
) -> Result<Vec<skill_agent_conversation::Model>, String> {
    ConvEntity::find()
        .order_by_desc(skill_agent_conversation::Column::UpdatedAt)
        .all(db)
        .await
        .map_err(|e| format!("查询会话列表失败: {}", e))
}

pub async fn get_conversation(
    db: &DatabaseConnection,
    id: i32,
) -> Result<Option<skill_agent_conversation::Model>, String> {
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
    let mut am: skill_agent_conversation::ActiveModel = m.into();
    am.updated_at = Set(Local::now().naive_local());
    am.update(db)
        .await
        .map_err(|e| format!("更新会话失败: {}", e))?;
    Ok(())
}

/// 更新会话标题（用户重命名 / 首轮自动命名共用）。
/// 顺带刷新 updated_at，保持会话列表「按最近更新倒序」的排序正确。
pub async fn update_conversation_title(
    db: &DatabaseConnection,
    id: i32,
    title: String,
) -> Result<(), String> {
    let Some(m) = get_conversation(db, id).await? else {
        return Ok(());
    };
    let mut am: skill_agent_conversation::ActiveModel = m.into();
    am.title = Set(Some(title));
    am.updated_at = Set(Local::now().naive_local());
    am.update(db)
        .await
        .map_err(|e| format!("更新会话标题失败: {}", e))?;
    Ok(())
}

/// 删除会话及其全部消息。
pub async fn delete_conversation(db: &DatabaseConnection, id: i32) -> Result<(), String> {
    MsgEntity::delete_many()
        .filter(skill_agent_message::Column::ConversationId.eq(id))
        .exec(db)
        .await
        .map_err(|e| format!("删除会话消息失败: {}", e))?;
    ConvEntity::delete_by_id(id)
        .exec(db)
        .await
        .map_err(|e| format!("删除会话失败: {}", e))?;
    Ok(())
}

// ==================== 消息 ====================

/// 插入一条消息（OpenAI 格式；空 content 存 NULL）。
/// `reasoning` 为 assistant 的思考链（仅展示用途，不参与 LLM 上下文）。
/// `usage` 为产生该消息那一轮 LLM 调用的 token 用量（仅 assistant 消息携带，
/// 用于用量统计持久化；tool/user 消息传 None）。
/// 返回新消息的 DB id（供「回溯删除」定位删除起点）。
pub async fn insert_message(
    db: &DatabaseConnection,
    conversation_id: i32,
    msg: &LlmMessage,
    reasoning: Option<&str>,
    usage: Option<&Usage>,
) -> Result<i32, String> {
    let now = Local::now().naive_local();
    let model = skill_agent_message::ActiveModel {
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

/// 某会话的全部消息（按时间升序）。
pub async fn list_messages(
    db: &DatabaseConnection,
    conversation_id: i32,
) -> Result<Vec<skill_agent_message::Model>, String> {
    MsgEntity::find()
        .filter(skill_agent_message::Column::ConversationId.eq(conversation_id))
        .order_by_asc(skill_agent_message::Column::Id)
        .all(db)
        .await
        .map_err(|e| format!("查询消息失败: {}", e))
}

/// 清空某会话的消息。
pub async fn clear_messages(db: &DatabaseConnection, conversation_id: i32) -> Result<(), String> {
    MsgEntity::delete_many()
        .filter(skill_agent_message::Column::ConversationId.eq(conversation_id))
        .exec(db)
        .await
        .map_err(|e| format!("清空消息失败: {}", e))?;
    Ok(())
}

/// 删除某会话中 id >= `from_id` 的全部消息（「回溯」：把对话回退到该消息之前）。
///
/// 消息按插入顺序分配自增 id，删除一段后 LLM 上下文从剩余消息重建，
/// 不会残留「无 user 开头的 assistant 回复」导致上下文结构错乱。
pub async fn delete_messages_from(
    db: &DatabaseConnection,
    conversation_id: i32,
    from_id: i32,
) -> Result<(), String> {
    MsgEntity::delete_many()
        .filter(skill_agent_message::Column::ConversationId.eq(conversation_id))
        .filter(skill_agent_message::Column::Id.gte(from_id))
        .exec(db)
        .await
        .map_err(|e| format!("回溯删除消息失败: {}", e))?;
    Ok(())
}

/// DB 消息模型 → LLM 消息。
pub fn message_to_llm(m: &skill_agent_message::Model) -> LlmMessage {
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
