//! 管理单轮聊天的历史引用、共享预算和来源复核，供自动回忆与工具检索共用。

use std::collections::HashSet;
use std::sync::{Arc, Mutex, RwLock};
use std::time::Duration;

use anyhow::{Result, ensure};
use ling_chat_notes::{
    NotesStore,
    memory::{MemoryScopeKey, MemorySource, memory_version},
    memory_retrieval::{MatchExplanation, SearchSettings},
};
use sea_orm::DatabaseConnection;
use serde::Serialize;
use tokio::sync::Semaphore;

use super::{
    index::{IndexMode, MemoryEncoder, MemoryIndexer},
    scope::MemoryScopeSnapshot,
    search::{MemorySearchResponse, MemorySearcher},
};
use crate::ai_service::game_system::game_status::GameStatus;
use crate::ai_service::game_system::status_ops::{
    TemporaryMemory, TemporaryPlacement, apply_temporary,
};
use crate::ai_service::types::{GameLine, LlmMessage};
use crate::db::managers::save_repo::SaveRepo;
use crate::utils::prompt::PromptRole;

pub const EPHEMERAL_MEMORY_RESULT: &str = "{\"ephemeral\":true,\"message\":\"历史检索资料仅供当轮参考，未写入聊天历史；需要时请重新检索。\"}";

/// 固定生成任务开始时的角色和历史，防止旧回复被写入新会话。
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RecallStamp {
    pub save_id: Option<i32>,
    pub role_id: i32,
    pub display_name: String,
    pub generation: u64,
    identity_id: Option<String>,
    revision: u64,
    lines: Vec<String>,
}

impl RecallStamp {
    pub fn capture(status: &GameStatus, display_name: String) -> Result<Self> {
        Ok(Self {
            save_id: status.active_save_id,
            role_id: status
                .current_role_id
                .ok_or_else(|| anyhow::anyhow!("当前没有对话角色"))?,
            display_name,
            generation: status.preview_generation,
            identity_id: status.player.identity_id.clone(),
            revision: 0,
            lines: line_fingerprints(&status.line_list)?,
        })
    }

    pub fn matches(&self, status: &GameStatus) -> bool {
        status.current_role_id == Some(self.role_id) && self.matches_history(status)
    }

    pub fn matches_history(&self, status: &GameStatus) -> bool {
        // 允许在本轮末尾追加新台词，但之前的历史必须保持不变。
        status.active_save_id == self.save_id
            && status.player.identity_id == self.identity_id
            && status.preview_generation == self.generation
            && status.line_list.len() >= self.lines.len()
            && line_fingerprints(&status.line_list[..self.lines.len()])
                .is_ok_and(|lines| lines == self.lines)
    }
}

// 只比较会影响对话的内容与感知权限，保存时补齐数据库 ID 不应误判为历史变化。
fn line_fingerprints(lines: &[GameLine]) -> Result<Vec<String>> {
    lines
        .iter()
        .map(|line| {
            let mut perceived = line.perceived_role_ids.clone();
            perceived.sort_unstable();
            perceived.dedup();
            Ok(memory_version(&(
                line.attribute(),
                line.sender_role_id(),
                &line.base.identity_id,
                &line.base.round_id,
                &line.base.display_name,
                line.content(),
                &line.base.action_content,
                &line.base.tool_call,
                perceived,
            ))?)
        })
        .collect()
}

// 自动回忆与多次工具检索共用本轮预算，避免重复注入和无限扩大上下文。
#[derive(Default)]
struct RecallBudget {
    seen: HashSet<String>,
    characters: usize,
    hits: usize,
    scope: Option<MemoryScopeKey>,
    sources: Vec<MemorySource>,
}

pub struct RecallSession {
    stamp: RwLock<RecallStamp>,
    budget: Mutex<RecallBudget>,
    recent_context: RwLock<String>,
    base_context: RwLock<Vec<LlmMessage>>,
}

impl std::fmt::Debug for RecallSession {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("RecallSession")
            .finish_non_exhaustive()
    }
}

