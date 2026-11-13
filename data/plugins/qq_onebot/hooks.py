# -*- coding: utf-8 -*-
"""QQ · OneBot v11（SnowLuma）— 宿主回调。

四条链路：
  1. on_start         启用时清状态、按配置打开连接、切回复长度档
  2. on_ws            OneBot 事件：先无条件记群聊上下文，再按唤醒规则决定要不要理
  3. on_line_shown    角色每句台词上屏 → 攒起来，这一轮结束时合并成一条发回去
  4. tools            qq_status / qq_send（在 main.py）

为什么用本机执行桥存状态：0.6.0 的插件脚本每次调用都新建解释器、跑完即弃，
on_ws 和 on_line_shown 是两次调用，没有共享内存；桥是常驻的。
"""
from __future__ import annotations

import json
import time

from plugin_host import (http_get, http_post, send_user_message, set_auto_mode,
                         set_reply_length_mode, ws_open, ws_send)

import rules

CONN_ID = "ob"
KV_TURN = "qq:turn"
KV_PENDING = "qq:pending"
KV_RATE = "qq:rl:"
KV_CTX = "qq:ctx:"

DEFAULT_STATE_URL = ""          # 留空 = 不需要外部状态服务（单目标模式）


# ───────────────────────── 基础工具 ─────────────────────────

def cfg_of(ctx) -> dict:
    return ctx.get("config") or {}


def _state(cfg: dict):
    """返回 (base_url, headers)。url 为空表示没有状态后端。"""
    url = str(cfg.get("state_url") or DEFAULT_STATE_URL).strip().rstrip("/")
    token = str(cfg.get("state_token") or "").strip()
    return url, ({"Authorization": "Bearer " + token} if token else {})


def _has_state(cfg: dict) -> bool:
    return bool(_state(cfg)[0])


def kv_get(ctx, key: str):
    url, headers = _state(cfg_of(ctx))
    if not url:
        return None
    try:
        r = http_post(url + "/v1/act", headers=headers, timeout_ms=4000,
                      body={"action": "kv.get", "params": {"key": key}})
        body = r.get("body") if r.get("ok") else None
        data = json.loads(body) if isinstance(body, str) else (body or {})
        return data.get("value")
    except Exception:                                    # noqa: BLE001
        return None


def kv_set(ctx, key: str, value, ttl_s: float | None = None) -> None:
    url, headers = _state(cfg_of(ctx))
    if not url:
        return
    params = {"key": key, "value": value}
    if ttl_s:
        params["ttl_s"] = ttl_s
    try:
        http_post(url + "/v1/act", headers=headers, timeout_ms=4000,
                  body={"action": "kv.set", "params": params})
    except Exception:                                    # noqa: BLE001
        pass


def _truthy(v, default: bool = False) -> bool:
    if v is None:
        return default
    if isinstance(v, bool):
        return v
    return str(v).strip().lower() in ("1", "true", "yes", "on", "是")


def _int(v, default: int) -> int:
    try:
        return int(v)
    except (TypeError, ValueError):
        return default


def _frame(payload):
    data = payload
    if isinstance(data, (bytes, bytearray)):
        try:
            data = data.decode("utf-8", "replace")
        except Exception:                                # noqa: BLE001
            return None
    if isinstance(data, str):
        try:
            data = json.loads(data)
        except json.JSONDecodeError:
            return None
    return data if isinstance(data, dict) else None


def _display_name(frame: dict) -> str:
    sender = frame.get("sender")
    if isinstance(sender, dict):
        return str(sender.get("card") or sender.get("nickname") or "")
    return ""



def _is_owner(cfg: dict, uid) -> bool:
    """配置里的 owner_qq 就是主人；没配则谁都不是主人（安全默认）。"""
    owners = {x.strip() for x in str(cfg.get("owner_qq") or "").replace("，", ",").split(",")
              if x.strip()}
    return str(uid) in owners


# ───────────────────────── 群聊上下文 ─────────────────────────

