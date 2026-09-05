use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use tokio::sync::oneshot;

use crate::ai_service::game_system::memory_builder::MemoryBuilder;
use crate::ai_service::llm::{LlmSlot, slot_snapshot};
use crate::ai_service::types::{GameLine, GameMemoryBank};

use super::MemorySectionLimits;
use super::compactor::{init_prompts, update_section};
use super::context::{line_visible_to_role, render_short_term, render_system_memory};

const RETRY_COOLDOWN: Duration = Duration::from_secs(60);

/// The only mutable state for one role's permanent-memory runtime.
///
/// A single synchronous mutex makes snapshot capture and every state transition
/// atomic: a reader can never observe a bank/revision pair from different
/// commits, nor a running job that belongs to a different epoch. No operation
/// awaits while holding this lock.
struct RoleMemoryState {
    bank: GameMemoryBank,
    bank_revision: u64,
    history_epoch: u64,
    next_job_id: u64,
    job: JobState,
    last_failure: Option<FailureState>,
}

impl RoleMemoryState {
    fn new(bank: GameMemoryBank) -> Self {
        Self {
            bank,
            bank_revision: 0,
            history_epoch: 0,
            next_job_id: 0,
            job: JobState::Idle,
            last_failure: None,
        }
    }
}

enum JobState {
    Idle,
    Running {
        identity: u64,
        epoch: u64,
        target_idx: i64,
        handle: Option<tokio::task::JoinHandle<()>>,
    },
}

struct FailureState {
    epoch: u64,
    count: u32,
    retry_after: Instant,
}

/// Immutable input captured under the one state lock and processed outside it.
struct CompactionJob {
    identity: u64,
    epoch: u64,
    target_idx: i64,
    old_bank: GameMemoryBank,
}

/// The unique runtime owner of one role's MemoryBank. DB I/O and LLM calls are
/// intentionally outside this type's state lock.
pub struct PersistentMemorySystem {
    role_id: i32,
    ai_name: String,
    llm: LlmSlot,
    state: Arc<Mutex<RoleMemoryState>>,
    pub enabled: bool,
    update_interval: usize,
    recent_window: usize,
    section_limits: MemorySectionLimits,
    section_prompts: HashMap<String, String>,
}

#[derive(Clone, Debug)]
pub struct MemorySnapshot {
    pub role_id: i32,
    pub bank: GameMemoryBank,
    pub revision: u64,
    pub updating: bool,
}

