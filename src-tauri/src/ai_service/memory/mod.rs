//! Permanent MemoryBank runtime boundary.
//!
//! This module owns in-memory compression state and context composition only.
//! Database reads/writes remain in `MemoryRepo` and save services; it never
//! accepts a save id or database connection.
mod compactor;
mod config;
mod context;
mod coordinator;
mod runtime;

pub use config::{MemoryConfig, MemorySectionLimits};
pub use coordinator::{MemoryCoordinator, MemoryMode};
pub use runtime::{MemorySnapshot, PersistentMemorySystem};
