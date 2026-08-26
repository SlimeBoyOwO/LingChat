//! ScriptManager — script discovery, lifecycle, and chapter orchestration.
//!
//! Replaces Python `ScriptManager` class. Scans the scripts directory for
//! `story_config.yaml` files, manages script start/run/complete, and provides
//! the user-input pause mechanism via oneshot channels.

use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;

use anyhow::{anyhow, Context, Result};
use serde_json::Value;

use crate::ai_service::game_system::script_engine::chapter::Chapter;
use crate::ai_service::game_system::script_engine::events::ScriptContext;
use crate::ai_service::game_system::script_engine::persistent_state;
use crate::ai_service::game_system::script_engine::responses::{
    event_names::SCRIPT_END, ScriptEndPayload,
};
use crate::ai_service::message_system::events::emit;
use crate::ai_service::types::{AdventureConfig, LineAttributeExt, LineBase, ScriptStatus};
use crate::db::entities::line::LineAttribute;
use crate::db::entities::role::RoleType;
use crate::db::managers::role_repo::RoleRepo;
use crate::utils::prompt::{sys_prompt_builder, sys_prompt_builder_by_settings, PromptOptions};
use tauri::Emitter;

/// YAML structure for `story_config.yaml` top-level keys.
#[derive(serde::Deserialize, Default)]
struct StoryConfigRaw {
    script_name: Option<String>,
    intro_chapter: Option<String>,
    description: Option<String>,
    #[serde(default)]
    recommand_start: Option<String>,
    #[serde(default)]
    adventure: Option<AdventureConfig>,
    #[serde(default)]
    script_settings: Option<serde_json::Map<String, Value>>,
    #[serde(default)]
    content_warning: Option<String>,
    #[serde(default)]
    main_character: Option<String>,
}

#[derive(serde::Deserialize, Default)]
struct DlcVersionManifest {
    #[serde(default)]
    min_engine: Option<String>,
}

fn version_triplet(value: &str) -> Result<[u64; 3]> {
    let core = value
        .trim()
        .split(|character| matches!(character, '-' | '+'))
        .next()
        .unwrap_or_default();
    let parts: Vec<_> = core.split('.').collect();
    if parts.len() != 3 {
        return Err(anyhow!("版本 '{}' 必须使用 major.minor.patch 格式", value));
    }
    Ok([
        parts[0]
            .parse()
            .with_context(|| format!("版本号非法: {value}"))?,
        parts[1]
            .parse()
            .with_context(|| format!("版本号非法: {value}"))?,
        parts[2]
            .parse()
            .with_context(|| format!("版本号非法: {value}"))?,
    ])
}

fn ensure_dlc_engine_compatible(script_path: &Path) -> Result<()> {
    let manifest_path = script_path.join("dlc.json");
    if !manifest_path.is_file() {
        return Ok(());
    }
    let text = fs::read_to_string(&manifest_path)
        .with_context(|| format!("无法读取 DLC 版本清单: {}", manifest_path.display()))?;
    let manifest: DlcVersionManifest = serde_json::from_str(&text)
        .with_context(|| format!("无法解析 DLC 版本清单: {}", manifest_path.display()))?;
    let Some(required) = manifest
        .min_engine
        .as_deref()
        .map(str::trim)
        .filter(|v| !v.is_empty())
    else {
        return Ok(());
    };
    let current = env!("CARGO_PKG_VERSION");
    if version_triplet(current)? < version_triplet(required)? {
        return Err(anyhow!(
            "DLC 要求 LingChat >= {}，当前引擎为 {}；已拒绝加载以避免未知事件损坏周目",
            required,
            current
        ));
    }
    Ok(())
}

/// Central orchestrator for the script/story mode engine.
pub struct ScriptManager {
    /// All discovered scripts by name (folder_key or display name).
    pub all_scripts: HashMap<String, ScriptStatus>,
    /// Display names claimed by multiple paths. None is runnable until the
    /// conflict is removed; keeping every path makes import checks fail closed.
    duplicate_name_claims: HashMap<String, Vec<PathBuf>>,
    /// Whether a script is currently running (shared so callers can read without lock).
    pub is_running: Arc<AtomicBool>,
}

impl ScriptManager {
    // ============================================================
    // Construction & script discovery
    // ============================================================

    /// Scan the scripts directory and build the script catalog.
    pub fn new(data_dir: &Path) -> Self {
        let mut manager = Self {
            all_scripts: HashMap::new(),
            duplicate_name_claims: HashMap::new(),
            is_running: Arc::new(AtomicBool::new(false)),
        };
        super::dlc_transaction::recover_pending_uninstalls(data_dir);
        manager.init_all_scripts(data_dir);
        super::reset_transaction::recover_pending_resets(data_dir, &manager.all_scripts);
        manager
    }

