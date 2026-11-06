//! 生成后的收尾：临时消息清理、上帝 Agent 后选、好感度评估、近期记忆落位、
//! 回复纠偏与回复后工具推荐。

use std::sync::Arc;

use anyhow::Result;
use tauri::Manager;

use crate::ai_service::god_agent::capabilities::{
    affection::{self as god_affection, NpcAffectionView},
    speaker::{self as god_speaker, SpeakerView},
    tool_hint as god_tool_hint,
};
use crate::ai_service::tools::settings::ToolRecommendMode;
use crate::ai_service::types::GameLine;
use crate::config::AppConfig;
use crate::db::entities::line::LineAttribute;
use crate::utils::prompt::{
    PromptRole, REPETITION_MIN_ROUNDS, RoundReply, correction_texts, repetition_text,
};

use super::{GeneratorSource, MessageGenerator, UserMessageContext};

impl MessageGenerator {
    /// Step 4: 后处理 — 若存在 temp_message，将 user 行中的 temp 段清理后重建记忆。
    pub(super) async fn cleanup_temp_message(&self, ctx: &UserMessageContext) -> Result<()> {
        let (Some(temp), Some(idx)) = (ctx.temp.as_deref(), ctx.line_index) else {
            return Ok(());
        };
        let mut gs = self.deps.game_status.lock().await;
        gs.role_manager.invalidate_memory_history();
        if let Some(line) = gs.line_list.get_mut(idx) {
            line.base.content = ctx.processed.replace(temp, "");
        }
        gs.refresh_memories(&self.deps.db).await?;
        Ok(())
    }

    // ============================================================
    // 上帝 Agent 集成
    // ============================================================

    /// 后处理：消息生成完毕后，上帝 Agent 决定下一个说话者。
    ///
    /// 返回 `(should_continue, next_role_id)`：
    /// - `should_continue=true` 表示应继续循环（NPC 说话）
    /// - `should_continue=false` 表示应停止（交还玩家或 God Agent 未激活）
    pub(super) async fn god_agent_post_select(
        &self,
        consecutive_npc_rounds: usize,
    ) -> Result<(bool, i32)> {
        let Some(god) = &self.deps.god_agent else {
            return Ok((false, 0));
        };

        // 检查是否超过连续 NPC 轮数上限
        if !god.within_npc_budget(consecutive_npc_rounds) {
            tracing::info!(
                "[GodAgent] 连续 {} 轮 NPC 发言，强制返回玩家",
                consecutive_npc_rounds
            );
            return Ok((false, 0));
        }

        let view = {
            let gs = self.deps.game_status.lock().await;
            if !god.should_activate(&gs) {
                return Ok((false, 0));
            }
            SpeakerView::capture(&gs, god.config_snapshot().recent_window)
        };

        let decision = god_speaker::decide_next_speaker(god, &view, &self.deps.app).await?;
        if decision.role_id == 0 {
            // 交还玩家
            return Ok((false, 0));
        }

        tracing::info!(
            "[GodAgent] post-select: role_id={}, name={:?}, reason={}",
            decision.role_id,
            decision.name,
            decision.reason
        );
        Ok((true, decision.role_id))
    }

