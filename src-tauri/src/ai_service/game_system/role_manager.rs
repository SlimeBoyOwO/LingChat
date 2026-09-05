use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};

use anyhow::{Result, anyhow};
use sea_orm::DatabaseConnection;

use crate::ai_service::game_system::game_status::HistoryChange;
use crate::ai_service::game_system::memory_builder::MemoryBuilder;
use crate::ai_service::llm::LlmSlot;
use crate::ai_service::memory::{MemoryConfig, MemoryCoordinator, MemoryMode};
use crate::ai_service::tts::VoiceMaker;
use crate::ai_service::tts::local::LocalTtsRuntime;
use crate::ai_service::types::{CharacterSettings, GameLine, GameMemoryBank, GameRole, LlmMessage};
use crate::config::tts::TtsConfig;
use crate::db::entities::line::LineAttribute;
use crate::db::managers::memory_repo::MemoryRepo;
use crate::db::managers::role_repo::RoleRepo;
use crate::utils::path::resolve_character_path;

/// 角色运行时管理器：维护当前活跃角色的内存状态。
pub struct GameRoleManager {
    pub loaded_roles: HashMap<i32, GameRole>,
    data_dir: PathBuf,

    /// LLM 客户端槽位（支持运行时热切换）。MemoryBank 压缩引擎依赖此字段。
    /// 槽位本身始终存在，内部值为 None 时表示尚未配置模型。
    llm: LlmSlot,
    /// Permanent-memory runtime collection. Save/context always read immutable
    /// coordinator snapshots; GameRole no longer stores a second bank copy.
    memory: MemoryCoordinator,
    /// TTS 引擎配置（适配器 URL、音频格式等）。
    tts_config: TtsConfig,
    /// 本地 TTS 共享运行时（进程内引擎 + 路径 + 全局开关）。
    /// 转发给每个 VoiceMaker，使 `sbv2_local` 适配器可以惰性引导。
    local_tts: Option<LocalTtsRuntime>,
    /// 角色服装覆盖（session store → register_role_by_id 时优先读取）
    clothes_overrides: HashMap<i32, String>,
}

impl GameRoleManager {
    pub fn new(
        data_dir: PathBuf,
        llm: LlmSlot,
        tts_config: TtsConfig,
        local_tts: Option<LocalTtsRuntime>,
        memory_config: MemoryConfig,
    ) -> Self {
        Self {
            loaded_roles: HashMap::new(),
            data_dir,
            llm,
            memory: MemoryCoordinator::new(memory_config),
            tts_config,
            local_tts,
            clothes_overrides: HashMap::new(),
        }
    }

    /// 设置角色服装覆盖（来自 session store，优先于 settings.yml 的默认值）。
    pub fn set_clothes_overrides(&mut self, overrides: HashMap<i32, String>) {
        self.clothes_overrides = overrides;
    }

    pub fn set_character_clothes_override(&mut self, role_id: i32, clothes: String) {
        self.clothes_overrides.insert(role_id, clothes);
    }

    /// 获取角色；若未加载则从 DB 惰性注册。
    pub async fn get_role(
        &mut self,
        db: &DatabaseConnection,
        role_id: i32,
    ) -> Result<&mut GameRole> {
        if !self.loaded_roles.contains_key(&role_id) {
            self.register_role_by_id(db, role_id).await?;
        }
        // Every loaded role receives a runtime owner even when permanent
        // compression is disabled or no LLM is currently configured. Disabled
        // runtimes simply never trigger; the runtime remains the only bank
        // source and restores replace its default snapshot directly.
        let display_name = self
            .loaded_roles
            .get(&role_id)
            .and_then(|role| role.display_name.clone())
            .unwrap_or_else(|| "AI".to_string());
        self.memory.ensure(
            role_id,
            &GameMemoryBank::default(),
            &display_name,
            self.llm.clone(),
        );
        Ok(self.loaded_roles.get_mut(&role_id).expect("角色刚刚插入"))
    }

    pub fn get_loaded(&self, role_id: i32) -> Option<&GameRole> {
        self.loaded_roles.get(&role_id)
    }