    /// Scan `data_dir/game_data/scripts/` for all `story_config.yaml` files.
    /// Parse every candidate before publishing the catalog: if two paths claim
    /// one display name, fail closed and activate neither path.
    fn init_all_scripts(&mut self, data_dir: &Path) {
        let scripts_dir = data_dir.join("game_data").join("scripts");
        if !scripts_dir.exists() {
            tracing::warn!("[ScriptManager] 剧本目录不存在: {:?}", scripts_dir);
            return;
        }
        let mut candidates = Vec::new();

        // 1. Scan `character/<character>/<script>/` (two levels)
        let char_dir = scripts_dir.join("character");
        if char_dir.exists() {
            if let Ok(entries) = fs::read_dir(&char_dir) {
                for entry in entries.flatten() {
                    let path = entry.path();
                    if path.is_dir() {
                        if let Ok(sub_entries) = fs::read_dir(&path) {
                            for sub in sub_entries.flatten() {
                                let sub_path = sub.path();
                                if sub_path.is_dir() {
                                    candidates.push(sub_path);
                                }
                            }
                        }
                    }
                }
            }
        }

        // 2. Scan `standalone/<script>/` (one level)
        let standalone_dir = scripts_dir.join("standalone");
        if standalone_dir.exists() {
            if let Ok(entries) = fs::read_dir(&standalone_dir) {
                for entry in entries.flatten() {
                    let path = entry.path();
                    if path.is_dir() {
                        candidates.push(path);
                    }
                }
            }
        }

        // 3. Root-level scripts (backward compat)
        if let Ok(entries) = fs::read_dir(&scripts_dir) {
            for entry in entries.flatten() {
                let path = entry.path();
                if path.is_dir()
                    && path
                        .file_name()
                        .map(|n| n != "character" && n != "standalone")
                        .unwrap_or(false)
                    && path.join("story_config.yaml").exists()
                {
                    candidates.push(path);
                }
            }
        }

        candidates.sort();
        let mut claims: HashMap<String, Vec<ScriptStatus>> = HashMap::new();
        for path in candidates {
            match Self::read_script_config(&path) {
                Ok(status) => claims.entry(status.name.clone()).or_default().push(status),
                Err(error) => {
                    tracing::warn!("[ScriptManager] 跳过无效剧本目录 {:?}: {}", path, error)
                }
            }
        }
        for (name, mut statuses) in claims {
            if statuses.len() == 1 {
                self.all_scripts.insert(name, statuses.pop().unwrap());
                continue;
            }
            let paths: Vec<PathBuf> = statuses
                .into_iter()
                .map(|status| status.script_path)
                .collect();
            tracing::error!(
                "[ScriptManager] 剧本名 '{}' 被多个目录声明，全部禁用: {:?}",
                name,
                paths
            );
            self.duplicate_name_claims.insert(name, paths);
        }

        tracing::info!("[ScriptManager] 发现 {} 个可用剧本", self.all_scripts.len());
    }

    /// Parse a runnable script and enforce any DLC engine-version gate.
    pub fn read_script_config(script_path: &Path) -> Result<ScriptStatus> {
        ensure_dlc_engine_compatible(script_path)?;
        Self::read_script_config_unchecked(script_path)
    }

    /// Parse cleanup/listing identity without applying `min_engine`. This must
    /// never be used to start or register a script; it exists so a downgraded
    /// engine can still show and uninstall an incompatible DLC safely.
    pub(crate) fn read_script_config_unchecked(script_path: &Path) -> Result<ScriptStatus> {
        let config_path = script_path.join("story_config.yaml");
        let content = fs::read_to_string(&config_path)
            .with_context(|| format!("无法读取剧本配置: {:?}", config_path))?;

        let raw: StoryConfigRaw = serde_yaml::from_str(&content)
            .with_context(|| format!("无法解析剧本配置: {:?}", config_path))?;

        let folder_key = script_path
            .file_name()
            .map(|n| n.to_string_lossy().to_string())
            .unwrap_or_default();

        let name = raw.script_name.unwrap_or_else(|| folder_key.clone());
        let intro_chapter = raw.intro_chapter.unwrap_or_else(|| "main".to_string());
        let description = raw.description.unwrap_or_default();
        let adventure = raw.adventure.unwrap_or_default();
        let settings = raw.script_settings.unwrap_or_default();

        Ok(ScriptStatus {
            folder_key,
            name,
            description,
            intro_chapter,
            settings,
            script_path: script_path.to_path_buf(),
            recommand_start: raw.recommand_start.unwrap_or_default(),
            adventure,
            content_warning: raw.content_warning,
            main_character: raw
                .main_character
                .map(|s| s.trim().to_string())
                .filter(|s| !s.is_empty()),
            running_client_id: None,
            current_chapter_key: String::new(),
            current_event_process: 0,
            vars: serde_json::Map::new(),
        })
    }