    /// 好感度定期评估：真实对话累计到间隔后，spawn 后台任务调整在场 NPC 的六维好感度。
    /// 触发判定与视图快照在锁内取、LLM 调用在锁外；旁白文本锚定到本轮 user 块。
    /// 游标先推进，即使评估失败也不会形成重试风暴。
    pub(super) async fn maybe_evaluate_affection(&self, recent_anchor_hint: Option<usize>) {
        let Some(god) = &self.deps.god_agent else {
            return;
        };
        // 好感度系统总开关（高级设置）：关闭后不评估、不写旁白台词。
        // config 可被 save_settings 热更新，这里取快照保证读到最新值。
        let god_config = god.config_snapshot();
        if !god_config.affection_enabled {
            return;
        }

        let snapshot = {
            let mut gs = self.deps.game_status.lock().await;
            // 剧本模式下的对话不进好感度评估（剧本事件接口另行扩展）。
            if gs.script_status.is_some() {
                None
            } else {
                let interval = god_config.affection_eval_interval.max(1);
                let real_count = gs
                    .line_list
                    .iter()
                    .filter(|l| crate::ai_service::game_system::auto_save::is_real_dialogue(l))
                    .count();
                if real_count < gs.affection_eval_cursor + interval {
                    None
                } else {
                    gs.affection_eval_cursor = real_count;
                    let (npcs, lines) =
                        NpcAffectionView::capture_all(&gs, god_config.recent_window);
                    if npcs.is_empty() {
                        None
                    } else {
                        // 指纹在这里取：临时消息段已被清理，玩家行内容已稳定。
                        let anchor =
                            recent_anchor_hint.and_then(|index| gs.recent_anchor_at(index));
                        Some((npcs, lines, anchor))
                    }
                }
            }
        };
        let Some((npcs, lines, anchor)) = snapshot else {
            return;
        };

        let god = Arc::clone(god);
        let app = self.deps.app.clone();
        tauri::async_runtime::spawn(async move {
            // 后台任务：失败只记日志，不影响已经呈现给用户的回复。
            if let Err(e) =
                god_affection::evaluate_affection(&god, &npcs, &lines, anchor, &app).await
            {
                tracing::warn!("[Affection] 好感度评估失败: {e:#}");
            }
        });
    }

    /// 把待落位的近期记忆写进台词表。剧本路径跳过：那边不进好感度评估，
    /// 也不持 generation_lock。
    pub(super) async fn flush_recent_memories(&self) {
        if matches!(
            self.deps.source,
            GeneratorSource::ScriptAiDialogue | GeneratorSource::ScriptFreeDialogue
        ) {
            return;
        }
        let mut gs = self.deps.game_status.lock().await;
        gs.flush_recent_memories(&self.deps.db).await;
    }

    /// 回复纠偏：上一轮回复的格式要素（情绪标签 / 正文 / 日语翻译）缺失、
    /// 台词数量越界、动作分布异常或情绪标签过长时，各写一条系统提示台词。
    pub(super) async fn maybe_correct_reply_shape(&self, round_replies: &[RoundReply]) {
        if !matches!(
            self.deps.source,
            GeneratorSource::UserChat | GeneratorSource::Proactive
        ) {
            return;
        }

        // 角色系统的第二语言输出开关决定系统提示词里有没有「日语翻译」那一行，
        // 缺了它才是格式错误（配置读取与天气首聊同款，每轮一次、代价可忽略）
        let expect_translation = AppConfig::load(&self.deps.app)
            .map(|config| config.llm_output_sec_lang)
            .unwrap_or(false);

        let mut gs = self.deps.game_status.lock().await;
        if gs.script_status.is_some() {
            return;
        }

        // 每类问题只取首个命中的轮次（多人模式下一次回复含多个角色的输出，
        // 逐轮排队会把提示堆一片）；什么都没产出的轮次（模型只调了工具）不参与判据。
        let mode = gs.reply_length_mode();
        let mut texts = round_replies
            .iter()
            .filter(|round| round.landed_lines > 0 || !round.shapes.is_empty())
            .find_map(|round| {
                let texts = correction_texts(mode, round, expect_translation);
                (!texts.is_empty()).then_some(texts)
            })
            .unwrap_or_default();

        // 台词条数连续多轮一模一样：条数可能仍在允许区间内，但每轮同长很机械。
        // 已经越界时交给上面的数量纠偏，不再叠一条「要有变化」；本轮没落下台词时
        // 不检查，否则会拿上一轮的结论重复排同一条提示。
        let produced_lines = round_replies.iter().any(|round| round.landed_lines > 0);
        if produced_lines {
            if let Some((count, streak)) = same_length_streak(&gs.line_list) {
                let in_range = match mode.accepted_range() {
                    Some((min, max)) => (min..=max).contains(&count),
                    None => true,
                };
                if streak >= REPETITION_MIN_ROUNDS && in_range {
                    texts.push(repetition_text(streak, count));
                }
            }
        }

        // 各条提示写进台词表；写失败只记一笔，不影响已经呈现给玩家的回复
        for text in texts {
            if let Err(error) = gs
                .add_prompt_line(&self.deps.db, PromptRole::System, &text)
                .await
            {
                tracing::warn!("[Correction] 纠偏提示写入失败: {error:#}");
            }
        }
    }

