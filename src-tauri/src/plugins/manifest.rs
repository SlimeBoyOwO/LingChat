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
    // package_type 只允许已知类型（插件安装器按此分流）。
    if !matches!(
        manifest.package_type.as_str(),
        "plugin" | "character" | "script" | "voice"
    ) {
        anyhow::bail!(
            "插件 '{}' 的 type '{}' 非法（只支持 plugin/character/script/voice）",
            manifest.id,
            manifest.package_type
        );
    }
    // 插件必须有工具声明；角色/剧本/语音包是内容物，不需要工具。
    if manifest.package_type == "plugin" && manifest.tools.is_empty() {
        anyhow::bail!("插件 '{}' 未声明任何工具", manifest.id);
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

    // ── network 白名单校验：结构上只接受干净 host / 路径前缀 ──
    for decl in &manifest.network {
        if decl.host.trim().is_empty() {
            anyhow::bail!("插件 '{}' 存在空 host 的网络声明", manifest.id);
        }
        if !is_clean_host(&decl.host) {
            anyhow::bail!(
                "插件 '{}' 网络声明 host '{}' 非法（只能含字母数字、点、连字符与端口）",
                manifest.id,
                decl.host
            );
        }
        for path in &decl.paths {
            if !path.starts_with('/') || path.contains('?') || path.contains('#') {
                anyhow::bail!(
                    "插件 '{}' 网络声明路径 '{}' 必须以 / 开头且不含 query/fragment",
                    manifest.id,
                    path
                );
            }
        }
    }

    // ── call_tool 声明工具名校验 ──
    for decl in &manifest.permissions.tools {
        if decl.name.trim().is_empty() {
            anyhow::bail!("插件 '{}' 存在空工具名的 call_tool 声明", manifest.id);
        }
    }

    // ── 大文件声明校验 ──
    for asset in &manifest.assets {
        if asset.name.trim().is_empty()
            || asset.url.trim().is_empty()
            || asset.sha256.trim().is_empty()
        {
            anyhow::bail!(
                "插件 '{}' 存在不完整的大文件声明（name/url/sha256 必填）",
                manifest.id
            );
        }
        let url: Value = asset.url.as_str().into();
        let _ = url;
        if !(asset.url.starts_with("https://") || asset.url.starts_with("http://")) {
            anyhow::bail!(
                "插件 '{}' 大文件 '{}' 的 url 必须是 http(s) 地址",
                manifest.id,
                asset.name
            );
        }
        let sha = asset.sha256.trim();
        if sha.len() != 64 || !sha.bytes().all(|b| b.is_ascii_hexdigit()) {
            anyhow::bail!(
                "插件 '{}' 大文件 '{}' 的 sha256 必须是 64 位 hex",
                manifest.id,
                asset.name
            );
        }
        if asset.size == 0 {
            anyhow::bail!(
                "插件 '{}' 大文件 '{}' 的 size 必须大于 0",
                manifest.id,
                asset.name
            );
        }
    }
    Ok(())
}

/// host 是否只含合法字符：字母数字、点、连字符、可选 `:端口`。
///
/// 不在此做 IP/内网段判定（那是审核侧规则；客户端只保证能安全拼进 URL）。
fn is_clean_host(host: &str) -> bool {
    let (name, port) = match host.rsplit_once(':') {
        Some((n, p)) => (n, p),
        None => (host, ""),
    };
    if name.is_empty()
        || !name
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || c == '.' || c == '-')
    {
        return false;
    }
    if !port.is_empty() && !port.bytes().all(|b| b.is_ascii_digit()) {
        return false;
    }
    true
}

#[cfg(test)]
mod tests {
    use super::*;

    const VALID: &str = r#"
id = "tavily"
name = "Tavily 搜索"
description = "联网搜索"
version = "0.1.0"

[[env]]
key = "TAVILY_API_KEY"
label = "Tavily Key"

[[tools]]
name = "tavily_search"
description = "搜索"
parameters = '{ "type":"object", "properties":{ "query":{"type":"string"} }, "required":["query"] }'
script = "tavily.py"
"#;

    #[test]
    fn parses_valid_manifest() {
        let m = parse(VALID).unwrap();
        assert_eq!(m.id, "tavily");
        assert_eq!(m.tools.len(), 1);
        assert_eq!(m.tools[0].name, "tavily_search");
        assert_eq!(m.env[0].key, "TAVILY_API_KEY");
    }