impl PersistentMemorySystem {
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        role_id: i32,
        initial_bank: &GameMemoryBank,
        llm: LlmSlot,
        enabled: bool,
        update_interval: usize,
        recent_window: usize,
        limits: MemorySectionLimits,
        display_name: &str,
    ) -> Self {
        Self {
            role_id,
            ai_name: display_name.to_owned(),
            llm,
            state: Arc::new(Mutex::new(RoleMemoryState::new(initial_bank.clone()))),
            enabled,
            update_interval,
            recent_window,
            section_limits: limits,
            section_prompts: init_prompts(),
        }
    }

    pub fn is_enabled(&self) -> bool {
        self.enabled
    }

    pub fn set_enabled(&mut self, enabled: bool) {
        self.enabled = enabled;
    }

    pub fn snapshot(&self) -> MemorySnapshot {
        let state = lock_state(&self.state);
        MemorySnapshot {
            role_id: self.role_id,
            bank: state.bank.clone(),
            revision: state.bank_revision,
            updating: matches!(state.job, JobState::Running { .. }),
        }
    }

    pub fn is_updating(&self) -> bool {
        matches!(lock_state(&self.state).job, JobState::Running { .. })
    }

    #[cfg(test)]
    pub(crate) fn history_revision_for_test(&self) -> u64 {
        lock_state(&self.state).history_epoch
    }

    pub async fn wait_until_idle(&self, timeout: Duration) -> bool {
        let deadline = tokio::time::Instant::now() + timeout;
        loop {
            if !self.is_updating() {
                return true;
            }
            if tokio::time::Instant::now() >= deadline {
                return false;
            }
            tokio::time::sleep(Duration::from_millis(1)).await;
        }
    }

    /// Take the task handle while holding the state lock, then abort/join it
    /// outside the lock. The completion guard uses the same lock, so this order
    /// cannot deadlock with Drop or a commit.
    pub async fn abort_and_wait(&self) {
        let handle = {
            let mut state = lock_state(&self.state);
            match &mut state.job {
                JobState::Running { handle, .. } => handle.take(),
                JobState::Idle => None,
            }
        };
        if let Some(handle) = handle {
            handle.abort();
            let _ = handle.await;
        }
        // If cancellation happened before the spawned future was first polled,
        // its completion guard never ran. Clear only the matching no-handle job.
        let mut state = lock_state(&self.state);
        if matches!(state.job, JobState::Running { handle: None, .. }) {
            state.job = JobState::Idle;
        }
    }

    /// Invalidates work but retains a valid committed bank (used while entering
    /// preview before a separate canonical history is installed).
    pub fn invalidate_history(&self) {
        let mut state = lock_state(&self.state);
        state.history_epoch = state.history_epoch.wrapping_add(1);
        state.last_failure = None;
    }

    /// A rewrite touching the processed prefix invalidates both a running job
    /// and its committed summary. The bank, pointer, revision, epoch, job and
    /// failure metadata are all observed under one lock.
    pub async fn rewrite_from(&self, from_idx: usize) {
        let mut state = lock_state(&self.state);
        state.history_epoch = state.history_epoch.wrapping_add(1);
        state.last_failure = None;
        if from_idx < state.bank.meta.last_processed_global_idx.max(0) as usize {
            state.bank = GameMemoryBank::default();
            state.bank_revision = state.bank_revision.wrapping_add(1);
        }
    }

    pub async fn reset_from(&self, bank: &GameMemoryBank) {
        let mut state = lock_state(&self.state);
        state.history_epoch = state.history_epoch.wrapping_add(1);
        state.last_failure = None;
        state.bank = bank.clone();
        state.bank_revision = state.bank_revision.wrapping_add(1);
    }

    pub async fn get_slice_start_index(&self, all_lines: &[GameLine]) -> usize {
        let processed = lock_state(&self.state)
            .bank
            .meta
            .last_processed_global_idx
            .max(0)
            .min(all_lines.len() as i64) as usize;
        if processed == 0 || self.recent_window == 0 {
            return processed;
        }
        let mut start = processed;
        let mut remaining = self.recent_window;
        while start > 0 {
            start -= 1;
            if line_visible_to_role(&all_lines[start], self.role_id) {
                remaining -= 1;
                if remaining == 0 {
                    break;
                }
            }
        }
        start
    }

    pub async fn get_system_memory_text(&self) -> String {
        render_system_memory(&lock_state(&self.state).bank, self.section_limits)
    }

    pub async fn get_short_term_user_text(&self) -> String {
        render_short_term(&lock_state(&self.state).bank, self.section_limits)
    }

    /// Capture the complete job identity under the state lock; all expensive
    /// rendering and every LLM await occur after that lock has been released.
    pub fn check_and_trigger_auto_update(&self, all_lines: &[GameLine]) {
        if !self.enabled {
            return;
        }
        let (last_idx, epoch) = {
            let mut state = lock_state(&self.state);
            if matches!(state.job, JobState::Running { .. }) {
                return;
            }
            if state.last_failure.as_ref().is_some_and(|failure| {
                failure.epoch == state.history_epoch && Instant::now() < failure.retry_after
            }) {
                return;
            }
            let idx = state.bank.meta.last_processed_global_idx;
            let idx = if idx < 0 || idx as usize > all_lines.len() {
                state.bank.meta.last_processed_global_idx = 0;
                state.bank.meta.updated_at = now_str();
                state.bank_revision = state.bank_revision.wrapping_add(1);
                0
            } else {
                idx as usize
            };
            (idx, state.history_epoch)
        };

        let (chat_text, visible_count) = self.build_chat_text_and_count(&all_lines[last_idx..]);
        if visible_count < self.update_interval {
            return;
        }
        let target_idx = all_lines.len() as i64;
        if chat_text.trim().is_empty() {
            let mut state = lock_state(&self.state);
            if state.history_epoch == epoch && matches!(state.job, JobState::Idle) {
                state.bank.meta.last_processed_global_idx = target_idx;
                state.bank.meta.updated_at = now_str();
                state.bank_revision = state.bank_revision.wrapping_add(1);
            }
            return;
        }

        let job = {
            let mut state = lock_state(&self.state);
            if state.history_epoch != epoch || matches!(state.job, JobState::Running { .. }) {
                return;
            }
            state.next_job_id = state.next_job_id.wrapping_add(1);
            let job = CompactionJob {
                identity: state.next_job_id,
                epoch: state.history_epoch,
                target_idx,
                old_bank: state.bank.clone(),
            };
            state.job = JobState::Running {
                identity: job.identity,
                epoch: job.epoch,
                target_idx,
                handle: None,
            };
            job
        };
        self.spawn_background_update(chat_text, job);
    }

    fn spawn_background_update(&self, chat_text: String, job: CompactionJob) {
        let state = self.state.clone();
        let llm_slot = self.llm.clone();
        let prompts = self.section_prompts.clone();
        let role_id = self.role_id;
        let ai_name = self.ai_name.clone();
        let limits = self.section_limits;
        let (start_tx, start_rx) = oneshot::channel();
        let job_identity = job.identity;
        let handle = tokio::spawn(async move {
            let _completion = JobCompletionGuard {
                state: state.clone(),
                identity: job.identity,
            };
            let _ = start_rx.await;
            let llm = match slot_snapshot(&llm_slot).await {
                Some(llm) => llm,
                None => {
                    record_failure(&state, job.identity, job.epoch);
                    tracing::warn!("MemoryBank: role_id={} LLM 槽位为空，跳过本次更新", role_id);
                    return;
                },
            };
            let old = &job.old_bank.data;
            let (st, lt, ui, pr) = tokio::join!(
                update_section(
                    &llm,
                    &prompts,
                    &chat_text,
                    "short_term",
                    &old.short_term,
                    limits.short_term,
                    &ai_name
                ),
                update_section(
                    &llm,
                    &prompts,
                    &chat_text,
                    "long_term",
                    &old.long_term,
                    limits.long_term,
                    &ai_name
                ),
                update_section(
                    &llm,
                    &prompts,
                    &chat_text,
                    "user_info",
                    &old.user_info,
                    limits.user_info,
                    &ai_name
                ),
                update_section(
                    &llm,
                    &prompts,
                    &chat_text,
                    "promises",
                    &old.promises,
                    limits.promises,
                    &ai_name
                ),
            );
            let results = [st, lt, ui, pr];
            if let Some((key, error)) = results.iter().enumerate().find_map(|(i, value)| {
                value
                    .as_ref()
                    .err()
                    .map(|e| (["short_term", "long_term", "user_info", "promises"][i], e))
            }) {
                tracing::warn!(
                    "MemoryBank: role_id={} 分段压缩失败 (key={}): {}",
                    role_id,
                    key,
                    error
                );
                record_failure(&state, job.identity, job.epoch);
                return;
            }
            let [st, lt, ui, pr] = results;
            let sections = [
                st.expect("checked"),
                lt.expect("checked"),
                ui.expect("checked"),
                pr.expect("checked"),
            ];
            if !commit(&state, &job, sections) {
                tracing::info!("MemoryBank: role_id={} 丢弃过期压缩结果", role_id);
            }
        });

        // Publish the handle before releasing the start barrier. Therefore
        // abort_and_wait always either owns a real handle or observes completion.
        let mut runtime = lock_state(&self.state);
        if let JobState::Running {
            identity,
            handle: slot,
            ..
        } = &mut runtime.job
        {
            if *identity == job_identity {
                *slot = Some(handle);
                let _ = start_tx.send(());
                return;
            }
        }
        // A concurrent invalidation cannot remove Running, but keep this safe
        // if that contract changes: do not leak an untracked LLM task.
        handle.abort();
    }

    fn build_chat_text_and_count(&self, lines: &[GameLine]) -> (String, usize) {
        let visible_count = lines
            .iter()
            .filter(|line| line_visible_to_role(line, self.role_id))
            .count();
        if visible_count == 0 {
            return (String::new(), 0);
        }
        let built = MemoryBuilder::new(self.role_id).build(lines);
        let chunks: Vec<String> = built
            .iter()
            .filter_map(|message| {
                let content = message.content.trim();
                if content.is_empty() || message.role == "system" {
                    None
                } else if message.role == "assistant" {
                    Some(format!("{}: {}", self.ai_name, content))
                } else if message.role == "user" {
                    Some(format!("User: {}", content))
                } else {
                    Some(format!("{}: {}", message.role, content))
                }
            })
            .collect();
        let chat_text = chunks.join("\n");
        if chat_text.is_empty() {
            (chat_text, visible_count)
        } else {
            (format!("{chat_text}\n"), visible_count)
        }
    }
}

