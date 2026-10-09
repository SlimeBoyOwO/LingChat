//! 会话标题生成：从 core.rs 拆出。

use std::sync::Arc;

use sea_orm::DatabaseConnection;

use crate::ai_service::llm::LlmClient;
use crate::ai_service::skill_agent::db;
use crate::ai_service::skill_agent::events::SkillAgentEvent;
use crate::ai_service::types::LlmMessage;

/// 首轮回复结束后后台生成会话标题（由 `run_chat` 收尾处 spawn，不阻塞回复流）。
pub(super) async fn auto_title_conversation(
    db: DatabaseConnection,
    llm: Arc<LlmClient>,
    channel: tauri::ipc::Channel<SkillAgentEvent>,
    conversation_id: i32,
    first_user_msg: String,
    reply_summary: String,
) {
    let Ok(Some(conv)) = db::get_conversation(&db, conversation_id).await else {
        return;
    };
    let titled = conv
        .title
        .as_deref()
        .map(str::trim)
        .is_some_and(|t| !t.is_empty());
    if titled {
        return;
    }
    let title = generate_title(&llm, &first_user_msg, &reply_summary).await;
    if title.is_empty() {
        return;
    }
    if db::update_conversation_title(&db, conversation_id, title.clone())
        .await
        .is_err()
    {
        return;
    }
    let _ = channel.send(SkillAgentEvent::ConversationTitle { title });
}

/// 生成 4-10 字会话标题。优先 LLM（非流式单次调用），失败/未配置时
async fn generate_title(llm: &LlmClient, first_user_msg: &str, reply_summary: &str) -> String {
    let mut candidate = String::new();
    if llm.config().is_usable() {
        let msgs = vec![
            LlmMessage::system(
                "你是会话命名助手。根据用户的提问与助手的回复，用中文生成 4-10 个字的短标题，\
                 概括这次对话的主题。只输出标题本身，不要引号、标点或任何解释。",
            ),
            LlmMessage::user(format!(
                "用户提问：{}\n助手回复：{}",
                first_user_msg, reply_summary
            )),
        ];
        match llm.complete(&msgs).await {
            Ok(text) => {
                let t = text
                    .trim()
                    .trim_matches(|c| matches!(c, '"' | '「' | '」' | '《' | '》'));
                if !t.is_empty() {
                    candidate = t.chars().take(20).collect();
                }
            },
            Err(e) => tracing::warn!("[SkillAgent] 自动生成会话标题失败，回退截取: {e}"),
        }
    }
    if candidate.is_empty() {
        candidate = first_user_msg
            .trim()
            .chars()
            .take(15)
            .collect::<String>()
            .trim_end_matches(['，', '。', '！', '？', '；', '、', ',', '.', '!', '?', ':'])
            .to_string();
    }
    candidate
}
