//! 剧本编辑器后端。
//!
//! PR2 新增。在这之前剧本从前端视角完全只读 —— `api/script.rs` 只有 5 个只读
//! 命令，没有任何写入/校验/重扫的能力，而 `fs` 插件的 scope 也覆盖不到
//! `<data_dir>/game_data/scripts`。
//!
//! 分层：
//!
//! | 模块 | 职责 |
//! |---|---|
//! | [`schema`] | 16 种事件及其全部字段的**单一真相源**，导出给前端驱动表单 |
//! | [`validate`] | 校验器：把引擎里的静默失败变成作者能看见的诊断 |
//! | [`commands`] | Tauri 命令层 |
//!
//! 设计约束：
//!
//! - **前端只见 JSON**。YAML 语义只存在于 Rust 一侧，不会出现两套解析行为分歧。
//! - **所有写入都是原子的**，且覆盖前留 `.bak`。
//! - **任何来自前端的路径都必须过 `utils/script_paths` 的校验**，命令层不自己拼路径。
//!
//! 通用文件读写（YAML ⇄ JSON、原子写、备份）在 `utils/yaml_file`，
//! 路径解析与安全校验在 `utils/script_paths`。

pub mod agent;
pub mod commands;
pub mod schema;
pub mod validate;

pub use commands::*;
