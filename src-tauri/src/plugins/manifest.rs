//! 插件 manifest.toml 解析与严格校验。

use anyhow::{Context, Result};
use serde_json::Value;

use super::types::PluginManifest;

/// 从 TOML 文本解析并校验插件 manifest。
pub fn parse(text: &str) -> Result<PluginManifest> {
    let manifest: PluginManifest = toml::from_str(text).context("解析 manifest.toml 失败")?;
    validate(&manifest)?;
    Ok(manifest)
}

/// 单次工具执行超时上限（毫秒）。超过则 manifest 校验失败，
/// 避免插件声明超大超时导致阻塞线程被长期占用。
const MAX_TIMEOUT_MS: u64 = 120_000;

/// 校验脚本名是单个文件名（禁止子目录与 `..`，防路径穿越）。
fn validate_script_name(plugin_id: &str, owner: &str, script: &str) -> Result<()> {
    if script.is_empty() {
        anyhow::bail!("插件 '{plugin_id}' {owner} 未指定脚本");
    }
    if std::path::Path::new(script).components().count() != 1 {
        anyhow::bail!("插件 '{plugin_id}' {owner} 的脚本必须为单个文件名（不允许子目录/..）");
    }
    Ok(())
}

/// 校验超时值在允许区间内。
fn validate_timeout(plugin_id: &str, owner: &str, timeout_ms: u64) -> Result<()> {
    if timeout_ms == 0 {
        anyhow::bail!("插件 '{plugin_id}' {owner} 的 timeout_ms 必须大于 0");
    }
    if timeout_ms > MAX_TIMEOUT_MS {
        anyhow::bail!(
            "插件 '{plugin_id}' {owner} 的 timeout_ms {timeout_ms} 超过上限 {MAX_TIMEOUT_MS}ms"
        );
    }
    Ok(())
}

/// 信号名只约束字符集（小写字母、数字、`_`、`-`、`.`、`:`），不强制命名空间——
/// 宿主现有事件名两种风格都有（`ai:reply`、`role:list-updated`）。
fn is_valid_signal_name(name: &str) -> bool {
    !name.is_empty()
        && name.chars().all(|c| {
            c.is_ascii_lowercase() || c.is_ascii_digit() || matches!(c, '_' | '-' | '.' | ':')
        })
}

/// 处理函数名必须是合法的 Python 标识符。
fn is_valid_handler_name(name: &str) -> bool {
    let mut chars = name.chars();
    match chars.next() {
        Some(c) if c.is_ascii_alphabetic() || c == '_' => {},
        _ => return false,
    }
    chars.all(|c| c.is_ascii_alphanumeric() || c == '_')
}

/// 重试次数上限。启动是程序启动路径上的旁路，不该被单个插件拖成长期任务。
const MAX_STARTUP_RETRIES: u64 = 5;

/// 重试间隔上限（毫秒）。
const MAX_RETRY_INTERVAL_MS: u64 = 60_000;

/// id 的字符集校验（插件 id 与前置插件名共用）。
fn is_valid_plugin_id(id: &str) -> bool {
    !id.is_empty()
        && id
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || c == '_' || c == '-')
}