    /// 回复后推荐：拿本轮落地的助手台词（正文 + 动作）静默检索一次工具。
    pub(super) async fn maybe_recommend_tools_after_reply(&self, turn_start: usize) {
        if self.deps.is_preview
            || !matches!(
                self.deps.source,
                GeneratorSource::UserChat | GeneratorSource::Proactive
            )
        {
            return;
        }
        let settings = self.deps.app.state::<crate::AppState>().tool_settings.get();
        // Tool RAG 关闭时全量下发，推荐在工具集上没有意义。
        if !settings.tool_rag || settings.tool_recommend_mode == ToolRecommendMode::Off {
            return;
        }

        let (query, tool_called) = {
            let gs = self.deps.game_status.lock().await;
            let turn = gs.line_list.iter().skip(turn_start);
            // 本轮已经调用过工具的，说明模型想做的事已经做了；再推一遍容易让它
            // 下一轮把同一件事又做一次。
            let tool_called = turn
                .clone()
                .any(|line| matches!(line.attribute(), LineAttribute::Tool));
            (reply_query(turn), tool_called)
        };
        if tool_called || query.is_empty() {
            return;
        }
        let Some((role_name, candidates)) = self.tool_hint_context().await else {
            return;
        };
        if candidates.is_empty() {
            return;
        }
        let rag = self.deps.app.state::<crate::AppState>().tool_rag.clone();
        if !rag.is_enabled() {
            return;
        }
        match god_tool_hint::recommend_by_rag(&rag, role_name.as_deref(), &query, &candidates).await
        {
            Ok(Some(hint)) => {
                tracing::info!("[ToolRAG] 回复后推荐工具: {:?}", hint.names);
                self.deps
                    .game_status
                    .lock()
                    .await
                    .set_pending_tool_hint(hint.names);
            },
            Ok(None) => {},
            Err(error) => tracing::warn!("[ToolRAG] 回复后推荐失败: {error:#}"),
        }
    }
}

/// 本轮落地的助手台词拼成检索查询：正文在前、动作在后。
///
/// 只认有发送者的 assistant 行：工具结果与空内容载体行不是「AI 说过的话」。
fn reply_query<'a>(lines: impl Iterator<Item = &'a GameLine>) -> String {
    let mut parts: Vec<&str> = Vec::new();
    for line in lines {
        if !matches!(line.attribute(), LineAttribute::Assistant) || line.sender_role_id().is_none()
        {
            continue;
        }
        let content = line.base.content.trim();
        if !content.is_empty() {
            parts.push(content);
        }
        if let Some(action) = line.base.action_content.as_deref().map(str::trim) {
            if !action.is_empty() {
                parts.push(action);
            }
        }
    }
    parts.join("\n")
}

// ============================================================
// 回复条数趋势
// ============================================================

/// 最近台词条数一致的趋势，返回（最后一轮的台词条数，与它相同的连续轮数）。
fn same_length_streak(lines: &[GameLine]) -> Option<(usize, usize)> {
    let mut rounds: Vec<(i32, usize)> = Vec::new();
    let mut current: Option<(i32, usize)> = None;
    for line in lines {
        // 工具结果与空内容载体行是同一回合的中段，不让它们结束当前轮；
        // 否则一次工具回合会被拆成「工具前」「工具后」两个同长段。
        if matches!(line.attribute(), LineAttribute::Tool) {
            continue;
        }
        if matches!(line.attribute(), LineAttribute::Assistant)
            && line.sender_role_id().is_none()
            && line
                .base
                .tool_call
                .as_deref()
                .is_some_and(|v| !v.trim().is_empty())
        {
            continue;
        }

        // assistant 行才是台词；其他无角色行结束一轮
        let role_id = match line.attribute() {
            LineAttribute::Assistant => line.sender_role_id(),
            _ => None,
        };
        let Some(role_id) = role_id else {
            rounds.extend(current.take());
            continue;
        };
        if current.is_some_and(|(sender, _)| sender == role_id) {
            if let Some((_, count)) = current.as_mut() {
                *count += 1;
            }
        } else {
            rounds.extend(current.take());
            current = Some((role_id, 1));
        }
    }
    rounds.extend(current);

    let (sender, count) = *rounds.last()?;
    let streak = rounds
        .iter()
        .rev()
        .take_while(|(round_sender, round_count)| *round_sender == sender && *round_count == count)
        .count();
    Some((count, streak))
}
