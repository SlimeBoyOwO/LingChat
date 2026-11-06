//! 生成前的准备：用户行、场景变化、上下文、上帝 Agent 预选与工具推荐。

use anyhow::Result;
use tauri::Manager;

use crate::ai_service::game_system::game_status::GameStatus;
use crate::ai_service::game_system::scene_store::SceneStore;
use crate::ai_service::game_system::status_ops::narration::{weather_change_text, weather_changed};
use crate::ai_service::game_system::status_ops::{PromptLine, apply_temporary};
use crate::ai_service::god_agent::capabilities::{
    speaker::{self as god_speaker, SpeakerView},
    tool_hint::{self as god_tool_hint, ToolHintView},
};
use crate::ai_service::message_system::processor::UserMessageOutcome;
use crate::ai_service::tools::settings::ToolRecommendMode;
use crate::ai_service::types::{LineAttributeExt, LineBase, LlmMessage, ToolDefinition};
use crate::api::data_dir;
use crate::db::entities::line::LineAttribute;
use crate::utils::prompt::PromptRole;

use super::{GeneratorSource, MessageGenerator, UserMessageContext};

impl MessageGenerator {
    /// Step 1: 预处理用户消息，构建 USER Line 并写入 GameStatus。
    ///
    /// 返回 `UserMessageContext` 供后续步骤使用。
    pub(super) async fn handle_user_message(
        &self,
        raw: Option<&str>,
    ) -> Result<UserMessageContext> {
        let Some(raw) = raw else {
            return Ok(UserMessageContext {
                processed: String::new(),
                temp: None,
                line_index: None,
                seq: None,
            });
        };

        let UserMessageOutcome { main, temp } = self.deps.processor.append_user_message(raw).await;

        let mut gs = self.deps.game_status.lock().await;
        let user_name = gs.player.user_name.clone();
        let line = LineBase {
            content: main.clone(),
            attribute: LineAttributeExt(LineAttribute::User),
            display_name: Some(user_name),
            sender_role_id: Some(0),
            ..Default::default()
        };
        gs.add_line(&self.deps.db, line).await?;
        let line_index = Some(gs.line_list.len().saturating_sub(1));
        let seq = Some(
            gs.line_list
                .iter()
                .filter(|l| {
                    l.base.sender_role_id == Some(0) && matches!(l.attribute(), LineAttribute::User)
                })
                .count() as u32,
        );

        Ok(UserMessageContext {
            processed: main,
            temp,
            line_index,
            seq,
        })
    }

    /// Step 1.5: 检测场景变化，若场景切换则添加系统旁白台词。
    pub(super) async fn detect_scene_change(&self) -> Result<()> {
        let mut gs = self.deps.game_status.lock().await;
        if !gs.scene_awareness_enabled
            || gs.current_scene_id.is_none()
            || gs.current_scene_id == gs.last_processed_scene_id
        {
            return Ok(());
        }

        let scene_id = gs.current_scene_id.clone().unwrap();
        let store = SceneStore::new(&data_dir());
        if let Ok(Some(scene)) = store.find_by_id(&scene_id) {
            if !scene.description.trim().is_empty() {
                let text = format!(
                    "你们一起去了新的场景 - \"{}\"，\"{}\"",
                    scene.name, scene.description
                );
                let line = PromptLine::new(PromptRole::Narrator, text)
                    .display_name("系统")
                    .into_base();
                let _ = gs.add_line(&self.deps.db, line).await;
            }
        }
        gs.last_processed_scene_id = gs.current_scene_id.clone();
        Ok(())
    }

    /// Step 1.6: 检测天气层变化。
    ///
    /// 切换特效本身不写旁白——设置页、天气同步、模型都可能连续改动，逐次落表
    /// 会刷屏。统一等到下一轮对话再与上次对话时的天气比对，有变化才补一句。
    pub(super) async fn detect_weather_change(&self) -> Result<()> {
        let mut gs = self.deps.game_status.lock().await;
        // 剧本自己安排演出，试玩不落台词表：只对齐基线，不补旁白
        if gs.script_status.is_some() || self.deps.is_preview {
            gs.last_processed_weather_effect = gs.weather_effect.clone();
            return Ok(());
        }
        if !weather_changed(&gs.last_processed_weather_effect, &gs.weather_effect) {
            return Ok(());
        }

        let to = gs.weather_effect.clone();
        let from = std::mem::replace(&mut gs.last_processed_weather_effect, to.clone());
        if let Some(text) = weather_change_text(&from, &to) {
            let line = PromptLine::new(PromptRole::Narrator, text)
                .display_name("系统")
                .into_base();
            let _ = gs.add_line(&self.deps.db, line).await;
        }
        Ok(())
    }

