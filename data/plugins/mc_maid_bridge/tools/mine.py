"""挖掘工具：挖玩家看着的那块、附近的某一种方块，或她看见过的某一格。"""

from bridge import ask, ready
from naming import names, narrate
from wording import (
    BLOCK_NAMES,
    DIRECTION_NAMES,
    as_int,
    blocker_phrase,
    mine_choices,
    mine_target,
    name_of,
)

from tools.common import ACTION_TIMEOUT_MS

MINE_TOOL = "maid_mine"


def _mine(ctx):
    """让她去挖方块：玩家看着的那块、附近的某一种方块，或者一个相对地址指的格子。"""
    maid, error = ready(ctx)
    if not maid:
        return {"ok": False, "error": error}

    args = ctx.get("args") or {}
    raw = args.get("target")
    raw = raw.strip() if isinstance(raw, str) else ""
    at = args.get("at")
    at = at.strip() if isinstance(at, str) else ""
    area = args.get("area")
    area = area.strip() if isinstance(area, str) else ""
    whole = args.get("whole") is True
    target = mine_target(raw)
    if at and target == "looked":
        # 地址已经指到具体一格：target 只用来核对种类，留空就是「那一格原本是什么」
        target = ""
    if not target and not at and not area:
        return {
            "ok": False,
            "error": f"没听懂要挖哪种方块：{raw or '（空）'}。可以说 {mine_choices()}。",
        }
    try:
        count = int(args.get("count"))
    except (TypeError, ValueError):
        count = 1
    count = max(1, min(64, count))

    request = {"maid_id": maid, "target": target, "count": count}
    if at:
        request["at"] = at
    if area:
        request["area"] = area
    elif whole:
        request["whole"] = True
    response, error = ask({"type": MINE_TOOL, "data": request}, ACTION_TIMEOUT_MS)
    if error:
        return {"ok": False, "error": error}
    frame = response.get("data") or {}
    if response.get("type") == "error":
        return {"ok": False, "error": narrate(frame.get("message")) or "游戏那边没能开始挖"}
    if not frame.get("accepted"):
        return {"ok": False, "error": _failure_text(frame, target)}
    # 老模组不认识 area / whole：它只挖了一块，别让模型以为自己已经在拆一整片
    if (area or whole) and not frame.get("area"):
        return {
            "ok": False,
            "error": "这个模组版本还不支持整片拆，只挖了那一块；先一块一块来吧。",
        }
    maid_name, player = names()
    return {"ok": True, "message": _mine_text(frame, maid_name, player)}


def _scene_where(scene):
    """失败与受理回执里的目标方位：左前方三步、低一层。"""
    facing = DIRECTION_NAMES.get((scene.get("facing") or "").strip(), "")
    dx = as_int(scene.get("dx"), 0)
    dz = as_int(scene.get("dz"), 0)
    dy = as_int(scene.get("dy"), 0)
    steps = max(0, int(round((dx * dx + dz * dz) ** 0.5)))
    if not facing and steps <= 0 and dy == 0:
        return ""
    parts = []
    if facing:
        parts.append(facing)
    if steps:
        parts.append(f"{steps}步")
    parts.append("同一层" if dy == 0 else (f"高{dy}层" if dy > 0 else f"低{-dy}层"))
    return "、".join(parts)


def _failure_text(frame, target=""):
    """失败原因后面补一句现场；指错格子时再把最近的同类方块（带地址）交代清楚。

    指错时要给的是「改计划」所需的料：那一格实际是什么、最近的同类在哪、地址是多少。
    只回一句「那里不是铁矿」，模型下一轮只会拿同一个地址再问一遍。
    """
    code = (frame.get("reason_code") or "").strip()
    scene = frame.get("scene")
    scene = scene if isinstance(scene, dict) else {}
    text = narrate(frame.get("reason")) or "游戏拒绝了这次挖掘"
    if code == "not_that_block":
        actual = (scene.get("id") or "").strip()
        want = (target or "").strip()
        want_name = name_of(BLOCK_NAMES, want, "") if want else ""
        if actual:
            text = f"那里其实是{name_of(BLOCK_NAMES, actual, '别的方块')}"
            text += f"，不是{want_name}" if want_name else ""
        else:
            text = "那里现在是空的" + (f"，没有{want_name}" if want_name else "")
        scene = {}
    elif code == "not_seen":
        maid, _ = names()
        text = f"{maid}没看见过那一格，先让{maid}看一眼周围再指"
    notes = []
    where = _scene_where(scene)
    if where:
        notes.append(f"那块在{where}")
    if scene.get("blocked_above"):
        notes.append("上方压着方块，站不直")
    if scene.get("stand_room") is False:
        notes.append("周围没有能站的地方")
    if notes:
        text = f"{text.rstrip('。')}；{'，'.join(notes)}。"
    nearest = frame.get("nearest")
    if isinstance(nearest, dict) and (nearest.get("id") or "").strip():
        text += _nearest_text(nearest)
    return text


def _nearest_text(one):
    """最近的同类方块：方位、挡路与它自己的地址，照着填 at 就能挖到。"""
    name = name_of(BLOCK_NAMES, one.get("id"), "同类方块")
    where = _scene_where(one)
    text = f"最近的{name}在{where}" if where else f"最近的{name}就在跟前"
    blocker = blocker_phrase(one)
    if blocker:
        text += f"，中间{blocker}"
    at = (one.get("at") or "").strip()
    if at:
        text += f"（maid_mine 的 at 填 {at}）"
    return f"；{text}。"


def _mine_text(frame, maid, player):
    """挖掘回执 → 一句话。这个工具只负责开始挖，不代表方块已经掉下来。

    挖穿、挖够、走不到都由模组之后推的 `maid_mine_done` 状态说明；在这之前，
    模型只能说她「正过去挖」。区域拆除（area）一次是一整片，不按单块的地址说。
    """
    block = frame.get("block") or ""
    if frame.get("area"):
        cells = as_int(frame.get("cells"), 0) or as_int(frame.get("count"), 1)
        if block:
            name = name_of(BLOCK_NAMES, block, "方块")
            text = f"{maid}扛起工具，开始拆这一片{name}（{cells}块）。"
        else:
            text = f"{maid}扛起工具，开始拆这一片（{cells}块）。"
        if frame.get("truncated"):
            text += "一次最多拆64块，先拆最近的这些。"
        return text
    fallback = (frame.get("name") or "").strip() or "方块"
    name = name_of(BLOCK_NAMES, block, fallback)
    if frame.get("addressed"):
        parts = [f"{maid}扛起工具，朝指的那块{name}去了。"]
    elif frame.get("looked"):
        parts = [f"{maid}扛起工具，往{player}看着的那块{name}去了。"]
    else:
        parts = [f"{maid}扛起工具，去挖{frame.get('count') or 1}块{name}。"]
    if not frame.get("has_tool", True):
        parts.append(f"{maid}手上没有趁手的工具，只能徒手慢慢抠。")
    parts.append(f"开始挖{name}中...")
    return "".join(parts)


TOOLS = {
    MINE_TOOL: _mine,
}