    #[test]
    fn rejects_unknown_fields() {
        let bad = VALID.replace("[[env]]", "[[extra]]\nfoo = 1");
        assert!(parse(&bad).is_err());
    }

    #[test]
    fn rejects_unsafe_script_path() {
        let bad = VALID.replace("script = \"tavily.py\"", "script = \"../../etc/passwd\"");
        assert!(parse(&bad).is_err());
    }

    #[test]
    fn rejects_bad_parameters_json() {
        let bad = VALID.replace(
            "parameters = '{ \"type\":\"object\"",
            "parameters = 'not json",
        );
        assert!(parse(&bad).is_err());
    }

    #[test]
    fn rejects_empty_id() {
        let bad = VALID.replace("id = \"tavily\"", "id = \"\"");
        assert!(parse(&bad).is_err());
    }

    #[test]
    fn parses_network_and_permissions() {
        let text = r#"
id = "tavily"
name = "Tavily 搜索"
description = "联网搜索"
version = "0.1.0"

[[network]]
host = "api.tavily.com"
paths = ["/search", "/extract"]
https_only = true

[[network]]
host = "tavily.com:443"

[[permissions.tools]]
name = "memory_add_note"

[[assets]]
name = "model.onnx"
url = "https://github.com/x/y/releases/download/v1/model.onnx"
sha256 = "9f2c1f2f0e3d4c5b6a7f8e9d0c1b2a3f4e5d6c7b8a9f0e1d2c3b4a5f6e7d8c9b0a"
size = 52428800

[[tools]]
name = "tavily_search"
description = "搜索"
parameters = '{ "type":"object", "properties":{ "query":{"type":"string"} }, "required":["query"] }'
script = "tavily.py"
"#;
        let m = parse(text).unwrap();
        assert_eq!(m.network.len(), 2);
        assert_eq!(m.network[0].host, "api.tavily.com");
        assert_eq!(m.network[0].paths, vec!["/search", "/extract"]);
        assert!(m.network[0].https_only);
        // 默认 https_only = true
        assert!(m.network[1].https_only);
        assert_eq!(m.permissions.tools[0].name, "memory_add_note");
        assert_eq!(m.assets[0].name, "model.onnx");
        assert_eq!(m.assets[0].size, 52_428_800);
    }

    #[test]
    fn rejects_bad_network_decl() {
        // host 带 scheme / 路径 → 拒绝
        let bad = VALID.replace(
            "script = \"tavily.py\"",
            "script = \"tavily.py\"\n[[network]]\nhost = \"https://api.tavily.com/path\"",
        );
        assert!(parse(&bad).is_err());
        // 路径不以 / 开头 → 拒绝
        let bad2 = VALID.replace(
            "script = \"tavily.py\"",
            "script = \"tavily.py\"\n[[network]]\nhost = \"api.tavily.com\"\npaths = [\"search\"]",
        );
        assert!(parse(&bad2).is_err());
    }

    #[test]
    fn rejects_bad_asset_decl() {
        // sha256 不是 64 位 hex
        let bad = VALID.replace(
            "script = \"tavily.py\"",
            "script = \"tavily.py\"\n[[assets]]\nname = \"m\"\nurl = \"https://x/y.zip\"\nsha256 = \"short\"\nsize = 1",
        );
        assert!(parse(&bad).is_err());
        // url 非 http(s)
        let bad2 = VALID.replace(
            "script = \"tavily.py\"",
            "script = \"tavily.py\"\n[[assets]]\nname = \"m\"\nurl = \"ftp://x/y.zip\"\nsha256 = \"9f2c1f2f0e3d4c5b6a7f8e9d0c1b2a3f4e5d6c7b8a9f0e1d2c3b4a5f6e7d8c9b0a\"\nsize = 1",
        );
        assert!(parse(&bad2).is_err());
    }

    #[test]
    fn rejects_empty_permission_tool() {
        let bad = VALID.replace(
            "script = \"tavily.py\"",
            "script = \"tavily.py\"\n[[permissions.tools]]\nname = \"\"",
        );
        assert!(parse(&bad).is_err());
    }
}
