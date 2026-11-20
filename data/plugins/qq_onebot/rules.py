# -*- coding: utf-8 -*-
"""群聊限制规则（仿 AstrBot 的那套判定顺序）。

纯函数：只吃「配置 + OneBot 事件」，返回要不要处理、为什么、以及清洗后的文本。
不碰网络、不碰宿主，方便单测。

判定顺序（先便宜后贵，任一条不过就丢）：
  1. post_type 必须是 message / message_sent
  2. 忽略自己发的（ignore_bot_self）
  3. 忽略 @全体（ignore_at_all）
  4. 会话白名单（enable_whitelist + whitelist，键形如 group:123 / private:456）
  5. 唤醒判定（群聊：@ 或前缀；私聊：按 friend_needs_prefix）
  6. 文本清洗 + 长度截断
"""
from __future__ import annotations

GROUP = "group"
PRIVATE = "private"


def _segments(msg) -> list:
    """OneBot 消息可能是字符串（CQ 码或纯文本）或段数组，统一成段数组。"""
    if isinstance(msg, list):
        return [s for s in msg if isinstance(s, dict)]
    return [{"type": "text", "data": {"text": str(msg or "")}}]


def _text_of(segs: list) -> str:
    return "".join(str(s.get("data", {}).get("text", "")) for s in segs
                   if s.get("type") == "text")


def _at_targets(segs: list) -> list[str]:
    return [str(s.get("data", {}).get("qq", "")) for s in segs if s.get("type") == "at"]


def session_key(frame: dict) -> str:
    if str(frame.get("message_type")) == GROUP:
        return f"group:{frame.get('group_id')}"
    return f"private:{frame.get('user_id')}"


def split_list(raw) -> list[str]:
    """配置里多值：支持列表，也支持逗号/换行/空格分隔的字符串。"""
    if isinstance(raw, list):
        items = [str(x) for x in raw]
    else:
        s = str(raw or "")
        for ch in (",", "\n", "，", ";", " "):
            s = s.replace(ch, "\n")
        items = s.split("\n")
    return [x.strip() for x in items if x and x.strip()]


def eval_event(cfg: dict, frame: dict) -> tuple[bool, str, str]:
    """返回 (要不要处理, 原因码, 送进对话的文本)。"""
    if not isinstance(frame, dict):
        return False, "bad_event", ""
    if str(frame.get("post_type") or "") not in ("message", "message_sent"):
        return False, "not_message", ""

    self_id = str(frame.get("self_id") or "")
    user_id = str(frame.get("user_id") or "")
    if _truthy(cfg.get("ignore_bot_self"), True) and self_id and user_id == self_id:
        return False, "self_message", ""

    segs = _segments(frame.get("message"))
    ats = _at_targets(segs)

    if _truthy(cfg.get("ignore_at_all"), True) and "all" in ats:
        return False, "at_all", ""

    # 会话白名单
    if _truthy(cfg.get("enable_whitelist"), False):
        allowed = set(split_list(cfg.get("whitelist")))
        key = session_key(frame)
        if allowed and key not in allowed:
            return False, f"not_in_whitelist({key})", ""

    is_group = str(frame.get("message_type")) == GROUP
    text = _text_of(segs).strip()

    # 唤醒判定
    at_me = bool(self_id) and self_id in ats
    prefixes = split_list(cfg.get("wake_prefix")) or ["/"]
    hit_prefix = ""
    for p in sorted(prefixes, key=len, reverse=True):
        if text.startswith(p):
            hit_prefix = p
            break

    if is_group:
        need_at = _truthy(cfg.get("at_required_in_group"), True)
        if need_at:
            if not (at_me or hit_prefix):
                return False, "wake_required", ""
        elif not (at_me or hit_prefix) and not _truthy(cfg.get("group_always_reply"), False):
            return False, "wake_required", ""
    else:
        if _truthy(cfg.get("friend_needs_prefix"), False) and not hit_prefix:
            return False, "friend_prefix_required", ""

    if hit_prefix:
        text = text[len(hit_prefix):].strip()
    if not text:
        return False, "empty_after_wake", ""

    # 长度截断（AstrBot 的 forward_threshold 类似思路，这里直接截）
    try:
        max_len = int(cfg.get("max_len") or 800)
    except (TypeError, ValueError):
        max_len = 800
    if max_len > 0 and len(text) > max_len:
        text = text[:max_len] + "…"

    # 交给模型时带上"谁在哪说的"，顺便支持 unique_session
    if _truthy(cfg.get("unique_session"), True):
        nickname = ""
        sender = frame.get("sender")
        if isinstance(sender, dict):
            nickname = str(sender.get("card") or sender.get("nickname") or "")
        where = (f"群{frame.get('group_id')}" if is_group else "私聊")
        who = f"{nickname}({user_id})" if nickname else user_id
        text = f"【QQ·{where}｜{who}】{text}"
    return True, "ok", text


def _truthy(v, default: bool = False) -> bool:
    if v is None:
        return default
    if isinstance(v, bool):
        return v
    return str(v).strip().lower() in ("1", "true", "yes", "on", "是")


def is_admin(cfg: dict, user_id) -> bool:
    return str(user_id) in set(split_list(cfg.get("admins")))

# ───────────────────────── 立绘表情映射 ─────────────────────────

# 插件里不能用 os（宿主禁了），所以这里把情绪→文件名做成静态表。
# 换角色/换素材时，跑一下 tools/gen_emotions.py 重新生成即可。
EMOTION_FILES = {
    "伤心": "伤心.webp",
    "兴奋": "兴奋.webp",
    "厌恶": "厌恶.webp",
    "头像": "头像.webp",
    "害怕": "害怕.webp",
    "害羞": "害羞.webp",
    "平静": "平静.webp",
    "心动": "心动.webp",
    "惊讶": "惊讶.webp",
    "慌张": "慌张.webp",
    "担心": "担心.webp",
    "无奈": "无奈.webp",
    "正常": "正常.webp",
    "生气": "生气.webp",
    "疑惑": "疑惑.webp",
    "紧张": "紧张.webp",
    "羞耻": "羞耻.webp",
    "自信": "自信.webp",
    "认真": "认真.webp",
    "调皮": "调皮.webp",
    "高兴": "高兴.webp",
}
EMOTION_FALLBACK_FILE = "正常.webp"


def emotion_file(cfg: dict, emotion, character="") -> str:
    """情绪 → 立绘的绝对路径（字符串拼接，不碰 os）。

    优先用当前角色自己的 avatar_upper；认不出角色才退回配置里的默认目录。
    """
    base = character_dir(character).strip().rstrip("/")
    if not base:
        base = str(cfg.get("emotion_image_dir") or "").strip().rstrip("/\\")
    if not base:
        return ""
    name = str(emotion or "").strip()
    fn = EMOTION_FILES.get(name) or EMOTION_FALLBACK_FILE
    return base + "/" + fn


CHARACTER_DIRS = {
    # 角色名 → 立绘目录（可留空：插件会退回配置里的 emotion_image_dir）
    # 用 tools/gen_characters.py 生成
}


def character_dir(character) -> str:
    """角色名 → 立绘目录；认不出来就返回空串（由调用方退回配置里的默认目录）。"""
    return CHARACTER_DIRS.get(str(character or "").strip(), "")
