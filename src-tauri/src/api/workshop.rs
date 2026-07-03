//! 创意工坊 — 从 GitHub Discussions 获取内容列表。
//!
//! 优先使用 GraphQL API（需要用户在高级设置中配置 GitHub Token）
//! 以获取准确的 upvote 计数；未配置 Token 时降级为 REST API。
//! 自动解析 Discussion body 中的头像图片和描述信息。

use regex::Regex;
use serde::{Deserialize, Serialize};
use std::sync::Mutex;
use tauri::AppHandle;

use crate::config;

const REST_URL: &str = "https://api.github.com/repos/SlimeBoyOwO/LingChat/discussions?per_page=100";
const GRAPHQL_URL: &str = "https://api.github.com/graphql";

const GRAPHQL_QUERY: &str = r#"
query {
  repository(owner: "SlimeBoyOwO", name: "LingChat") {
    discussions(first: 100, orderBy: {field: CREATED_AT, direction: DESC}) {
      nodes {
        number
        title
        body
        url
        upvoteCount
        category { name }
        author { login }
        createdAt
      }
    }
  }
}
"#;

/// 内存缓存（5 分钟 TTL），减少重复 API 请求。
/// 存储 (数据, 时间戳, 是否来自 GraphQL)。
static CACHE: Mutex<Option<(Vec<Discussion>, std::time::Instant, bool)>> = Mutex::new(None);
const CACHE_TTL: std::time::Duration = std::time::Duration::from_secs(300);

// ─── Response Types ────────────────────────────────────────────

/// Discussion 的分类
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DiscussionCategory {
    pub name: String,
}

/// Discussion 作者
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DiscussionAuthor {
    pub login: String,
}

/// 单个 Discussion 条目（统一返回类型）
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Discussion {
    pub number: u64,
    pub title: String,
    pub body: String,
    pub html_url: String,
    pub category: DiscussionCategory,
    /// 作者（GitHub API 字段名为 "user"，用 alias 兼容）
    #[serde(alias = "user", default)]
    pub author: Option<DiscussionAuthor>,
    pub created_at: String,
    /// 真实 upvote 数（GraphQL 有效，REST 为 0）
    #[serde(default)]
    pub upvotes: u64,
    /// 是否有真实 upvote 数据（GraphQL=true，REST=false）
    #[serde(default)]
    pub has_upvotes: bool,
    /// 👍 emoji 数量（REST reactions.+1，GraphQL 时为 0 用 upvotes 代替）
    #[serde(default)]
    pub reactions_upvotes: u64,
    /// 解析出的头像图片 URL
    #[serde(default)]
    pub avatar_url: Option<String>,
    /// 解析出的描述文本
    #[serde(default)]
    pub description: Option<String>,
    /// 解析出的标签列表（## 标签 段落内容按空白分割）
    #[serde(default)]
    pub tags: Vec<String>,
}

// ─── REST API Types ─────────────────────────────────────────────

#[derive(Debug, Deserialize)]
struct RestDiscussion {
    number: u64,
    title: String,
    body: String,
    html_url: String,
    category: DiscussionCategory,
    #[serde(alias = "user", default)]
    author: Option<DiscussionAuthor>,
    created_at: String,
    #[serde(default)]
    reactions: Option<RestReactions>,
}

#[derive(Debug, Deserialize)]
struct RestReactions {
    #[serde(rename = "+1", default)]
    plus_one: u64,
}

// ─── GraphQL Types ──────────────────────────────────────────────

#[derive(Debug, Deserialize)]
struct GqlResponse {
    #[serde(default)]
    data: Option<GqlData>,
    #[serde(default)]
    errors: Option<Vec<GqlError>>,
}

#[derive(Debug, Deserialize)]
struct GqlError {
    message: String,
}

#[derive(Debug, Deserialize)]
struct GqlData {
    repository: GqlRepo,
}

#[derive(Debug, Deserialize)]
struct GqlRepo {
    discussions: GqlDiscussions,
}

#[derive(Debug, Deserialize)]
struct GqlDiscussions {
    nodes: Vec<GqlDiscussion>,
}

#[derive(Debug, Deserialize)]
struct GqlDiscussion {
    number: u64,
    title: String,
    body: String,
    url: String,
    #[serde(rename = "upvoteCount", default)]
    upvote_count: u64,
    category: DiscussionCategory,
    #[serde(default)]
    author: Option<DiscussionAuthor>,
    #[serde(rename = "createdAt")]
    created_at: String,
}

