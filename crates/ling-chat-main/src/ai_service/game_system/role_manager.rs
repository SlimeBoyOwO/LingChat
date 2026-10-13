use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};

use anyhow::{Result, anyhow};
use sea_orm::DatabaseConnection;

use crate::ai_service::game_system::memory_builder::MemoryBuilder;
use crate::ai_service::tts::VoiceMaker;
use crate::ai_service::tts::local::LocalTtsRuntime;
use crate::ai_service::types::{
    AffectionVector, CharacterSettings, GameLine, GameRole, NegativeVector,
};
use crate::config::tts::TtsConfig;
use crate::db::entities::line::LineAttribute;
use crate::db::managers::role_repo::RoleRepo;

/// 角色运行时管理器：维护当前活跃角色的内存状态。
pub struct GameRoleManager {
    pub loaded_roles: HashMap<i32, GameRole>,
    data_dir: PathBuf,
    pub db: DatabaseConnection,

    /// TTS 引擎配置（适配器 URL、音频格式等）。
    tts_config: TtsConfig,
    /// 本地 TTS 共享运行时（进程内引擎 + 路径 + 全局开关）。
    /// 转发给每个 VoiceMaker，使 `sbv2_local` 适配器可以惰性引导。
    local_tts: Option<LocalTtsRuntime>,
    /// 记忆窗口内没有 user 消息时，是否在裁切后的首条 assistant 前注入一条 user「继续」
    /// （来自 `AppConfig::memory_inject_continue_user`）。
    memory_inject_continue_user: bool,
    /// 角色服装覆盖（session store → register_role_by_id 时优先读取）
    clothes_overrides: HashMap<i32, String>,
}