    // ============================================================
    // Script listing
    // ============================================================

    pub fn get_script_list(&self) -> Vec<String> {
        self.all_scripts.keys().cloned().collect()
    }

    pub fn get_standalone_script_list(&self) -> Vec<String> {
        self.all_scripts
            .iter()
            .filter(|(_, s)| !s.adventure.is_adventure)
            .map(|(k, _)| k.clone())
            .collect()
    }

    pub fn get_script(&self, name: &str) -> Option<&ScriptStatus> {
        self.all_scripts.get(name)
    }

    pub(crate) fn script_name_claim_paths(&self, name: &str) -> Vec<PathBuf> {
        if let Some(paths) = self.duplicate_name_claims.get(name) {
            return paths.clone();
        }
        self.all_scripts
            .get(name)
            .map(|status| vec![status.script_path.clone()])
            .unwrap_or_default()
    }

    /// Reconcile a freshly scanned catalog while preserving live per-script
    /// progress and this manager's shared lifecycle reservation.
    pub(crate) fn merge_scanned_catalog(&mut self, fresh: Self) -> usize {
        let Self {
            all_scripts: fresh_scripts,
            duplicate_name_claims: fresh_duplicates,
            ..
        } = fresh;
        self.all_scripts
            .retain(|name, _| fresh_scripts.contains_key(name));
        for (name, scanned) in fresh_scripts {
            match self.all_scripts.get_mut(&name) {
                Some(old) => {
                    old.folder_key = scanned.folder_key;
                    old.description = scanned.description;
                    old.intro_chapter = scanned.intro_chapter;
                    old.settings = scanned.settings;
                    old.script_path = scanned.script_path;
                    old.recommand_start = scanned.recommand_start;
                    old.adventure = scanned.adventure;
                    old.content_warning = scanned.content_warning;
                    old.main_character = scanned.main_character;
                }
                None => {
                    self.all_scripts.insert(name, scanned);
                }
            }
        }
        self.duplicate_name_claims = fresh_duplicates;
        self.all_scripts.len()
    }

    /// 运行中动态加载一个剧本目录（DLC 导入后立刻可玩，无需重启）。
    /// 返回剧本名（`all_scripts` 的键）。
    pub fn load_script_dir(&mut self, script_path: &Path) -> Result<String> {
        let status = Self::read_script_config(script_path)?;
        let name = status.name.clone();
        if !self.script_name_claim_paths(&name).is_empty() {
            return Err(anyhow!("剧本 '{}' 已存在或处于重名禁用状态", name));
        }
        self.all_scripts.insert(name.clone(), status);
        tracing::info!("[ScriptManager] 动态加载剧本: {} ({:?})", name, script_path);
        Ok(name)
    }

    /// 按剧本目录动态卸载（DLC 移除用）。返回被卸载的剧本名。
    /// 调用方负责在剧本未运行时执行。
    pub fn unload_script_dir(&mut self, script_path: &Path) -> Option<String> {
        let name = self
            .all_scripts
            .iter()
            .find(|(_, s)| s.script_path == script_path)
            .map(|(k, _)| k.clone());
        if let Some(ref n) = name {
            self.all_scripts.remove(n);
            tracing::info!("[ScriptManager] 动态卸载剧本: {} ({:?})", n, script_path);
            return name;
        }

        let conflict_name = self
            .duplicate_name_claims
            .iter()
            .find(|(_, paths)| paths.iter().any(|path| path == script_path))
            .map(|(name, _)| name.clone());
        if let Some(name) = conflict_name {
            let mut remaining = self.duplicate_name_claims.remove(&name).unwrap_or_default();
            remaining.retain(|path| path != script_path);
            if remaining.len() == 1 {
                let remaining_path = remaining.pop().unwrap();
                match Self::read_script_config(&remaining_path) {
                    Ok(status) => {
                        self.all_scripts.insert(name.clone(), status);
                        tracing::info!("[ScriptManager] 重名冲突解除，启用剧本: {}", name);
                    }
                    Err(error) => tracing::warn!(
                        "[ScriptManager] 重名冲突解除后仍无法加载 {:?}: {}",
                        remaining_path,
                        error
                    ),
                }
            } else if !remaining.is_empty() {
                self.duplicate_name_claims.insert(name.clone(), remaining);
            }
            return Some(name);
        }
        None
    }

    // ============================================================
    // Script lifecycle
    // ============================================================