    pub fn get_loaded_mut(&mut self, role_id: i32) -> Option<&mut GameRole> {
        self.loaded_roles.get_mut(&role_id)
    }

    /// 返回指定角色记忆库的"系统记忆文本"（ta的信息 / 约定 / 长期经历）。
    /// 记忆系统未启用或角色未加载时返回空字符串。供 memory.get_current 工具调用。
    pub async fn get_role_memory_text(&self, role_id: i32) -> String {
        match self.memory.runtime(role_id) {
            Some(sys) if sys.is_enabled() => sys.get_system_memory_text().await,
            _ => String::new(),
        }
    }

    pub fn reset_roles(&mut self) {
        self.loaded_roles.clear();
        self.memory.clear();
    }

    /// Drop roles and permanent-memory runtimes that were introduced by a
    /// temporary resource scope (currently editor preview). The retained IDs
    /// are captured before the scope begins, so their existing `GameRole`,
    /// VoiceMaker and MemoryBank runtime objects are never replaced or reset.
    ///
    /// Scene/script references must be restored before this call. Runtime
    /// entries are detached before their owned compaction tasks are aborted,
    /// which prevents a discarded default bank from being observed by a later
    /// formal snapshot while still joining the task without holding a lock.
    /// Synchronously remove temporary roles/runtimes and return detached
    /// runtime owners. The caller must abort/join these only after it has
    /// released GameStatus (and therefore RoleManager); see PreviewSession.
    pub(crate) fn detach_role_resources(
        &mut self,
        retained_role_ids: &HashSet<i32>,
    ) -> Vec<crate::ai_service::memory::PersistentMemorySystem> {
        self.loaded_roles
            .retain(|role_id, _| retained_role_ids.contains(role_id));
        self.memory.detach_not_in(retained_role_ids)
    }

    /// Cleanup helper for callers that do not hold a broader async state lock.
    /// PreviewSession must use `detach_role_resources` and recycle outside its
    /// GameStatus critical section.
    pub async fn retain_role_resources(&mut self, retained_role_ids: &HashSet<i32>) {
        for runtime in self.detach_role_resources(retained_role_ids) {
            runtime.abort_and_wait().await;
        }
    }

    /// Identity set for the complete formal role resource scope. Usually the
    /// two collections are identical, but retaining their union makes cleanup
    /// robust if an earlier failure created only a role or only a runtime.
    pub fn role_resource_ids(&self) -> HashSet<i32> {
        self.loaded_roles
            .keys()
            .copied()
            .chain(self.memory.runtime_role_ids())
            .collect()
    }

    pub fn clear_role_memory(&mut self, role_id: i32) {
        if let Some(role) = self.loaded_roles.get_mut(&role_id) {
            role.memory.clear();
            tracing::info!("角色 {} 的短期记忆已清除", role_id);
        } else {
            tracing::warn!("角色 {} 未在运行时加载，无法清除记忆", role_id);
        }
    }

    pub fn reactivate_all_voice_makers(&self) {
        for role in self.loaded_roles.values() {
            if let Some(vm) = &role.voice_maker {
                vm.reactivate();
            }
        }
        tracing::info!("所有角色 TTS 已重新启用");
    }

