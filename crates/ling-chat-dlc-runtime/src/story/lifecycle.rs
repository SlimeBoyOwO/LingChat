use anyhow::{Context, Result, anyhow};
use ling_chat_main::{
    ai_service::{
        game_system::script_engine::events::ScriptContext,
        types::{LineAttributeExt, LineBase, ScriptStatus},
    },
    db::{entities::line::LineAttribute, managers::role_repo::RoleRepo},
    utils::prompt::{PromptOptions, sys_prompt_builder_by_settings},
};
use std::{fs, path::Path};
use tauri::Emitter;
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

pub fn check_config(script_path: &Path, current: &str) -> Result<()> {
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
    if version_triplet(current)? < version_triplet(required)? {
        return Err(anyhow!(
            "DLC 要求 LingChat >= {}，当前引擎为 {}；已拒绝加载以避免未知事件损坏周目",
            required,
            current
        ));
    }
    Ok(())
}

/// 把主角切成剧本 `main_character` 声明的角色（按资源目录名在角色库中查找，
/// 与编辑器试玩 `resolve_preview_main_role` 同一规则）。
///
/// 原 `(main_role_id, current_role_id)` 拍进 `script_start_role_ids`，剧本结束
/// 由 `on_script_end_inner` 恢复；切换后剧本里的 MAIN、立绘覆盖与自由对话人设
/// 都以声明角色为准，不受进剧本前所聊角色影响。目标角色缺 SYSTEM 人设行时按
/// `character_switch` 的同一约定补一条，避免 free_dialogue 在无人设上下文中生成。
pub async fn initialize(script: &ScriptStatus, ctx: &mut ScriptContext<'_>) -> Result<()> {
    let Some(folder) = script.main_character.as_deref() else {
        return Ok(());
    };

    // A self-contained story may declare its own registered role as MAIN.
    let local = RoleRepo::get_script_roles(ctx.db, &script.path_key())
        .await?
        .into_iter()
        .find(|role| role.resource_folder.as_deref() == Some(folder));
    let roles = RoleRepo::get_all_main_roles(ctx.db).await?;
    let Some(role_id) = local
        .as_ref()
        .or_else(|| {
            roles
                .iter()
                .find(|r| r.resource_folder.as_deref() == Some(folder))
        })
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

#[cfg(test)]
mod tests {
    #[test]
    fn package_version_policy_receives_the_host_version() {
        let dir = tempfile::tempdir().unwrap();
        assert!(super::check_config(dir.path(), "0.5.2").is_ok());
        std::fs::write(dir.path().join("dlc.json"), r#"{"min_engine":"0.5.2"}"#).unwrap();
        assert!(super::check_config(dir.path(), "0.5.2").is_ok());
        assert!(super::check_config(dir.path(), "0.5.1").is_err());
        std::fs::write(dir.path().join("dlc.json"), r#"{"min_engine":"invalid"}"#).unwrap();
        assert!(super::check_config(dir.path(), "0.5.2").is_err());
    }
}
