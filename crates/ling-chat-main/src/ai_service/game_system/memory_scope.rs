use std::collections::HashMap;

use anyhow::{Context, Result, ensure};
use ling_chat_notes::NotesStore;
use ling_chat_notes::memory::{
    MemoryChunk, MemoryDocument, MemoryScope, MemoryScopeKey, MemorySource, memory_version,
};
use sea_orm::{DatabaseConnection, EntityTrait, TransactionTrait};

use crate::ai_service::types::GameLine;
use crate::db::entities::{line::LineAttribute, role};
use crate::db::managers::save_repo::SaveRepo;

use super::persistent_memory_system::{line_visible_to_role, tool_call_ids, tool_result_call_id};

#[derive(Clone, Debug)]
pub struct ScopedMemoryLine {
    pub source: MemorySource,
    pub transaction_sources: Vec<MemorySource>,
    pub attribute: LineAttribute,
    pub sender_role_id: Option<i32>,
    pub display_name: Option<String>,
    pub content: String,
    pub action_content: Option<String>,
    pub tool_call: Option<String>,
}

#[derive(Clone, Debug)]
pub struct MemoryScopeSnapshot {
    key: MemoryScopeKey,
    lines: Vec<ScopedMemoryLine>,
    line_positions: HashMap<i32, usize>,
    notes: Vec<MemoryDocument>,
}

impl MemoryScopeSnapshot {
    pub async fn load(
        db: &DatabaseConnection,
        store: &NotesStore,
        save_id: i32,
        role_id: i32,
        role_display_name: &str,
    ) -> Result<Self> {
        ensure!(save_id > 0 && role_id > 0, "必须指定有效的角色和存档");
        ensure!(
            !role_display_name.trim().is_empty(),
            "必须提供宿主解析的角色展示名"
        );
        let transaction = db.begin().await?;
        ensure!(
            role::Entity::find_by_id(role_id)
                .one(&transaction)
                .await?
                .is_some(),
            "角色不存在"
        );
        let history = SaveRepo::get_memory_gameline_list(&transaction, save_id).await?;
        transaction.commit().await?;
        let store = store.clone();
        let role_display_name = role_display_name.to_owned();
        tokio::task::spawn_blocking(move || {
            Self::from_history(&store, save_id, role_id, &role_display_name, history)
        })
        .await?
    }

    fn from_history(
        store: &NotesStore,
        save_id: i32,
        role_id: i32,
        role_display_name: &str,
        mut history: Vec<GameLine>,
    ) -> Result<Self> {
        let history_version = Self::history_version_for(save_id, role_id, &mut history)?;
        let lines = visible_history(&history, role_id)?;
        let line_positions = lines
            .iter()
            .enumerate()
            .map(|(index, line)| {
                let MemorySource::Line { line_id, .. } = line.source else {
                    unreachable!()
                };
                (line_id, index)
            })
            .collect();
        let mut notes = store
            .load(role_display_name)?
            .iter()
            .map(|note| MemoryDocument::from_note(role_id, note))
            .collect::<std::result::Result<Vec<_>, _>>()?;
        notes.sort_by(|left, right| left.id.cmp(&right.id));
        ensure!(
            notes.windows(2).all(|pair| pair[0].id != pair[1].id),
            "笔记包含重复 ID"
        );
        let notes_version = memory_version(&("notes-v1", role_id, role_display_name, &notes))?;
        Ok(Self {
            key: MemoryScopeKey {
                save_id,
                role_id,
                history_version,
                notes_version,
            },
            lines,
            line_positions,
            notes,
        })
    }

    pub fn key(&self) -> &MemoryScopeKey {
        &self.key
    }

    pub(crate) fn history_version_for(
        save_id: i32,
        role_id: i32,
        history: &mut [GameLine],
    ) -> Result<String> {
        for line in history.iter_mut() {
            line.perceived_role_ids.sort_unstable();
            line.perceived_role_ids.dedup();
        }
        let versions = history
            .iter()
            .map(line_version)
            .collect::<Result<Vec<_>>>()?;
        Ok(memory_version(&("history-v2", save_id, role_id, versions))?)
    }

    pub fn lines(&self) -> &[ScopedMemoryLine] {
        &self.lines
    }

    pub fn notes(&self) -> &[MemoryDocument] {
        &self.notes
    }

    pub fn allows_document(&self, document: &MemoryDocument) -> bool {
        if document.validate().is_err() || document.role_id != self.key.role_id {
            return false;
        }
        match document.scope {
            MemoryScope::Save { save_id } => {
                save_id == self.key.save_id && self.allows_sources(&document.sources)
            },
            MemoryScope::Role => self.notes.iter().any(|note| note == document),
        }
    }

    pub fn allows_chunk(&self, document: &MemoryDocument, chunk: &MemoryChunk) -> bool {
        self.allows_document(document)
            && chunk.validate_for(document).is_ok()
            && (document.scope == MemoryScope::Role || self.allows_sources(&chunk.sources))
    }

