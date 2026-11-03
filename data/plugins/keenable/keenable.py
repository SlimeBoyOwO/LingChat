"""Keenable 插件：联网搜索。

依赖宿主注入的 plugin_host.http_post（复用 reqwest，Android 兼容）。
无需 API Key：未设置 KEENABLE_API_KEY 时使用免费公共端点；
设置后改用带 Key 的端点（请求头 X-API-Key）。
ctx 是 dict：用 ctx["tool_name"] / ctx["args"] / ctx["env"] 访问。
"""

from plugin_host import http_post

PUBLIC_SEARCH_URL = "https://api.keenable.ai/v1/search/public"
KEYED_SEARCH_URL = "https://api.keenable.ai/v1/search"
SNIPPET_MAX_CHARS = 1000


def run(ctx):
    """插件入口：按 ctx["tool_name"] 分派。"""
    tool = ctx["tool_name"]
    if tool == "keenable_search":
        return _search(ctx)
    return {"ok": False, "error": {"code": "unknown_tool", "message": "未知工具" + tool}}


def _search(ctx):
    args = ctx["args"]
    max_results = args.get("max_results", 5)
    headers = {"X-Keenable-Title": "LingChat"}
    key = ctx["env"].get("KEENABLE_API_KEY")
    if key:
        headers["X-API-Key"] = key
        url = KEYED_SEARCH_URL
    else:
        url = PUBLIC_SEARCH_URL
    body = {"query": args["query"], "max_results": max_results}
    data = http_post(url, headers=headers, body=body, timeout_ms=30000)
    if not data.get("ok"):
        return {"ok": False, "error": data.get("error"), "status": data.get("status")}
    body_resp = data.get("body", {}) or {}
    results = []
    for r in body_resp.get("results", []) or []:
        if not r.get("url"):
            continue
        # 优先使用简短的 description，没有时截断较长的 snippet
        content = r.get("description") or (r.get("snippet") or "")[:SNIPPET_MAX_CHARS]
        results.append({"title": r.get("title"), "url": r.get("url"), "content": content})
    return {"ok": True, "results": results[:max_results]}