struct JobCompletionGuard {
    state: Arc<Mutex<RoleMemoryState>>,
    identity: u64,
}

impl Drop for JobCompletionGuard {
    fn drop(&mut self) {
        let mut state = lock_state(&self.state);
        if matches!(state.job, JobState::Running { identity, .. } if identity == self.identity) {
            state.job = JobState::Idle;
        }
    }
}

fn record_failure(state: &Arc<Mutex<RoleMemoryState>>, identity: u64, epoch: u64) {
    let mut state = lock_state(state);
    if matches!(state.job, JobState::Running { identity: current, epoch: current_epoch, .. } if current == identity && current_epoch == epoch)
        && state.history_epoch == epoch
    {
        let count = state
            .last_failure
            .as_ref()
            .filter(|failure| failure.epoch == epoch)
            .map_or(0, |failure| failure.count)
            .saturating_add(1);
        state.last_failure = Some(FailureState {
            epoch,
            count,
            retry_after: Instant::now() + RETRY_COOLDOWN,
        });
    }
}

fn commit(state: &Arc<Mutex<RoleMemoryState>>, job: &CompactionJob, sections: [String; 4]) -> bool {
    let mut state = lock_state(state);
    if !matches!(state.job, JobState::Running { identity, epoch, target_idx, .. } if identity == job.identity && epoch == job.epoch && target_idx == job.target_idx)
        || state.history_epoch != job.epoch
    {
        return false;
    }
    let [short_term, long_term, user_info, promises] = sections;
    state.bank.data.short_term = short_term;
    state.bank.data.long_term = long_term;
    state.bank.data.user_info = user_info;
    state.bank.data.promises = promises;
    state.bank.meta.last_processed_global_idx = job.target_idx;
    state.bank.meta.updated_at = now_str();
    state.bank_revision = state.bank_revision.wrapping_add(1);
    state.last_failure = None;
    true
}