    /// Main entry point: initialize and run a script by name.
    /// This is a long-running async operation — it awaits user input inside.
    pub async fn start_script(&self, name: &str, ctx: &mut ScriptContext<'_>) -> Result<()> {
        let script = self
            .all_scripts
            .get(name)
            .ok_or_else(|| anyhow!("剧本不存在: '{}'", name))?
            .clone();

        self.is_running.store(true, Ordering::SeqCst);

        Self::run_to_completion(&script, ctx, &self.is_running).await
    }

    /// Execute a script from start to finish without needing `&self`.
    /// This is the entry point for the API layer, which holds script data
    /// independently and builds its own `ScriptContext`.
    pub async fn execute_script(
        script: &ScriptStatus,
        ctx: &mut ScriptContext<'_>,
        is_running: &AtomicBool,
    ) -> Result<()> {
        is_running.store(true, Ordering::SeqCst);
        Self::run_to_completion(script, ctx, is_running).await
    }

    /// Init → run → teardown, with teardown guaranteed on the error path.
    ///
    /// The previous code chained `?` between the three steps, so any failure
    /// skipped `on_script_end`: `is_running` stayed `true`, `script_status`
    /// stayed `Some(..)`, `script:end` was never emitted and the frontend froze
    /// in story mode with no way out. Callers only logged the error.
    async fn run_to_completion(
        script: &ScriptStatus,
        ctx: &mut ScriptContext<'_>,
        is_running: &AtomicBool,
    ) -> Result<()> {
        // Deliberately sequential rather than an `async { .. }.await` block:
        // an async block would capture `ctx: &mut _` by move, leaving it
        // unusable for the teardown below.
        let mut outcome = Self::init_script(script, ctx).await;
        if outcome.is_ok() {
            outcome = Self::run_script(ctx).await;
        }

        if let Err(e) = &outcome {
            tracing::error!("[ScriptManager] 剧本 '{}' 执行失败: {:#}", script.name, e);
            // Surface it before teardown: emits `ai:error` + `status:reset`,
            // both of which the frontend already listens for.
            crate::ai_service::message_system::events::emit_error(ctx.app, e);
        }

        // A failed run must not be recorded as completed — that would unlock
        // follow-up adventures the player never actually finished.
        let completed = outcome.is_ok();
        if let Err(e) = Self::on_script_end(ctx, is_running, completed).await {
            tracing::error!("[ScriptManager] 剧本收尾失败: {:#}", e);
        }

        outcome
    }

    /// Initialize a script: register its roles, set script_status, load player info.
    pub async fn init_script(script: &ScriptStatus, ctx: &mut ScriptContext<'_>) -> Result<()> {
        // 正式剧本：先拍台词表长度快照，剧本结束时据此截断（防提示词污染）。
        // 必须在注册剧本角色之前拍——角色 SYSTEM 人设行也在剧本期间写入，
        // 属于要一并截掉的部分。试玩由 PreviewSession 自己的快照/还原负责。
        if !ctx.is_preview {
            let mut gs = ctx.game_status.lock().await;
            gs.script_start_line_len = Some(gs.line_list.len());
            // 舞台状态同样拍快照：剧本演出里的 hide_character（"角色消失"）
            // 会改写 onstage/present 集合，剧本结束时需要恢复到进剧本前的样子，
            // 否则自由对话的立绘不再显示。
            gs.script_start_onstage_ids = Some(gs.onstage_role_ids.clone());
            gs.script_start_present_ids = Some(gs.present_role_ids.clone());
            // A script owns an isolated stage. Roles carried from free dialogue
            // would otherwise leak into empty/missing scenes and backend state.
            gs.onstage_role_ids.clear();
            gs.present_role_ids.clear();
        }
        // Story previews are isolated from persistent state. Real runs may opt
        // in to a small allow-list of variables via `persistent_vars`.
        let active_script = if ctx.is_preview {
            persistent_state::prepare_preview(script)
        } else {
            persistent_state::prepare_playthrough(script, ctx.data_dir)
        };
        ctx.game_status.lock().await.script_status = Some(active_script);

        // Load player info from script settings
        if let Some(user_name) = script.settings.get("user_name").and_then(|v| v.as_str()) {
            if !user_name.is_empty() {
                ctx.game_status.lock().await.player.user_name = user_name.to_string();
            }
        }
        if let Some(user_subtitle) = script
            .settings
            .get("user_subtitle")
            .and_then(|v| v.as_str())
        {
            ctx.game_status.lock().await.player.user_subtitle = user_subtitle.to_string();
        }

        // Register script roles from characters/ subdirectory (if exists)
        Self::register_script_roles(script, ctx).await?;

        // 剧本声明主角（main_character）时，进入即切换并锁定到该角色（仅正式游玩；
        // 试玩由 PreviewSession 按自己的规则解析 MAIN 并整体还原）。
        if !ctx.is_preview {
            Self::bind_declared_main_character(script, ctx).await?;
        }

        tracing::info!("[ScriptManager] 剧本 '{}' 初始化完成", script.name);
        Ok(())
    }