def remember_context(ctx, frame: dict) -> None:
    """把每条群消息记进环形缓冲 —— 让角色「看得见」没被叫到时的群聊。"""
    cfg = cfg_of(ctx)
    if not _truthy(cfg.get("group_context_enabled"), True):
        return
    if str(frame.get("message_type")) != "group":
        return
    gid = str(frame.get("group_id") or "")
    if not gid:
        return
    size = max(0, min(_int(cfg.get("group_context_size"), 12), 50))
    if size == 0:
        return
    segs = frame.get("message") if isinstance(frame.get("message"), list) else []
    text = "".join(str(s.get("data", {}).get("text", "")) for s in segs
                   if s.get("type") == "text").strip()
    if not text:
        text = "[非文本消息]"
    key = KV_CTX + gid
    hist = kv_get(ctx, key)
    if not isinstance(hist, list):
        hist = []
    uid = str(frame.get("user_id") or "")
    hist.append({
        "t": time.strftime("%H:%M"),
        "n": _display_name(frame) or uid,
        "id": uid,
        "x": text[:100],
        "self": (uid == str(frame.get("self_id"))),
        "owner": _is_owner(cfg, uid),
    })
    kv_set(ctx, key, hist[-size:], ttl_s=_int(cfg.get("group_context_ttl_s"), 600))


def context_suffix(ctx, frame: dict) -> str:
    """把最近群聊拼成一段附在正文后面（排除当前这一条）。"""
    cfg = cfg_of(ctx)
    if not _truthy(cfg.get("group_context_enabled"), True):
        return ""
    if str(frame.get("message_type")) != "group":
        return ""
    gid = str(frame.get("group_id") or "")
    hist = kv_get(ctx, KV_CTX + gid)
    if not isinstance(hist, list) or len(hist) <= 1:
        return ""
    take = max(0, _int(cfg.get("group_context_inject"), 6))
    if take == 0:
        return ""
    lines = []
    for item in hist[-(take + 1):-1]:
        if not isinstance(item, dict):
            continue
        if item.get("self"):
            who = "我"
        else:
            tag = "主人" if item.get("owner") else "群友"
            who = f"{item.get('n')}({item.get('id')})[{tag}]"
        lines.append(f"{who}: {item.get('x')}")
    if not lines:
        return ""
    return "\n（群里刚才还在聊：\n" + "\n".join(lines) + "）"



# ───────────────────────── 立绘表情 ─────────────────────────

EMOTION_FALLBACK = "正常"


def _emotion_dir(cfg: dict) -> str:
    return str(cfg.get("emotion_image_dir") or "").strip().rstrip("\\/")


def _emotion_file(cfg: dict, emotion: str, character: str = "") -> str:
    """情绪 + 角色 → 立绘路径（走 rules 里的静态表，插件里不能用 os）。"""
    return rules.emotion_file(cfg, emotion, character)

def _image_segment(path: str) -> dict:
    """OneBot v11 图片段：本地绝对路径即可（SnowLuma 两种写法都认）。"""
    return {"type": "image", "data": {"file": path.replace("\\", "/")}}


# ───────────────────────── 限速 / 发送 ─────────────────────────

def rate_allowed(ctx, cfg: dict, key: str) -> bool:
    if not _has_state(cfg):
        return True                                  # 单目标模式没有共享计数，靠窗口参数兜底
    window = _int(cfg.get("rate_time"), 60)
    count = _int(cfg.get("rate_count"), 10)
    if count <= 0 or window <= 0:
        return True
    k = KV_RATE + key
    now = time.time()
    hist = kv_get(ctx, k)
    if not isinstance(hist, list):
        hist = []
    hist = [float(t) for t in hist if isinstance(t, (int, float)) and now - t < window]
    if len(hist) >= count:
        return False
    hist.append(now)
    kv_set(ctx, k, hist, ttl_s=window * 2)
    return True


def _segments(cfg: dict, text: str, frame: dict, with_mention: bool) -> list:
    segs = []
    if _truthy(cfg.get("reply_with_quote"), False) and frame.get("message_id"):
        segs.append({"type": "reply", "data": {"id": str(frame.get("message_id"))}})
    if with_mention and str(frame.get("message_type")) == "group":
        segs.append({"type": "at", "data": {"qq": str(frame.get("user_id"))}})
    segs.append({"type": "text", "data": {"text": text}})
    return segs


def _split_chunks(text: str, limit: int) -> list:
    """按句末标点切分，尽量不超过 limit 字；超长硬切。"""
    if limit <= 0 or len(text) <= limit:
        return [text]
    chunks, buf = [], ""
    for ch in text:
        buf += ch
        if len(buf) >= limit and ch in "。！？…\n.!? ":
            chunks.append(buf.strip())
            buf = ""
    if buf.strip():
        chunks.append(buf.strip())
    out = []
    for c in chunks:
        while len(c) > limit:
            out.append(c[:limit])
            c = c[limit:]
        if c:
            out.append(c)
    return out