impl RecallSession {
    pub fn new(stamp: RecallStamp, messages: &[LlmMessage]) -> Arc<Self> {
        // 保存注入检索资料之前的上下文，笔记修改后才能撤掉旧引用并继续对话。
        Arc::new(Self {
            stamp: RwLock::new(stamp),
            budget: Mutex::new(RecallBudget::default()),
            recent_context: RwLock::new(Self::recent(messages)),
            base_context: RwLock::new(messages.to_vec()),
        })
    }

    fn recent(messages: &[LlmMessage]) -> String {
        // 只截取少量近期对话，供“那次”“它”这类指代查询补充线索。
        let recent = messages
            .iter()
            .rev()
            .filter(|message| {
                matches!(message.role.as_str(), "user" | "assistant")
                    && message.tool_calls.is_none()
            })
            .take(4)
            .collect::<Vec<_>>()
            .into_iter()
            .rev()
            .map(|message| message.content.as_str())
            .collect::<Vec<_>>()
            .join("\n");
        recent
            .chars()
            .rev()
            .take(240)
            .collect::<Vec<_>>()
            .into_iter()
            .rev()
            .collect()
    }

    pub fn stamp(&self) -> RecallStamp {
        self.stamp
            .read()
            .unwrap_or_else(|error| error.into_inner())
            .clone()
    }

    pub fn matches(&self, status: &GameStatus) -> bool {
        self.stamp
            .read()
            .unwrap_or_else(|error| error.into_inner())
            .matches(status)
    }

    pub fn is_current(&self, stamp: &RecallStamp) -> bool {
        *self.stamp.read().unwrap_or_else(|error| error.into_inner()) == *stamp
    }

    pub fn base_context(&self) -> Vec<LlmMessage> {
        self.base_context
            .read()
            .unwrap_or_else(|error| error.into_inner())
            .clone()
    }

    pub fn append_context(&self, message: LlmMessage) {
        self.base_context
            .write()
            .unwrap_or_else(|error| error.into_inner())
            .push(message);
    }

    pub fn prepend_context(&self, message: LlmMessage) {
        self.base_context
            .write()
            .unwrap_or_else(|error| error.into_inner())
            .insert(0, message);
    }

    /// 写笔记前先作废旧引用；递增版本让队列中尚未发布的旧回复也失效。
    pub fn invalidate_references(&self) {
        let mut stamp = self
            .stamp
            .write()
            .unwrap_or_else(|error| error.into_inner());
        stamp.revision = stamp.revision.wrapping_add(1);
        let mut budget = self
            .budget
            .lock()
            .unwrap_or_else(|error| error.into_inner());
        budget.scope = None;
        budget.sources.clear();
    }

    /// 切换角色后用新上下文重置引用和预算，避免继承旧角色的记忆。
    pub fn rebase(&self, mut stamp: RecallStamp, messages: &[LlmMessage]) {
        let mut current = self
            .stamp
            .write()
            .unwrap_or_else(|error| error.into_inner());
        stamp.revision = current.revision.wrapping_add(1);
        *current = stamp;
        *self
            .budget
            .lock()
            .unwrap_or_else(|error| error.into_inner()) = RecallBudget::default();
        *self
            .recent_context
            .write()
            .unwrap_or_else(|error| error.into_inner()) = Self::recent(messages);
        *self
            .base_context
            .write()
            .unwrap_or_else(|error| error.into_inner()) = messages.to_vec();
    }

