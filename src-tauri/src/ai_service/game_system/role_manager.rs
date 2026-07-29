use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};
use std::sync::Arc;

use anyhow::{anyhow, Result};
use sea_orm::DatabaseConnection;

use crate::ai_service::game_system::memory_builder::MemoryBuilder;
use crate::ai_service::game_system::persistent_memory_system::PersistentMemorySystem;
use crate::ai_service::llm::LlmSlot;
use crate::ai_service::tts::VoiceMaker;
use crate::ai_service::tts::local::engine::LocalTtsEngine;
use crate::ai_service::tts::local::paths::LocalTtsPaths;
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
    /// 每个角色的 MemoryBank 后台压缩引擎（惰性构造）。
    memory_bank_systems: HashMap<i32, PersistentMemorySystem>,
    /// TTS 引擎配置（适配器 URL、音频格式等）。
    tts_config: TtsConfig,
    /// Local TTS in-process engine (SBV2). Forwarded to each VoiceMaker so
    /// the `sbv2_local` adapter can lazy-init the DeBerta holder + voice.
    local_tts_engine: Option<Arc<LocalTtsEngine>>,
    /// Filesystem layout for local TTS assets. Forwarded alongside the engine.
    local_tts_paths: Option<LocalTtsPaths>,
    local_tts_switch: Option<crate::ai_service::tts::local::LocalTtsSwitch>,
    /// 全局永久记忆开关（来自 `AppConfig::use_persistent_memory`）。
    use_persistent_memory: bool,
    /// 触发记忆摘要的新消息数（来自 `AppConfig::memory_update_interval`）。
    memory_update_interval: u32,
    /// 摘要时保留的最近消息数（来自 `AppConfig::memory_recent_window`）。
    memory_recent_window: u32,
    /// 角色服装覆盖（session store → register_role_by_id 时优先读取）
    clothes_overrides: HashMap<i32, String>,
}