    pub fn eligible_documents<'document>(
        &self,
        candidates: impl IntoIterator<Item = &'document MemoryDocument>,
    ) -> Vec<&'document MemoryDocument> {
        candidates
            .into_iter()
            .filter(|document| self.allows_document(document))
            .collect()
    }

    fn allows_sources(&self, sources: &[MemorySource]) -> bool {
        sources.iter().all(|source| {
            let MemorySource::Line { line_id, .. } = source else {
                return false;
            };
            self.line_positions.get(line_id).is_some_and(|index| {
                let current = &self.lines[*index];
                current.source == *source
                    && current
                        .transaction_sources
                        .iter()
                        .all(|required| sources.contains(required))
            })
        })
    }
}

fn visible_history(history: &[GameLine], role_id: i32) -> Result<Vec<ScopedMemoryLine>> {
    let mut visible = Vec::new();
    let mut index = 0;
    while index < history.len() {
        let line = &history[index];
        if matches!(line.attribute(), LineAttribute::Assistant) {
            match tool_call_ids(line) {
                Ok(Some(mut pending)) => {
                    let mut end = index + 1;
                    let mut allowed = line_visible_to_memory(line, role_id);
                    while end < history.len() && !pending.is_empty() {
                        let result = &history[end];
                        if !matches!(result.attribute(), LineAttribute::Tool) {
                            break;
                        }
                        allowed &= line_visible_to_memory(result, role_id);
                        let Some(call_id) = tool_result_call_id(result) else {
                            break;
                        };
                        if !pending.remove(&call_id)
                            || serde_json::from_str::<serde_json::Value>(result.content())?
                                .get("result")
                                .is_none()
                        {
                            allowed = false;
                            break;
                        }
                        end += 1;
                    }
                    if allowed && pending.is_empty() {
                        append_sources(&mut visible, &history[index..end])?;
                    }
                    index = end;
                    continue;
                },
                Err(()) => {
                    index += 1;
                    continue;
                },
                Ok(None) => {},
            }
        }
        if !matches!(line.attribute(), LineAttribute::Tool) && line_visible_to_memory(line, role_id)
        {
            append_sources(&mut visible, std::slice::from_ref(line))?;
        }
        index += 1;
    }
    Ok(visible)
}

fn line_visible_to_memory(line: &GameLine, role_id: i32) -> bool {
    line_visible_to_role(line, role_id)
        || (!matches!(line.attribute(), LineAttribute::System)
            && line
                .base
                .action_content
                .as_deref()
                .is_some_and(|action| !action.trim().is_empty())
            && (line.sender_role_id() == Some(role_id)
                || line.perceived_role_ids.contains(&role_id)))
}

fn line_version(line: &GameLine) -> Result<String> {
    Ok(memory_version(&(
        "line-v2",
        line.base.id,
        line.attribute(),
        line.sender_role_id(),
        &line.base.display_name,
        line.content(),
        &line.base.action_content,
        &line.base.tool_call,
        &line.perceived_role_ids,
    ))?)
}

