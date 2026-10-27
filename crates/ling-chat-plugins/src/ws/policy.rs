//! WS 策略：URL 白名单与 `${config.x}` / `${env.X}` 占位符。

use std::collections::HashMap;

use serde_json::Value;

/// 校验 client 连接的目标 url 是否在白名单内。空白名单 = 允许全网段。
pub(crate) fn check_ws_url_allowed(url: &str, allow: &[String]) -> Result<(), String> {
    if allow.is_empty() {
        return Ok(());
    }
    let url = url.trim();
    if allow.iter().any(|pat| ws_url_matches(url, pat.trim())) {
        Ok(())
    } else {
        Err(format!(
            "不允许连接该 URL（未在 manifest 的 ws_allow 中声明）: {url}"
        ))
    }
}

/// 单个模式匹配：`*` 匹配全部；否则按 [`glob_match`]（前缀 + `*` 通配）。
fn ws_url_matches(url: &str, pattern: &str) -> bool {
    if pattern.is_empty() {
        return false;
    }
    if pattern == "*" {
        return true;
    }
    glob_match(url, pattern)
}

/// 极简 glob：无 `*` 时按前缀匹配；有 `*` 时首段锚定前缀、末段锚定后缀、
/// 中间段顺序查找。不处理 `?` 与字符类。模式以 `*` 结尾时等价于纯前缀。
///
/// 无 `*` 的前缀匹配必须停在 URL 分隔符上：否则白名单里写 `wss://a.com`
/// 会连同 `wss://a.com.evil.com` 一起放行，白名单形同虚设。
fn glob_match(text: &str, pattern: &str) -> bool {
    if !pattern.contains('*') {
        let Some(rest) = text.strip_prefix(pattern) else {
            return false;
        };
        if rest.is_empty() {
            return true;
        }
        // 模式自身已收在分隔符上（如 `.../`），后续路径不再设限。
        if matches!(pattern.chars().last(), Some('/' | ':' | '?' | '#')) {
            return true;
        }
        return matches!(rest.chars().next(), Some('/' | ':' | '?' | '#'));
    }
    let parts: Vec<&str> = pattern.split('*').collect();
    let mut pos = 0usize;
    if !parts[0].is_empty() {
        if !text.starts_with(parts[0]) {
            return false;
        }
        pos = parts[0].len();
    }
    let last = parts.len() - 1;
    for (i, part) in parts.iter().enumerate().skip(1) {
        if part.is_empty() {
            continue;
        }
        if i == last {
            return text[pos..].ends_with(part);
        }
        match text[pos..].find(part) {
            Some(idx) => pos += idx + part.len(),
            None => return false,
        }
    }
    true
}

/// 把 `${config.key}` / `${env.KEY}` 占位符替换为运行期值。未命中的占位符原样保留。
pub(crate) fn resolve_placeholders(
    text: &str,
    config: &HashMap<String, Value>,
    env: &HashMap<String, String>,
) -> String {
    let mut out = String::with_capacity(text.len());
    let mut rest = text;
    while let Some(start) = rest.find("${") {
        out.push_str(&rest[..start]);
        let after = &rest[start + 2..];
        let Some(end) = after.find('}') else {
            // 没有闭合，剩余部分原样输出
            out.push_str(&rest[start..]);
            return out;
        };
        let key = &after[..end];
        match lookup_placeholder(key, config, env) {
            Some(v) => out.push_str(&v),
            None => {
                out.push_str("${");
                out.push_str(key);
                out.push('}');
            },
        }
        rest = &after[end + 1..];
    }
    out.push_str(rest);
    out
}

/// 解析占位符键（`config.x` / `env.X`）。
fn lookup_placeholder(
    key: &str,
    config: &HashMap<String, Value>,
    env: &HashMap<String, String>,
) -> Option<String> {
    if let Some(k) = key.strip_prefix("config.") {
        config.get(k).map(|v| match v {
            Value::String(s) => s.clone(),
            other => other.to_string(),
        })
    } else if let Some(k) = key.strip_prefix("env.") {
        env.get(k).cloned()
    } else {
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn empty_allow_allows_any() {
        assert!(check_ws_url_allowed("wss://evil.com/x", &[]).is_ok());
    }

    #[test]
    fn prefix_and_glob_matching() {
        let allow = vec!["wss://example.com".to_string()];
        assert!(check_ws_url_allowed("wss://example.com/ws", &allow).is_ok());
        assert!(check_ws_url_allowed("wss://evil.com/ws", &allow).is_err());
    }

    #[test]
    fn prefix_must_stop_at_url_boundary() {
        let allow = vec!["wss://example.com".to_string()];
        assert!(check_ws_url_allowed("wss://example.com:8443/ws", &allow).is_ok());
        // 不能让 example.com 顺带放行 example.com.evil.com
        assert!(check_ws_url_allowed("wss://example.com.evil.com/ws", &allow).is_err());
    }

    #[test]
    fn wildcard_host_matching() {
        let allow = vec!["wss://*.example.com/*".to_string()];
        assert!(check_ws_url_allowed("wss://a.example.com/ws", &allow).is_ok());
        assert!(check_ws_url_allowed("wss://a.example.com.evil.com/ws", &allow).is_err());
    }

    #[test]
    fn star_allows_any() {
        let allow = vec!["*".to_string()];
        assert!(check_ws_url_allowed("ws://127.0.0.1:9000", &allow).is_ok());
    }

    #[test]
    fn resolve_config_and_env() {
        let mut config = HashMap::new();
        config.insert("token".to_string(), Value::String("abc".to_string()));
        let mut env = HashMap::new();
        env.insert("HOST".to_string(), "example.com".to_string());
        let out = resolve_placeholders(
            "wss://${env.HOST}/ws?t=${config.token}&x=${config.missing}",
            &config,
            &env,
        );
        assert_eq!(out, "wss://example.com/ws?t=abc&x=${config.missing}");
    }

    #[test]
    fn resolve_leaves_unclosed_placeholder() {
        assert_eq!(
            resolve_placeholders("a${b", &HashMap::new(), &HashMap::new()),
            "a${b"
        );
    }
}
