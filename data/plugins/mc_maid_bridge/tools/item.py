"""交物品工具：把她背包里的东西交给玩家。"""

from bridge import ask, ready
from naming import names, narrate
from wording import as_int, item_choices, item_name, item_target

from tools.common import ACTION_TIMEOUT_MS

GIVE_TOOL = "maid_give_item"


def _give_item(ctx):
    """把她背包里的东西交给玩家：同步完成，成败当场给答复。"""
    maid, error = ready(ctx)
    if not maid:
        return {"ok": False, "error": error}

    maid_name, player = names()
    args = ctx.get("args") or {}
    raw = args.get("item")
    raw = raw.strip() if isinstance(raw, str) else ""
    target = item_target(raw)
    if not target:
        if not raw:
            return {
                "ok": False,
                "error": f"没说要交给{player}哪件东西。可以说 {item_choices()}。",
            }
        # 认不出的名字照样发给模组：失败回执会带背包清单，模型能按 id 再来一次
        target = raw
    try:
        count = int(args.get("count"))
    except (TypeError, ValueError):
        count = 1
    count = max(1, min(64, count))

    response, error = ask(
        {"type": GIVE_TOOL, "data": {"maid_id": maid, "item": target, "count": count}},
        ACTION_TIMEOUT_MS,
    )
    if error:
        return {"ok": False, "error": error}
    frame = response.get("data") or {}
    if response.get("type") == "error":
        return {
            "ok": False,
            "error": narrate(frame.get("message")) or f"游戏那边没能把东西交给{player}",
        }
    if not frame.get("accepted"):
        return {"ok": False, "error": _give_error(frame, maid_name)}
    return {"ok": True, "message": _give_text(frame, maid_name, player)}


def _give_text(frame, maid, player):
    """交物品的回执 → 一句话。只说她真的给了多少，不替玩家收下。"""
    name = item_name(frame.get("item"))
    given = as_int(frame.get("count"), 1)
    wanted = as_int(frame.get("requested"), given)
    if given < wanted:
        return f"{maid}身上只剩{given}个{name}，索性一股脑都塞给了{player}。"
    return f"{maid}把{given}个{name}递到了{player}手里。"


def _give_error(frame, maid):
    """失败回执 → 原因；模组带了背包清单时一并列出，模型下一句就能按 id 改口。"""
    reason = narrate(frame.get("reason")) or "游戏拒绝了这次交接"
    available = [one for one in (frame.get("available") or []) if isinstance(one, dict) and one.get("item")]
    if not available:
        return reason
    listed = "、".join(
        f"{item_name(one.get('item'))}（{one.get('item')}）×{as_int(one.get('count'), 1)}"
        for one in available
    )
    return f"{reason}。{maid}的背包里装着：{listed}。"


TOOLS = {
    GIVE_TOOL: _give_item,
}
