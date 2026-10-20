"""挖掘工具：挖玩家看着的那块，或附近的某一种方块。"""

from bridge import ask, ready
from naming import names, narrate
from wording import BLOCK_NAMES, mine_choices, mine_target, name_of

from tools.common import ACTION_TIMEOUT_MS

MINE_TOOL = "maid_mine"


def _mine(ctx):
    """让她去挖方块：玩家看着的那块，或附近的某一种方块。"""
    maid, error = ready(ctx)
    if not maid:
        return {"ok": False, "error": error}

    args = ctx.get("args") or {}
    raw = args.get("target")
    raw = raw.strip() if isinstance(raw, str) else ""
    target = mine_target(raw)
    if not target:
        return {
            "ok": False,
            "error": f"没听懂要挖哪种方块：{raw or '（空）'}。可以说 {mine_choices()}。",
        }
    try:
        count = int(args.get("count"))
    except (TypeError, ValueError):
        count = 1
    count = max(1, min(64, count))

    response, error = ask(
        {"type": MINE_TOOL, "data": {"maid_id": maid, "target": target, "count": count}},
        ACTION_TIMEOUT_MS,
    )
    if error:
        return {"ok": False, "error": error}
    frame = response.get("data") or {}
    if response.get("type") == "error":
        return {"ok": False, "error": narrate(frame.get("message")) or "游戏那边没能开始挖"}
    if not frame.get("accepted"):
        return {"ok": False, "error": narrate(frame.get("reason")) or "游戏拒绝了这次挖掘"}
    maid_name, player = names()
    return {"ok": True, "message": _mine_text(frame, maid_name, player)}


def _mine_text(frame, maid, player):
    """挖掘回执 → 一句话。这个工具只负责开始挖，不代表方块已经掉下来。

    挖穿、挖够、走不到都由模组之后推的 `maid_mine_done` 状态说明；在这之前，
    模型只能说她「正过去挖」。
    """
    block = frame.get("block") or ""
    fallback = (frame.get("name") or "").strip() or "方块"
    name = name_of(BLOCK_NAMES, block, fallback)
    if frame.get("looked"):
        parts = [f"{maid}正过去挖{player}看着的那块{name}。"]
    else:
        parts = [f"{maid}正过去挖{frame.get('count') or 1}块{name}。"]
    if not frame.get("has_tool", True):
        parts.append(f"{maid}手上没有趁手的工具，只能徒手挖，会慢一些。")
    parts.append(f"挖掘才刚开始，{name}还留在原地。")
    return "".join(parts)


TOOLS = {
    MINE_TOOL: _mine,
}
