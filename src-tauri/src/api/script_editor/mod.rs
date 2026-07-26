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
//! | [`paths`] | 剧本 key ⇄ 磁盘路径、三种布局枚举、路径穿越防护、名称合法性 |
//! | [`io`] | YAML ⇄ JSON、原子写、`.bak` 备份、章节文档归一 |
//! | [`schema`] | 16 种事件及其全部字段的**单一真相源**，导出给前端驱动表单 |
//! | [`validate`] | 校验器：把引擎里的静默失败变成作者能看见的诊断 |
//! | [`commands`] | Tauri 命令层 |
//!
//! 设计约束：
//!
//! - **前端只见 JSON**。YAML 语义只存在于 Rust 一侧，不会出现两套解析行为分歧。
//! - **所有写入都是原子的**，且覆盖前留 `.bak`。
//! - **删除不是真删**，章节进 `Chapters/.trash/`，整包进 `game_data/.script_trash/`。
//! - **任何来自前端的路径都必须过 [`paths`] 的校验**，命令层不自己拼路径。

pub mod commands;
pub mod io;
pub mod paths;
pub mod schema;
pub mod validate;

pub use commands::*;
