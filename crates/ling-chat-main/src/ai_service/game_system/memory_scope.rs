//! 从当前存档的有效历史链建立角色可见范围，索引和缓存都以此校验来源。

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
    // 工具请求和所有返回值共用完整来源集合，不能只保留其中一部分。
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
        // 沿存档链读取原始历史，不使用已压缩摘要，也不混入其他存档分支。
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
        // 笔记按宿主解析的角色名读取，模型不能自行指定其他角色目录。
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
        // 感知角色集合只比较成员，避免顺序或重复项引发无意义的索引重建。
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

    /// 同时检查文档内容版本、归属和当前来源，撤回或编辑后旧文档立即失效。
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
                    // 并行工具返回可以乱序，但必须齐全且全部对当前角色可见。
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
        // 孤立工具返回不进入记忆，防止缺少请求时误读其含义。
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
        "line-v3",
        line.base.id,
        line.attribute(),
        line.sender_role_id(),
        &line.base.identity_id,
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
