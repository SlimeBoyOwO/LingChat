//! 上帝 Agent 核心：决策逻辑、prompt 构建、发言者选择。

use std::sync::Arc;

use anyhow::{anyhow, Result};

use crate::ai_service::game_system::game_status::GameStatus;
use crate::ai_service::god_agent::config::GodAgentConfig;
use crate::ai_service::god_agent::tools;
use crate::ai_service::llm::LlmClient;
use crate::ai_service::types::{GameLine, LlmMessage};

// ============================================================
// GodAgentCore
// ============================================================

pub struct GodAgentCore {
    pub llm: Arc<LlmClient>,
    pub config: GodAgentConfig,
}

impl GodAgentCore {
    pub fn new(llm: Arc<LlmClient>, config: GodAgentConfig) -> Self {
        Self { llm, config }
    }

    // ============================================================
    // 激活判断
    // ============================================================

    /// 判断上帝 Agent 是否应在当前场景下激活。
    ///
    /// 条件：
    /// - 自由对话模式（`script_status.is_none()`）
    /// - 在场角色数 > 1（含玩家，即玩家 + 至少 1 个 NPC）
    pub fn should_activate(&self, gs: &GameStatus) -> bool {
        gs.script_status.is_none() && gs.present_role_ids.len() > 1
    }

    // ============================================================
    // Prompt 构建
    // ============================================================

    /// 构建上帝 Agent 的决策 prompt。
    ///
    /// 参考 `MemoryBuilder` 的格式化模式，将最近 N 条台词按角色分组呈现，
    /// 同时附上每个在场 NPC 的角色信息。
    fn build_decision_prompt(
        &self,
        lines: &[GameLine],
        npc_ids: &[i32],
        current_speaker: Option<i32>,
        gs: &GameStatus,
    ) -> Vec<LlmMessage> {
        // --- 角色信息 ---
        let mut role_info_block = String::from("【当前在场的非玩家角色列表】\n");
        for &rid in npc_ids {
            let name = gs
                .role_manager
                .get_loaded(rid)
                .and_then(|r| r.display_name.clone())
                .unwrap_or_else(|| format!("角色{}", rid));
            let subtitle = gs
                .role_manager
                .get_loaded(rid)
                .and_then(|r| r.settings.ai_subtitle.clone())
                .unwrap_or_default();
            let info = gs
                .role_manager
                .get_loaded(rid)
                .and_then(|r| r.settings.info.clone())
                .unwrap_or_default();
            role_info_block.push_str(&format!(
                "- role_id={}: {}\n  简介: {}\n  设定: {}\n",
                rid, name,
                if subtitle.is_empty() { "无" } else { &subtitle },
                if info.is_empty() { "无" } else { &info },
            ));
        }

        // --- 最近对话 ---
        let mut dialog_block = String::from("【最近对话记录（由旧到新）】\n");
        if lines.is_empty() {
            dialog_block.push_str("（无对话记录）\n");
        } else {
            for line in lines {
                let name = line
                    .base
                    .display_name
                    .as_deref()
                    .unwrap_or("未知");
                let sid = line.base.sender_role_id.unwrap_or(-1);
                let emotion = line
                    .base
                    .original_emotion
                    .as_deref()
                    .filter(|v| !v.is_empty())
                    .map(|v| format!("【{}】", v))
                    .unwrap_or_default();
                let content = &line.base.content;
                dialog_block.push_str(&format!(
                    "[role_id={}] {}: {}{}\n",
                    sid, name, emotion, content
                ));
            }
        }

        // --- 当前发言者提示 ---
        let current_hint = match current_speaker {
            Some(0) => "当前发言者是「玩家」。请选择下一个发言的 NPC 角色。\n".to_string(),
            Some(rid) => {
                let name = gs
                    .role_manager
                    .get_loaded(rid)
                    .and_then(|r| r.display_name.clone())
                    .unwrap_or_else(|| format!("角色{}", rid));
                format!("当前发言者是「{}」(role_id={})，刚刚说完话。请判断：\n- 如果对话应该继续（比如另一个角色有强烈反应或话题未完），选择下一个发言的 NPC\n- 如果应该交还给玩家，选择 role_id=0\n", name, rid)
            }
            None => String::new(),
        };

        let system_prompt = format!(
            "你是一个多人对话的导演（上帝视角）。你的任务是：根据当前场景中的角色列表和最近的对话历史，\
             判断下一个应该发言的角色。\n\
             \n\
             {}\n\
             {}\n\
             {}\n\
             请调用 select_next_speaker 工具来选择下一个发言者。",
            role_info_block,
            dialog_block,
            current_hint,
        );

        vec![LlmMessage::system(system_prompt)]
    }

    // ============================================================
    // 决策
    // ============================================================

    /// 调用 LLM function calling 决策下一个说话者。
    ///
    /// 返回 `(selected_role_id, reason)`。
    /// - `selected_role_id == 0` 表示玩家
    pub async fn decide_next_speaker(
        &self,
        gs: &GameStatus,
        current_speaker: Option<i32>,
    ) -> Result<(i32, String)> {
        // 收集非玩家在场角色 ID
        let npc_ids: Vec<i32> = gs
            .present_role_ids
            .iter()
            .filter(|&&id| id != 0)
            .copied()
            .collect();

        // 只有 0-1 个 NPC：无需 LLM 决策
        if npc_ids.len() <= 1 {
            return Ok((npc_ids.first().copied().unwrap_or(0), "single_npc".into()));
        }

        // 取最近 N 条台词
        let window = self.config.recent_window;
        let lines: Vec<GameLine> = gs
            .line_list
            .iter()
            .rev()
            .take(window)
            .cloned()
            .collect::<Vec<_>>()
            .into_iter()
            .rev()
            .collect();

        // 构建 prompt
        let messages = self.build_decision_prompt(&lines, &npc_ids, current_speaker, gs);

        // 调用 LLM function calling
        let tools = vec![tools::select_next_speaker_tool()];
        let response = self
            .llm
            .complete_with_tools(&messages, &tools, Some("auto"))
            .await?;

        // 解析 tool call
        if let Some(ref tool_calls) = response.tool_calls {
            if let Some(tc) = tool_calls.first() {
                if let Some(result) = tools::parse_speaker_selection(tc) {
                    // 校验：选中的 role_id 必须在 present_role_ids 中或为 0
                    if result.0 == 0 || gs.present_role_ids.contains(&result.0) {
                        return Ok(result);
                    }
                    tracing::warn!(
                        "上帝Agent 选择了不在场的角色 {}，忽略",
                        result.0
                    );
                }
            }
        }

        // Fallback：如果 LLM 返回了文本但未调用工具，尝试从内容解析
        if let Some(ref content) = response.content {
            tracing::warn!("上帝Agent 未调用工具，返回了文本: {content}");
        }

        Err(anyhow!("上帝Agent 无法解析下一个发言者"))
    }
}