/// 校验 manifest 语义约束。
pub fn validate(manifest: &PluginManifest) -> Result<()> {
    if manifest.id.is_empty() {
        anyhow::bail!("插件 id 不能为空");
    }
    // id 只允许字母数字下划线，用于目录名与工具前缀，避免路径穿越。
    if !is_valid_plugin_id(&manifest.id) {
        anyhow::bail!(
            "插件 id '{}' 只能包含字母、数字、下划线与连字符",
            manifest.id
        );
    }
    if manifest.tools.is_empty()
        && manifest.resources.is_empty()
        && manifest.subscribe.is_empty()
        && manifest.startup.is_none()
    {
        anyhow::bail!(
            "插件 '{}' 未声明任何工具、资源、信号订阅或启动入口",
            manifest.id
        );
    }
    for dep in &manifest.depends_on {
        if !is_valid_plugin_id(dep) {
            anyhow::bail!("插件 '{}' 的前置插件名 '{dep}' 非法", manifest.id);
        }
        if *dep == manifest.id {
            anyhow::bail!("插件 '{}' 不能把自己声明为前置插件", manifest.id);
        }
    }
    if let Some(startup) = &manifest.startup {
        let owner = "启动入口";
        validate_script_name(&manifest.id, owner, &startup.script)?;
        if !is_valid_handler_name(&startup.handler) {
            anyhow::bail!(
                "插件 '{}' {owner} 的处理函数名 '{}' 不是合法标识符",
                manifest.id,
                startup.handler
            );
        }
        validate_timeout(&manifest.id, owner, startup.timeout_ms)?;
        if startup.retries > MAX_STARTUP_RETRIES {
            anyhow::bail!(
                "插件 '{}' {owner} 的 retries {} 超过上限 {MAX_STARTUP_RETRIES}",
                manifest.id,
                startup.retries
            );
        }
        if startup.retry_interval_ms > MAX_RETRY_INTERVAL_MS {
            anyhow::bail!(
                "插件 '{}' {owner} 的 retry_interval_ms {} 超过上限 {MAX_RETRY_INTERVAL_MS}ms",
                manifest.id,
                startup.retry_interval_ms
            );
        }
    }
    for tool in &manifest.tools {
        if tool.name.is_empty() {
            anyhow::bail!("插件 '{}' 存在工具名为空的声明", manifest.id);
        }
        validate_script_name(&manifest.id, &format!("工具 '{}'", tool.name), &tool.script)?;
        validate_timeout(
            &manifest.id,
            &format!("工具 '{}'", tool.name),
            tool.timeout_ms,
        )?;
        // parameters 必须是合法 JSON object（JSON Schema）。
        let params: Value = serde_json::from_str(&tool.parameters).with_context(|| {
            format!(
                "插件 '{}' 工具 '{}' 的 parameters 不是合法 JSON",
                manifest.id, tool.name
            )
        })?;
        if !params.is_object() {
            anyhow::bail!(
                "插件 '{}' 工具 '{}' 的 parameters 必须是 JSON object",
                manifest.id,
                tool.name
            );
        }
    }
    for sub in &manifest.subscribe {
        let owner = format!("信号订阅 '{}'", sub.signal);
        if !is_valid_signal_name(&sub.signal) {
            anyhow::bail!(
                "插件 '{}' 声明的信号名 '{}' 非法：只能包含小写字母、数字与 _-.:",
                manifest.id,
                sub.signal
            );
        }
        validate_script_name(&manifest.id, &owner, &sub.script)?;
        if !is_valid_handler_name(&sub.handler) {
            anyhow::bail!(
                "插件 '{}' {owner} 的处理函数名 '{}' 不是合法标识符",
                manifest.id,
                sub.handler
            );
        }
        validate_timeout(&manifest.id, &owner, sub.timeout_ms)?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::plugins::types::MatchValue;
    use serde_json::json;

    const HEAD: &str = r#"
id = "demo"
name = "Demo"
description = "演示"
version = "0.1.0"
"#;

    #[test]
    fn parses_scalar_and_list_match_values() {
        let text = format!(
            r#"{HEAD}
[[subscribe]]
signal = "scene:switch"
script = "hook.py"
handler = "on_scene"
match = {{ scene_id = "night", mode = ["a", "b"] }}
"#
        );
        let manifest = parse(&text).expect("应能解析");
        let sub = &manifest.subscribe[0];
        assert_eq!(sub.signal, "scene:switch");
        assert_eq!(sub.handler, "on_scene");
        // 未声明 timeout_ms 时取默认值
        assert_eq!(sub.timeout_ms, 30_000);
        // 标量是等值，数组是「命中任一」——两者不能反过来
        assert_eq!(sub.matches["scene_id"], MatchValue::One(json!("night")));
        assert_eq!(
            sub.matches["mode"],
            MatchValue::Many(vec![json!("a"), json!("b")])
        );
    }

    #[test]
    fn subscribe_only_plugin_is_accepted() {
        let text = format!(
            r#"{HEAD}
[[subscribe]]
signal = "app:start"
script = "boot.py"
handler = "on_start"
"#
        );
        assert!(
            parse(&text).is_ok(),
            "只有订阅、没有工具和资源的插件应可加载"
        );
    }

    #[test]
    fn rejects_bad_handler_name() {
        let text = format!(
            r#"{HEAD}
[[subscribe]]
signal = "app:start"
script = "boot.py"
handler = "1on_start"
"#
        );
        assert!(parse(&text).is_err());
    }

    #[test]
    fn rejects_script_path_traversal() {
        let text = format!(
            r#"{HEAD}
[[subscribe]]
signal = "app:start"
script = "../boot.py"
handler = "on_start"
"#
        );
        assert!(parse(&text).is_err());
    }

    #[test]
    fn rejects_invalid_signal_name() {
        let text = format!(
            r#"{HEAD}
[[subscribe]]
signal = "Scene Switch"
script = "boot.py"
handler = "on_start"
"#
        );
        assert!(parse(&text).is_err());
    }

    #[test]
    fn rejects_timeout_over_cap() {
        let text = format!(
            r#"{HEAD}
[[subscribe]]
signal = "app:start"
script = "boot.py"
handler = "on_start"
timeout_ms = 120001
"#
        );
        assert!(parse(&text).is_err());
    }

    #[test]
    fn rejects_unknown_manifest_key() {
        let text = format!(
            r#"{HEAD}
on_start = "boot.py"

[[tools]]
name = "t"
description = "d"
script = "t.py"
parameters = '{{ "type": "object" }}'
"#
        );
        assert!(parse(&text).is_err(), "未知字段应拒绝，而不是静默忽略");
    }
}
