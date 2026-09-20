"""光影工坊：把「现在是什么气氛」翻译成舞台灯光。

控光能力全在宿主的三个内置工具里（插件沙箱只能走 ctx["call_tool"]，
碰不到渲染层）：lighting_list_presets / lighting_apply / lighting_get。

本插件只加语义层：
- id、中文名与各语言的显示名互相容错，界面切成英文也不用记住 warm_window 这种串；
- 拿一句情绪描述去和预设的心情关键词对分，挑最贴合的那个。
"""

LIST_TOOL = "lighting_list_presets"
APPLY_TOOL = "lighting_apply"
GET_TOOL = "lighting_get"

# 命中这些词就按「不要临时灯光」处理。放在真实预设名之后判断，
# 所以词表可以宽松些——即便用户说的里面夹了「关」「无」这样的字，
# 只要对得上某个预设名，仍然优先当成换灯。
CLEAR_WORDS = (
    "跟随场景",
    "跟随",
    "取消",
    "清除",
    "关掉",
    "关闭",
    "恢复",
    "不要灯光",
    "clear",
    "follow_scene",
    "none",
    "off",
)

# 分数低于这个就不套，宁可不换也别乱换
MIN_SCORE = 3


def run(ctx):
    """入口：按 ctx["tool_name"] 分派。"""
    tool = ctx["tool_name"]
    call_tool = ctx["call_tool"]
    args = ctx["args"] or {}

    if tool == "lighting_studio_list":
        return _list(call_tool)
    if tool == "lighting_studio_apply":
        return _apply_by_name(call_tool, str(args.get("preset") or ""))
    if tool == "lighting_studio_mood":
        return _apply_by_mood(call_tool, str(args.get("mood") or ""))
    return {"ok": False, "error": {"code": "unknown_tool", "message": "未知工具 " + str(tool)}}


# ---------- 宿主工具包装 ----------


def _presets(call_tool):
    """预设清单；取不到时返回空列表，由调用方给出口而非崩在断言上。"""
    try:
        data = call_tool(LIST_TOOL, {})
    except Exception as exc:  # noqa: BLE001 - call_tool 失败抛 ValueError
        return {"error": str(exc)}
    if not isinstance(data, list):
        return {"error": "预设列表返回格式异常"}
    return {"presets": data}


def _current(call_tool):
    try:
        data = call_tool(GET_TOOL, {})
    except Exception:  # noqa: BLE001
        return None
    return data if isinstance(data, dict) else None


SOURCE_NAMES = {
    "scene": "场景自带灯光",
    "global": "用户在设置面板手动选的预设",
    "panel": "用户在设置面板手动选的预设",
    "script": "剧本事件",
    "tool": "插件或助手",
    "off": "光影总开关已关闭",
}


def _active(call_tool, presets):
    """把「屏幕上此刻在打什么灯」说成一句中文。

    这里只认 active_*，不认 override_*：override 只是后端记的覆盖，看不到用户
    手动选的全局预设。拿 override 判断就会发生「用户明明切成了冷月夜，助手却说
    现在是暖窗光、不用改」这类事。
    """
    current = _current(call_tool)
    if not current:
        return None
    source = str(current.get("active_source") or "")
    preset_id = current.get("active_preset")
    if preset_id:
        name = next(
            (p.get("name") for p in presets if p.get("id") == preset_id),
            str(preset_id),
        )
        summary = "当前画面正在打「" + str(name) + "」"
    elif source == "off":
        name = None
        summary = "光影总开关关着，画面没有打光"
    else:
        name = None
        summary = "当前没有套用任何预设，画面用的是" + SOURCE_NAMES.get(source, "场景自带灯光")
    summary += "（来源：" + SOURCE_NAMES.get(source, source or "未知") + "）"
    return {"preset": preset_id, "name": name, "source": source, "summary": summary}


def _push(call_tool, payload):
    try:
        result = call_tool(APPLY_TOOL, payload)
    except Exception as exc:  # noqa: BLE001
        return {"ok": False, "error": {"code": "apply_failed", "message": str(exc)}}
    return result if isinstance(result, dict) else {"ok": True, "result": result}


# ---------- 匹配 ----------


def _names(preset):
    """一盏灯的所有写法：后端自带名 + 各语言界面上的显示名。

    宿主把 en / ja / zh-HK 的名字一起给过来，是因为用户看着英文下拉说的
    「Warm Window Light」，到插件这里仍然要落到 warm_window 上。
    """
    out = []
    for raw in [str(preset.get("name") or "")] + [str(n) for n in preset.get("names") or []]:
        name = raw.strip()
        if name and name not in out:
            out.append(name)
    return out


def _is_cjk(ch):
    o = ord(ch)
    # 汉字 + 假名：这些语言连写、没有空格，只能按二字组匹配
    return 0x3400 <= o <= 0x9FFF or 0xF900 <= o <= 0xFAFF or 0x3040 <= o <= 0x30FF


def _words(text):
    """切出按空格书写的单词（小写）；汉字与假名交给二字组。"""
    out, cur = [], []
    for ch in text:
        if ord(ch) < 128 and (ch.isalnum() or ch in "-'"):
            cur.append(ch)
        else:
            if cur:
                out.append("".join(cur).lower())
                cur = []
    if cur:
        out.append("".join(cur).lower())
    return out