    /// 按 DB/settings.yml 的最新 TTS 配置重建**所有已加载角色**的 VoiceMaker。
    ///
    /// 历史页「生成语音」前的预热：保证配好 TTS 后第一次点击就能成功，覆盖三种
    /// 滞后场景——角色先于 TTS 配置注册（voice_maker 为 None）、provider 被禁用
    /// 等待后台恢复、设置被改但运行时对象未刷新。新配置下仍无 VoiceMaker 的
    /// （tts_type 为空）保持现状不动。返回刷新成功的角色数。
    pub async fn rebuild_voice_makers_from_db(&mut self, db: &DatabaseConnection) -> usize {
        let role_ids: Vec<i32> = self.loaded_roles.keys().copied().collect();
        let mut ok = 0usize;
        for role_id in role_ids {
            let resource_path = self
                .loaded_roles
                .get(&role_id)
                .and_then(|r| r.resource_path.clone());
            let settings =
                match RoleRepo::get_role_settings_by_id(db, &self.data_dir, role_id).await {
                    Ok(Some(s)) => s,
                    _ => continue,
                };
            let Some(vm) = build_voice_maker(
                &self.data_dir,
                &settings,
                resource_path.as_deref(),
                &self.tts_config,
                self.local_tts.as_ref(),
            ) else {
                continue;
            };
            let Some(role) = self.loaded_roles.get_mut(&role_id) else {
                continue;
            };
            role.settings.tts_type = settings.tts_type.clone();
            role.settings.voice_lang = settings.voice_lang.clone();
            role.settings.voice_models = settings.voice_models.clone();
            role.voice_maker = Some(vm);
            ok += 1;
        }
        if ok > 0 {
            tracing::info!("生成语音预热：已按最新设置刷新 {} 个角色的 VoiceMaker", ok);
        }
        ok
    }

    pub fn clear_all_memories(&mut self) {
        for r in self.loaded_roles.values_mut() {
            r.memory.clear();
        }
        tracing::info!("所有角色的短期记忆已清除");
    }

    async fn register_role_by_id(&mut self, db: &DatabaseConnection, role_id: i32) -> Result<()> {
        let role = RoleRepo::get_role_by_id(db, role_id).await?;
        let role = role.ok_or_else(|| anyhow!("角色 ID {} 未在数据库中找到", role_id))?;

        let settings = RoleRepo::get_role_settings_by_id(db, &self.data_dir, role.id).await?;
        let settings = settings.ok_or_else(|| anyhow!("角色 ID {} 的设置相关文件缺失", role_id))?;

        let display_name = settings.ai_name.clone();
        let resource_path = role.resource_folder.clone();

        let voice_maker = build_voice_maker(
            &self.data_dir,
            &settings,
            resource_path.as_deref(),
            &self.tts_config,
            self.local_tts.as_ref(),
        );

        tracing::info!(
            "角色的服装各个优先级的设置如下：{}, {}, {}",
            self.clothes_overrides
                .get(&role.id)
                .map(|s| s.as_str())
                .unwrap_or("None"),
            settings.clothes_name.as_deref().unwrap_or("None"),
            "default"
        );

        // 服装优先级：session store 覆盖 → settings.yml 默认 → "default"
        let clothes = self
            .clothes_overrides
            .get(&role.id)
            .cloned()
            .or_else(|| settings.clothes_name.clone())
            .unwrap_or_else(|| "default".into());

        tracing::info!("角色 {} 的服装设置为：{}", role.id, clothes);

        let new_role = GameRole {
            role_id: Some(role.id),
            display_name: Some(display_name),
            settings,
            resource_path,
            current_clothes: clothes,
            voice_maker,
            ..Default::default()
        };
        self.loaded_roles.insert(role.id, new_role);
        Ok(())
    }

    /// 通过 script_key/script_role_key 获取运行时角色。
    pub async fn get_role_by_script_keys(
        &mut self,
        db: &DatabaseConnection,
        script_key: &str,
        script_role_key: &str,
    ) -> Result<&mut GameRole> {
        let role = RoleRepo::get_role_by_script_keys(db, script_key, script_role_key)
            .await?
            .ok_or_else(|| {
                anyhow!(
                    "数据库中未找到角色：script_key={}, script_role_key={}，说明本角色所属剧本未初始化",
                    script_key, script_role_key
                )
            })?;
        self.get_role(db, role.id).await
    }

