"""放置工具：把背包里的一块方块放到主人指着的那一面，或一个指定格子。"""

from bridge import ask, ready
from naming import names, narrate
from wording import BLOCK_NAMES, mine_target, name_of

from tools.common import ACTION_TIMEOUT_MS

PLACE_TOOL = "maid_place"

# 模型可能写中文方位或英文 token，统一成模组认识的四个词
FACINGS = {
    "north": "north",
    "北": "north",
    "北边": "north",
    "south": "south",
    "南": "south",
    "南边": "south",
    "east": "east",
    "东": "east",
    "东边": "east",
    "west": "west",
    "西": "west",
    "西边": "west",
}


def _place(ctx):
    """让她放下一块方块：主人指着的那一面，或者一个相对地址。"""
    maid, error = ready(ctx)
    if not maid:
        return {"ok": False, "error": error}

    args = ctx.get("args") or {}
    raw = args.get("item")
    raw = raw.strip() if isinstance(raw, str) else ""
    at = args.get("at")
    at = at.strip() if isinstance(at, str) else ""
    facing_raw = args.get("facing")
    facing_raw = facing_raw.strip().lower() if isinstance(facing_raw, str) else ""
    block = mine_target(raw)
    if not block or block == "looked":
        return {
            "ok": False,
            "error": f"没听懂要放什么方块：{raw or '（空）'}。可以说石头、木板、玻璃、台阶这些。",
        }

    request = {"maid_id": maid, "item": block}
    if at:
        request["at"] = at
    facing = FACINGS.get(facing_raw, "")
    if facing:
        request["facing"] = facing
    response, error = ask({"type": PLACE_TOOL, "data": request}, ACTION_TIMEOUT_MS)
    if error:
        return {"ok": False, "error": error}
    frame = response.get("data") or {}
    if response.get("type") == "error":
        message = narrate(frame.get("message"))
        if "不支持" in (message or ""):
            message = "这个模组版本还不支持放置方块"
        return {"ok": False, "error": message or "游戏那边没能开始放"}
    if not frame.get("accepted"):
        return {"ok": False, "error": _failure_text(frame, block)}
    maid_name, _ = names()
    name = name_of(BLOCK_NAMES, block, "方块")
    return {"ok": True, "message": f"{maid_name}拿着一块{name}，往那一格去了。"}


def _failure_text(frame, block):
    """失败回执：模组的原因已经够具体，这里补一句材料与那一格的情况。"""
    maid, _ = names()
    code = (frame.get("reason_code") or "").strip()
    name = name_of(BLOCK_NAMES, block, "方块")
    scene = frame.get("scene")
    scene = scene if isinstance(scene, dict) else {}
    text = narrate(frame.get("reason")) or "游戏拒绝了这次放置"
    if code == "no_item":
        text = f"{maid}身上没有{name}，需要别人给"
    elif code == "not_replaceable":
        actual = (scene.get("id") or "").strip()
        if actual:
            text = f"那一格被{name_of(BLOCK_NAMES, actual, '别的东西')}占着，放不下{name}"
    elif code == "no_support":
        text = f"那一格悬空，{name}没有能贴着的面"
    elif code == "occupied":
        text = "那一格里有活物，放下去会压到"
    elif code == "no_sight":
        text = f"{maid}看不太清那一格，中间隔着东西"
    text = text.rstrip("。")
    at = (frame.get("at") or "").strip()
    if at and code != "bad_address":
        text += f"（那一格是 {at}）"
    return text + "。"


TOOLS = {
    PLACE_TOOL: _place,
}