impl GameRoleManager {
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        data_dir: PathBuf,
        db: DatabaseConnection,
        tts_config: TtsConfig,
        local_tts: Option<LocalTtsRuntime>,
        memory_inject_continue_user: bool,
    ) -> Self {
        Self {
            loaded_roles: HashMap::new(),
            data_dir,
            db,
            tts_config,
            local_tts,
            memory_inject_continue_user,
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

    pub fn remap_character_costume(&mut self, role_id: i32, old: &str, new: &str) {
        if self
            .clothes_overrides
            .get(&role_id)
            .is_some_and(|name| name == old)
        {
            self.clothes_overrides.insert(role_id, new.into());
        }
        if let Some(role) = self.loaded_roles.get_mut(&role_id) {
            if role.current_clothes == old {
                role.current_clothes = new.into();
            }
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
        Ok(self.loaded_roles.get_mut(&role_id).expect("角色刚刚插入"))
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
            let settings =
                match RoleRepo::get_role_settings_by_id(db, &self.data_dir, role_id).await {
                    Ok(Some(s)) => s,
                    _ => continue,
                };
            let Some(vm) = build_voice_maker(
                &self.data_dir,
                &settings,
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
        let worldbook = settings
            .resource_path
            .as_deref()
            .and_then(|path| crate::ai_service::worldbook::Worldbook::load(Path::new(path)));

        let voice_maker = build_voice_maker(
            &self.data_dir,
            &settings,
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
            worldbook,
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

    /// 调整角色好感度与负面情绪的内存值；角色未加载时返回 None。
    /// 持久化由调用方写入存档全局变量（见 `affection::var_key`）。
    /// 返回调整后的（好感六维, 负面六维）。
    pub fn adjust_affection(
        &mut self,
        role_id: i32,
        deltas: &[(String, i32)],
        negative_deltas: &[(String, i32)],
    ) -> Option<(AffectionVector, NegativeVector)> {
        let role = self.loaded_roles.get_mut(&role_id)?;
        for (dim, delta) in deltas {
            role.affection.add_delta(dim, *delta);
        }
        for (dim, delta) in negative_deltas {
            role.negative.add_delta(dim, *delta);
        }
        Some((role.affection, role.negative))
    }

    /// 用存档全局变量中的好感度覆盖所有已加载角色的内存值（读档恢复用）。
    pub fn overlay_affections_from_vars(&mut self, vars: &HashMap<String, serde_json::Value>) {
        for role in self.loaded_roles.values_mut() {
            let Some(rid) = role.role_id else {
                continue;
            };
            if let Some(state) = vars
                .get(&crate::ai_service::affection::var_key(rid))
                .and_then(crate::ai_service::affection::state_from_value)
            {
                role.affection = state.vector;
                role.negative = state.negative;
            }
        }
    }

    /// 所有已加载角色的当前好感度状态（role_id 字符串键，便于 JSON 序列化）。
    pub fn loaded_affections(
        &self,
    ) -> HashMap<String, crate::ai_service::affection::AffectionState> {
        self.loaded_roles
            .iter()
            .filter_map(|(id, role)| {
                role.role_id.map(|_| {
                    (
                        id.to_string(),
                        crate::ai_service::affection::AffectionState {
                            total: role.affection.average(),
                            vector: role.affection,
                            negative: role.negative,
                        },
                    )
                })
            })
            .collect()
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

    /// 根据台词同步角色的上下文记忆（裁切窗口外的台词不进上下文）。
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
                // 跳过 id 为 0 的角色（ 0 代表的是玩家，不参与记忆同步）
                if sid != 0 {
                    involved_ids.insert(sid);
                }
            }
            for rid in &line.perceived_role_ids {
                involved_ids.insert(*rid);
            }
        }

        for rid in involved_ids {
            // 保证角色已加载
            let _ = self.get_role(db, rid).await?;

            // 确保人设 SYSTEM 提示存在
            let has_prompt = Self::find_first_system_prompt(source_lines, rid).is_some();
            let mut final_sliced = source_lines.to_vec();
            if !has_prompt {
                if let Some(sp) = Self::find_first_system_prompt(lines, rid) {
                    final_sliced.insert(0, sp.clone());
                } else {
                    tracing::warn!("role_id={} 没有找到 SYSTEM 属性的台词，可能人设丢失", rid);
                }
            }

            let built = MemoryBuilder::new(rid)
                .with_continue_user(self.memory_inject_continue_user)
                .build(&final_sliced);

            if let Some(role) = self.loaded_roles.get_mut(&rid) {
                role.memory = built;
            }
        }

        Ok(())
    }

    // ── 内部辅助方法（已有，未修改） ──

    /// 用最新角色配置更新已加载角色的 TTS 设置，并立即重建 VoiceMaker。
    ///
    /// 返回角色当前是否已经加载；未加载时磁盘配置仍会在下次注册角色时生效。
    pub fn update_role_voice_settings(
        &mut self,
        role_id: i32,
        settings: &CharacterSettings,
    ) -> bool {
        if !self.loaded_roles.contains_key(&role_id) {
            tracing::info!("角色 {} 尚未加载，TTS 设置将在下次加载时生效", role_id);
            return false;
        }

        let voice_maker = build_voice_maker(
            &self.data_dir,
            settings,
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

    /// 刷新已加载角色的形象配置和主对话立绘、气泡布局。
    ///
    /// 这些字段必须一起拷：内存里的 `role.settings` 是之后
    /// `init_game` / 切角色时 `onstage_roles` 的数据源，漏拷会让保存后的
    /// 显示方式在下一次 init 时被打回旧值。
    pub fn update_role_visual_settings(
        &mut self,
        role_id: i32,
        settings: &CharacterSettings,
    ) -> bool {
        let Some(role) = self.loaded_roles.get_mut(&role_id) else {
            tracing::info!("角色 {} 尚未加载，Live2D 设置将在下次加载时生效", role_id);
            return false;
        };
        role.settings.live2d = settings.live2d.clone();
        role.settings.avatar_mode = settings.avatar_mode.clone();
        role.settings.avatar_mode_p = settings.avatar_mode_p.clone();
        role.settings.pet_frameless = settings.pet_frameless;
        role.settings.scale = settings.scale;
        role.settings.offset_x = settings.offset_x;
        role.settings.offset_y = settings.offset_y;
        role.settings.scale_p = settings.scale_p;
        role.settings.offset_x_p = settings.offset_x_p;
        role.settings.offset_y_p = settings.offset_y_p;
        role.settings.bubble_top = settings.bubble_top;
        role.settings.bubble_left = settings.bubble_left;
        role.settings.clothes = settings.clothes.clone();
        role.settings.clothes_name = settings.clothes_name.clone();
        role.settings.body_part = settings.body_part.clone();
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

    fn find_first_system_prompt(lines: &[GameLine], role_id: i32) -> Option<&GameLine> {
        lines.iter().find(|l| {
            matches!(l.attribute(), LineAttribute::System) && l.sender_role_id() == Some(role_id)
        })
    }
}

/// 根据 `CharacterSettings.tts_type` 与 `voice_models` 构造角色的 `VoiceMaker`。
///
/// 未启用 TTS / 配置缺失时返回 `None`。对应 Python `GameRole` 构造时调用
/// `voice_maker = VoiceMaker(...)`。
pub(crate) fn build_voice_maker(
    data_dir: &Path,
    settings: &CharacterSettings,
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
    match vm.set_tts_settings(&voice_cfg, tts_type, &settings.ai_name) {
        Ok(()) => Some(vm),
        Err(e) => {
            tracing::warn!("VoiceMaker 初始化失败: {e}");
            None
        },
    }
}