    /// 把主角切成剧本 `main_character` 声明的角色（按资源目录名在角色库中查找，
    /// 与编辑器试玩 `resolve_preview_main_role` 同一规则）。
    ///
    /// 原 `(main_role_id, current_role_id)` 拍进 `script_start_role_ids`，剧本结束
    /// 由 `on_script_end_inner` 恢复；切换后剧本里的 MAIN、立绘覆盖与自由对话人设
    /// 都以声明角色为准，不受进剧本前所聊角色影响。目标角色缺 SYSTEM 人设行时按
    /// `character_switch` 的同一约定补一条，避免 free_dialogue 在无人设上下文中生成。
    async fn bind_declared_main_character(
        script: &ScriptStatus,
        ctx: &mut ScriptContext<'_>,
    ) -> Result<()> {
        let Some(folder) = script.main_character.as_deref() else {
            return Ok(());
        };

        let roles = RoleRepo::get_all_main_roles(ctx.db).await?;
        let Some(role_id) = roles
            .iter()
            .find(|r| r.resource_folder.as_deref() == Some(folder))
            .map(|r| r.id)
        else {
            return Err(anyhow!(
                "剧本声明的主角目录「{}」不在角色库中（game_data/characters/{} 不存在或未注册）；\
                 请确认角色存在，或修正 story_config.yaml 的 main_character",
                folder,
                folder
            ));
        };

        let role_name = {
            let mut gs = ctx.game_status.lock().await;
            if gs.main_role_id == Some(role_id) {
                // 当前主角已是声明角色：无需切换，也不拍快照（结束时不做恢复）
                return Ok(());
            }
            gs.get_role(ctx.db, role_id).await?;
            let loaded = gs
                .role_manager
                .get_loaded(role_id)
                .ok_or_else(|| anyhow!("主角 {} 加载后不可用", role_id))?;
            let name = loaded
                .display_name
                .clone()
                .unwrap_or_else(|| folder.to_string());

            let has_system_prompt = gs.line_list.iter().any(|line| {
                matches!(line.attribute(), LineAttribute::System)
                    && line.sender_role_id() == Some(role_id)
            });
            if !has_system_prompt {
                let prompt = sys_prompt_builder_by_settings(
                    &loaded.settings,
                    PromptOptions {
                        output_sec_lang: true,
                        no_emotion_limit: true,
                    },
                );
                gs.add_line(
                    ctx.db,
                    LineBase {
                        content: prompt,
                        attribute: LineAttributeExt(LineAttribute::System),
                        sender_role_id: Some(role_id),
                        display_name: Some(name.clone()),
                        ..Default::default()
                    },
                )
                .await?;
            }

            gs.script_start_role_ids = Some((gs.main_role_id, gs.current_role_id));
            gs.main_role_id = Some(role_id);
            gs.current_role_id = Some(role_id);
            name
        };

        // 通知前端当前对话角色已切换（与 character_switch 工具同一事件）
        let payload = serde_json::json!({
            "type": "character_switch",
            "roleId": role_id,
            "characterName": role_name,
        });
        if let Err(e) = ctx.app.emit("character:switch", &payload) {
            tracing::warn!("[ScriptManager] emit character:switch 失败: {e}");
        }
        tracing::info!(
            "[ScriptManager] 剧本 '{}' 主角已切换并锁定: {} (id={})",
            script.name,
            role_name,
            role_id
        );
        Ok(())
    }