    /// 在发起请求、落库和发布前复核；本轮正常追加历史不算旧来源失效。
    pub async fn validate(&self, db: &DatabaseConnection, store: &NotesStore) -> Result<()> {
        loop {
            let (stamp, expected, sources) = {
                let stamp = self.stamp.read().unwrap_or_else(|error| error.into_inner());
                let budget = self
                    .budget
                    .lock()
                    .unwrap_or_else(|error| error.into_inner());
                (stamp.clone(), budget.scope.clone(), budget.sources.clone())
            };
            let Some(expected) = expected else {
                // 本轮还没有引用资料，就没有来源需要复核。
                return Ok(());
            };
            ensure!(
                stamp.save_id == Some(expected.save_id) && stamp.role_id == expected.role_id,
                "记忆所属会话已变化"
            );
            let current = MemoryScopeSnapshot::load(
                db,
                store,
                expected.save_id,
                expected.role_id,
                &stamp.display_name,
            )
            .await?;
            // 等待数据库期间上下文可能已重建，用新版本重新检查。
            if !self.is_current(&stamp) {
                continue;
            }
            if current.key() != &expected {
                // 笔记变化先停止使用旧资料；历史只追加时还要逐条检查已引用来源。
                ensure!(
                    current.key().notes_version == expected.notes_version,
                    "笔记已变化，取消旧请求"
                );
                let saved = SaveRepo::get_memory_gameline_list(db, expected.save_id).await?;
                if !self.is_current(&stamp) {
                    continue;
                }
                ensure!(
                    line_fingerprints(&saved)?.starts_with(&stamp.lines),
                    "记忆历史已变化，取消旧请求"
                );
                ensure!(
                    sources
                        .iter()
                        .all(
                            |source| current.lines().iter().any(|line| line.source == *source
                                && line
                                    .transaction_sources
                                    .iter()
                                    .all(|required| sources.contains(required)))
                        ),
                    "记忆来源已失效，取消旧请求"
                );
                ensure!(self.stamp() == stamp, "来源复核期间会话已变化");
                let mut budget = self
                    .budget
                    .lock()
                    .unwrap_or_else(|error| error.into_inner());
                ensure!(
                    budget.scope.as_ref() == Some(&expected),
                    "来源复核期间检索范围已变化"
                );
                budget.scope = Some(current.key().clone());
            }
            return Ok(());
        }
    }

    pub fn references(&self, response: &MemorySearchResponse) -> Result<Vec<RecallReference>> {
        let stamp = self.stamp.read().unwrap_or_else(|error| error.into_inner());
        ensure!(
            stamp.save_id == Some(response.scope_key.save_id)
                && stamp.role_id == response.scope_key.role_id,
            "检索结果不属于当前角色与存档"
        );
        let mut budget = self
            .budget
            .lock()
            .unwrap_or_else(|error| error.into_inner());
        if let Some(scope) = &budget.scope {
            ensure!(scope == &response.scope_key, "记忆来源在本轮内已变化");
        }
        let mut references = Vec::new();
        // 同一段正文只注入一次，条数和字符数均按整个回合累计。
        for hit in &response.result.hits {
            let key = memory_version(&hit.hit.chunk.content)?;
            let characters = hit.reference.chars().count();
            if budget.hits >= 5 || budget.characters + characters > 1800 || !budget.seen.insert(key)
            {
                continue;
            }
            budget.characters += characters;
            budget.hits += 1;
            for source in &hit.hit.chunk.sources {
                if matches!(source, MemorySource::Line { .. }) && !budget.sources.contains(source) {
                    budget.sources.push(source.clone());
                }
            }
            references.push(RecallReference {
                reference: hit.reference.clone(),
                explanation: hit.explanation.clone(),
            });
        }
        if !references.is_empty() {
            budget.scope = Some(response.scope_key.clone());
        }
        Ok(references)
    }
}

#[derive(Clone, Debug, Serialize)]
pub struct RecallReference {
    pub reference: String,
    pub explanation: MatchExplanation,
}

#[derive(Clone, Debug, Serialize)]
pub struct RecallDebugSnapshot {
    pub status: String,
    pub save_id: Option<i32>,
    pub role_id: i32,
    pub response: Option<MemorySearchResponse>,
    #[serde(skip)]
    pub stamp: RecallStamp,
}

pub struct MemoryRecall {
    indexer: MemoryIndexer,
    searcher: MemorySearcher,
    slots: Arc<Semaphore>,
    latest: Mutex<Option<RecallDebugSnapshot>>,
}

impl MemoryRecall {
    pub fn new(encoder: Arc<dyn MemoryEncoder>) -> Arc<Self> {
        Arc::new(Self {
            indexer: MemoryIndexer::new(encoder.clone()),
            searcher: MemorySearcher::new(encoder),
            slots: Arc::new(Semaphore::new(1)),
            latest: Mutex::new(None),
        })
    }

    pub fn debug_snapshot(&self) -> Option<RecallDebugSnapshot> {
        self.latest
            .lock()
            .unwrap_or_else(|error| error.into_inner())
            .clone()
    }

