use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};
use std::sync::Arc;

use anyhow::{anyhow, Result};
use sea_orm::DatabaseConnection;

use crate::ai_service::game_system::memory_builder::MemoryBuilder;
use crate::ai_service::game_system::persistent_memory_system::PersistentMemorySystem;
use crate::ai_service::llm::LlmClient;
use crate::ai_service::tts::VoiceMaker;
use crate::ai_service::types::{CharacterSettings, GameLine, GameMemoryBank, GameRole, LlmMessage};
use crate::config::tts::TtsConfig;
use crate::db::entities::line::LineAttribute;
use crate::db::managers::memory_repo::MemoryRepo;
use crate::db::managers::role_repo::RoleRepo;

/// 角色运行时管理器：维护当前活跃角色的内存状态。
pub struct GameRoleManager {
    pub loaded_roles: HashMap<i32, GameRole>,
    data_dir: PathBuf,

    /// LLM 客户端（迟注入）。MemoryBank 压缩引擎依赖此字段。
    llm: Option<Arc<LlmClient>>,
    /// 每个角色的 MemoryBank 后台压缩引擎（惰性构造）。
    memory_bank_systems: HashMap<i32, PersistentMemorySystem>,
    /// TTS 引擎配置（适配器 URL、音频格式等）。
    tts_config: TtsConfig,
}

