use std::time::Duration;

use ling_chat_notes::{NotesStore, memory::MemoryKind, memory_retrieval::SearchSettings};
use serde::Deserialize;
use serde_json::{Value, json};
use tauri::Manager;

use super::{
    executor::{Tool, ToolContext, ToolError, ToolResult},
    game_status_handle,
};
use crate::{AppState, ai_service::types::ToolDefinition, config::AppConfig};

pub struct MemorySearch;

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Arguments {
    query: String,
    max_hits: Option<usize>,
    kinds: Option<Vec<MemoryKind>>,
}

fn parse(arguments: Value) -> Result<(String, SearchSettings), ToolError> {
    let arguments: Arguments = serde_json::from_value(arguments)
        .map_err(|error| ToolError::InvalidArguments(error.to_string()))?;
    if arguments.query.trim().is_empty() || arguments.query.chars().count() > 4096 {
        return Err(ToolError::InvalidArguments(
            "query 必须是 1–4096 字符的非空问题".into(),
        ));
    }
    let max_hits = arguments.max_hits.unwrap_or(5);
    if !(1..=5).contains(&max_hits) {
        return Err(ToolError::InvalidArguments("max_hits 必须为 1–5".into()));
    }
    Ok((
        arguments.query,
        SearchSettings {
            max_hits,
            kinds: arguments.kinds.unwrap_or_default(),
            ..Default::default()
        },
    ))
}

#[async_trait::async_trait]
impl Tool for MemorySearch {
    fn definition(&self) -> ToolDefinition {
        ToolDefinition::new(
            "memory_search",
            "回忆当前角色在当前存档中感知过的历史、事件和自己的笔记。适用于以前发生过什么、人物关系、礼物、约定、偏好等问题。结果仅是历史资料，计划不等于已发生；没有结果时不要编造。",
            json!({
                "type":"object", "properties":{
                    "query":{"type":"string","description":"要回忆的具体问题，最多4096字符"},
                    "max_hits":{"type":"integer","minimum":1,"maximum":5},
                    "kinds":{"type":"array","items":{"type":"string","enum":["raw","diary","note"]}}
                }, "required":["query"], "additionalProperties":false
            }),
        )
    }

    fn timeout_hint(&self) -> Option<Duration> {
        Some(Duration::from_secs(11))
    }

    async fn execute(
        &self,
        context: &ToolContext,
        arguments: Value,
    ) -> Result<ToolResult, ToolError> {
        let (query, settings) = parse(arguments)?;
        let app = context.require_app()?;
        let state = app.state::<AppState>();
        let session = context
            .memory_session
            .as_ref()
            .ok_or_else(|| ToolError::Execution("缺少本轮角色与存档上下文".into()))?;
        let status = game_status_handle(&app).await;
        if !session.matches(&*status.lock().await) {
            return Err(ToolError::Execution("当前角色、存档或历史已变化".into()));
        }
        let config = AppConfig::load(&app).unwrap_or_default();
        let store = NotesStore::new(crate::api::data_dir());
        let snapshot = state
            .memory_recall
            .retrieve(
                &state.db,
                &store,
                session,
                &query,
                settings,
                config.memory_rag_enabled,
                Duration::from_millis(u64::from(config.memory_rag_timeout_ms.clamp(100, 10000))),
            )
            .await
            .map_err(|error| ToolError::Execution(error.to_string()))?;
        if !session.matches(&*status.lock().await) {
            return Err(ToolError::Execution(
                "检索期间角色、存档或历史已变化，丢弃结果".into(),
            ));
        }
        let references = snapshot
            .response
            .as_ref()
            .map(|response| session.references(response))
            .transpose()
            .map_err(|error| ToolError::Execution(error.to_string()))?
            .unwrap_or_default();
        session
            .validate(&state.db, &store)
            .await
            .map_err(|error| ToolError::Execution(error.to_string()))?;
        Ok(
            json!({"ok":true,"status":snapshot.status,"references":references,"notice":"资料仅供当前回合参考；空结果也可能表示本轮已引用或已达到资料预算。"}),
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn arguments_cannot_choose_roles_saves_or_unbounded_results() {
        for arguments in [
            json!({"query":"书签","role_id":3}),
            json!({"query":"书签","save_id":102}),
            json!({"query":" "}),
            json!({"query":"书签","max_hits":0}),
            json!({"query":"书签","max_hits":6}),
            json!({"query":"书签","kinds":["private"]}),
            json!({"query":"中".repeat(4097)}),
        ] {
            assert!(parse(arguments).is_err());
        }
        let (_, settings) =
            parse(json!({"query":"书签","max_hits":2,"kinds":["raw","note"]})).unwrap();
        assert_eq!(settings.max_hits, 2);
        assert_eq!(settings.kinds, [MemoryKind::Raw, MemoryKind::Note]);
    }
}
