"""头饰工具：把背包里的东西戴到她头上，或者摘下来。

她背包最后一格是头饰栏，那一格里的东西顶在头上显示——放一块方块就是顶着一块方块。
这一格不算背包存货：交东西、换工具都不会动它。
"""

from bridge import ask, ready
from naming import names, narrate
from wording import item_target, item_name

from tools.common import ACTION_TIMEOUT_MS

HEADWEAR_TOOL = "maid_headwear"

# 「摘下来」的说法。模型多半传 take_off / 空，中文直接说「摘下」也认。
TAKE_OFF = frozenset({"", "none", "off", "take_off", "takeoff", "removed", "摘下", "取下来", "拿下来"})


def _headwear(ctx):
    """给她戴上或摘下头饰。戴的东西必须已经在她背包里。"""
    maid, error = ready(ctx)
    if not maid:
        return {"ok": False, "error": error}

    maid_name, player = names()
    raw = (ctx.get("args") or {}).get("item")
    raw = raw.strip() if isinstance(raw, str) else ""
    if raw.lower() in TAKE_OFF:
        data = {"maid_id": maid, "action": "take_off"}
    else:
        data = {"maid_id": maid, "action": "put", "item": item_target(raw) or raw}

    response, error = ask({"type": HEADWEAR_TOOL, "data": data}, ACTION_TIMEOUT_MS)
    if error:
        return {"ok": False, "error": error}
    frame = response.get("data") or {}
    if response.get("type") == "error":
        return {"ok": False, "error": narrate(frame.get("message")) or "游戏那边没能改她的头饰"}
    if not frame.get("accepted"):
        return {"ok": False, "error": narrate(frame.get("reason")) or "游戏拒绝了这次改动"}
    name = item_name(frame.get("item"))
    if frame.get("action") == "take_off":
        return {"ok": True, "message": f"{maid_name}把头上的{name}摘下来了。"}
    return {"ok": True, "message": f"{maid_name}把{name}顶到头上当装饰了。"}


TOOLS = {
    HEADWEAR_TOOL: _headwear,
}