    /// Synchronize role contexts for one explicit canonical-history change.
    ///
    /// `Append` is intentionally scoped to roles visible in the appended suffix;
    /// `Rewrite`/`ReplaceAll` invalidate affected runtime jobs and rebuild every
    /// loaded role so a removed role cannot retain stale short-term context.
    /// `Preview` is an explicit context-only history change: it is selected at
    /// the GameStatus boundary for every mutation while preview mode is active.
    pub async fn sync_memories(
        &mut self,
        db: &DatabaseConnection,
        lines: &[GameLine],
        change: HistoryChange,
    ) -> Result<()> {
        if let Some(from_idx) = change.rewrite_from() {
            self.rewrite_memory_history(from_idx).await;
        }

        let source_lines: &[GameLine] = match change.append_from() {
            Some(from_idx) => &lines[from_idx.min(lines.len())..],
            None => lines,
        };
        // Collect roles visible in the current update scope.
        let mut involved_ids: HashSet<i32> = HashSet::new();
        for line in source_lines {
            if let Some(sid) = line.sender_role_id() {
                // 跳过 id 为 0 的角色（ 0 代表的是玩家，不参与记忆同步）
                if sid != 0 {
                    involved_ids.insert(sid);
                }
            }
            for rid in &line.perceived_role_ids {
                involved_ids.insert(*rid);
            }
        }

        if change.rewrite_from().is_some()
            || matches!(change, HistoryChange::Preview | HistoryChange::Restore)
        {
            // Rebuild every loaded role after a rewrite or matching restore.
            // Roles no longer visible still receive an empty current-history
            // build instead of retaining their previous conversation context.
            involved_ids.extend(self.loaded_roles.keys().copied());
        }

        for rid in involved_ids {
            // 保证角色已加载
            let _ = self.get_role(db, rid).await?;

            // Stage 1: role loading owns persona resources; the coordinator
            // independently owns the permanent bank from this point on.
            let display_name = self
                .loaded_roles
                .get(&rid)
                .and_then(|role| role.display_name.clone())
                .unwrap_or_else(|| "AI".to_string());
            self.memory.ensure(
                rid,
                &GameMemoryBank::default(),
                &display_name,
                self.llm.clone(),
            );
            let mb_enabled = self.memory.config().enabled;

            // 阶段 2: MemoryBank 启用时 — 同步后台结果 + 触发压缩 + 获取记忆文本
            let (mb_exists, slice_start, system_addendum, short_term_prefix) = {
                let sys = self.memory.runtime(rid);
                match sys {
                    Some(s) if s.is_enabled() => {
                        if !self.memory.is_preview() {
                            // Compression always sees canonical global history;
                            // an Append suffix is only an optimization for the
                            // affected-role set, never a replacement history.
                            s.check_and_trigger_auto_update(lines);
                        }
                        let start = s.get_slice_start_index(lines).await;
                        let sys_text = s.get_system_memory_text().await;
                        let short = s.get_short_term_user_text().await;
                        (true, start, sys_text, short)
                    },
                    Some(_) => (true, 0, String::new(), String::new()),
                    None => (false, 0, String::new(), String::new()),
                }
            };

            // 阶段 3: 裁剪 + 构建角色记忆
            let sliced: Vec<GameLine> = if slice_start > 0 && slice_start <= lines.len() {
                lines[slice_start..].to_vec()
            } else {
                lines.to_vec()
            };

            // 确保人设 SYSTEM 提示存在
            let has_prompt = Self::find_first_system_prompt(&sliced, rid).is_some();
            let mut final_sliced = sliced;
            if !has_prompt {
                if let Some(sp) = Self::find_first_system_prompt(lines, rid) {
                    final_sliced.insert(0, sp.clone());
                } else {
                    tracing::warn!("role_id={} 没有找到 SYSTEM 属性的台词，可能人设丢失", rid);
                }
            }

            let built = MemoryBuilder::new(rid).build(&final_sliced);

            // 阶段 4: 写入角色记忆
            if let Some(role) = self.loaded_roles.get_mut(&rid) {
                let use_mb = mb_exists && mb_enabled && !system_addendum.is_empty();
                role.memory = if use_mb {
                    Self::merge_memory_bank_into_context(
                        built,
                        &system_addendum,
                        &short_term_prefix,
                    )
                } else {
                    built
                };
            }
        }

        Ok(())
    }

    // ── MemoryBank 集成方法 ──