fn lock_state(state: &Arc<Mutex<RoleMemoryState>>) -> std::sync::MutexGuard<'_, RoleMemoryState> {
    state
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner())
}

fn now_str() -> String {
    chrono::Local::now().format("%Y-%m-%d %H:%M:%S").to_string()
}

// These regressions intentionally exercise the new production runtime rather
// than recreating its state with atomics/enums. They replace the invariants
// formerly tested in the deleted legacy runtime.
#[cfg(all(test, feature = "memory-test-api"))]
mod tests {
    use std::sync::Arc;
    use std::time::Duration;

    use super::*;
    use crate::ai_service::types::{LineAttributeExt, LineBase};
    use crate::db::entities::line::LineAttribute;
    use crate::memory_test_api::scripted_provider::ScriptedProvider;

    fn line(attribute: LineAttribute, sender: Option<i32>, perceived: Vec<i32>) -> GameLine {
        GameLine::from_base(
            LineBase {
                content: "line".into(),
                attribute: LineAttributeExt(attribute),
                sender_role_id: sender,
                ..Default::default()
            },
            perceived,
        )
    }

    fn runtime(
        provider: ScriptedProvider,
        recent_window: usize,
        processed: i64,
    ) -> PersistentMemorySystem {
        let mut bank = GameMemoryBank::default();
        bank.meta.last_processed_global_idx = processed;
        PersistentMemorySystem::new(
            7,
            &bank,
            provider.slot(),
            true,
            1,
            recent_window,
            MemorySectionLimits::default(),
            "AI",
        )
    }