def _cjk_bigrams(text):
    """相邻两个 CJK 字组成的词片，中间隔着别的文字就不拼接。"""
    out, prev = [], None
    for ch in text:
        cur = ch if _is_cjk(ch) else None
        if cur and prev:
            out.append(prev + cur)
        prev = cur
    return out


def _is_clear(text):
    t = (text or "").strip().lower()
    if not t:
        return False
    return any(word in t for word in CLEAR_WORDS)


def _mood_hits(query, preset):
    """query 里确实出现了哪些心情关键词——回显给用户看为什么选它。"""
    hits = []
    for token in preset.get("mood") or []:
        t = str(token).strip().lower()
        if t and t in query:
            hits.append(str(token))
    return hits


def _score(query, preset):
    """整名、整关键词命中给重分，单词或中文二字组落在名字/说明里给轻分。

    二字组只对中日韩这种连写文字用；英文必须按单词切，否则 "glare" 切出来的
    "la"、"ar" 会撞上描述里不相干的词，把别的灯顶上来。
    """
    score = len(_mood_hits(query, preset)) * 3
    names = _names(preset)
    hay = ("".join(names) + str(preset.get("description") or "")).lower()
    for name in names:
        if name.lower() in query:
            score += 3
    for word in _words(query):
        if len(word) >= 3 and word in hay:
            score += 1
    for gram in _cjk_bigrams(query):
        if gram in hay:
            score += 1
    return score


def _exact(presets, key):
    """id、中文名或任一语言的显示名对上就算，两个方向都认：
    「霓虹」→ 霓虹夜（用户只说半截），「关掉霓虹夜」→ 霓虹夜（夹了别的话）。
    多个预设都说得通时取更具体的那个，避免短名字抢长名字。
    """
    k = (key or "").strip().lower()
    if not k:
        return None
    best, best_len = None, 0
    for p in presets:
        for needle in [str(p.get("id") or "").lower()] + [n.lower() for n in _names(p)]:
            if not needle:
                continue
            if k == needle:
                return p
            if needle in k and len(needle) > best_len:
                best, best_len = p, len(needle)
    return best


def _fuzzy(presets, key):
    k = (key or "").strip().lower()
    if not k:
        return None
    best, best_score = None, 0
    for p in presets:
        s = _score(k, p)
        if s > best_score:
            best, best_score = p, s
    return best if best_score >= MIN_SCORE else None


def _summary(preset, extra=None):
    out = {
        "id": preset.get("id"),
        "name": preset.get("name"),
        "description": preset.get("description"),
    }
    if extra:
        out.update(extra)
    return out


def _not_found(presets, key):
    return {
        "ok": False,
        "error": {
            "code": "preset_not_found",
            "message": "没找到和「" + key + "」对得上的光影预设，灯光保持原样。可用："
            + "、".join(str(p.get("name")) for p in presets),
        },
    }


# ---------- 三个工具的实现 ----------


def _list(call_tool):
    loaded = _presets(call_tool)
    if "error" in loaded:
        return {"ok": False, "error": {"code": "preset_load_failed", "message": loaded["error"]}}
    presets = loaded["presets"]
    return {
        "ok": True,
        "active": _active(call_tool, presets),
        "presets": [
            {
                "id": p.get("id"),
                "name": p.get("name"),
                # 各语言界面上的写法：用户照屏幕说哪一种，助手就照那一种回
                "names": _names(p),
                "description": p.get("description"),
                "mood": p.get("mood") or [],
                # 用户在设置面板自建的：名字是他起的，匹配到时可以直接照说
                "custom": bool(p.get("custom")),
            }
            for p in presets
        ],
    }


def _apply_by_name(call_tool, key):
    loaded = _presets(call_tool)
    if "error" in loaded:
        return {"ok": False, "error": {"code": "preset_load_failed", "message": loaded["error"]}}
    presets = loaded["presets"]

    # 先认预设，再认「取消」——用户说「关掉霓虹夜」时他要的是霓虹夜
    preset = _exact(presets, key)
    if not preset and _is_clear(key):
        result = _push(call_tool, {"clear": True})
        if result.get("ok"):
            result["cleared"] = True
            result["message"] = "已取消临时灯光，回到跟随场景（设置页的光影总开关不受影响）"
        return result
    if not preset:
        preset = _fuzzy(presets, key)
    if not preset:
        return _not_found(presets, key)

    result = _push(call_tool, {"preset": preset.get("id")})
    if result.get("ok"):
        result["applied"] = _summary(preset)
        result["message"] = "已套用光影：" + str(preset.get("name"))
    return result


def _apply_by_mood(call_tool, mood):
    query = (mood or "").strip().lower()
    if not query:
        return {
            "ok": False,
            "error": {"code": "missing_mood", "message": "mood 不能为空，例如「雨夜有点难过」"},
        }

    loaded = _presets(call_tool)
    if "error" in loaded:
        return {"ok": False, "error": {"code": "preset_load_failed", "message": loaded["error"]}}
    presets = loaded["presets"]

    ranked = sorted(presets, key=lambda p: -_score(query, p))
    best = ranked[0] if ranked else None
    if not best or _score(query, best) < MIN_SCORE:
        return _not_found(presets, mood)

    result = _push(call_tool, {"preset": best.get("id")})
    if result.get("ok"):
        result["applied"] = _summary(best, {"reason_mood": _mood_hits(query, best)})
        result["message"] = "按「" + mood + "」选了光影：" + str(best.get("name"))
    return result