    /// Enter or leave preview permanent-memory mode.
    ///
    /// Entering first invalidates every normal-session job, then publishes
    /// Preview mode. Thus a completion either commits before this transition
    /// begins or observes the new epoch and is discarded; preview mutations
    /// cannot commit into the formal bank. Callers must not separately
    /// invalidate history for the normal preview-entry path.
    pub fn set_memory_preview(&mut self, preview: bool) {
        if preview && !self.memory.is_preview() {
            self.invalidate_memory_history();
        }
        self.memory.set_mode(if preview {
            MemoryMode::Preview
        } else {
            MemoryMode::Normal
        });
    }

    pub fn is_memory_preview(&self) -> bool {
        self.memory.is_preview()
    }

    /// 台词历史即将重建；让所有进行中的摘要任务在提交时自动作废。
    pub fn invalidate_memory_history(&self) {
        for system in self.memory.runtimes() {
            system.invalidate_history();
        }
    }

    /// Wait for every loaded role's owned memory task to finish.
    pub async fn wait_memory_updates(&self, timeout: std::time::Duration) -> bool {
        let deadline = tokio::time::Instant::now() + timeout;
        for system in self.memory.runtimes() {
            let remaining = deadline.saturating_duration_since(tokio::time::Instant::now());
            if !system.wait_until_idle(remaining).await {
                return false;
            }
        }
        true
    }

    /// Cancel and join every loaded role's owned memory task.
    pub async fn abort_memory_updates(&self) {
        for system in self.memory.runtimes() {
            system.abort_and_wait().await;
        }
    }

    /// Apply a history rewrite to every loaded memory runtime. Rewrites before
    /// an already processed prefix reset that bank; appends must not call this.
    pub async fn rewrite_memory_history(&self, from_idx: usize) {
        for system in self.memory.runtimes() {
            system.rewrite_from(from_idx).await;
        }
    }

    /// 从 DB 加载 MemoryBank 到运行时缓存。应在 "载入存档" 时调用。
    pub async fn load_memory_banks_from_db(
        &mut self,
        db: &DatabaseConnection,
        save_id: i32,
        role_ids: Option<&[i32]>,
    ) -> Result<()> {
        // MemoryRepo owns compatibility selection and JSON decoding. A malformed
        // newest row returns a contextual error; malformed superseded rows do not.
        let mut banks = MemoryRepo::load_for_save(db, save_id).await?;

        // 未指定角色时，当前会话中的角色也必须纳入目标集合；否则新存档缺少
        // 某角色时旧 bank 会泄漏到新会话。
        let mut target_ids: Vec<i32> = match role_ids {
            Some(ids) => ids.to_vec(),
            None => self
                .loaded_roles
                .keys()
                .chain(banks.keys())
                .copied()
                .collect(),
        };
        target_ids.sort_unstable();
        target_ids.dedup();

        for rid in target_ids {
            let bank = banks.remove(&rid).unwrap_or_default();
            let _ = self.get_role(db, rid).await?;

            // Missing DB rows reset the unique runtime to default instead of
            // retaining a previous save's bank. Short-term context is rebuilt
            // from current history after the load.
            if let Some(role) = self.loaded_roles.get_mut(&rid) {
                role.memory.clear();
            }

            let display_name = self
                .loaded_roles
                .get(&rid)
                .and_then(|role| role.display_name.clone())
                .unwrap_or_else(|| "AI".to_string());
            self.memory
                .ensure(rid, &bank, &display_name, self.llm.clone());

            if let Some(sys) = self.memory.runtime(rid) {
                sys.reset_from(&bank).await;
            }
        }
        Ok(())
    }