// ─── Markdown 解析 ─────────────────────────────────────────────

fn parse_avatar(body: &str) -> Option<String> {
    let re = Regex::new(r"!\[[^\]]*\]\(([^)]+)\)").ok()?;
    re.captures(body)
        .and_then(|cap| cap.get(1))
        .map(|m| m.as_str().to_string())
}

fn parse_description(body: &str) -> Option<String> {
    let heading_re = Regex::new(r"(?m)^##\s*(?:相关信息|介绍|描述|简介|说明|概述)\s*$").ok()?;
    let heading_match = heading_re.find(body)?;
    let start = heading_match.end();
    let rest = &body[start..];
    let next_heading_re = Regex::new(r"(?m)^##\s").ok()?;
    let end = next_heading_re
        .find(rest)
        .map(|m| m.start())
        .unwrap_or(rest.len());
    let content = rest[..end].trim().to_string();
    if content.is_empty() {
        return None;
    }
    Some(content)
}

/// 提取 "## 标签" 段落的内容，按空白字符分割为标签列表
fn parse_tags(body: &str) -> Vec<String> {
    let normalized = body.replace("\r\n", "\n");
    let heading_re = Regex::new(r"(?m)^##\s*标签\s*$").ok();
    let heading_match = match heading_re.as_ref().and_then(|re| re.find(&normalized)) {
        Some(m) => m,
        None => return Vec::new(),
    };
    let start = heading_match.end();
    let rest = &normalized[start..];
    let next_heading_re = Regex::new(r"(?m)^##\s").ok();
    let end = next_heading_re
        .as_ref()
        .and_then(|re| re.find(rest))
        .map(|m| m.start())
        .unwrap_or(rest.len());
    let content = rest[..end].trim();
    if content.is_empty() {
        return Vec::new();
    }
    // 按空白字符分割，过滤空串
    content.split_whitespace().map(|s| s.to_string()).collect()
}

fn enrich_discussions(discussions: &mut [Discussion]) {
    for d in discussions {
        let normalized = d.body.replace("\r\n", "\n");
        let avatar_url = parse_avatar(&normalized);
        let description = parse_description(&normalized);
        let tags = parse_tags(&d.body);
        d.avatar_url = avatar_url;
        d.description = description;
        d.tags = tags;
    }
}

// ─── HTTP Client ────────────────────────────────────────────────

fn build_client() -> Result<reqwest::Client, String> {
    reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(15))
        .user_agent("LingChat-Workshop/1.0")
        .build()
        .map_err(|e| format!("创建 HTTP 客户端失败: {}", e))
}

// ─── GraphQL Fetch ──────────────────────────────────────────────

async fn fetch_via_graphql(token: &str) -> Result<Vec<Discussion>, String> {
    let client = build_client()?;
    let resp = client
        .post(GRAPHQL_URL)
        .header("Authorization", format!("Bearer {}", token))
        .header("Accept", "application/json")
        .json(&serde_json::json!({ "query": GRAPHQL_QUERY }))
        .send()
        .await
        .map_err(|e| format!("GraphQL 请求失败: {}", e))?;

    if !resp.status().is_success() {
        let msg = format!(
            "GraphQL API 返回 {} {}（请检查 Token 是否正确）",
            resp.status().as_u16(),
            resp.status().canonical_reason().unwrap_or("")
        );
        return Err(msg);
    }

    let gql_resp: GqlResponse = resp
        .json()
        .await
        .map_err(|e| format!("解析 GraphQL 响应失败: {}", e))?;

    if let Some(errors) = gql_resp.errors {
        if !errors.is_empty() {
            return Err(format!("GraphQL 错误: {}", errors[0].message));
        }
    }

    let nodes = gql_resp
        .data
        .ok_or_else(|| "GraphQL 返回空数据".to_string())?
        .repository
        .discussions
        .nodes;

    let mut discussions: Vec<Discussion> = nodes
        .into_iter()
        .map(|g| Discussion {
            number: g.number,
            title: g.title,
            body: g.body,
            html_url: g.url,
            category: g.category,
            author: g.author,
            created_at: g.created_at,
            upvotes: g.upvote_count,
            has_upvotes: true,
            reactions_upvotes: 0,
            avatar_url: None,
            description: None,
            tags: Vec::new(),
        })
        .collect();

    enrich_discussions(&mut discussions);
    Ok(discussions)
}