impl GameRoleManager {
    pub fn new(data_dir: PathBuf, llm: Option<Arc<LlmClient>>, tts_config: TtsConfig) -> Self {
        Self {
            loaded_roles: HashMap::new(),
            data_dir,
            llm,
            memory_bank_systems: HashMap::new(),
            tts_config,
        }
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
            log::info!("角色 {} 的短期记忆已清除", role_id);
        } else {
            log::warn!("角色 {} 未在运行时加载，无法清除记忆", role_id);
        }
    }

    pub fn reactivate_all_voice_makers(&self) {
        for role in self.loaded_roles.values() {
            if let Some(vm) = &role.voice_maker {
                vm.reactivate();
            }
        }
        log::info!("所有角色 TTS 已重新启用");
    }

    pub fn clear_all_memories(&mut self) {
        for r in self.loaded_roles.values_mut() {
            r.memory.clear();
        }
        log::info!("所有角色的短期记忆已清除");
    }

    async fn register_role_by_id(
        &mut self,
        db: &DatabaseConnection,
        role_id: i32,
    ) -> Result<()> {
        let role = RoleRepo::get_role_by_id(db, role_id).await?;
        let role = role.ok_or_else(|| anyhow!("角色 ID {} 未在数据库中找到", role_id))?;

        let settings =
            RoleRepo::get_role_settings_by_id(db, &self.data_dir, role.id).await?;
        let settings = settings
            .ok_or_else(|| anyhow!("角色 ID {} 的设置相关文件缺失", role_id))?;

        let display_name = settings.ai_name.clone();
        let resource_path = role.resource_folder.clone();

        let voice_maker = build_voice_maker(&self.data_dir, &settings, resource_path.as_deref(), &self.tts_config);

        let new_role = GameRole {
            role_id: Some(role.id),
            display_name: Some(display_name),
            settings,
            resource_path,
            current_clothes: "default".into(),
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
    ///
    /// 若用户开启了永久记忆（`use_persistent_memory`），则会：
    /// 1. 检查是否触发后台压缩
    /// 2. 裁剪上下文窗口（避免无限膨胀）
    /// 3. 将 MemoryBank 文本合并到 system / user 消息中
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

        // 收集涉及到的角色 ID
        let mut involved_ids: HashSet<i32> = HashSet::new();
        for line in source_lines {
            if let Some(sid) = line.sender_role_id() {
                involved_ids.insert(sid);
            }
            for rid in &line.perceived_role_ids {
                involved_ids.insert(*rid);
            }
        }

        for rid in involved_ids {
            // 保证角色已加载
            let _ = self.get_role(db, rid).await?;

            // Phase 1: 提取角色数据后释放借用，再惰性构造 MemoryBank 系统
            let (display_name, bank_clone, mb_enabled) = {
                let role = self
                    .loaded_roles
                    .get(&rid)
                    .expect("role just loaded");
                let name = role
                    .display_name
                    .clone()
                    .unwrap_or_else(|| "AI".to_string());
                let bank = role.memory_bank.clone();
                let enabled = role
                    .settings
                    .extra
                    .get("use_persistent_memory")
                    .and_then(|v| v.as_bool())
                    .unwrap_or(false);
                (name, bank, enabled)
            };
            self.ensure_memory_bank_system(
                rid,
                &bank_clone,
                &display_name,
                mb_enabled,
                250, // update_interval default
                30,  // recent_window default
            );

            // Phase 2: MemoryBank 启用时 — 同步后台结果 + 触发压缩 + 获取记忆文本
            let (mb_exists, slice_start, system_addendum, short_term_prefix) = {
                let sys = self.memory_bank_systems.get(&rid);
                match sys {
                    Some(s) if s.is_enabled() => {
                        // 非阻塞同步后台压缩结果
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

            // Phase 3: 裁剪 + 构建角色记忆
            let sliced: Vec<GameLine> = if slice_start > 0 && slice_start < source_lines.len() {
                source_lines[slice_start..].to_vec()
            } else {
                source_lines.to_vec()
            };

            // 确保人设 SYSTEM 提示存在
            let has_prompt = Self::find_first_system_prompt(&sliced, rid).is_some();
            let mut final_sliced = sliced;
            if !has_prompt {
                if let Some(sp) = Self::find_first_system_prompt(source_lines, rid) {
                    final_sliced.insert(0, sp.clone());
                } else {
                    log::warn!(
                        "role_id={} 没有找到 SYSTEM 属性的台词，可能人设丢失",
                        rid
                    );
                }
            }

            let built = MemoryBuilder::new(rid).build(&final_sliced);

            // Phase 4: 写入角色记忆
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

    /// 惰性构造角色的 `PersistentMemorySystem`（若尚未创建且 LLM 客户段就绪）。
    /// 惰性构造角色的 `PersistentMemorySystem`。
    ///
    /// 调用方保证在 `enabled=true` 时 `self.llm` 已就绪（构造函数注入）。
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
        let Some(ref llm) = self.llm else {
            // 仅在 enabled 但 LLM 缺失时告警（正常启动流程不应到达）
            if enabled {
                log::warn!("MemoryBank: role_id={} 永久记忆已开启但 LLM 未就绪", role_id);
            }
            return;
        };
        self.memory_bank_systems.insert(
            role_id,
            PersistentMemorySystem::new(
                role_id,
                bank,
                llm.clone(),
                enabled,
                update_interval,
                recent_window,
                display_name,
            ),
        );
    }

    /// 从 DB 加载 MemoryBank 到运行时缓存。应在 "载入存档" 时调用。
    pub async fn load_memory_banks_from_db(
        &mut self,
        db: &DatabaseConnection,
        save_id: i32,
        role_ids: Option<&[i32]>,
    ) -> Result<()> {
        let memories = MemoryRepo::get_memories(db, save_id, None).await?;

        // 每个 role 取最新（id 最大）的记录
        let mut best: HashMap<i32, (i32, serde_json::Value)> = HashMap::new();
        for m in &memories {
            let Some(rid) = m.role_id else { continue };
            let mid = m.id;
            if !best.contains_key(&rid) || mid > best[&rid].0 {
                best.insert(rid, (mid, serde_json::from_str(&m.info).unwrap_or_default()));
            }
        }

        let target_ids: Vec<i32> = match role_ids {
            Some(ids) => ids.to_vec(),
            None => best.keys().copied().collect(),
        };

        for rid in target_ids {
            let _ = self.get_role(db, rid).await?;

            // 更新 role.memory_bank（DB → 内存）
            if let Some((_, info)) = best.get(&rid) {
                if let Ok(mb) = serde_json::from_value::<GameMemoryBank>(info.clone()) {
                    if let Some(role) = self.loaded_roles.get_mut(&rid) {
                        role.memory_bank = mb.clone();
                    }
                }
            }

            // 提取数据（释放借用后传递给 ensure）
            let (bank, display_name, enabled) = {
                let role = self
                    .loaded_roles
                    .get(&rid)
                    .expect("role just loaded");
                (
                    role.memory_bank.clone(),
                    role.display_name
                        .clone()
                        .unwrap_or_else(|| "AI".to_string()),
                    role.settings
                        .extra
                        .get("use_persistent_memory")
                        .and_then(|v| v.as_bool())
                        .unwrap_or(false),
                )
            };
            self.ensure_memory_bank_system(
                rid,
                &bank,
                &display_name,
                enabled,
                250,
                30,
            );

            // 若已有压缩系统且 DB 有数据，同步重置
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

    /// 将运行时缓存（GameRole.memory_bank）写入 DB。应在 "保存存档" 时调用。
    pub async fn persist_memory_banks_to_db(
        &mut self,
        db: &DatabaseConnection,
        save_id: i32,
        role_ids: Option<&[i32]>,
    ) -> Result<()> {
        // 先同步所有压缩系统的最新状态
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

    /// 将 MemoryBank 文本合并到 LLM 消息中。
    ///
    /// - `system_addendum`：合并到第一条 system 消息末尾
    /// - `short_term_prefix`：保留参数（Python 版对应的 user 前缀合并已注释，此处同步）
    ///
    /// 另会合并连续出现的多条 system 消息为一条。
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

/// 根据 `CharacterSettings.tts_type` 与 `voice_models` 构造角色的 `VoiceMaker`。
///
/// 未启用 TTS / 配置缺失时返回 `None`。对应 Python `GameRole` 构造时调用
/// `voice_maker = VoiceMaker(...)`。
fn build_voice_maker(
    data_dir: &Path,
    settings: &CharacterSettings,
    resource_path: Option<&str>,
    tts_config: &TtsConfig,
) -> Option<VoiceMaker> {
    let tts_type = settings.tts_type.as_deref().unwrap_or("").trim();
    if tts_type.is_empty() {
        return None;
    }
    let voice_cfg = settings.voice_models.as_ref()?;

    let audio_format = tts_config.audio_format.clone();
    let lang = tts_config.voice_lang.clone();

    let temp_dir = data_dir.join("voice");
    let mut vm = VoiceMaker::new(temp_dir, audio_format, tts_config.clone());
    vm.set_lang(&lang);
    if let Some(p) = resource_path {
        vm.set_character_path(Some(PathBuf::from(p)));
    }
    match vm.set_tts_settings(voice_cfg, tts_type, &settings.ai_name) {
        Ok(()) => Some(vm),
        Err(e) => {
            log::warn!("VoiceMaker 初始化失败: {e}");
            None
        }
    }
}
