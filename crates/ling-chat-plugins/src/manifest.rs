//! 插件 manifest.toml 解析与严格校验。

use anyhow::{Context, Result};
use serde_json::Value;

use super::types::{ConfigKind, PluginManifest, WsMode};

/// 从 TOML 文本解析并校验插件 manifest。
pub fn parse(text: &str) -> Result<PluginManifest> {
    let manifest: PluginManifest = toml::from_str(text).context("解析 manifest.toml 失败")?;
    validate(&manifest)?;
    Ok(manifest)
}

/// 单次工具执行超时上限（毫秒）。超过则 manifest 校验失败，
/// 避免插件声明超大超时导致阻塞线程被长期占用。
const MAX_TIMEOUT_MS: u64 = 120_000;

/// 单个工具可声明的示例问句上限：示例会进 Tool RAG 的本地向量索引，
/// 放太多只会放大首次嵌入耗时，不会提升命中。
const MAX_TOOL_EXAMPLES: usize = 8;

/// 单条示例问句的长度上限（字符）。
const MAX_TOOL_EXAMPLE_CHARS: usize = 120;

/// 校验脚本路径是插件目录内的相对路径。
///
/// 允许 `tools/entry.py` 这种分层写法；目录段必须是合法的 Python 标识符（包名），
/// 最后一段必须是 `.py` 文件。绝对路径、反斜杠、`.`/`..`、空段一律拒绝，防路径穿越。
fn validate_script_path(plugin_id: &str, owner: &str, script: &str) -> Result<()> {
    const MAX_DEPTH: usize = 8;
    const MAX_LEN: usize = 240;
    if script.is_empty() {
        anyhow::bail!("插件 '{plugin_id}' {owner} 未指定脚本");
    }
    if script.len() > MAX_LEN {
        anyhow::bail!("插件 '{plugin_id}' {owner} 的脚本路径过长（上限 {MAX_LEN} 字符）");
    }
    if script.contains('\\') {
        anyhow::bail!("插件 '{plugin_id}' {owner} 的脚本路径必须用 '/' 分隔");
    }
    if script.starts_with('/') {
        anyhow::bail!("插件 '{plugin_id}' {owner} 的脚本路径必须是相对路径");
    }
    let parts: Vec<&str> = script.split('/').collect();
    if parts.len() > MAX_DEPTH {
        anyhow::bail!("插件 '{plugin_id}' {owner} 的脚本目录层级过深（上限 {MAX_DEPTH} 层）");
    }
    let Some((file, dirs)) = parts.split_last() else {
        anyhow::bail!("插件 '{plugin_id}' {owner} 未指定脚本");
    };
    for dir in dirs {
        if !is_python_identifier(dir) {
            anyhow::bail!("插件 '{plugin_id}' {owner} 的脚本路径段 '{dir}' 不是合法的目录名");
        }
    }
    let Some(stem) = file.strip_suffix(".py") else {
        anyhow::bail!("插件 '{plugin_id}' {owner} 的脚本必须是 .py 文件");
    };
    let starts_well = stem
        .chars()
        .next()
        .is_some_and(|c| c.is_ascii_alphabetic() || c == '_');
    if !starts_well
        || !stem
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || c == '_' || c == '-')
    {
        anyhow::bail!("插件 '{plugin_id}' {owner} 的脚本文件名 '{file}' 不合法");
    }
    Ok(())
}