// ─── REST Fetch ────────────────────────────────────────────────

fn format_rate_limit_error(resp: &reqwest::Response, status: reqwest::StatusCode) -> String {
    let remaining = resp
        .headers()
        .get("x-ratelimit-remaining")
        .and_then(|v| v.to_str().ok())
        .unwrap_or("?");
    let reset = resp
        .headers()
        .get("x-ratelimit-reset")
        .and_then(|v| v.to_str().ok())
        .and_then(|v| v.parse::<u64>().ok())
        .map(|ts| {
            let now = std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap_or_default()
                .as_secs();
            let secs = ts.saturating_sub(now);
            if secs >= 3600 {
                format!("{} 小时", secs / 3600)
            } else if secs >= 60 {
                format!("{} 分钟", secs / 60)
            } else {
                format!("{} 秒", secs)
            }
        });

    if remaining == "0" {
        format!(
            "GitHub API 请求频率已达上限（60 次/小时），请在 {} 后重试。\n小提示：可在高级设置中配置 GitHub Token 解除限制。",
            reset.unwrap_or_else(|| "一段时间".to_string())
        )
    } else {
        format!(
            "GitHub API 返回 {} {}（剩余 {} 次）",
            status.as_u16(),
            status.canonical_reason().unwrap_or(""),
            remaining,
        )
    }
}

async fn fetch_via_rest() -> Result<Vec<Discussion>, String> {
    let client = build_client()?;
    let resp = client
        .get(REST_URL)
        .header("Accept", "application/vnd.github+json")
        .send()
        .await
        .map_err(|e| format!("获取讨论列表失败: {}", e))?;

    if !resp.status().is_success() {
        return Err(format_rate_limit_error(&resp, resp.status()));
    }

    let rest_list: Vec<RestDiscussion> = resp
        .json()
        .await
        .map_err(|e| format!("解析讨论列表 JSON 失败: {}", e))?;

    let mut discussions: Vec<Discussion> = rest_list
        .into_iter()
        .map(|r| Discussion {
            number: r.number,
            title: r.title,
            body: r.body,
            html_url: r.html_url,
            category: r.category,
            author: r.author,
            created_at: r.created_at,
            upvotes: 0,
            has_upvotes: false,
            reactions_upvotes: r.reactions.as_ref().map_or(0, |rx| rx.plus_one),
            avatar_url: None,
            description: None,
            tags: Vec::new(),
        })
        .collect();

    enrich_discussions(&mut discussions);
    Ok(discussions)
}

// ─── Tauri Command ─────────────────────────────────────────────

/// 获取 GitHub Discussions 列表。
///
/// - 配置了 GitHub Token → GraphQL API（真实 upvote 数）
/// - 未配置 Token → REST API（用 👍 表情反应代替）
/// - 5 分钟内重复调用直接返回缓存
#[tauri::command]
pub async fn fetch_discussions(app: AppHandle) -> Result<Vec<Discussion>, String> {
    let token = config::get_setting_string(&app, config::keys::GITHUB_TOKEN);
    let has_token = token.as_ref().map_or(false, |t| !t.trim().is_empty());

    // 检查缓存
    if let Ok(cache) = CACHE.lock() {
        if let Some((ref data, ref ts, ref cached_has_token)) = *cache {
            if ts.elapsed() < CACHE_TTL && *cached_has_token == has_token {
                return Ok(data.clone());
            }
        }
    }

    // 有 Token → GraphQL
    let result = if let Some(ref t) = token {
        if !t.trim().is_empty() {
            match fetch_via_graphql(t.trim()).await {
                Ok(discussions) => Ok((discussions, true)),
                Err(e) => {
                    // GraphQL 失败时降级到 REST
                    tracing::warn!("GraphQL 请求失败，降级到 REST API: {}", e);
                    fetch_via_rest().await.map(|d| (d, false))
                }
            }
        } else {
            fetch_via_rest().await.map(|d| (d, false))
        }
    } else {
        fetch_via_rest().await.map(|d| (d, false))
    };

    match result {
        Ok((discussions, is_graphql)) => {
            if let Ok(mut cache) = CACHE.lock() {
                *cache = Some((discussions.clone(), std::time::Instant::now(), is_graphql));
            }
            Ok(discussions)
        }
        Err(e) => Err(e),
    }
}