fn append_sources(visible: &mut Vec<ScopedMemoryLine>, lines: &[GameLine]) -> Result<()> {
    let sources = lines
        .iter()
        .map(|line| {
            Ok(MemorySource::Line {
                line_id: line.base.id.context("索引素材必须已经取得持久化台词 ID")?,
                version: line_version(line)?,
            })
        })
        .collect::<Result<Vec<_>>>()?;
    for (line, source) in lines.iter().zip(&sources) {
        visible.push(ScopedMemoryLine {
            source: source.clone(),
            transaction_sources: sources.clone(),
            attribute: line.attribute().clone(),
            sender_role_id: line.sender_role_id(),
            display_name: line.base.display_name.clone(),
            content: line.content().to_owned(),
            action_content: line.base.action_content.clone(),
            tool_call: line.base.tool_call.clone(),
        });
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ai_service::types::LineBase;
    use ling_chat_notes::memory::{EventStatus, MemoryKind};

    fn line(id: i32, attribute: LineAttribute, content: &str) -> GameLine {
        GameLine::from_base(
            LineBase {
                id: Some(id),
                attribute: attribute.into(),
                content: content.into(),
                ..Default::default()
            },
            vec![2],
        )
    }

    fn transaction() -> Vec<GameLine> {
        let mut request = line(1, LineAttribute::Assistant, "查好了再告诉你。");
        request.base.tool_call = Some(
            serde_json::json!([
                {"id":"a","type":"function","function":{"name":"first","arguments":"{}"}},
                {"id":"b","type":"function","function":{"name":"second","arguments":"{}"}}
            ])
            .to_string(),
        );
        vec![
            request,
            line(
                2,
                LineAttribute::Tool,
                r#"{"tool_call_id":"b","result":"第二个结果"}"#,
            ),
            line(
                3,
                LineAttribute::Tool,
                r#"{"tool_call_id":"a","result":"第一个结果"}"#,
            ),
        ]
    }

    fn scope(history: Vec<GameLine>) -> Result<MemoryScopeSnapshot> {
        let dir = tempfile::tempdir().unwrap();
        MemoryScopeSnapshot::from_history(&NotesStore::new(dir.path()), 101, 2, "角色甲", history)
    }

    fn raw_document(scope: &MemoryScopeSnapshot) -> MemoryDocument {
        let mut document = MemoryDocument {
            id: "raw-tool".into(),
            kind: MemoryKind::Raw,
            role_id: 2,
            scope: MemoryScope::Save { save_id: 101 },
            content: "工具事务".into(),
            tags: vec![],
            recorded_at: None,
            event_time: None,
            event_status: EventStatus::Unknown,
            version: "doc-v1".into(),
            sources: scope
                .lines()
                .iter()
                .map(|line| line.source.clone())
                .collect(),
        };
        document.version = document.content_version().unwrap();
        document
    }

    #[test]
    fn complete_parallel_tools_require_all_sources_in_documents_and_chunks() {
        let scope = scope(transaction()).unwrap();
        assert_eq!(scope.lines().len(), 3);
        let mut document = raw_document(&scope);
        assert!(
            scope
                .lines()
                .iter()
                .all(|line| line.transaction_sources == document.sources)
        );
        assert!(scope.allows_document(&document));
        let mut chunk = MemoryChunk {
            id: "chunk-1".into(),
            document_id: document.id.clone(),
            ordinal: 0,
            content: document.content.clone(),
            version: "chunk-v1".into(),
            sources: document.sources.clone(),
        };
        chunk.version = chunk.content_version().unwrap();
        assert!(scope.allows_chunk(&document, &chunk));
        chunk.sources.pop();
        chunk.version = chunk.content_version().unwrap();
        assert!(!scope.allows_chunk(&document, &chunk));
        document.sources.pop();
        document.version = document.content_version().unwrap();
        assert!(!scope.allows_document(&document));
    }

    #[test]
    fn malformed_incomplete_or_partly_hidden_tools_never_enter_sources() {
        let complete = transaction();
        let mut malformed_legacy = complete.clone();
        malformed_legacy[0].base.tool_call = None;
        malformed_legacy[0].base.content = "[ {\"id\":\"a\",\"function\":".into();
        assert!(scope(malformed_legacy).unwrap().lines().is_empty());
        let mut hidden_result = complete.clone();
        hidden_result[1].perceived_role_ids.clear();
        let mut hidden_request = complete.clone();
        hidden_request[0].perceived_role_ids.clear();
        let mut malformed_request = complete.clone();
        malformed_request[0].base.tool_call = Some("[invalid".into());
        let mut duplicate_request = complete.clone();
        let mut calls: serde_json::Value =
            serde_json::from_str(duplicate_request[0].base.tool_call.as_ref().unwrap()).unwrap();
        calls[1]["id"] = "a".into();
        duplicate_request[0].base.tool_call = Some(calls.to_string());
        let mut duplicate_result = complete.clone();
        duplicate_result[2].base.content = duplicate_result[1].base.content.clone();
        let mut missing_result = complete.clone();
        missing_result[1].base.content = r#"{"tool_call_id":"b"}"#.into();
        let mut malformed_result = complete.clone();
        malformed_result[1].base.content = "invalid json".into();
        let mut interrupted = complete.clone();
        interrupted.insert(1, line(4, LineAttribute::User, "先停一下。"));
        let cases = vec![
            complete[..2].to_vec(),
            complete[1..].to_vec(),
            hidden_result,
            hidden_request,
            malformed_request,
            duplicate_request,
            duplicate_result,
            missing_result,
            malformed_result,
            interrupted,
        ];
        for (case, history) in cases.into_iter().enumerate() {
            let scope = scope(history).unwrap();
            assert!(
                scope
                    .lines()
                    .iter()
                    .all(|line| line.attribute != LineAttribute::Tool && line.tool_call.is_none()),
                "工具用例 {case} 未被排除"
            );
        }
    }

    #[test]
    fn legacy_protocol_is_supported_without_indexing_system_or_private_thoughts() {
        let mut history = transaction();
        let calls = history[0].base.tool_call.take().unwrap();
        history[0].base.content = format!("{calls}\n\n旧格式工具说明");
        history[0].base.thinking = Some("内部思考，不是经历".into());
        history.insert(0, line(4, LineAttribute::System, "角色系统规则"));
        let scope = scope(history).unwrap();
        assert_eq!(scope.lines().len(), 3);
        assert!(scope.lines().iter().all(
            |line| !line.content.contains("角色系统规则") && !line.content.contains("内部思考")
        ));
        assert!(scope.allows_document(&raw_document(&scope)));
    }

    #[test]
    fn unsaved_sources_are_rejected_and_perception_order_is_canonical() {
        let mut history = vec![line(1, LineAttribute::User, "旧事实")];
        history[0].perceived_role_ids = vec![3, 2, 2];
        let original = scope(history.clone()).unwrap();
        history[0].perceived_role_ids = vec![2, 3];
        assert_eq!(original.key(), scope(history.clone()).unwrap().key());
        history[0].base.id = None;
        assert!(scope(history).is_err());
    }
}
