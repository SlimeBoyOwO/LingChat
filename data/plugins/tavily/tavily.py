"""Tavily 插件：搜索与网页提取。

依赖宿主注入的 plugin_host.http_post（复用 reqwest，Android 兼容）。
api_key 从环境变量 TAVILY_API_KEY 读取，与 tvly CLI 一致。
ctx 是 dict：用 ctx["tool_name"] / ctx["args"] / ctx["env"] 访问。
"""

from plugin_host import http_post

SEARCH_URL = "https://api.tavily.com/search"
EXTRACT_URL = "https://api.tavily.com/extract"


def run(ctx):
    """插件入口：按 ctx["tool_name"] 分派。"""
    tool = ctx["tool_name"]
    if tool == "tavily_search":
        return _search(ctx)
    if tool == "tavily_extract":
        return _extract(ctx)
    return {"ok": False, "error": {"code": "unknown_tool", "message": "未知工具" + tool}}


def _api_key(ctx):
    key = ctx["env"].get("TAVILY_API_KEY")
    if not key:
        raise ValueError("缺少环境变量 TAVILY_API_KEY")
    return key


def _search(ctx):
    args = ctx["args"]
    # Tavily 官方鉴权：Authorization: Bearer <api_key>（见 docs.tavily.com）。
    headers = {"Authorization": "Bearer " + _api_key(ctx)}
    body = {
        "query": args["query"],
        "max_results": args.get("max_results", 5),
        "include_answer": False,
    }
    data = http_post(SEARCH_URL, headers=headers, body=body, timeout_ms=30000)
    if not data.get("ok"):
        return {"ok": False, "error": data.get("error"), "status": data.get("status")}
    # http_host 把响应包成 {status, ok, body}，Tavily 的结果在 body 里
    body_resp = data.get("body", {}) or {}
    results = body_resp.get("results", [])
    return {
        "ok": True,
        "results": [
            {
                "title": r.get("title"),
                "url": r.get("url"),
                "content": r.get("content"),
            }
            for r in results
        ],
    }


def _extract(ctx):
    args = ctx["args"]
    headers = {"Authorization": "Bearer " + _api_key(ctx)}
    body = {"urls": [args["url"]]}
    data = http_post(EXTRACT_URL, headers=headers, body=body, timeout_ms=30000)
    if not data.get("ok"):
        return {"ok": False, "error": data.get("error"), "status": data.get("status")}
    body_resp = data.get("body", {}) or {}
    return {"ok": True, "results": body_resp.get("results", [])}
