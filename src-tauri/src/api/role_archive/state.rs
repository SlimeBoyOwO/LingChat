//! 角色压缩包导入/导出的全局并发状态与 RAII 守卫。
//!
//! 实体已上移到 [`crate::utils::archive`]，与插件压缩包导入共用同一份实例：
//! 全局只允许一个导入任务，前端进度条同一时刻也仅展示一个。

pub use crate::utils::archive::{ArchiveImportState as RoleArchiveState, ImportTaskEntry};

pub(crate) use crate::utils::archive::{ImportingGuard, TaskRemoveGuard};