    #[tokio::test]
    async fn recent_window_counts_only_role_visible_non_system_lines() {
        let lines = vec![
            line(LineAttribute::System, Some(7), vec![7]),
            line(LineAttribute::Assistant, Some(7), vec![]),
            line(LineAttribute::Assistant, Some(8), vec![8]),
            line(LineAttribute::User, Some(0), vec![7]),
            line(LineAttribute::Assistant, Some(8), vec![8]),
            line(LineAttribute::User, Some(0), vec![7]),
        ];
        assert_eq!(
            runtime(ScriptedProvider::default(), 2, 5)
                .get_slice_start_index(&lines)
                .await,
            1
        );
    }

    #[tokio::test]
    async fn recent_window_falls_back_to_zero_when_visible_history_is_short() {
        let mut empty = line(LineAttribute::User, Some(0), vec![7]);
        empty.base.content = "   ".into();
        let lines = vec![
            line(LineAttribute::System, Some(7), vec![7]),
            empty,
            line(LineAttribute::Assistant, Some(7), vec![]),
            line(LineAttribute::User, Some(0), vec![7]),
        ];
        assert_eq!(
            runtime(ScriptedProvider::default(), 5, 3)
                .get_slice_start_index(&lines)
                .await,
            0
        );
    }

    #[tokio::test]
    async fn default_short_term_placeholder_is_not_injected() {
        assert_eq!(
            runtime(ScriptedProvider::default(), 30, 0)
                .get_short_term_user_text()
                .await,
            ""
        );
    }

    #[tokio::test]
    async fn zero_recent_window_starts_at_processed_boundary() {
        let lines = vec![
            line(LineAttribute::System, Some(7), vec![7]),
            line(LineAttribute::Assistant, Some(7), vec![]),
            line(LineAttribute::User, Some(0), vec![7]),
        ];
        assert_eq!(
            runtime(ScriptedProvider::default(), 0, lines.len() as i64)
                .get_slice_start_index(&lines)
                .await,
            lines.len()
        );
    }

    #[tokio::test]
    async fn actual_runtime_commits_all_sections_and_pointer_atomically() {
        let provider = ScriptedProvider::default();
        let memory = runtime(provider.clone(), 0, 0);
        let lines = vec![line(LineAttribute::User, Some(0), vec![7])];
        memory.check_and_trigger_auto_update(&lines);
        assert!(memory.wait_until_idle(Duration::from_secs(2)).await);
        let snapshot = memory.snapshot();
        assert_eq!(snapshot.bank.data.short_term, "[scripted:short_term]");
        assert_eq!(snapshot.bank.data.long_term, "[scripted:long_term]");
        assert_eq!(snapshot.bank.data.user_info, "[scripted:user_info]");
        assert_eq!(snapshot.bank.data.promises, "[scripted:promises]");
        assert_eq!(snapshot.bank.meta.last_processed_global_idx, 1);
        assert_eq!(snapshot.revision, 1);
        assert_eq!(provider.calls(), 4);
    }

    #[tokio::test]
    async fn stale_task_cannot_commit_or_leave_failure_cooldown() {
        let provider = ScriptedProvider {
            delay_ms: 30,
            ..Default::default()
        };
        let memory = runtime(provider.clone(), 0, 0);
        let lines = vec![line(LineAttribute::User, Some(0), vec![7])];
        memory.check_and_trigger_auto_update(&lines);
        while provider.calls() == 0 {
            tokio::task::yield_now().await;
        }
        memory.rewrite_from(0).await;
        assert!(memory.wait_until_idle(Duration::from_secs(2)).await);
        let stale = memory.snapshot();
        assert_eq!(stale.bank, GameMemoryBank::default());
        assert_eq!(stale.revision, 0);

        // A failure cooldown from the stale epoch must not suppress the new
        // history. The second production task really calls all four sections.
        memory.check_and_trigger_auto_update(&lines);
        assert!(memory.wait_until_idle(Duration::from_secs(2)).await);
        assert_eq!(memory.snapshot().bank.meta.last_processed_global_idx, 1);
        assert_eq!(provider.calls(), 8);
    }

