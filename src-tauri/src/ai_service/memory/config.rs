/// Per-section character limits. Zero means no truncation.
///
/// Limits affect the runtime LLM view and compaction input only; the persisted
/// MemoryBank JSON remains unchanged.
#[derive(Clone, Copy, Debug)]
pub struct MemorySectionLimits {
    pub short_term: usize,
    pub long_term: usize,
    pub user_info: usize,
    pub promises: usize,
}

impl Default for MemorySectionLimits {
    fn default() -> Self {
        Self {
            short_term: 500,
            long_term: 2000,
            user_info: 800,
            promises: 800,
        }
    }
}

/// Runtime-only permanent-memory settings collected at the boundary.
///
/// This intentionally has no persistence or save-id state: persistence is
/// target-save specific and remains owned by the save layer.
#[derive(Clone, Copy, Debug)]
pub struct MemoryConfig {
    pub enabled: bool,
    pub update_interval: usize,
    pub recent_window: usize,
    pub limits: MemorySectionLimits,
}