    /// Register script-specific NPC roles into DB and load them.
    pub async fn register_script_roles(
        script: &ScriptStatus,
        ctx: &mut ScriptContext<'_>,
    ) -> Result<()> {
        let characters_dir = script.script_path.join("characters");
        if !characters_dir.exists() {
            return Ok(());
        }

        let entries = fs::read_dir(&characters_dir)
            .with_context(|| format!("无法读取角色目录: {:?}", characters_dir))?;

        // Get RoleManager for mutating role state
        // We need to work with game_status directly
        for entry in entries.flatten() {
            let path = entry.path();
            if !path.is_dir() {
                continue;
            }

            let role_folder = path
                .file_name()
                .map(|n| n.to_string_lossy().to_string())
                .unwrap_or_default();

            // Read settings.yml BEFORE the existence check: the lookup key comes
            // out of it. Previously the check used `role_folder` while creation
            // passed `settings.script_role_key`, so whenever settings.yml omitted
            // that field the two disagreed — `find_or_create_role` skipped its
            // own lookup (it needs Some(script_role_key)) and inserted a fresh
            // duplicate row on *every* script start, while `character: <folder>`
            // in the YAML could never resolve to any of them.
            let settings_path = path.join("settings.yml");
            if !settings_path.exists() {
                tracing::warn!("[ScriptManager] 角色缺少 settings.yml: {:?}", settings_path);
                continue;
            }

            let content = fs::read_to_string(&settings_path)
                .with_context(|| format!("无法读取角色设定: {:?}", settings_path))?;

            let settings: crate::ai_service::types::CharacterSettings =
                serde_yaml::from_str(&content)
                    .with_context(|| format!("无法解析角色设定: {:?}", settings_path))?;

            // 剧本角色必须显式带 script_role_key —— 它在数据库里把「剧本 NPC」与
            // game_data/characters/ 的主角色区分开。缺了它就不能加载（上游硬要求），
            // 不能再用目录名回退替代，否则数据库角色记录会混乱。
            // 官方剧本均无 characters/ 目录，故不受影响；编辑器创建/导入角色时已强制写入。
            let role_key = match settings
                .script_role_key
                .as_deref()
                .map(|s| s.trim())
                .filter(|s| !s.is_empty())
            {
                Some(k) => k.to_string(),
                None => {
                    tracing::warn!(
                        "[ScriptManager] 角色 {:?} 缺少 script_role_key，跳过加载（剧本 NPC 必须显式声明该字段）",
                        role_folder
                    );
                    continue;
                }
            };

            // Upsert by the namespaced script/role keys. Existing rows are
            // refreshed from current settings so reinstall cannot reuse stale
            // display/resource metadata.
            let path_key = script.path_key();

            // Register role in DB via RoleRepo
            let role_id = RoleRepo::find_or_create_role(
                ctx.db,
                &settings.ai_name,
                RoleType::Npc,
                Some(&path_key),
                Some(&role_key),
                Some(&role_folder),
            )
            .await?;

            tracing::info!(
                "[ScriptManager] 注册剧本角色: {} (id={}, script={}, role_key={})",
                settings.ai_name,
                role_id,
                path_key,
                role_key
            );

            // The same DB id may already be cached from an older install or
            // editor revision. Evict first so this run uses the upserted
            // resource folder/settings instead of stale runtime metadata.
            {
                let mut game_status = ctx.game_status.lock().await;
                game_status.role_manager.evict_role(role_id);
                let _ = game_status.get_role(ctx.db, role_id).await?;
            }

            // Add system prompt line for this role
            let prompt = settings.system_prompt.clone().unwrap_or_default();
            let prompt_options = PromptOptions {
                output_sec_lang: true,
                no_emotion_limit: true,
            };
            if !prompt.is_empty() {
                let ai_prompt = sys_prompt_builder(
                    &ctx.game_status.lock().await.player.user_name,
                    &settings.ai_name,
                    &prompt,
                    settings.system_prompt_example.as_deref(),
                    settings.system_prompt_example_old.as_deref(),
                    prompt_options,
                );
                let sys_line = LineBase {
                    content: ai_prompt,
                    attribute: LineAttributeExt(LineAttribute::System),
                    sender_role_id: Some(role_id),
                    display_name: Some(settings.ai_name.clone()),
                    ..Default::default()
                };
                ctx.game_status
                    .lock()
                    .await
                    .add_line(ctx.db, sys_line)
                    .await?;
            }
        }

        Ok(())
    }

    /// The main chapter loop: load chapters and run them until "end".
    pub async fn run_script(ctx: &mut ScriptContext<'_>) -> Result<()> {
        let script = ctx
            .game_status
            .lock()
            .await
            .script_status
            .as_ref()
            .ok_or_else(|| anyhow!("ScriptStatus 未设置"))?
            .clone();

        let mut next_chapter = script.intro_chapter.clone();

        // Resolve "Intro/intro" style paths → find the actual yaml file
        let chapters_dir = script.script_path.join("Chapters");

        while next_chapter != "end" {
            let chapter_path = if next_chapter.ends_with(".yaml") {
                chapters_dir.join(&next_chapter)
            } else {
                chapters_dir.join(format!("{}.yaml", next_chapter))
            };

            let content = fs::read_to_string(&chapter_path)
                .with_context(|| format!("无法读取章节文件: {:?}", chapter_path))?;

            let chapter_config: Value = serde_yaml::from_str(&content)
                .with_context(|| format!("无法解析章节文件: {:?}", chapter_path))?;

            let script_ref = ctx
                .game_status
                .lock()
                .await
                .script_status
                .as_ref()
                .ok_or_else(|| anyhow!("ScriptStatus 丢失"))?
                .clone();

            let mut chapter = Chapter::new(next_chapter.clone(), chapter_config, &script_ref);

            // Update tracking fields
            if let Some(ref mut ss) = ctx.game_status.lock().await.script_status {
                ss.current_chapter_key = next_chapter.clone();
                ss.current_event_process = 0;
            }

            next_chapter = chapter.run(ctx).await?;
        }

        Ok(())
    }