/// Python 标识符的 ASCII 子集：字母或下划线开头，其余为字母、数字、下划线。
fn is_python_identifier(segment: &str) -> bool {
    let mut chars = segment.chars();
    match chars.next() {
        Some(c) if c.is_ascii_alphabetic() || c == '_' => {},
        _ => return false,
    }
    chars.all(|c| c.is_ascii_alphanumeric() || c == '_')
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
        && manifest.ws.is_empty()
        && manifest.startup.is_none()
    {
        anyhow::bail!(
            "插件 '{}' 未声明任何工具、资源、信号订阅、WS 连接或启动入口",
            manifest.id
        );
    }
    for path in &manifest.read {
        // 与插件运行期传入的请求路径共用同一份校验（host_api）。
        crate::host_api::check_relative_data_path(path.trim()).map_err(|e| {
            anyhow::anyhow!("插件 '{}' 的 read 声明 '{path}' 非法：{e}", manifest.id)
        })?;
    }
    for dep in &manifest.depends_on {
        if !is_valid_plugin_id(dep) {
            anyhow::bail!("插件 '{}' 的前置插件名 '{dep}' 非法", manifest.id);
        }
        if *dep == manifest.id {
            anyhow::bail!("插件 '{}' 不能把自己声明为前置插件", manifest.id);
        }
    }
    if let Some(playback) = &manifest.voice_playback {
        let declared = manifest
            .config
            .iter()
            .find(|field| field.key == playback.config);
        match declared {
            Some(field) if field.kind == ConfigKind::Boolean => {},
            Some(_) => anyhow::bail!(
                "插件 '{}' 的 [voice_playback] config '{}' 必须是 boolean 配置项",
                manifest.id,
                playback.config
            ),
            None => anyhow::bail!(
                "插件 '{}' 的 [voice_playback] config '{}' 不在 [[config]] 声明里",
                manifest.id,
                playback.config
            ),
        }
        if let Some(conn) = &playback.ws_conn {
            if !manifest.ws.iter().any(|decl| decl.id == *conn) {
                anyhow::bail!(
                    "插件 '{}' 的 [voice_playback] ws_conn '{conn}' 不在 [[ws]] 声明里",
                    manifest.id
                );
            }
        }
    }
    if let Some(startup) = &manifest.startup {
        let owner = "启动入口";
        validate_script_path(&manifest.id, owner, &startup.script)?;
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
        validate_script_path(&manifest.id, &format!("工具 '{}'", tool.name), &tool.script)?;
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
        if tool.examples.len() > MAX_TOOL_EXAMPLES {
            anyhow::bail!(
                "插件 '{}' 工具 '{}' 的 examples 最多 {MAX_TOOL_EXAMPLES} 条",
                manifest.id,
                tool.name
            );
        }
        for example in &tool.examples {
            let length = example.trim().chars().count();
            if length > MAX_TOOL_EXAMPLE_CHARS {
                anyhow::bail!(
                    "插件 '{}' 工具 '{}' 的 examples 单条不能超过 {MAX_TOOL_EXAMPLE_CHARS} 字符",
                    manifest.id,
                    tool.name
                );
            }
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
        validate_script_path(&manifest.id, &owner, &sub.script)?;
        if !is_valid_handler_name(&sub.handler) {
            anyhow::bail!(
                "插件 '{}' {owner} 的处理函数名 '{}' 不是合法标识符",
                manifest.id,
                sub.handler
            );
        }
        validate_timeout(&manifest.id, &owner, sub.timeout_ms)?;
    }
    for entry in &manifest.ws_allow {
        validate_ws_allow_entry(&manifest.id, entry)?;
    }
    for ws in &manifest.ws {
        let owner = format!("WS 连接 '{}'", ws.id);
        if !is_valid_plugin_id(&ws.id) {
            anyhow::bail!(
                "插件 '{}' {owner} 的 id '{}' 只能包含字母、数字、下划线与连字符",
                manifest.id,
                ws.id
            );
        }
        match ws.mode {
            WsMode::Client => {
                // url 可选（缺省由脚本 ws_open 传入）；给空串则视为笔误，拒绝。
                if ws.url.as_deref().is_some_and(|u| u.trim().is_empty()) {
                    anyhow::bail!("插件 '{}' {owner} 的 url 为空", manifest.id);
                }
            },
            WsMode::Server => {
                let bind = ws.bind.as_deref().map(str::trim).unwrap_or("");
                if bind.is_empty() {
                    anyhow::bail!(
                        "插件 '{}' {owner} 为 server 模式，必须声明 bind（监听地址）",
                        manifest.id
                    );
                }
            },
        }
        // 内联处理脚本：script 与 handler 必须成对出现，否则无法派发。
        match (&ws.script, &ws.handler) {
            (Some(script), Some(handler)) => {
                validate_script_path(&manifest.id, &owner, script)?;
                if !is_valid_handler_name(handler) {
                    anyhow::bail!(
                        "插件 '{}' {owner} 的处理函数名 '{handler}' 不是合法标识符",
                        manifest.id
                    );
                }
                validate_timeout(&manifest.id, &owner, ws.timeout_ms)?;
            },
            (None, None) => {},
            _ => anyhow::bail!(
                "插件 '{}' {owner} 的 script 与 handler 必须同时声明或同时省略",
                manifest.id
            ),
        }
    }
    Ok(())
}

/// 校验 `ws_allow` 白名单项：`*`（等价全网段）或 `<scheme>://<host>[:port]`
/// （host 段可含 `*` 通配）。这里只查形状，实际前缀匹配在 `plugins::ws`。
fn validate_ws_allow_entry(plugin_id: &str, entry: &str) -> Result<()> {
    let entry = entry.trim();
    if entry.is_empty() {
        anyhow::bail!("插件 '{plugin_id}' 的 ws_allow 存在空项");
    }
    if entry == "*" {
        return Ok(());
    }
    let Some((scheme, rest)) = entry.split_once("://") else {
        anyhow::bail!("插件 '{plugin_id}' 的 ws_allow 项 '{entry}' 缺少 scheme://");
    };
    if scheme.is_empty() {
        anyhow::bail!("插件 '{plugin_id}' 的 ws_allow 项 '{entry}' scheme 为空");
    }
    let host = rest.split('/').next().unwrap_or("");
    if host.is_empty() {
        anyhow::bail!("插件 '{plugin_id}' 的 ws_allow 项 '{entry}' 缺少主机名");
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::parse;

    /// 最小可用 manifest，`extra` 会追加到工具声明里。
    fn manifest_with_tool(extra: &str) -> String {
        format!(
            r#"
id = "demo"
name = "Demo"
description = "demo"
version = "1.0.0"

[[tools]]
name = "demo_tool"
description = "示例工具"
parameters = '{{ "type":"object" }}'
script = "tools/entry.py"
{extra}
"#
        )
    }

    #[test]
    fn tool_examples_parse_and_default_to_empty() {
        let parsed = parse(&manifest_with_tool(
            r#"examples = ["现在几点了", "今天几号"]"#,
        ))
        .expect("应通过校验");
        assert_eq!(parsed.tools[0].examples, vec!["现在几点了", "今天几号"]);

        let parsed = parse(&manifest_with_tool("")).expect("应通过校验");
        assert!(parsed.tools[0].examples.is_empty(), "缺省应为空数组");
    }

    #[test]
    fn tool_examples_reject_too_many_items() {
        let items: Vec<String> = (0..=8).map(|i| format!("\"问题{i}\"")).collect();
        let error = parse(&manifest_with_tool(&format!(
            "examples = [{}]",
            items.join(", ")
        )))
        .expect_err("超过上限应失败");
        assert!(error.to_string().contains("examples 最多 8 条"));
    }

    #[test]
    fn tool_examples_reject_overlong_item() {
        let long = "问".repeat(121);
        let error = parse(&manifest_with_tool(&format!("examples = [\"{long}\"]")))
            .expect_err("超长示例应失败");
        assert!(error.to_string().contains("单条不能超过"));
    }
}
