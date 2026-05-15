use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};

use anyhow::{anyhow, Result};
use sea_orm::DatabaseConnection;

use crate::ai_service::game_system::memory_builder::MemoryBuilder;
use crate::ai_service::tts::VoiceMaker;
use crate::ai_service::types::{CharacterSettings, GameLine, GameRole, LlmMessage};
use crate::db::entities::line::LineAttribute;
use crate::db::managers::role_repo::RoleRepo;

/// 角色运行时管理器：维护当前活跃角色的内存状态。
pub struct GameRoleManager {
    pub loaded_roles: HashMap<i32, GameRole>,
    data_dir: PathBuf,
}

impl GameRoleManager {
    pub fn new(data_dir: PathBuf) -> Self {
        Self {
            loaded_roles: HashMap::new(),
            data_dir,
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

        let voice_maker = build_voice_maker(&self.data_dir, &settings, resource_path.as_deref());

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
    /// 注：Python 版还会在开启"永久记忆"时将 MemoryBank 合入 system/user 消息；
    /// 本轮暂不落地 MemoryBank，因此只执行基础的 MemoryBuilder 构建。
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
                involved_ids.insert(sid);
            }
            for rid in &line.perceived_role_ids {
                involved_ids.insert(*rid);
            }
        }

        for rid in involved_ids {
            // 保证角色已加载
            let _ = self.get_role(db, rid).await?;

            let mut sliced: Vec<GameLine> = source_lines.to_vec();

            // 若切片中缺少本角色的 SYSTEM 提示，则尝试从完整源里补上
            let has_prompt = Self::find_first_system_prompt(&sliced, rid).is_some();
            if !has_prompt {
                if let Some(sp) = Self::find_first_system_prompt(source_lines, rid) {
                    sliced.insert(0, sp.clone());
                } else {
                    log::warn!(
                        "MemoryBank: role_id={} 没有找到 SYSTEM 属性的台词，可能人设丢失",
                        rid
                    );
                }
            }

            let built = MemoryBuilder::new(rid).build(&sliced);
            if let Some(role) = self.loaded_roles.get_mut(&rid) {
                role.memory = built;
            }
        }

        Ok(())
    }

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
) -> Option<VoiceMaker> {
    let tts_type = settings.tts_type.as_deref().unwrap_or("").trim();
    if tts_type.is_empty() {
        return None;
    }
    let voice_cfg = settings.voice_models.as_ref()?;

    let audio_format = std::env::var("TTS_AUDIO_FORMAT").unwrap_or_else(|_| "wav".to_string());
    let lang = std::env::var("VOICE_LANG").unwrap_or_else(|_| "ja".to_string());

    let temp_dir = data_dir.join("voice");
    let mut vm = VoiceMaker::new(temp_dir, audio_format);
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
