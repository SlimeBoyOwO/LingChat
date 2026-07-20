//! archive 模块的集成测试入口。
//!
//! 该 crate 仅依赖 ling_chat 的公共 API；`#[cfg(test)] mod tests` 已从
//! `utils/archive.rs` 迁出，避免业务代码与测试共用私有命名空间。

mod format_detection;
mod helpers;
mod resolve;
mod safety;
mod sanitize;
mod sevenz_pipeline;
mod zip_pipeline;