    /// 用最新角色配置更新已加载角色的 TTS 设置，并立即重建 VoiceMaker。
    ///
    /// 返回角色当前是否已经加载；未加载时磁盘配置仍会在下次注册角色时生效。
    pub fn update_role_voice_settings(
        &mut self,
        role_id: i32,
        settings: &CharacterSettings,
    ) -> bool {
        let Some(resource_path) = self
            .loaded_roles
            .get(&role_id)
            .map(|role| role.resource_path.clone())
        else {
            tracing::info!("角色 {} 尚未加载，TTS 设置将在下次加载时生效", role_id);
            return false;
        };

        let voice_maker = build_voice_maker(
            &self.data_dir,
            settings,
            resource_path.as_deref(),
            &self.tts_config,
            self.local_tts.as_ref(),
        );
        let voice_maker_ready = voice_maker.is_some();

        let role = self
            .loaded_roles
            .get_mut(&role_id)
            .expect("更新 TTS 设置时已加载的角色消失了");
        role.settings.tts_type = settings.tts_type.clone();
        role.settings.voice_lang = settings.voice_lang.clone();
        role.settings.voice_models = settings.voice_models.clone();
        role.voice_maker = voice_maker;

        tracing::info!(
            "角色 {} TTS 已实时刷新: type={}, lang={}, ready={}",
            role_id,
            role.settings.tts_type.as_deref().unwrap_or(""),
            role.settings.voice_lang.as_deref().unwrap_or(""),
            voice_maker_ready,
        );
        true
    }

    pub fn update_role_live2d_settings(
        &mut self,
        role_id: i32,
        settings: &CharacterSettings,
    ) -> bool {
        let Some(role) = self.loaded_roles.get_mut(&role_id) else {
            tracing::info!("角色 {} 尚未加载，Live2D 设置将在下次加载时生效", role_id);
            return false;
        };
        role.settings.live2d = settings.live2d.clone();
        true
    }

    /// 更新已加载角色的语音语言并重新初始化其 VoiceMaker。
    pub fn update_role_voice_lang(&mut self, role_id: i32, lang: &str) {
        let Some(role) = self.loaded_roles.get_mut(&role_id) else {
            tracing::warn!("update_role_voice_lang: 角色 {} 未加载", role_id);
            return;
        };

        // 同步角色 settings 中的 voice_lang
        role.settings.voice_lang = Some(lang.to_string());

        let Some(vm) = role.voice_maker.as_mut() else {
            tracing::info!("角色 {} 无 VoiceMaker，仅更新设置项", role_id);
            return;
        };

        let tts_type = role.settings.tts_type.clone().unwrap_or_default();
        if tts_type.is_empty() {
            tracing::warn!("角色 {} 未设置 tts_type，无法切换语言", role_id);
            return;
        }

        // OpenTTS 音色标识：角色级优先，留空由 VoiceMaker 回退到全局配置
        let voice_cfg = role.settings.voice_models.clone().unwrap_or_default();
        let name = role.settings.ai_name.clone();

        vm.update_lang_and_refresh(&voice_cfg, &tts_type, &name, lang);
    }

    /// Read one role's memory runtime snapshot.
    pub fn memory_snapshot(
        &self,
        role_id: i32,
    ) -> Option<crate::ai_service::memory::MemorySnapshot> {
        let system = self.memory.runtime(role_id)?;
        Some(system.snapshot())
    }

    #[cfg(test)]
    pub(crate) fn memory_runtime_for_test(
        &self,
        role_id: i32,
    ) -> Option<&crate::ai_service::memory::PersistentMemorySystem> {
        self.memory.runtime(role_id)
    }

    pub async fn memory_system_text(&self, role_id: i32) -> String {
        match self.memory.runtime(role_id) {
            Some(system) => system.get_system_memory_text().await,
            None => String::new(),
        }
    }

    pub async fn memory_short_term_text(&self, role_id: i32) -> String {
        match self.memory.runtime(role_id) {
            Some(system) => system.get_short_term_user_text().await,
            None => String::new(),
        }
    }

    /// Capture all loaded banks once, in stable role order, for a save session.
    pub fn memory_bank_snapshots(&self) -> Vec<(i32, GameMemoryBank, u64)> {
        let mut target_ids: Vec<i32> = self.loaded_roles.keys().copied().collect();
        target_ids.sort_unstable();
        target_ids.dedup();
        let mut snapshots = Vec::with_capacity(target_ids.len());
        for rid in target_ids {
            if let Some(sys) = self.memory.runtime(rid) {
                let snapshot = sys.snapshot();
                snapshots.push((rid, snapshot.bank, snapshot.revision));
            } else {
                // This can only occur for a manually injected test role that
                // has not yet crossed the manager facade. Production loading
                // always calls get_role(), which installs its runtime first.
                tracing::warn!("MemoryBank: loaded role {} has no runtime", rid);
            }
        }
        snapshots
    }

