//! `GameStatus` 上的操作集合：把一条提示行落到台词表，或只在下一轮请求里出现一次。
//!
//! 三种口径：
//! - 正常插入台词表：用 [`PromptLine`] 构造，走 `GameStatus::add_line`；
//! - 插入到某轮 user 台词之后：近期记忆队列 + 落位（[`Pending`]）；
//! - 请求级注入：临时记忆与工具推荐名字，取走即弃（[`RequestScoped`]）。
//!
//! 「排队」不是第四种口径，而是前两种在生成进行中不能写入时的延迟形态：插件旁白
//! 排的是第一种，好感度旁白排的是第二种。

pub mod affection;
pub mod line;
pub mod narration;
pub mod pending;
pub mod request;

use std::time::Duration;

use sea_orm::DatabaseConnection;

use crate::ai_service::game_system::game_status::GameStatus;
use crate::ai_service::types::GameLine;
use crate::db::entities::line::LineAttribute;
use crate::utils::prompt::PromptRole;

pub use line::PromptLine;
pub use pending::{LineFingerprint, Pending, RecentMemory, RecentMemoryAnchor};
pub use request::{RequestScoped, TemporaryMemory, TemporaryPlacement, apply_temporary};

use pending::{init_insert_index, recent_insert_index, resolve_recent_index};

impl GameStatus {
    /// 攒一条插件旁白，等下一轮开头由 [`Self::flush_pending_prompt_lines`] 写进台词表。
    ///
    /// `ttl` 与 `key` 是给游戏事件用的：有有效期的条目不会立刻进台词表，等到真的
    /// 要开口那一刻再判一次——已经过期的直接丢掉。
    /// `key` 相同的条目（比如连着挨打）只留 `severity` 最大的那条。
    pub fn queue_prompt_line(
        &mut self,
        role: PromptRole,
        text: impl Into<String>,
        ttl: Option<Duration>,
        key: Option<String>,
        severity: i64,
    ) -> usize {
        self.pending
            .queue_prompt_line(role, text.into(), ttl, key, severity)
    }

    /// 把攒下的插件旁白按顺序写进台词表。
    pub async fn flush_pending_prompt_lines(&mut self, db: &DatabaseConnection) -> usize {
        let batches = self.pending.take_prompt_batches();
        let mut written = 0;
        for (role, text) in batches {
            match self.add_prompt_line(db, role, &text).await {
                Ok(_) => written += 1,
                Err(e) => tracing::warn!("[GameStatus] 补写插件旁白失败：{e}"),
            }
        }
        if written > 0 {
            tracing::info!("[GameStatus] 补写排队中的插件旁白 {written} 条");
        }
        written
    }

    /// 用指定下标上的行构造锚点；该行不是 User 行时返回 None。
    /// 评估时调用，此时玩家行的临时消息段已清理，指纹取到的是最终内容。
    pub fn recent_anchor_at(&self, index: usize) -> Option<RecentMemoryAnchor> {
        let line = self.line_list.get(index)?;
        if !matches!(line.attribute(), LineAttribute::User) {
            return None;
        }
        let mut perceived_role_ids: Vec<i32> = self
            .present_role_ids
            .iter()
            .copied()
            .filter(|id| *id != 0)
            .collect();
        perceived_role_ids.sort_unstable();
        Some(RecentMemoryAnchor {
            index,
            fingerprint: LineFingerprint::of(line),
            session: self.preview_generation,
            perceived_role_ids,
        })
    }

    /// 末尾最后一条 User 行的下标，供调用方确定本轮锚点。
    pub fn last_user_line_index(&self) -> Option<usize> {
        self.line_list
            .len()
            .checked_sub(1)
            .filter(|&index| matches!(self.line_list[index].attribute(), LineAttribute::User))
    }

    /// 入队一条近期记忆，等本轮结束或下一轮开头落位。
    pub fn queue_recent_memory(&mut self, memory: RecentMemory) {
        self.pending.queue_recent_memory(memory);
    }

    /// 清空待落位的近期记忆。换档/清档时调用，避免旧会话的产出延迟写入。
    pub fn clear_pending_recent_memories(&mut self) {
        self.pending.clear_recent_memories();
    }

