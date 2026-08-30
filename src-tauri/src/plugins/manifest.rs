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

/// 校验 manifest 语义约束。
pub fn validate(manifest: &PluginManifest) -> Result<()> {
    if manifest.id.is_empty() {
        anyhow::bail!("插件 id 不能为空");
    }
    // id 只允许字母数字下划线，用于目录名与工具前缀，避免路径穿越。
    if !manifest
        .id
        .chars()
        .all(|c| c.is_ascii_alphanumeric() || c == '_' || c == '-')
    {
        anyhow::bail!("插件 id '{}' 只能包含字母、数字、下划线与连字符", manifest.id);
    }
    if manifest.tools.is_empty() && manifest.resources.is_empty() {
        anyhow::bail!("插件 '{}' 未声明任何工具或资源", manifest.id);
    }
    for tool in &manifest.tools {
        if tool.name.is_empty() {
            anyhow::bail!("插件 '{}' 存在工具名为空的声明", manifest.id);
        }
        if tool.script.is_empty() {
            anyhow::bail!("插件 '{}' 工具 '{}' 未指定脚本", manifest.id, tool.name);
        }
        if tool.timeout_ms == 0 {
            anyhow::bail!(
                "插件 '{}' 工具 '{}' 的 timeout_ms 必须大于 0",
                manifest.id,
                tool.name
            );
        }
        if tool.timeout_ms > MAX_TIMEOUT_MS {
            anyhow::bail!(
                "插件 '{}' 工具 '{}' 的 timeout_ms {} 超过上限 {MAX_TIMEOUT_MS}ms",
                manifest.id,
                tool.name,
                tool.timeout_ms
            );
        }
        // script 只允许相对路径文件名，禁止路径穿越。
        let script_path = std::path::Path::new(&tool.script);
        if script_path.components().count() != 1 {
            anyhow::bail!(
                "插件 '{}' 工具 '{}' 的脚本必须为单个文件名（不允许子目录/..）",
                manifest.id,
                tool.name
            );
        }
        // parameters 必须是合法 JSON object（JSON Schema）。
        let params: Value = serde_json::from_str(&tool.parameters)
            .with_context(|| format!("插件 '{}' 工具 '{}' 的 parameters 不是合法 JSON", manifest.id, tool.name))?;
        if !params.is_object() {
            anyhow::bail!(
                "插件 '{}' 工具 '{}' 的 parameters 必须是 JSON object",
                manifest.id,
                tool.name
            );
        }
    }
    Ok(())
}
