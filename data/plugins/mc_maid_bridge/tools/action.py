"""动作工具：走到玩家/床边、坐下、站起、躺下。"""

from bridge import ask, ready
from naming import names, narrate

from tools.common import ACTION_TIMEOUT_MS

# 带目标的动作与各自的默认目标。执行器只校验 JSON Schema 的一个子集、不认 enum，
# 所以合法值在这里兜底（模组那头还会再校验一次）。
TARGET_DEFAULTS = {"go_to": "player", "lie_down": "bed"}

# 受理了但还没做完时给模型的话：必须让它知道自己还没到位，否则它会当场宣布已经办到。
# 只陈述此刻的状态，不承诺「之后会通知」——模型会照着这句对玩家说出口。
PENDING_TEXT = {
    "go_to": "{maid}正在走向{player}身边，还没到。",
    "lie_down": "{maid}正在往床边走，还没躺下。",
}

# 受理即完成的动作给模型的话。
DONE_TEXT = {
    "sit": "{maid}已经坐下了。",
    "stand": "{maid}已经站起来了。",
}


def action_tool(action):
    """把一个动作包成工具入口：工具名带插件前缀，动作名不带。"""

    def handler(ctx):
        return _do_action(ctx, action)

    return handler


def _do_action(ctx, action):
    maid, error = ready(ctx)
    if not maid:
        return {"ok": False, "error": error}

    data = {"maid_id": maid, "action": action}
    target = _action_target(action, ctx.get("args") or {})
    if target:
        data["target"] = target

    response, error = ask({"type": "maid_action", "data": data}, ACTION_TIMEOUT_MS)
    if error:
        return {"ok": False, "error": error}
    frame = response.get("data") or {}
    if not frame.get("accepted"):
        return {"ok": False, "error": narrate(frame.get("reason")) or "游戏拒绝了这次动作"}
    maid_name, player = names()
    if frame.get("async"):
        text = PENDING_TEXT.get(action, "{maid}已经开始，还没做完。")
        return {"ok": True, "message": text.format(maid=maid_name, player=player)}
    text = DONE_TEXT.get(action, "{maid}已经做完了。")
    return {"ok": True, "message": text.format(maid=maid_name, player=player)}


def _action_target(action, args):
    """这次动作要带的目标；不带目标的动作返回空串。

    认不出的取值一律退回默认：执行器的 schema 校验不认 enum，模型可能塞别的字符串。
    """
    default = TARGET_DEFAULTS.get(action)
    if not default:
        return ""
    raw = args.get("target")
    raw = raw.strip().lower() if isinstance(raw, str) else ""
    if raw not in ("player", "bed"):
        return default
    # 躺下只认床：player 这种取值对它没有意义
    if action == "lie_down" and raw != "bed":
        return default
    return raw


TOOLS = {
    "maid_go_to": action_tool("go_to"),
    "maid_sit_down": action_tool("sit"),
    "maid_stand_up": action_tool("stand"),
    "maid_lie_down": action_tool("lie_down"),
}
