//! 阶段推导与剧本包骨架：阶段由文件系统事实推导，不落库、不强制流程。

pub mod budget;
pub mod evidence;
pub mod prompt;
pub mod queue;

use self::evidence::parse_plan;
use crate::utils::script_paths;
use std::path::{Path, PathBuf};

/// 流程产物目录。点号目录不进引擎扫描与编辑器枚举；「详情」浮窗直接列这一层。
pub const AGENT_DIR: &str = ".agent";

pub const DESIGN_REL_PATH: &str = ".agent/design.md";

/// 章节细节稿（按章分节）：正式产物，与设计稿按章对齐。
pub const CHAPTER_DETAILS_REL_PATH: &str = ".agent/chapter-details.md";

/// 队列视图（渲染自机器状态）：只在路由时当"上一轮状态"交给流程 Agent，不解析。
pub const QUEUE_REL_PATH: &str = ".agent/queue.md";

/// 打完勾的队列项归档于此，只给人回看历史，不注入提示词。
pub const QUEUE_DONE_REL_PATH: &str = ".agent/queue-done.md";

pub const ASSETS_REL_PATH: &str = ".agent/assets.md";

/// 自动骨架的标记行。`progress_block` 靠它判断"这份配置还没被补全"。
pub const SKELETON_MARKER: &str = "本文件由系统自动创建";

/// 确保剧本包有能加载的骨架：缺 `story_config.yaml` 补最小一份、缺 `Chapters/` 就建；已存在不动。
pub fn ensure_package_skeleton(dir: &Path, key: &str) -> std::io::Result<bool> {
    if !dir.join("Chapters").is_dir() {
        std::fs::create_dir_all(dir.join("Chapters"))?;
    }
    let config = dir.join("story_config.yaml");
    if config.exists() {
        return Ok(false);
    }

    let name = dir
        .file_name()
        .map(|s| s.to_string_lossy().to_string())
        .unwrap_or_default();
    let mut text = format!(
        include_str!("../prompts/stage_ensure_package_skeleton.txt"),
        SKELETON_MARKER = SKELETON_MARKER,
        name = name
    );
    let segs: Vec<&str> = key.split('/').collect();
    if segs.len() == 3 && segs[0] == "character" {
        text.push_str(&format!(
            "\n# ===== 羁绊冒险专属配置（按目录布局推断）=====\n\
             adventure:\n  is_adventure: true\n  bound_character_folder: \"{}\"\n  \
             trigger:\n    mode: \"manual\"\n",
            segs[1]
        ));
    }
    text.push_str("\nscript_settings:\n  user_name: \"玩家\"\n");
    std::fs::write(&config, text)?;
    Ok(true)
}

/// 创作阶段：剧本状态的投影，只回答剧本走到哪一步（S1–S3 合并为 [`Stage::Setup`]），不决定注入哪些手册、要不要开思考。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Stage {
    /// 没绑定剧本（[`derive`] 的默认值）；绑了 key 但包或可解析的计划还没有时是 [`Stage::Setup`]。
    #[default]
    Routing,
    Setup,
    /// 设计稿已声明章节，但尚未写完。
    Forge,
    Polish,
    /// 有剧本包、无设计稿、且已有章节：修改既有剧本。
    Modify,
}

#[derive(Debug, Clone, Default)]
pub struct StageSnapshot {
    pub stage: Stage,
    pub script_key: Option<String>,
    pub script_dir: Option<PathBuf>,
    /// 设计稿全文（`None` = 读不到）。事实层只读一次盘：这里读过之后不再各自重读。
    pub design: Option<String>,
    pub plan: Vec<String>,
    pub written: Vec<String>,
}

impl StageSnapshot {
    /// 下一个待写章节 id（仅 [`Stage::Forge`] 有意义）。
    pub fn next_chapter(&self) -> Option<&str> {
        self.plan
            .iter()
            .map(String::as_str)
            .find(|id| !self.written.iter().any(|w| w == id))
    }

    pub fn last_written(&self) -> Option<&str> {
        self.plan
            .iter()
            .map(String::as_str)
            .rfind(|id| self.written.iter().any(|w| w == id))
    }
}

pub fn derive(script_key: Option<&str>) -> StageSnapshot {
    let Some(key) = script_key else {
        return StageSnapshot::default();
    };
    let Ok(script_dir) = script_paths::resolve_script_dir(key) else {
        return StageSnapshot {
            stage: Stage::Setup,
            script_key: Some(key.to_string()),
            ..Default::default()
        };
    };

    let written = script_paths::enumerate_chapter_ids(&script_dir);
    let design = std::fs::read_to_string(script_dir.join(DESIGN_REL_PATH)).ok();
    let plan = design.as_deref().map(parse_plan).unwrap_or_default();

    let stage = if plan.is_empty() {
        if written.is_empty() {
            // 无设计稿：已有章节说明是既存剧本，否则还在设计与大纲阶段
            Stage::Setup
        } else {
            Stage::Modify
        }
    } else if plan.iter().all(|id| written.iter().any(|w| w == id)) {
        Stage::Polish
    } else {
        Stage::Forge
    };

    StageSnapshot {
        stage,
        script_key: Some(key.to_string()),
        script_dir: Some(script_dir),
        design,
        plan,
        written,
    }
}
