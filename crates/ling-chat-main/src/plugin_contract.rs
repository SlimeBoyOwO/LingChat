//! 宿主与插件系统之间的窄接口契约。
//!
//! 目的：让宿主业务侧（main）**不反向依赖整个插件实现**。宿主需要知道的插件能力
//! 收敛到这里两样东西——
//! - [`PluginResourceSource`]：状态容器只持有一个 trait 对象，用于合并插件资源到
//!   各内容列表；真正的实现在插件侧（`PluginManager`）。
//! - [`ReplyHook`]：宿主生成 `ai:reply` 时经全局回调转发给订阅的插件，宿主不直接
//!   引用派发逻辑。
//!
//! [`ResourceKind`] 与 [`PluginResourceEntry`] 是被两侧共同使用的值类型，也放在此处，
//! 插件侧从本模块 `use` 它们。

use std::path::PathBuf;
use std::sync::{Arc, OnceLock};

use serde::{Deserialize, Serialize};

use crate::ai_service::message_system::responses::ReplyResponse;

/// 插件可携带的资源类型（与 game_data 下同名子目录一一对应）。
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ResourceKind {
    Characters,
    Scripts,
    Musics,
    Backgrounds,
    Ambients,
}

impl ResourceKind {
    /// 插件内与游戏目录同名同构的子目录名。
    pub fn subdir(&self) -> &'static str {
        match self {
            Self::Characters => "characters",
            Self::Scripts => "scripts",
            Self::Musics => "musics",
            Self::Backgrounds => "backgrounds",
            Self::Ambients => "ambients",
        }
    }

    pub fn as_str(&self) -> &'static str {
        self.subdir()
    }
}

/// 单条插件资源条目（前端资源管理列表 & 各内容列表合并共用）。
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "snake_case")]
pub struct PluginResourceEntry {
    pub kind: ResourceKind,
    /// 定位 key：角色 = folder 名；剧本 = script_name；图/音 = 文件名（含扩展名）。
    pub key: String,
    /// 显示名：角色 = settings.yml title；剧本 = script_name；图/音 = 文件 stem。
    pub name: String,
    /// 资源绝对路径（目录或文件）。
    pub path: PathBuf,
    pub plugin_id: String,
    /// 与游戏现有资源同名冲突（列表中被游戏版压制）。
    pub conflict: bool,
    /// 已被玩家软删除隐藏。
    pub hidden: bool,
}

impl PluginResourceEntry {
    /// 软删除标记值：`"<kind>/<key>"`。
    pub fn hidden_mark(&self) -> String {
        format!("{}/{}", self.kind.subdir(), self.key)
    }
}

/// 宿主侧只读访问插件资源的窄接口（由插件侧的 `PluginManager` 实现）。
#[async_trait::async_trait]
pub trait PluginResourceSource: Send + Sync {
    /// 文件类资源（背景图 / 音乐 / 环境音）的可见条目：过滤隐藏、游戏同名、插件间重复。
    async fn visible_file_entries(&self, kind: ResourceKind) -> Vec<PluginResourceEntry>;
}

/// 回复信号回调：宿主每产出一条 `ai:reply` 就调用一次，用于转发给订阅了
/// `ai_reply` 的插件。回调内部自行 `spawn` 到 async runtime。
pub type ReplyHook = Arc<dyn Fn(&ReplyResponse) + Send + Sync>;

static REPLY_HOOK: OnceLock<ReplyHook> = OnceLock::new();

/// 启动时登记回复回调（外壳 setup 注入，闭包捕获 `AppHandle`）。
pub fn set_reply_hook(hook: ReplyHook) {
    let _ = REPLY_HOOK.set(hook);
}

/// 通知一条回复。未登记回调时静默跳过（如单元测试环境）。
pub fn notify_reply(resp: &ReplyResponse) {
    if let Some(hook) = REPLY_HOOK.get() {
        hook(resp);
    }
}