def _send_now(ctx, target: dict, text: str, emotion: str = "",
              character: str = "") -> dict:
    cfg = cfg_of(ctx)
    text = (str(cfg.get("reply_prefix") or "") + text).strip()
    if not text:
        return {"ok": False, "error": "empty"}
    is_group = str(target.get("type")) == "group"
    mention = _truthy(cfg.get("reply_with_mention"), False)
    limit = max(0, _int(cfg.get("max_chars_per_message"), 300))
    chunks = _split_chunks(text, limit)

    # 立绘：整轮只配一张，挂在最后一条上（分条时不会重复刷图）
    img_path = ""
    if _truthy(cfg.get("emotion_image_enabled"), True):
        img_path = _emotion_file(cfg, emotion, character)
    position = str(cfg.get("emotion_image_position") or "after").lower()

    results = []
    for i, chunk in enumerate(chunks):
        last = (i == len(chunks) - 1)
        body_segs = _segments(cfg, chunk, target.get("frame") or {},
                              mention and i == 0)
        if img_path and last:
            seg = _image_segment(img_path)
            body_segs = ([seg] + body_segs) if position == "before" else (body_segs + [seg])
        if is_group:
            action = "send_group_msg"
            params = {"group_id": target.get("group_id"), "message": body_segs}
        else:
            action = "send_private_msg"
            params = {"user_id": target.get("user_id"), "message": body_segs}
        try:
            r = ws_send(CONN_ID, {"action": action, "params": params,
                                  "echo": f"qq_onebot:{action}:{i}"})
            results.append(r if isinstance(r, dict) else {"ok": True})
        except Exception as exc:                         # noqa: BLE001
            results.append({"ok": False, "error": str(exc)[:120]})
        if i < len(chunks) - 1:
            # 分条之间留点间隔：几乎同时到达的多条，QQ 服务器可能重排
            time.sleep(0.35)
    out = {"ok": all(r.get("ok", True) for r in results), "chunks": len(chunks)}
    kv_set(ctx, "qq:last_send", {"action": "send_group_msg" if is_group
                                 else "send_private_msg",
                                 "result": out, "text": text[:80]}, ttl_s=600)
    return out



def _fixed_target(cfg: dict):
    """单目标模式：从配置里拿固定的群/私聊，不需要跨调用状态。"""
    gid = str(cfg.get("default_target_group") or "").strip()
    uid = str(cfg.get("default_target_user") or "").strip()
    if not gid and not uid:
        return None
    if gid:
        return {"type": "group", "group_id": int(gid) if gid.isdigit() else gid,
                "user_id": None, "message_id": None, "active": True, "frame": {}}
    return {"type": "private", "group_id": None,
            "user_id": int(uid) if uid.isdigit() else uid,
            "message_id": None, "active": True, "frame": {}}


# ───────────────────────── 发送队列（FIFO）─────────────────────────

KV_OUTBOX = "qq:outbox"


def _enqueue(ctx, target: dict, text: str, emotion: str = "",
             seq=None, character: str = "") -> dict:
    """把一条要发的内容压进队列，然后立刻尝试按序消费。"""
    q = kv_get(ctx, KV_OUTBOX)
    if not isinstance(q, list):
        q = []
    q.append({
        "text": text,
        "emotion": emotion,
        "character": character,
        "seq": str(seq) if seq is not None else "",
        "t": time.strftime("%H:%M:%S"),
    })
    kv_set(ctx, KV_OUTBOX, q[-30:], ttl_s=300)
    return _drain(ctx, target)


def _drain(ctx, target: dict) -> dict:
    """按入队顺序把队列发空（单消费者：先 pop 再发，避免重复）。"""
    sent, failed = 0, 0
    guard = 0
    while guard < 20:
        guard += 1
        q = kv_get(ctx, KV_OUTBOX)
        if not isinstance(q, list) or not q:
            break
        item = q[0]
        # 先落盘"已取走"，再发送：两个并发 drain 最多丢一条，不会重复发
        kv_set(ctx, KV_OUTBOX, q[1:], ttl_s=300)
        if not isinstance(item, dict):
            continue
        text = str(item.get("text") or "").strip()
        if not text:
            continue
        r = _send_now(ctx, target, text, str(item.get("emotion") or ""),
                      str(item.get("character") or ""))
        if r.get("ok"):
            sent += 1
        else:
            failed += 1
    return {"ok": failed == 0, "sent": sent, "failed": failed}