impl GameRoleManager {
    pub fn new(
        data_dir: PathBuf,
        llm: LlmSlot,
        tts_config: TtsConfig,
        local_tts_engine: Option<Arc<LocalTtsEngine>>,
        local_tts_paths: Option<LocalTtsPaths>,
        local_tts_switch: Option<crate::ai_service::tts::local::LocalTtsSwitch>,
        use_persistent_memory: bool,
        memory_update_interval: u32,
        memory_recent_window: u32,
    ) -> Self {
        Self {
            loaded_roles: HashMap::new(),
            data_dir,
            llm,
            memory_bank_systems: HashMap::new(),
            tts_config,
            local_tts_engine,
            local_tts_paths,
            local_tts_switch,
            use_persistent_memory,
            memory_update_interval,
            memory_recent_window,
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

    /// Inject the in-process local TTS engine + resolved paths. Called once
    /// from `lib.rs` setup right after `LocalTtsState` is constructed, so
    /// every subsequently-built VoiceMaker picks them up.
    pub fn set_local_tts_engine(
        &mut self,
        engine: Option<Arc<LocalTtsEngine>>,
        paths: Option<LocalTtsPaths>,
    ) {
        self.local_tts_engine = engine;
        self.local_tts_paths = paths;
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
        Ok(self
            .loaded_roles
            .get_mut(&role_id)
            .expect("role just inserted"))
    }

    pub fn get_loaded(&self, role_id: i32) -> Option<&GameRole> {
        self.loaded_roles.get(&role_id)
    }

    pub fn get_loaded_mut(&mut self, role_id: i32) -> Option<&mut GameRole> {
        self.loaded_roles.get_mut(&role_id)
    }

    pub fn reset_roles(&mut self) {
        self.loaded_roles.clear();
        self.memory_bank_systems.clear();
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
            self.local_tts_engine.as_ref(),
            self.local_tts_paths.as_ref(),
            self.local_tts_switch.as_ref(),
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

    /// 根据台词同步角色的状态和记忆。
    pub async fn sync_memories(
        &mut self,
        db: &DatabaseConnection,
        lines: &[GameLine],
        recent_n: Option<usize>,
    ) -> Result<()> {
        let source_lines: &[GameLine] = match recent_n {
            Some(n) if n < lines.len() => &lines[lines.len() - n..],
            _ => lines,
        };
        let mut involved_ids: HashSet<i32> = HashSet::new();
        for line in source_lines {
            if let Some(sid) = line.sender_role_id() {
                if sid != 0 {
                    involved_ids.insert(sid);
                }
            }
            for rid in &line.perceived_role_ids {
                involved_ids.insert(*rid);
            }
        }

        for rid in involved_ids {
            let _ = self.get_role(db, rid).await?;

            let (display_name, bank_clone, mb_enabled) = {
                let role = self.loaded_roles.get(&rid).expect("role just loaded");
                let name = role
                    .display_name
                    .clone()
                    .unwrap_or_else(|| "AI".to_string());
                let bank = role.memory_bank.clone();
                let enabled = self.use_persistent_memory;
                (name, bank, enabled)
            };
            self.ensure_memory_bank_system(
                rid,
                &bank_clone,
                &display_name,
                mb_enabled,
                self.memory_update_interval as usize,
                self.memory_recent_window as usize,
            );

            let (mb_exists, slice_start, system_addendum, short_term_prefix) = {
                let sys = self.memory_bank_systems.get(&rid);
                match sys {
                    Some(s) if s.is_enabled() => {
                        if let Some(role) = self.loaded_roles.get_mut(&rid) {
                            s.sync_to_role(role);
                        }
                        s.check_and_trigger_auto_update(source_lines);
                        let start = s.get_slice_start_index().await;
                        let sys_text = s.get_system_memory_text().await;
                        let short = s.get_short_term_user_text().await;
                        (true, start, sys_text, short)
                    }
                    Some(_) => (true, 0, String::new(), String::new()),
                    None => (false, 0, String::new(), String::new()),
                }
            };

            let sliced: Vec<GameLine> = if slice_start > 0 && slice_start < source_lines.len() {
                source_lines[slice_start..].to_vec()
            } else {
                source_lines.to_vec()
            };

            let has_prompt = Self::find_first_system_prompt(&sliced, rid).is_some();
            let mut final_sliced = sliced;
            if !has_prompt {
                if let Some(sp) = Self::find_first_system_prompt(source_lines, rid) {
                    final_sliced.insert(0, sp.clone());
                } else {
                    tracing::warn!("role_id={} 没有找到 SYSTEM 属性的台词，可能人设丢失", rid);
                }
            }

            let built = MemoryBuilder::new(rid).build(&final_sliced);

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

    fn ensure_memory_bank_system(
        &mut self,
        role_id: i32,
        bank: &GameMemoryBank,
        display_name: &str,
        enabled: bool,
        update_interval: usize,
        recent_window: usize,
    ) {
        if self.memory_bank_systems.contains_key(&role_id) {
            return;
        }
        if self.llm.try_read().map(|g| g.is_none()).unwrap_or(true) {
            if enabled {
                tracing::warn!(
                    "MemoryBank: role_id={} 永久记忆已开启但 LLM 槽位为空",
                    role_id
                );
            }
            return;
        }
        self.memory_bank_systems.insert(
            role_id,
            PersistentMemorySystem::new(
                role_id,
                bank,
                self.llm.clone(),
                enabled,
                update_interval,
                recent_window,
                display_name,
            ),
        );
    }

    pub async fn load_memory_banks_from_db(
        &mut self,
        db: &DatabaseConnection,
        save_id: i32,
        role_ids: Option<&[i32]>,
    ) -> Result<()> {
        let memories = MemoryRepo::get_memories(db, save_id, None).await?;

        let mut best: HashMap<i32, (i32, serde_json::Value)> = HashMap::new();
        for m in &memories {
            let Some(rid) = m.role_id else { continue };
            let mid = m.id;
            if !best.contains_key(&rid) || mid > best[&rid].0 {
                best.insert(
                    rid,
                    (mid, serde_json::from_str(&m.info).unwrap_or_default()),
                );
            }
        }

        let target_ids: Vec<i32> = match role_ids {
            Some(ids) => ids.to_vec(),
            None => best.keys().copied().collect(),
        };

        for rid in target_ids {
            let _ = self.get_role(db, rid).await?;

            if let Some((_, info)) = best.get(&rid) {
                if let Ok(mb) = serde_json::from_value::<GameMemoryBank>(info.clone()) {
                    if let Some(role) = self.loaded_roles.get_mut(&rid) {
                        role.memory_bank = mb.clone();
                    }
                }
            }

            let (bank, display_name, enabled) = {
                let role = self.loaded_roles.get(&rid).expect("role just loaded");
                (
                    role.memory_bank.clone(),
                    role.display_name
                        .clone()
                        .unwrap_or_else(|| "AI".to_string()),
                    self.use_persistent_memory,
                )
            };
            self.ensure_memory_bank_system(
                rid,
                &bank,
                &display_name,
                enabled,
                self.memory_update_interval as usize,
                self.memory_recent_window as usize,
            );

            if let Some((_, info)) = best.get(&rid) {
                if let Ok(mb) = serde_json::from_value::<GameMemoryBank>(info.clone()) {
                    if let Some(sys) = self.memory_bank_systems.get(&rid) {
                        sys.reset_from(&mb).await;
                    }
                }
            }
        }
        Ok(())
    }

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
            self.local_tts_engine.as_ref(),
            self.local_tts_paths.as_ref(),
            self.local_tts_switch.as_ref(),
        );
        let voice_maker_ready = voice_maker.is_some();

        let role = self
            .loaded_roles
            .get_mut(&role_id)
            .expect("loaded role disappeared while updating TTS settings");
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

    pub fn update_role_voice_lang(&mut self, role_id: i32, lang: &str) {
        let Some(role) = self.loaded_roles.get_mut(&role_id) else {
            tracing::warn!("update_role_voice_lang: 角色 {} 未加载", role_id);
            return;
        };

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

        let voice_cfg = role.settings.voice_models.clone().unwrap_or_default();
        let name = role.settings.ai_name.clone();

        vm.update_lang_and_refresh(&voice_cfg, &tts_type, &name, lang);
    }

    pub async fn persist_memory_banks_to_db(
        &mut self,
        db: &DatabaseConnection,
        save_id: i32,
        role_ids: Option<&[i32]>,
    ) -> Result<()> {
        for rid in self.loaded_roles.keys().copied().collect::<Vec<_>>() {
            if let Some(sys) = self.memory_bank_systems.get(&rid) {
                if let Some(role) = self.loaded_roles.get_mut(&rid) {
                    sys.sync_to_role(role);
                }
            }
        }

        let target_ids: Vec<i32> = match role_ids {
            Some(ids) => ids.to_vec(),
            None => self.loaded_roles.keys().copied().collect(),
        };

        for rid in target_ids {
            if let Some(role) = self.loaded_roles.get(&rid) {
                let info = serde_json::to_string(&role.memory_bank)?;
                MemoryRepo::upsert_memory(db, save_id, rid, &info, None).await?;
            }
        }
        Ok(())
    }

    fn merge_memory_bank_into_context(
        memory: Vec<LlmMessage>,
        system_addendum: &str,
        _short_term_prefix: &str,
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

    fn find_first_system_prompt(lines: &[GameLine], role_id: i32) -> Option<&GameLine> {
        lines.iter().find(|l| {
            matches!(l.attribute(), LineAttribute::System) && l.sender_role_id() == Some(role_id)
        })
    }

    pub fn memory_as_json(&self, role_id: i32) -> Option<Vec<LlmMessage>> {
        self.loaded_roles.get(&role_id).map(|r| r.memory.clone())
    }
}

fn build_voice_maker(
    data_dir: &Path,
    settings: &CharacterSettings,
    resource_path: Option<&str>,
    tts_config: &TtsConfig,
    local_tts_engine: Option<&Arc<LocalTtsEngine>>,
    local_tts_paths: Option<&LocalTtsPaths>,
    local_tts_switch: Option<&crate::ai_service::tts::local::LocalTtsSwitch>,
) -> Option<VoiceMaker> {
    let tts_type = settings.tts_type.as_deref().unwrap_or("").trim();
    if tts_type.is_empty() {
        return None;
    }
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
    vm.set_local_tts_engine(local_tts_engine.cloned(), local_tts_paths.cloned());
    vm.set_local_tts_switch(local_tts_switch.cloned());
    vm.set_lang(&lang);
    if let Some(p) = resource_path {
        vm.set_character_path(Some(resolve_character_path(data_dir, p)));
    }
    match vm.set_tts_settings(&voice_cfg, tts_type, &settings.ai_name) {
        Ok(()) => Some(vm),
        Err(e) => {
            tracing::warn!("VoiceMaker 初始化失败: {e}");
            None
        }
    }
}