    /// Cleanup after script ends: emit script_end event, clear script_status,
    /// mark adventures complete.
    /// Tear down the running script.
    ///
    /// `completed` records whether the run finished normally. Only a completed
    /// run is added to `completed_scripts` (which gates adventure unlocks);
    /// a run that ended in an error still gets fully torn down so the UI is
    /// released, but is not credited to the player.
    async fn on_script_end_inner(
        ctx: &mut ScriptContext<'_>,
        is_running: &AtomicBool,
        completed: bool,
        release_running: bool,
    ) -> Result<()> {
        tracing::info!("[ScriptManager] 剧本结束 (completed={})", completed);

        // OS 窗口标题是纯内存演出，结束时无条件还原（乱码标题不能泄漏到主菜单）
        super::events::window_title_event::restore_window_title(ctx.app);

        // 文件监视器同理：剧本结束必须停掉，否则它会盯着已结束的演出
        {
            let mut channels = ctx.channels.lock().await;
            if let Some(task) = channels.watch_task.take() {
                task.abort();
            }
            channels.watch_jump = None;
        }

        // Extract data under one lock, then mutate under a second lock. The
        // frontend end event is emitted only after persistence and teardown so
        // the remounted main menu cannot race stale act/theme state.

        // tokio::sync::Mutex is NOT reentrant — nesting lock().await deadlocks.
        let (folder, is_adventure) = {
            let gs = ctx.game_status.lock().await;
            match gs.script_status.as_ref() {
                Some(ss) => (Some(ss.path_key()), ss.adventure.is_adventure),
                None => (None, false),
            }
        };

        // Save only the explicitly allow-listed story variables. Persistence
        // failures must never prevent the normal teardown path from releasing
        // the UI, and editor previews never touch the player's state file.
        if !ctx.is_preview {
            let snapshot = ctx.game_status.lock().await.script_status.clone();
            if let Some(snapshot) = snapshot {
                if let Err(error) = persistent_state::save_playthrough(&snapshot, ctx.data_dir) {
                    tracing::warn!("[ScriptState] 剧本状态保存失败: {:#}", error);
                }
            }
        }

        // Now re-acquire the lock and do all writes in one critical section
        {
            let mut gs = ctx.game_status.lock().await;
            if let Some(folder) = folder {
                if completed {
                    gs.completed_scripts.insert(folder.clone());
                    if is_adventure {
                        tracing::info!("[ScriptManager] 羁绊冒险完成: {}", folder);
                    }
                } else {
                    tracing::warn!("[ScriptManager] 剧本未正常结束，不记为已完成: {}", folder);
                }
            }
            gs.script_status = None;
        }

        // 防提示词污染：正式剧本结束后，把剧本期间写入共享台词表的内容整段
        // 截掉，角色记忆按截断后的列表重建——剧本台词/旁白/自由对话轮次不会
        // 漏进自由对话的 LLM 上下文。试玩由 PreviewSession 还原，不走这里。
        let mut restored_role_id: Option<i32> = None;
        if !ctx.is_preview {
            let mut gs = ctx.game_status.lock().await;
            if let Some(len) = gs.script_start_line_len.take() {
                if gs.line_list.len() > len {
                    gs.line_list.truncate(len);
                    tracing::info!("[ScriptManager] 已截断剧本期间台词表（回退到 {} 行）", len);
                    if let Err(e) = gs.refresh_memories(ctx.db).await {
                        tracing::warn!("[ScriptManager] 剧本结束后重建记忆失败: {:#}", e);
                    }
                }
            }
            // 舞台状态恢复：剧本演出的 hide_character 不得带走自由对话的立绘
            if let Some(onstage) = gs.script_start_onstage_ids.take() {
                gs.onstage_role_ids = onstage;
            }
            if let Some(present) = gs.script_start_present_ids.take() {
                gs.present_role_ids = present;
            }
            // 主角锁定恢复：声明 main_character 的剧本进入时切换的 (main, current)
            // 角色必须还给自由对话，否则退出剧本后角色一直停在剧本主角上
            if let Some((main, current)) = gs.script_start_role_ids.take() {
                gs.main_role_id = main;
                gs.current_role_id = current;
                restored_role_id = current;
            }
        }

        // 恢复了进剧本前的角色后，同步通知前端（锁外 emit，与切换时同一事件）
        if let Some(role_id) = restored_role_id {
            let role_name = {
                let gs = ctx.game_status.lock().await;
                gs.role_manager
                    .get_loaded(role_id)
                    .and_then(|r| r.display_name.clone())
                    .unwrap_or_else(|| format!("角色{}", role_id))
            };
            let payload = serde_json::json!({
                "type": "character_switch",
                "roleId": role_id,
                "characterName": role_name,
            });
            if let Err(e) = ctx.app.emit("character:switch", &payload) {
                tracing::warn!("[ScriptManager] 剧本结束恢复角色 emit character:switch 失败: {e}");
            }
            tracing::info!("[ScriptManager] 主角已恢复为进剧本前角色: {} (id={})", role_name, role_id);
        }

        // A normal backend run can finish long before the player consumes the
        // frontend dialogue queue. Keep validated window tickets alive until the
        // queue-ordered script:end processor closes them; stop/error/preview
        // teardown still cleans immediately.
        if !completed && release_running {
            crate::ai_service::game_system::script_engine::events::glitch_window_event::close_all_glitch_windows(
                ctx.app,
            );
            crate::api::script_popups::close_all();
        }

        if release_running {
            is_running.store(false, Ordering::SeqCst);
        }

        // This releases the frontend from story mode. Normal runs publish only
        // after is_running is false. Editor preview deliberately keeps its
        // reservation until PreviewSession has restored shared GameStatus.
        if let Err(error) = emit(ctx.app, SCRIPT_END, &ScriptEndPayload { completed }) {
            // No frontend consumer will arrive to close deferred tickets/windows.
            crate::ai_service::game_system::script_engine::events::glitch_window_event::close_all_glitch_windows(
                ctx.app,
            );
            crate::api::script_popups::close_all();
            tracing::error!("[ScriptManager] script:end 发送失败，已执行原生窗口回退清理: {error:#}");
        }

        tracing::info!("[ScriptManager] 剧本状态已清除");

        Ok(())
    }