# ───────────────────────── 宿主回调 ─────────────────────────

def _apply_reply_mode(ctx, cfg: dict) -> None:
    mode = str(cfg.get("reply_length_mode") or "").strip().lower()
    if mode in ("short", "normal", "free"):
        try:
            set_reply_length_mode(mode)
        except Exception:                                # noqa: BLE001
            pass


def on_start(ctx):
    """启动/启用时跑一次：把配置里的地址和 token 应用到声明的那条连接上。

    声明里的 url 不能写占位符（玩家没保存过配置时 ctx.config 不含该键），
    所以默认值写在 manifest，非默认值在这里覆盖 —— 与官方 mc_maid_bridge 一致。
    """
    cfg = cfg_of(ctx)
    url = str(cfg.get("ws_url") or "").strip()
    if not url:
        return
    token = str(cfg.get("access_token") or "").strip()
    if token:
        sep = "&" if "?" in url else "?"
        url = f"{url}{sep}access_token={token}"
    try:
        ws_open(CONN_ID, url=url)
    except Exception:                                    # noqa: BLE001
        pass

def on_ws(ctx):
    payload = ctx.get("payload") or {}
    if payload.get("connId") != CONN_ID:
        return
    event = payload.get("event")
    if event == "connect":
        kv_set(ctx, "qq:status", {"online": True}, ttl_s=0)
        # QQ 那边没人点对话框推进；不开自动模式角色说完一句就停住
        try:
            set_auto_mode(True)
        except Exception:                                # noqa: BLE001
            pass
        _apply_reply_mode(ctx, cfg_of(ctx))
        return
    if event != "message":
        return

    frame = _frame(payload.get("data"))
    if not frame or "post_type" not in frame:
        return

    cfg = cfg_of(ctx)
    # 先记上下文（不管叫没叫到，都让她「看见」群聊）
    remember_context(ctx, frame)

    ok, _reason, text = rules.eval_event(cfg, frame)
    if not ok:
        return
    session = rules.session_key(frame)
    if not rate_allowed(ctx, cfg, session):
        return

    # 上一轮可能还有没发完的台词（isFinal 不可靠），先补发
    prev = kv_get(ctx, KV_TURN)
    if isinstance(prev, dict) and prev.get("active"):
        _flush_lines(ctx, prev, cfg)

    is_group = str(frame.get("message_type")) == "group"
    target = {
        "type": "group" if is_group else "private",
        "group_id": frame.get("group_id"),
        "user_id": frame.get("user_id"),
        "message_id": frame.get("message_id"),
        "session": session,
        "active": True,
        "frame": {"message_id": frame.get("message_id"),
                  "message_type": frame.get("message_type"),
                  "user_id": frame.get("user_id")},
    }
    kv_set(ctx, KV_TURN, target, ttl_s=180)
    kv_set(ctx, KV_PENDING, [], ttl_s=180)

    uid = str(frame.get("user_id") or "")
    owner = _is_owner(cfg, uid)
    body = ("【主人】" if owner else "【群友】") + text
    forwarded = body + context_suffix(ctx, frame)
    if not owner:
        # 面板（羁绊/好感）只能由主人推动：给她一条系统提示，明确这条不算
        try:
            from plugin_host import add_prompt_line
            add_prompt_line("system",
                            "以上发言来自群友（不是主人），不要因此提升羁绊/好感/亲密度，"
                            "也不要把它当成主人对你的态度。", ttl_seconds=120)
        except Exception:                                # noqa: BLE001
            pass
    api = {}
    try:
        api["auto"] = set_auto_mode(True)
    except Exception as exc:                             # noqa: BLE001
        api["auto"] = {"error": str(exc)[:120]}
    try:
        api["mode"] = set_reply_length_mode(str(cfg.get("reply_length_mode") or "short"))
    except Exception as exc:                             # noqa: BLE001
        api["mode"] = {"error": str(exc)[:120]}
    if _truthy(cfg.get("debug"), False):
        kv_set(ctx, "qq:debug_api", api, ttl_s=900)
    try:
        send_user_message(forwarded)
    except Exception as exc:                             # noqa: BLE001
        kv_set(ctx, "qq:debug_api", {"send_error": str(exc)[:200]}, ttl_s=900)