    #[tokio::test]
    async fn failure_cooldown_is_epoch_scoped_in_the_production_runtime() {
        let provider = ScriptedProvider {
            fail_section: Some("promises".into()),
            ..Default::default()
        };
        let memory = runtime(provider.clone(), 0, 0);
        let lines = vec![line(LineAttribute::User, Some(0), vec![7])];
        memory.check_and_trigger_auto_update(&lines);
        assert!(memory.wait_until_idle(Duration::from_secs(2)).await);
        assert_eq!(provider.calls(), 4);
        assert_eq!(memory.snapshot().bank.meta.last_processed_global_idx, 0);

        // Same history is in cooldown and cannot trigger another four calls.
        memory.check_and_trigger_auto_update(&lines);
        tokio::time::sleep(Duration::from_millis(5)).await;
        assert_eq!(provider.calls(), 4);

        // A new history epoch must not inherit that stale failure cooldown.
        memory.invalidate_history();
        memory.check_and_trigger_auto_update(&lines);
        assert!(memory.wait_until_idle(Duration::from_secs(2)).await);
        assert_eq!(provider.calls(), 8);
    }

    #[tokio::test]
    async fn abort_and_panic_both_release_one_flight_for_retry() {
        let delayed = ScriptedProvider {
            delay_ms: 100,
            ..Default::default()
        };
        let slot = delayed.clone().slot();
        let memory = PersistentMemorySystem::new(
            7,
            &GameMemoryBank::default(),
            slot.clone(),
            true,
            1,
            0,
            MemorySectionLimits::default(),
            "AI",
        );
        let lines = vec![line(LineAttribute::User, Some(0), vec![7])];
        memory.check_and_trigger_auto_update(&lines);
        while delayed.calls() == 0 {
            tokio::task::yield_now().await;
        }
        memory.abort_and_wait().await;
        assert!(!memory.snapshot().updating);

        let panicking = ScriptedProvider {
            panic_section: Some("promises".into()),
            ..Default::default()
        };
        *slot.write().await = Some(Arc::new(crate::ai_service::llm::LlmClient::new(
            crate::ai_service::llm::LlmConfig {
                provider: "scripted".into(),
                model: "scripted".into(),
                api_key: String::new(),
                base_url: String::new(),
                timeout_secs: 30,
                temperature: None,
                top_p: None,
                enable_thinking: false,
                reasoning_effort: None,
                fast_mode: false,
            },
            reqwest::Client::new(),
            Box::new(panicking),
        )));
        memory.check_and_trigger_auto_update(&lines);
        assert!(memory.wait_until_idle(Duration::from_secs(2)).await);
        assert!(!memory.snapshot().updating);

        // The failure state is scoped to an epoch; a rewrite/new history must
        // clear its cooldown before it can retry.
        memory.invalidate_history();

        // Replacing the real LLM slot proves a prior abort/panic did not leave
        // RoleMemoryState in Running or permanently suppress a retry.
        let successful = ScriptedProvider::default();
        *slot.write().await = Some(Arc::new(crate::ai_service::llm::LlmClient::new(
            crate::ai_service::llm::LlmConfig {
                provider: "scripted".into(),
                model: "scripted".into(),
                api_key: String::new(),
                base_url: String::new(),
                timeout_secs: 30,
                temperature: None,
                top_p: None,
                enable_thinking: false,
                reasoning_effort: None,
                fast_mode: false,
            },
            reqwest::Client::new(),
            Box::new(successful.clone()),
        )));
        memory.check_and_trigger_auto_update(&lines);
        assert!(memory.wait_until_idle(Duration::from_secs(2)).await);
        assert_eq!(memory.snapshot().bank.meta.last_processed_global_idx, 1);
        assert_eq!(successful.calls(), 4);
    }

    #[tokio::test]
    async fn rewrite_boundary_resets_only_processed_prefix() {
        let mut bank = GameMemoryBank::default();
        bank.data.long_term = "valid prefix".into();
        bank.meta.last_processed_global_idx = 4;
        let memory = PersistentMemorySystem::new(
            7,
            &bank,
            ScriptedProvider::default().slot(),
            true,
            1,
            0,
            MemorySectionLimits::default(),
            "AI",
        );
        memory.rewrite_from(4).await;
        assert_eq!(memory.snapshot().bank, bank);
        assert_eq!(memory.snapshot().revision, 0);
        memory.rewrite_from(2).await;
        assert_eq!(memory.snapshot().bank, GameMemoryBank::default());
        assert_eq!(memory.snapshot().revision, 1);
    }
}