    pub async fn on_script_end(
        ctx: &mut ScriptContext<'_>,
        is_running: &AtomicBool,
        completed: bool,
    ) -> Result<()> {
        Self::on_script_end_inner(ctx, is_running, completed, true).await
    }

    pub(crate) async fn on_preview_script_end(
        ctx: &mut ScriptContext<'_>,
        is_running: &AtomicBool,
    ) -> Result<()> {
        Self::on_script_end_inner(ctx, is_running, false, false).await
    }

    // ============================================================
    // Adventure management
    // ============================================================

    pub fn get_character_adventures(&self, character_folder: &str) -> Vec<&ScriptStatus> {
        self.all_scripts
            .values()
            .filter(|s| {
                s.adventure.is_adventure && s.adventure.bound_character_folder == character_folder
            })
            .collect()
    }

    pub fn get_all_adventures(&self) -> Vec<&ScriptStatus> {
        self.all_scripts
            .values()
            .filter(|s| s.adventure.is_adventure)
            .collect()
    }

    pub fn get_assets_dir(&self, script_name: Option<&str>) -> PathBuf {
        match script_name.and_then(|n| self.all_scripts.get(n)) {
            Some(script) => script.script_path.join("Assets"),
            None => PathBuf::from(""),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{ensure_dlc_engine_compatible, version_triplet};

    #[test]
    fn parses_engine_versions_for_dlc_gating() {
        assert_eq!(version_triplet("0.5.1").unwrap(), [0, 5, 1]);
        assert_eq!(version_triplet("1.2.3-beta+7").unwrap(), [1, 2, 3]);
        assert!(version_triplet("0.5").is_err());
        assert!(version_triplet("latest").is_err());
        assert!(version_triplet("0.5.0").unwrap() < version_triplet("0.5.1").unwrap());
    }

    #[test]
    fn rejects_dlc_requiring_a_newer_engine() {
        let unique = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let dir = std::env::temp_dir().join(format!(
            "lingchat-dlc-version-{}-{unique}",
            std::process::id()
        ));
        std::fs::create_dir_all(&dir).unwrap();

        std::fs::write(dir.join("dlc.json"), r#"{"min_engine":"0.5.1"}"#).unwrap();
        assert!(ensure_dlc_engine_compatible(&dir).is_ok());
        std::fs::write(dir.join("dlc.json"), r#"{"min_engine":"999.0.0"}"#).unwrap();
        assert!(ensure_dlc_engine_compatible(&dir).is_err());
        std::fs::write(dir.join("dlc.json"), "not-json").unwrap();
        assert!(ensure_dlc_engine_compatible(&dir).is_err());

        let _ = std::fs::remove_dir_all(dir);
    }
}