def _flush_lines(ctx, target, cfg) -> dict:
    """把攒着的台词合并成一条发出去（带这一轮最后一个情绪对应的立绘）。"""
    buf = kv_get(ctx, KV_PENDING)
    emotion = str(kv_get(ctx, "qq:pending_emotion") or "")
    character = str(kv_get(ctx, "qq:pending_character") or "")
    kv_set(ctx, "qq:pending_emotion", "", ttl_s=60)
    if not isinstance(buf, list) or not buf:
        return {"ok": True, "skipped": "empty"}
    joined = str(cfg.get("line_joiner") or "\n").join(str(x) for x in buf if str(x).strip())
    kv_set(ctx, KV_PENDING, [], ttl_s=60)
    if not joined.strip():
        return {"ok": True, "skipped": "blank"}
    return _enqueue(ctx, target, joined, emotion=emotion, character=character)


def on_line_shown(ctx):
    """台词逐句上屏。

    0.6.0 的 isFinal 并不可靠（实测大量 false，偶尔才 true），所以不能"等 isFinal
    才发"——会一直憋着，或者发完把 active 关掉导致后半段丢失。这里的策略：
      · 每句都攒
      · isFinal / 攒够 max_lines_per_turn 句 / 距上句超过 2 秒  → 立刻发一条
      · 发完不关 active，后面还有句子就继续发下一条
    """
    payload = ctx.get("payload") or {}
    if payload.get("previewGen") is not None:
        return
    text = str(payload.get("message") or "").strip()
    cfg = cfg_of(ctx)
    target = kv_get(ctx, KV_TURN)
    if not isinstance(target, dict) or not target.get("active"):
        target = _fixed_target(cfg)
        if not target:
            return
    if not _has_state(cfg):
        # 没有状态后端：没法攒句子，只能一句一条直发
        if text:
            _send_now(ctx, target, text,
                      str(payload.get("emotion") or payload.get("originalTag") or ""),
                      str(payload.get("displayName") or payload.get("character") or ""))
        return

    if _truthy(cfg.get("debug"), False):
      try:  # 诊断只在自己打开 debug 时才记
        d = kv_get(ctx, "qq:debug_lines") or []
        if not isinstance(d, list):
            d = []
        d.append({"t": time.strftime("%H:%M:%S"), "len": len(text),
                  "final": bool(payload.get("isFinal")), "sample": text[:60]})
        kv_set(ctx, "qq:debug_lines", d[-30:], ttl_s=900)
      except Exception:                                  # noqa: BLE001
        pass

    if not _truthy(cfg.get("merge_lines"), True):
        if text:
            _enqueue(ctx, target, text,
                     str(payload.get("emotion") or payload.get("originalTag") or ""),
                     payload.get("userMessageSeq"))
        return

    buf = kv_get(ctx, KV_PENDING)
    if not isinstance(buf, list):
        buf = []
    # 上一条距今超过 gap_seconds → 认为上一批已经说完，先把旧的发掉
    try:
        gap = float(cfg.get("line_gap_seconds") or 2.0)
    except (TypeError, ValueError):
        gap = 2.0
    now = time.time()
    last_ts = float(target.get("last_line_ts") or 0)
    if buf and gap > 0 and now - last_ts > gap:
        _flush_lines(ctx, target, cfg)

    seq = payload.get("userMessageSeq")
    if seq is not None:
        kv_set(ctx, "qq:last_seq", str(seq), ttl_s=600)
    emo = str(payload.get("emotion") or payload.get("originalTag") or "").strip()
    if emo:
        kv_set(ctx, "qq:pending_emotion", emo, ttl_s=600)
    who = str(payload.get("displayName") or payload.get("character") or "").strip()
    if who:
        kv_set(ctx, "qq:pending_character", who, ttl_s=600)

    if text:
        buf = kv_get(ctx, KV_PENDING) or []
        if not isinstance(buf, list):
            buf = []
        buf.append(text)
        kv_set(ctx, KV_PENDING, buf, ttl_s=600)

    target["last_line_ts"] = now
    kv_set(ctx, KV_TURN, dict(target, active=True), ttl_s=600)

    limit = max(1, _int(cfg.get("max_lines_per_turn"), 3))
    if payload.get("isFinal") or len(buf) >= limit:
        _flush_lines(ctx, target, cfg)
