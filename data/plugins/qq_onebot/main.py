# -*- coding: utf-8 -*-
"""QQ · OneBot v11 插件 — 工具入口（群聊事件在 hooks.py）。

两个工具：
  qq_status  看连接/规则/当前回复目标
  qq_send    主动发一条到指定群或私聊
"""
from __future__ import annotations

import json

from plugin_host import ws_send

import rules

CONN_ID = "ob"


def _emotion_file(cfg, emotion):
    """走 rules 里的静态表（插件里不能用 os）。"""
    return rules.emotion_file(cfg, emotion)

def _cfg(ctx) -> dict:
    return ctx.get("config") or {}


def _kv(ctx, key: str):
    """读桥上的小状态（插件没有跨调用内存）。"""
    from plugin_host import http_post
    cfg = _cfg(ctx)
    url = str(cfg.get("state_url") or "http://127.0.0.1:8767").rstrip("/")
    token = str(cfg.get("state_token") or "").strip()
    headers = {"Authorization": "Bearer " + token} if token else {}
    try:
        r = http_post(url + "/v1/act", headers=headers, timeout_ms=4000,
                      body={"action": "kv.get", "params": {"key": key}})
        body = r.get("body") if r.get("ok") else None
        data = json.loads(body) if isinstance(body, str) else (body or {})
        return data.get("value")
    except Exception:                                    # noqa: BLE001
        return None


def _summary(cfg: dict) -> dict:
    return {
        "wake_prefix": rules.split_list(cfg.get("wake_prefix")) or ["/"],
        "at_required_in_group": str(cfg.get("at_required_in_group")).lower() not in ("false", "0", "no"),
        "friend_needs_prefix": str(cfg.get("friend_needs_prefix")).lower() in ("true", "1", "yes"),
        "ignore_bot_self": str(cfg.get("ignore_bot_self")).lower() not in ("false", "0", "no"),
        "ignore_at_all": str(cfg.get("ignore_at_all")).lower() not in ("false", "0", "no"),
        "whitelist_enabled": str(cfg.get("enable_whitelist")).lower() in ("true", "1", "yes"),
        "whitelist": rules.split_list(cfg.get("whitelist")),
        "rate": {"window_s": cfg.get("rate_time"), "count": cfg.get("rate_count")},
        "unique_session": str(cfg.get("unique_session")).lower() not in ("false", "0", "no"),
        "admins": rules.split_list(cfg.get("admins")),
    }


def tool_context(ctx) -> dict:
    """主动看群里刚才在聊什么（不用等被 @）。

    group_id 省略时用"当前这一轮"的群；也可以显式指定别的群。
    """
    cfg = _cfg(ctx)
    args = ctx.get("args") or {}
    try:
        limit = int(args.get("limit") or cfg.get("group_context_inject") or 10)
    except (TypeError, ValueError):
        limit = 10
    limit = max(1, min(limit, 50))

    gid = args.get("group_id")
    if gid is None:
        turn = _kv(ctx, "qq:turn")
        if isinstance(turn, dict) and str(turn.get("type")) == "group":
            gid = turn.get("group_id")
    if gid is None:
        return {"ok": False, "error": "group_id_required",
                "hint": "传 group_id；或者等有人在群里把你叫起来之后我就能知道当前是哪个群"}

    hist = _kv(ctx, f"qq:ctx:{gid}")
    if not isinstance(hist, list):
        return {"ok": True, "group_id": gid, "count": 0, "messages": [],
                "hint": "这个群还没有缓冲（可能刚重启，或 group_context_enabled 被关了）"}
    items = hist[-limit:]
    return {
        "ok": True,
        "group_id": gid,
        "count": len(items),
        "messages": [
            {"time": it.get("t"),
             "sender": "我" if it.get("self") else it.get("n"),
             "qq": it.get("id"),
             "is_owner": bool(it.get("owner")),
             "text": it.get("x")}
            for it in items if isinstance(it, dict)
        ],
        "note": "这是最近若干条群聊；你可以据此接话，不用等别人叫你。",
    }


def tool_connect(ctx) -> dict:
    """手动（重新）打开 OneBot 连接；启动时连不上或改了地址/token 后用它。"""
    from plugin_host import ws_open
    cfg = _cfg(ctx)
    url = str(cfg.get("ws_url") or "").strip()
    if not url:
        return {"ok": False, "error": "ws_url_empty"}
    token = str(cfg.get("access_token") or "").strip()
    if token:
        sep = "&" if "?" in url else "?"
        url = f"{url}{sep}access_token={token}"
    try:
        r = ws_open("ob", url=url)
    except Exception as exc:                             # noqa: BLE001
        return {"ok": False, "error": str(exc)[:200]}
    return {"ok": True, "url": url.split("?")[0], "result": r}


def tool_status(ctx) -> dict:
    cfg = _cfg(ctx)
    target = _kv(ctx, "qq:turn")
    online = _kv(ctx, "qq:status")
    return {
        "ok": True,
        "connection": {"conn_id": CONN_ID, "url": cfg.get("ws_url"),
                       "online_hint": bool(online), "token_set": bool(cfg.get("access_token"))},
        "reply_target": target,
        "rules": _summary(cfg),
        "note": "群聊没反应时按这个顺序查：① 有没有 @ 或带前缀 ② 白名单 ③ 限速 ④ SnowLuma 的 WS 是不是在听",
    }


def tool_send(ctx) -> dict:
    cfg = _cfg(ctx)
    args = ctx.get("args") or {}
    text = str(args.get("text") or "").strip()
    if not text:
        return {"ok": False, "error": "text_required"}
    prefix = str(cfg.get("reply_prefix") or "")
    msg = prefix + text
    group_id = args.get("group_id")
    user_id = args.get("user_id")
    if group_id is None and user_id is None:
        return {"ok": False, "error": "target_required",
                "hint": "群聊传 group_id，私聊传 user_id"}
    segs = [{"type": "text", "data": {"text": msg}}]
    emotion = str(args.get("emotion") or "").strip()
    if emotion:
        path = _emotion_file(cfg, emotion)
        if path:
            segs.append({"type": "image", "data": {"file": path.replace("\\", "/")}})
    if group_id is not None:
        action, params = "send_group_msg", {"group_id": int(group_id), "message": segs}
    else:
        action, params = "send_private_msg", {"user_id": int(user_id), "message": segs}
    try:
        r = ws_send(CONN_ID, {"action": action, "params": params,
                              "echo": "qq_onebot:tool_send"})
    except Exception as exc:                             # noqa: BLE001
        return {"ok": False, "error": "ws_send_failed", "detail": str(exc)[:200]}
    return {"ok": True, "sent": bool(r) if isinstance(r, dict) else True,
            "action": action, "target": group_id if group_id is not None else user_id,
            "chars": len(msg)}


TOOLS = {
    "qq_connect": tool_connect,
    "qq_status": tool_status,
    "qq_send": tool_send,
    "qq_context": tool_context,
}


def run(ctx):
    tool = str(ctx.get("tool_name") or "").strip()
    handler = TOOLS.get(tool)
    if handler is None:
        return {"ok": False, "error": "unknown_tool", "tool": tool,
                "allowed": sorted(TOOLS)}
    try:
        return handler(ctx)
    except Exception as exc:                             # noqa: BLE001
        return {"ok": False, "error": "internal_error", "detail": str(exc)[:300]}