    /// Step 2: 根据 current_role_id 获取当前角色的 memory 上下文。
    pub(super) async fn get_current_context(
        &self,
        user_input: Option<&str>,
    ) -> Result<Vec<LlmMessage>> {
        let mut gs = self.deps.game_status.lock().await;
        let Some(rid) = gs.current_role_id else {
            tracing::error!("生成消息的时候没有当前角色，取消生成");
            return Ok(Vec::new());
        };
        // 今日首聊天气先入列，再取 memory，保证本轮上下文就能看到这条提醒
        self.inject_weather_first_talk(&mut gs).await;
        let mut context = {
            let role = gs.get_role(&self.deps.db, rid).await?;
            // 好感度不再逐轮注入上下文；变化时以旁白台词写入历史（见 maybe_evaluate_affection）
            let mut context = role.memory.clone();
            // 世界书注入（上游）：随本轮上下文进，不写回记忆
            if let Some(book) = &role.worldbook {
                book.inject(&mut context, user_input);
            }
            context
        };
        // 临时记忆只供这一次请求，取走即弃
        let temporary = gs.take_temporary_memories();
        drop(gs);
        apply_temporary(&mut context, &temporary);
        Ok(context)
    }

    /// 今日首次对话时，往 game_status 添加一条「系统」天气台词。
    ///
    /// 以 `add_line` 入列（User 属性，不产生第二条 system 消息）；
    /// 剧本系来源与编辑器试玩不注入，其余由 `first_talk_reminder`
    /// 的"每日一次 + 配置开关 + 缓存有货"三重门决定。
    async fn inject_weather_first_talk(&self, gs: &mut GameStatus) {
        let script_source = matches!(
            self.deps.source,
            GeneratorSource::ScriptAiDialogue | GeneratorSource::ScriptFreeDialogue
        );
        let allowed = !script_source && !self.deps.is_preview;
        if let Some(reminder) = crate::ai_service::tools::builtin::weather::first_talk_reminder(
            &self.deps.app,
            gs,
            allowed,
        ) {
            let line = PromptLine::new(PromptRole::Narrator, reminder)
                .display_name("系统")
                .into_base();
            let _ = gs.add_line(&self.deps.db, line).await;
        }
    }

    // ============================================================
    // 上帝 Agent 集成
    // ============================================================

    /// 预处理：用户发消息时，上帝 Agent 决定哪个角色先回应。
    ///
    /// 视图必须在锁内取完并释放锁再调用——发言权的切换由 `select_next_speaker`
    /// 工具内部完成，它会再取 game_status 锁，持锁调用会自死锁。
    pub(super) async fn god_agent_pre_select(&self) -> Result<()> {
        let Some(god) = &self.deps.god_agent else {
            return Ok(());
        };

        let view = {
            let gs = self.deps.game_status.lock().await;
            if !god.should_activate(&gs) {
                return Ok(());
            }
            SpeakerView::capture(&gs, god.config_snapshot().recent_window)
        };

        let decision = god_speaker::decide_next_speaker(god, &view, &self.deps.app).await?;
        if decision.role_id == 0 {
            return Ok(()); // 选择玩家，保持现状
        }

        tracing::info!(
            "[GodAgent] pre-select: role_id={}, name={:?}, reason={}",
            decision.role_id,
            decision.name,
            decision.reason
        );
        Ok(())
    }

    /// 预处理：本轮推荐几件工具。
    ///
    /// 三种模式见 `ToolRecommendMode`。两种推荐都只产出「提示文本 + 本轮并进工具集的
    /// 名字」——提示文本走临时记忆通道，命中的工具在工具闭环里并入本轮定义
    /// （不然模型看得见名字也调不了），不写台词表、不进记忆，下一轮重新算。
    pub(super) async fn tool_hint(&self, user_text: &str) -> Option<god_tool_hint::ToolHint> {
        if self.deps.is_preview {
            return None;
        }
        let mode = self
            .deps
            .app
            .state::<crate::AppState>()
            .tool_settings
            .get()
            .tool_recommend_mode;
        if mode == ToolRecommendMode::Off {
            // 关掉推荐后，上一轮回复后暂存的那份也不再呈现。
            self.deps.game_status.lock().await.clear_pending_tool_hint();
            return None;
        }
        let hint = match mode {
            ToolRecommendMode::Agent => self.agent_tool_hint().await,
            ToolRecommendMode::Rag => self.rag_tool_hint(user_text).await,
            ToolRecommendMode::Off => None,
        };
        if hint.is_some() {
            // 本轮又算出了推荐，它比上一轮回复后的那份更新：同一时刻只留一份，
            // 不然两条推荐提示会一起进请求。
            self.deps.game_status.lock().await.clear_pending_tool_hint();
            return hint;
        }
        self.pending_tool_hint().await
    }

