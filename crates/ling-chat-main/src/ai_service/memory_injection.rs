//! 统一的记忆注入入口：近期记忆写进台词表并锚定到某一轮，临时记忆与系统级记忆
//! 只对本次请求有效。近期记忆的排队与落位在 `GameStatus` 里，那里才持有台词表。

use crate::ai_service::types::{GameLine, LineAttributeExt, LineBase, LlmMessage};
use crate::db::entities::line::LineAttribute;
use crate::utils::prompt::PromptRole;

/// 一条待注入的记忆。
pub enum MemoryInjection {
    /// 近期记忆：写进台词表，锚定到某一轮的 user 块末尾，之后作为历史存在。
    Recent(RecentMemory),
    /// 临时记忆：只进本次请求的上下文，不落台词表。
    Temporary(TemporaryMemory),
    /// 系统级记忆：接在 system 块之后，只对本次请求有效。
    SystemLevel(String),
}

/// 写进台词表的一条提示/旁白。
#[derive(Clone, Debug)]
pub struct RecentMemory {
    pub text: String,
    pub role: PromptRole,
    /// 目标轮次的锚点；`None` 表示初始化阶段没有 user 行可挂。
    pub anchor: Option<RecentMemoryAnchor>,
}

/// 锚定到某一轮 user 行的锚点。
#[derive(Clone, Debug)]
pub struct RecentMemoryAnchor {
    pub index: usize,
    /// 下标漂移后用来确认还是同一行。
    pub fingerprint: LineFingerprint,
    /// 捕获时的会话代号（`GameStatus::preview_generation`）。
    pub session: u64,
    /// 捕获时的在场角色，写入行的感知集合。
    pub perceived_role_ids: Vec<i32>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct LineFingerprint {
    pub content: String,
    pub attribute: LineAttribute,
    pub sender_role_id: Option<i32>,
    pub display_name: Option<String>,
}

impl LineFingerprint {
    pub fn of(line: &GameLine) -> Self {
        Self {
            content: line.base.content.clone(),
            attribute: line.base.attribute.inner().clone(),
            sender_role_id: line.base.sender_role_id,
            display_name: line.base.display_name.clone(),
        }
    }

    pub fn matches(&self, line: &GameLine) -> bool {
        self.attribute == *line.base.attribute.inner()
            && self.sender_role_id == line.base.sender_role_id
            && self.display_name == line.base.display_name
            && self.content == line.base.content
    }
}

/// 只对本次请求有效的临时记忆。
#[derive(Clone, Debug)]
pub struct TemporaryMemory {
    pub text: String,
    pub placement: TemporaryPlacement,
}

/// 临时记忆的落点。
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TemporaryPlacement {
    /// 并入最后一条 user 消息。
    IntoUser,
    /// 作为末尾独立的 user 消息。
    TrailingUser,
}

/// 锚点搜索窗口：中段插入会让下标前后漂移，在这个范围内按指纹兜底。
const ANCHOR_SEARCH_WINDOW: usize = 32;

/// 定位锚点行：先按下标命中，不中再在前后窗口内按指纹找。
pub fn resolve_recent_index(lines: &[GameLine], anchor: &RecentMemoryAnchor) -> Option<usize> {
    if lines
        .get(anchor.index)
        .is_some_and(|line| anchor.fingerprint.matches(line))
    {
        return Some(anchor.index);
    }

    for distance in 1..=ANCHOR_SEARCH_WINDOW {
        if let Some(index) = anchor.index.checked_sub(distance) {
            if lines
                .get(index)
                .is_some_and(|line| anchor.fingerprint.matches(line))
            {
                return Some(index);
            }
        }
        let index = anchor.index + distance;
        if index < lines.len()
            && lines
                .get(index)
                .is_some_and(|line| anchor.fingerprint.matches(line))
        {
            return Some(index);
        }
    }
    None
}

/// 近期记忆的落点：锚点之后连续 User 块的末尾（第一条 tool/assistant 之前）。
/// 遇到下一轮的真实玩家发言也停下，避免挂到新一轮上。
pub fn recent_insert_index(lines: &[GameLine], anchor_index: usize) -> usize {
    let mut index = anchor_index + 1;
    while index < lines.len() {
        let line = &lines[index];
        if !matches!(line.attribute(), LineAttribute::User) {
            break;
        }
        if crate::ai_service::game_system::auto_save::is_real_dialogue(line) {
            break;
        }
        index += 1;
    }
    index
}

/// 初始化阶段没有锚点时的落点：开头连续 System 行之后。
pub fn init_insert_index(lines: &[GameLine]) -> usize {
    lines
        .iter()
        .take_while(|line| matches!(line.attribute(), LineAttribute::System))
        .count()
}

/// 把请求级记忆应用到本轮上下文。近期记忆走台词表落位，不在这里处理。
pub fn apply_request_injections(context: &mut Vec<LlmMessage>, injections: &[MemoryInjection]) {
    let system_texts: Vec<&str> = injections
        .iter()
        .filter_map(|injection| match injection {
            MemoryInjection::SystemLevel(text) => Some(text.as_str()),
            _ => None,
        })
        .collect();
    if !system_texts.is_empty() {
        // 接在开头连续 system 块之后，不打断人设本身的顺序。
        let insert_at = context
            .iter()
            .take_while(|message| message.role == "system")
            .count();
        context.insert(insert_at, LlmMessage::system(system_texts.join("\n\n")));
    }

    for injection in injections {
        let MemoryInjection::Temporary(memory) = injection else {
            continue;
        };
        match memory.placement {
            TemporaryPlacement::IntoUser => match context.last_mut() {
                Some(last) if last.role == "user" => {
                    if !last.content.is_empty() {
                        last.content.push_str("\n\n");
                    }
                    last.content.push_str(&memory.text);
                },
                _ => context.push(LlmMessage::user(memory.text.clone())),
            },
            TemporaryPlacement::TrailingUser => {
                context.push(LlmMessage::user(memory.text.clone()));
            },
        }
    }
}

/// 构造一条待写入台词表的近期记忆行。
pub fn build_recent_line(memory: &RecentMemory, perceived_role_ids: Vec<i32>) -> GameLine {
    GameLine::from_base(
        LineBase {
            content: memory.role.build_prompt(&memory.text),
            attribute: LineAttributeExt(LineAttribute::User),
            display_name: Some(memory.role.display_name().to_string()),
            ..Default::default()
        },
        perceived_role_ids,
    )
}
