use std::collections::HashMap;

use crate::ai_service::llm::LlmSlot;
use crate::ai_service::types::GameMemoryBank;

use super::{MemoryConfig, PersistentMemorySystem};

/// Whether permanent compaction may run while contexts are rebuilt.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum MemoryMode {
    Normal,
    Preview,
}

/// Owns the per-role permanent-memory runtimes and no other game resources.
///
/// Role loading, DB access, TTS and short-term `GameRole.memory` remain outside
/// this type. The coordinator deliberately exposes only runtime operations so
/// `GameRoleManager` cannot again become the MemoryBank state-machine owner.
pub struct MemoryCoordinator {
    config: MemoryConfig,
    mode: MemoryMode,
    runtimes: HashMap<i32, PersistentMemorySystem>,
}

impl MemoryCoordinator {
    pub fn new(config: MemoryConfig) -> Self {
        Self {
            config,
            mode: MemoryMode::Normal,
            runtimes: HashMap::new(),
        }
    }

    pub fn config(&self) -> MemoryConfig {
        self.config
    }

    pub fn is_preview(&self) -> bool {
        self.mode == MemoryMode::Preview
    }

    pub fn set_mode(&mut self, mode: MemoryMode) {
        self.mode = mode;
    }

    pub fn runtime(&self, role_id: i32) -> Option<&PersistentMemorySystem> {
        self.runtimes.get(&role_id)
    }

    pub(crate) fn runtimes(&self) -> impl Iterator<Item = &PersistentMemorySystem> {
        self.runtimes.values()
    }

    pub(crate) fn runtime_role_ids(&self) -> impl Iterator<Item = i32> + '_ {
        self.runtimes.keys().copied()
    }

    #[cfg(test)]
    pub(crate) fn insert_for_test(&mut self, role_id: i32, runtime: PersistentMemorySystem) {
        self.runtimes.insert(role_id, runtime);
    }

    /// Lazily install (or hot-enable) the unique runtime for one loaded role.
    pub fn ensure(
        &mut self,
        role_id: i32,
        bank: &GameMemoryBank,
        display_name: &str,
        llm: LlmSlot,
    ) {
        let llm_ready = llm.try_read().map(|slot| slot.is_some()).unwrap_or(false);
        if let Some(runtime) = self.runtimes.get_mut(&role_id) {
            runtime.set_enabled(self.config.enabled && llm_ready);
            return;
        }
        if self.config.enabled && !llm_ready {
            tracing::warn!(
                "MemoryBank: role_id={} 永久记忆已开启但 LLM 槽位为空；保留 disabled runtime 等待下次角色重载",
                role_id
            );
        }
        self.runtimes.insert(
            role_id,
            PersistentMemorySystem::new(
                role_id,
                bank,
                llm,
                self.config.enabled && llm_ready,
                self.config.update_interval,
                self.config.recent_window,
                self.config.limits,
                display_name,
            ),
        );
    }

    pub fn clear(&mut self) {
        self.runtimes.clear();
    }

    /// Synchronously detach runtimes that belong only to a discarded resource
    /// scope. The caller must abort/join the returned task owners only after
    /// releasing its GameStatus/RoleManager lock.
    ///
    /// Detaching first ensures no later snapshot/save can discover a temporary
    /// runtime while cancellation is pending.
    pub(crate) fn detach_not_in(
        &mut self,
        retained_role_ids: &std::collections::HashSet<i32>,
    ) -> Vec<PersistentMemorySystem> {
        let removed_ids: Vec<i32> = self
            .runtimes
            .keys()
            .filter(|role_id| !retained_role_ids.contains(role_id))
            .copied()
            .collect();
        let removed: Vec<PersistentMemorySystem> = removed_ids
            .into_iter()
            .filter_map(|role_id| self.runtimes.remove(&role_id))
            .collect();
        removed
    }
}