    /// 取上一轮回复后暂存的推荐（没有本轮新推荐时用它）。
    pub(super) async fn pending_tool_hint(&self) -> Option<god_tool_hint::ToolHint> {
        if self.deps.is_preview
            || !matches!(
                self.deps.source,
                GeneratorSource::UserChat | GeneratorSource::Proactive
            )
        {
            return None;
        }
        // 主动对话没有玩家发言，直接走到这里：设置关闭时同样不呈现。
        let mode = self
            .deps
            .app
            .state::<crate::AppState>()
            .tool_settings
            .get()
            .tool_recommend_mode;
        if mode == ToolRecommendMode::Off {
            self.deps.game_status.lock().await.clear_pending_tool_hint();
            return None;
        }
        let names = {
            let mut gs = self.deps.game_status.lock().await;
            gs.take_pending_tool_hint()?
        };
        let (_, candidates) = self.tool_hint_context().await?;
        god_tool_hint::hint_for_names(&candidates, &names)
    }

    /// 推荐用的公共前置：当前角色的显示名与它可调用的全部工具定义。
    ///
    /// 剧本模式的工具列表是冻结快照，推荐没有意义；元工具（`tool_search`）也不在
    /// 候选里——推荐检索入口等于让模型再绕一圈。
    pub(super) async fn tool_hint_context(&self) -> Option<(Option<String>, Vec<ToolDefinition>)> {
        let role_name = {
            let mut gs = self.deps.game_status.lock().await;
            if gs.script_status.is_some() {
                return None;
            }
            let role_id = gs.current_role_id?;
            gs.get_role(&self.deps.db, role_id)
                .await
                .ok()
                .and_then(|role| role.display_name.clone())
        };
        let base_allowed = self
            .deps
            .tool_registry
            .allowed_tools(self.deps.source, role_name.as_deref());
        let mut definitions = self
            .deps
            .tool_registry
            .definitions_for_allowed(&base_allowed);
        definitions.retain(|def| {
            def.function.name != crate::ai_service::tools::tool_rag::TOOL_SEARCH_NAME
        });
        Some((role_name, definitions))
    }

    /// 上帝 Agent 推荐：读最近的对话再挑（多一次小模型调用）。
    async fn agent_tool_hint(&self) -> Option<god_tool_hint::ToolHint> {
        let god = self.deps.god_agent.as_ref()?;
        let (_, definitions) = self.tool_hint_context().await?;

        // 视图在锁内取完就放锁：按需 LLM 调用不能攥着 game_status。
        let view = {
            let gs = self.deps.game_status.lock().await;
            ToolHintView::capture(&gs, god.config_snapshot().recent_window)
        };
        // 只把「这个角色真能调用」的工具交给上帝 Agent 挑，否则会推一批调不动的名字。
        let tools: Vec<(String, String)> = definitions
            .into_iter()
            .map(|def| (def.function.name, def.function.description))
            .collect();
        let view = view.with_tools(tools);

        match god_tool_hint::recommend(god, &view, &self.deps.app).await {
            Ok(hint) => {
                if let Some(hint) = &hint {
                    tracing::info!("[GodAgent] 本轮推荐工具: {:?}", hint.names);
                }
                hint
            },
            // 推荐是锦上添花，失败不能拖累这一轮回复。
            Err(error) => {
                tracing::warn!("[GodAgent] 工具推荐失败: {error:#}");
                None
            },
        }
    }

    /// 静默检索推荐：拿玩家这句话检索一次工具，分数过阈值的才推荐。
    ///
    /// 不额外调用模型，只多一次本地向量计算；命中项照常进激活缓存，后续轮次一直可见
    /// 直到被 LRU 淘汰。
    async fn rag_tool_hint(&self, user_text: &str) -> Option<god_tool_hint::ToolHint> {
        // 只取第一句：多行输入（比如桥接插件在正文后追加的场景旁白）不代表玩家的意图。
        let query = user_text
            .lines()
            .map(str::trim)
            .find(|line| !line.is_empty())?;
        let (role_name, candidates) = self.tool_hint_context().await?;
        if candidates.is_empty() {
            return None;
        }
        let rag = self.deps.app.state::<crate::AppState>().tool_rag.clone();
        match god_tool_hint::recommend_by_rag(&rag, role_name.as_deref(), query, &candidates).await
        {
            Ok(hint) => {
                if let Some(hint) = &hint {
                    tracing::info!("[ToolRAG] 本轮推荐工具: {:?}", hint.names);
                }
                hint
            },
            Err(error) => {
                tracing::warn!("[ToolRAG] 自动推荐失败: {error:#}");
                None
            },
        }
    }
}
