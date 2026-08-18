//! 会话快照存储键 — 记录上一次运行时的 UI 状态（服装、音乐、环境音等），
//! 启动时自动恢复。与 keys.rs 不同，这里的键存储的是"快照"而非"设置"。

// ========== 从 keys.rs 迁入（原 game.* → session.*） ==========
/// 上次游玩的角色 ID（启动时自动恢复）
pub const LAST_CHARACTER_ID: &str = "session.last_character_id";
/// 当前场景 ID（会话内的实时值，AI 切换场景也会写这里）
pub const LAST_SCENE_ID: &str = "session.last_scene_id";
/// 基准场景 ID —— 用户自己选定的那个「家」。
///
/// 与 [`LAST_SCENE_ID`] 的区别是**谁写的**：只有用户在设置里点选场景才会更新这里，
/// AI 在对话中切换或生成场景只动 LAST_SCENE_ID。这样剧情跑到天文馆之后，
/// `scene_return` 和下次启动都还知道该回到哪里。
pub const BASE_SCENE_ID: &str = "session.base_scene_id";
/// 场景感知开关（切换场景时是否自动产生旁白）
pub const SCENE_AWARENESS_ENABLED: &str = "session.scene_awareness_enabled";

// ========== 角色服装（按角色 ID 存储） ==========
/// 构建角色服装键：session.last_clothes.<role_id>
pub fn last_clothes_key(role_id: i32) -> String {
    format!("session.last_clothes.{}", role_id)
}

// ========== 音乐 / 环境音 ==========
/// 上次播放的背景音乐曲目路径（"None" 表示无）
pub const LAST_BGM_TRACK: &str = "session.last_bgm_track";
/// 背景音乐是否暂停
pub const LAST_BGM_PAUSED: &str = "session.last_bgm_paused";
/// 背景音乐播放模式（loop-single / loop-list / random）
pub const LAST_BGM_MODE: &str = "session.last_bgm_mode";
/// 环境音轨道列表（JSON 数组字符串）
pub const LAST_AMBIENT_TRACKS: &str = "session.last_ambient_tracks";