    pub async fn retrieve(
        self: &Arc<Self>,
        db: &DatabaseConnection,
        store: &NotesStore,
        session: &RecallSession,
        query: &str,
        settings: SearchSettings,
        enabled: bool,
        timeout: Duration,
    ) -> Result<RecallDebugSnapshot> {
        settings.validate()?;
        ling_chat_notes::memory_retrieval::SearchQuery::new(query, "")?;
        let stamp = session.stamp();
        let mut snapshot = RecallDebugSnapshot {
            status: "disabled".into(),
            save_id: stamp.save_id,
            role_id: stamp.role_id,
            response: None,
            stamp: stamp.clone(),
        };
        if enabled && !query.trim().is_empty() {
            if let Some(save_id) = stamp.save_id {
                // 一次只跑一个索引检索任务；忙时立即降级，避免每条消息都排队。
                if let Ok(permit) = self.slots.clone().try_acquire_owned() {
                    let service = self.clone();
                    let db = db.clone();
                    let store = store.clone();
                    let query = query.to_owned();
                    let recent = session
                        .recent_context
                        .read()
                        .unwrap_or_else(|error| error.into_inner())
                        .clone();
                    // 超时只结束本轮等待，后台任务仍持有许可，防止重复堆积推理任务。
                    let worker = tokio::spawn(async move {
                        let _permit = permit;
                        let history = SaveRepo::get_memory_gameline_list(&db, save_id).await?;
                        let saved = line_fingerprints(&history)?;
                        ensure!(
                            stamp.lines.starts_with(&saved),
                            "当前对话与持久化历史不一致，跳过旧索引"
                        );
                        service
                            .indexer
                            .sync(
                                &db,
                                &store,
                                save_id,
                                stamp.role_id,
                                &stamp.display_name,
                                IndexMode::Incremental,
                            )
                            .await?;
                        service
                            .searcher
                            .search(
                                &db,
                                &store,
                                save_id,
                                stamp.role_id,
                                &stamp.display_name,
                                &query,
                                &recent,
                                &settings,
                            )
                            .await
                    });
                    match tokio::time::timeout(timeout, worker).await {
                        Ok(Ok(Ok(response))) => {
                            snapshot.status = "ready".into();
                            snapshot.response = Some(response);
                        },
                        Ok(Ok(Err(error))) => {
                            tracing::warn!("记忆检索降级: {error}");
                            snapshot.status = "unavailable".into();
                        },
                        Ok(Err(error)) => {
                            tracing::warn!("记忆检索任务失败: {error}");
                            snapshot.status = "unavailable".into();
                        },
                        // 等待超时后不再使用这次结果，后台任务结束后才释放许可。
                        Err(_) => snapshot.status = "timeout".into(),
                    }
                } else {
                    snapshot.status = "busy".into();
                }
            } else {
                snapshot.status = "no_active_save".into();
            }
        } else if enabled {
            snapshot.status = "empty_query".into();
        }
        *self
            .latest
            .lock()
            .unwrap_or_else(|error| error.into_inner()) = Some(snapshot.clone());
        Ok(snapshot)
    }
}

/// 历史资料作为请求级旁白并入本次用户问题，不写回原始聊天历史。
pub fn inject_references(messages: &mut Vec<LlmMessage>, references: &[RecallReference]) {
    if references.is_empty() {
        return;
    }
    let body = references
        .iter()
        .map(|reference| reference.reference.as_str())
        .collect::<Vec<_>>()
        .join("\n");
    let text =
        PromptRole::Narrator.build_prompt(&format!("以下为历史资料，仅供本轮参考：\n{body}"));
    apply_temporary(
        messages,
        &[TemporaryMemory {
            text,
            placement: TemporaryPlacement::PrependToUser,
        }],
    );
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn references_are_merged_into_the_existing_user_message() {
        let mut messages = vec![LlmMessage::system("persona"), LlmMessage::user("question")];
        inject_references(
            &mut messages,
            &[RecallReference {
                reference: "过去发生过的事".into(),
                explanation: MatchExplanation::default(),
            }],
        );

        assert_eq!(
            messages
                .iter()
                .filter(|message| message.role == "system")
                .count(),
            1
        );
        assert_eq!(
            messages
                .iter()
                .filter(|message| message.role == "user")
                .count(),
            1
        );
        assert!(
            messages[1]
                .content
                .starts_with("{旁白: 以下为历史资料，仅供本轮参考：")
        );
        assert!(messages[1].content.ends_with("question"));
    }
}