    /// 将 MemoryBank 文本合并到 LLM 消息中。
    ///
    /// - `system_addendum`：合并到第一条 system 消息末尾
    /// - `short_term_prefix`：前置到第一条 user 消息；没有 user 时在 system 后插入
    ///
    /// 另会合并连续出现的多条 system 消息为一条。
    fn merge_memory_bank_into_context(
        memory: Vec<LlmMessage>,
        system_addendum: &str,
        short_term_prefix: &str,
    ) -> Vec<LlmMessage> {
        let mut out = memory;

        if !system_addendum.trim().is_empty() {
            if let Some(first) = out.first_mut() {
                if first.role == "system" {
                    let content = &first.content;
                    if !content.contains(system_addendum) {
                        first.content = format!("{}{}", content, system_addendum);
                    }
                } else {
                    out.insert(0, LlmMessage::system(system_addendum));
                }
            } else {
                out.push(LlmMessage::system(system_addendum));
            }
        }

        if !short_term_prefix.trim().is_empty() {
            let insert_at = out
                .iter()
                .position(|message| message.role != "system")
                .unwrap_or(out.len());
            if out
                .get(insert_at)
                .is_some_and(|message| message.role == "user")
            {
                let first_user = &mut out[insert_at];
                if !first_user.content.contains(short_term_prefix) {
                    first_user.content = format!("{}{}", short_term_prefix, first_user.content);
                }
            } else {
                out.insert(insert_at, LlmMessage::user(short_term_prefix));
            }
        }

        // 合并连续 system 消息
        let mut cleaned: Vec<LlmMessage> = Vec::new();
        for msg in out {
            if let Some(last) = cleaned.last_mut() {
                if last.role == "system" && msg.role == "system" {
                    last.content = format!("{}\n{}", last.content, msg.content);
                    continue;
                }
            }
            cleaned.push(msg);
        }
        cleaned
    }

    // ── 内部辅助方法（已有，未修改） ──

    fn find_first_system_prompt(lines: &[GameLine], role_id: i32) -> Option<&GameLine> {
        lines.iter().find(|l| {
            matches!(l.attribute(), LineAttribute::System) && l.sender_role_id() == Some(role_id)
        })
    }

    /// 提供给 memory_builder 之外的工具：把 `memory` 合并成 `[{role,content}, ...]` 的 serde 形式。
    pub fn memory_as_json(&self, role_id: i32) -> Option<Vec<LlmMessage>> {
        self.loaded_roles.get(&role_id).map(|r| r.memory.clone())
    }
}

#[cfg(test)]
mod memory_bank_context_tests {
    use super::GameRoleManager;
    use crate::ai_service::llm::LlmSlot;
    use crate::ai_service::memory::{MemoryConfig, MemorySectionLimits, PersistentMemorySystem};
    use crate::ai_service::types::{GameMemoryBank, LlmMessage};
    use crate::config::tts::TtsConfig;
    use std::path::PathBuf;
    use std::sync::Arc;
    use tokio::sync::RwLock;

    #[test]
    fn invalidation_covers_systems_for_roles_no_longer_present_in_history() {
        let llm: LlmSlot = Arc::new(RwLock::new(None));
        let mut manager = GameRoleManager::new(
            PathBuf::new(),
            llm.clone(),
            TtsConfig::default(),
            None,
            MemoryConfig {
                enabled: true,
                update_interval: 250,
                recent_window: 30,
                limits: MemorySectionLimits::default(),
            },
        );
        for role_id in [1, 2] {
            manager.memory.insert_for_test(
                role_id,
                PersistentMemorySystem::new(
                    role_id,
                    &GameMemoryBank::default(),
                    llm.clone(),
                    true,
                    250,
                    30,
                    MemorySectionLimits::default(),
                    "AI",
                ),
            );
        }

        manager.invalidate_memory_history();
        assert!(
            manager
                .memory
                .runtimes()
                .all(|system| system.history_revision_for_test() == 1)
        );
    }