    /// 把待落位的近期记忆写进台词表：锚点之后连续 User 块的末尾。
    /// 锚点失效或本轮没有 user 行时丢弃，返回实际写入条数。
    pub async fn flush_recent_memories(&mut self, db: &DatabaseConnection) -> usize {
        if self.script_status.is_some() {
            self.pending.clear_recent_memories();
            tracing::warn!("[Memory] 剧本模式下丢弃待落位的近期记忆");
            return 0;
        }

        let pending: Vec<RecentMemory> = self
            .pending
            .take_recent_memories()
            .into_iter()
            .filter(|memory| {
                memory
                    .anchor
                    .as_ref()
                    .is_none_or(|anchor| anchor.session == self.preview_generation)
            })
            .collect();
        if pending.is_empty() {
            return 0;
        }

        let has_dialogue = self.line_list.iter().any(|line| {
            matches!(
                line.attribute(),
                LineAttribute::User | LineAttribute::Assistant
            )
        });
        let mut current_perceived: Vec<i32> = self
            .present_role_ids
            .iter()
            .copied()
            .filter(|id| *id != 0)
            .collect();
        current_perceived.sort_unstable();

        // 先解析全部插入点，再按下标从大到小插入，避免前一次插入让后一次下标漂移。
        let mut planned: Vec<(usize, GameLine)> = Vec::new();
        for memory in pending {
            let (insert_at, perceived) = match &memory.anchor {
                Some(anchor) => {
                    let Some(anchor_index) = resolve_recent_index(&self.line_list, anchor) else {
                        tracing::warn!("[Memory] 近期记忆锚点已失效，丢弃");
                        continue;
                    };
                    (
                        recent_insert_index(&self.line_list, anchor_index),
                        anchor.perceived_role_ids.clone(),
                    )
                },
                None if !has_dialogue => (
                    init_insert_index(&self.line_list),
                    current_perceived.clone(),
                ),
                None => {
                    tracing::warn!("[Memory] 本轮没有 user 台词，丢弃近期记忆");
                    continue;
                },
            };
            planned.push((
                insert_at,
                PromptLine::new(memory.role, memory.text.as_str()).into_game_line(perceived),
            ));
        }
        if planned.is_empty() {
            return 0;
        }

        self.role_manager.invalidate_memory_history();
        planned.sort_by_key(|(index, _)| *index);
        let min_index = planned[0].0;
        let written = planned.len();
        for (index, line) in planned.into_iter().rev() {
            let at = index.min(self.line_list.len());
            self.line_list.insert(at, line);
        }
        // 插入点早于持久记忆的压缩指针时回拨，避免摘要区和原文重复。
        self.role_manager.rewind_memory_pointers(min_index).await;
        if let Err(e) = self.refresh_memories(db).await {
            tracing::warn!("[Memory] 近期记忆写入后刷新记忆失败: {e:#}");
        }
        written
    }

    /// 入队一条只在下一轮请求出现的临时记忆，文本重复时不再入队。
    pub fn queue_temporary_memory(&mut self, memory: TemporaryMemory) {
        self.request.queue_temporary(memory);
    }

    /// 取走全部临时记忆，只用于构建一次请求上下文。
    pub fn take_temporary_memories(&mut self) -> Vec<TemporaryMemory> {
        self.request.take_temporary()
    }

    pub fn clear_pending_temporary_memories(&mut self) {
        self.request.clear_temporary();
    }

    /// 暂存回复后推荐的工具名，覆盖上一份。
    pub fn set_pending_tool_hint(&mut self, names: Vec<String>) {
        self.request.set_tool_hint(names);
    }

    /// 取走暂存的推荐工具名，只用于构建一次请求上下文。
    pub fn take_pending_tool_hint(&mut self) -> Option<Vec<String>> {
        self.request.take_tool_hint()
    }

    /// 查看暂存的推荐工具名（调试指令用，不取走）。
    pub fn pending_tool_hint(&self) -> Option<&[String]> {
        self.request.tool_hint()
    }

    pub fn clear_pending_tool_hint(&mut self) {
        self.request.clear_tool_hint();
    }
}