    #[test]
    fn short_term_summary_is_prepended_to_the_first_user_message_once() {
        let output = GameRoleManager::merge_memory_bank_into_context(
            vec![LlmMessage::system("persona"), LlmMessage::user("hello")],
            "
MEMORY
",
            "【近期回顾】summary

",
        );
        assert_eq!(output[0].role, "system");
        assert!(output[0].content.contains("MEMORY"));
        assert_eq!(output[1].role, "user");
        assert_eq!(
            output[1].content,
            "【近期回顾】summary

hello"
        );
        assert_eq!(output[1].content.matches("【近期回顾】summary").count(), 1);
    }

    #[test]
    fn short_term_summary_is_inserted_before_an_earlier_assistant_block() {
        let output = GameRoleManager::merge_memory_bank_into_context(
            vec![
                LlmMessage::system("persona"),
                LlmMessage::assistant("older assistant"),
                LlmMessage::user("later user"),
            ],
            "",
            "【近期回顾】summary

",
        );
        assert_eq!(output[0].role, "system");
        assert_eq!(
            output[1],
            LlmMessage::user(
                "【近期回顾】summary

"
            )
        );
        assert_eq!(output[2].role, "assistant");
        assert_eq!(output[3].content, "later user");
    }

    #[test]
    fn short_term_summary_is_inserted_when_no_user_message_exists() {
        let output = GameRoleManager::merge_memory_bank_into_context(
            vec![
                LlmMessage::system("persona"),
                LlmMessage::assistant("hello"),
            ],
            "",
            "【近期回顾】summary

",
        );
        assert_eq!(output[0].role, "system");
        assert_eq!(
            output[1],
            LlmMessage::user(
                "【近期回顾】summary

"
            )
        );
        assert_eq!(output[2].role, "assistant");
    }
}

/// 根据 `CharacterSettings.tts_type` 与 `voice_models` 构造角色的 `VoiceMaker`。
///
/// 未启用 TTS / 配置缺失时返回 `None`。对应 Python `GameRole` 构造时调用
/// `voice_maker = VoiceMaker(...)`。
fn build_voice_maker(
    data_dir: &Path,
    settings: &CharacterSettings,
    resource_path: Option<&str>,
    tts_config: &TtsConfig,
    local_tts: Option<&LocalTtsRuntime>,
) -> Option<VoiceMaker> {
    let tts_type = settings.tts_type.as_deref().unwrap_or("").trim();
    if tts_type.is_empty() {
        return None;
    }
    // OpenTTS 音色标识：角色级 voice_models.opentts_voice 优先，
    // 留空时由 VoiceMaker 回退到全局 TTS 配置（tts.opentts_voice）
    let voice_cfg = settings.voice_models.clone().unwrap_or_default();

    let audio_format = tts_config.audio_format.clone();
    let lang = settings
        .voice_lang
        .as_deref()
        .map(|s| s.trim())
        .filter(|s| !s.is_empty())
        .unwrap_or(&tts_config.voice_lang)
        .to_string();

    let temp_dir = data_dir.join("voice");
    let mut vm = VoiceMaker::new(temp_dir, audio_format, tts_config.clone());
    vm.set_local_runtime(local_tts.cloned());
    vm.set_lang(&lang);
    vm.set_voice_dialect(settings.voice_dialect.clone());
    if let Some(p) = resource_path {
        vm.set_character_path(Some(resolve_character_path(data_dir, p)));
    }
    match vm.set_tts_settings(&voice_cfg, tts_type, &settings.ai_name) {
        Ok(()) => Some(vm),
        Err(e) => {
            tracing::warn!("VoiceMaker 初始化失败: {e}");
            None
        },
    }
}
