"""六个工具的入口（manifest 的 `[[tools]]` 指向本文件）。

`run(ctx)` 返回的文本就是模型看到的结果，所以说实话最要紧：没连上、没绑定女仆、模组说
办不到，一律如实回报，绝不假装已经执行——模型会照着这个结果跟玩家交代。

工具分两类：
- 动作（走出去、坐下、站起、躺下、看过来）：把意图翻成一条 `maid_action` 帧，受理与否
  当场给答复；走过去这类耗时的，结果稍后由 `action_done` 事件旁白交代。
- 查询（看一眼周围）：现取一份快照翻成几句事实，只回给模型看，不写台词表。
"""

from bridge import ask, ready
from wording import (
    BLOCK_NAMES,
    DIMENSION_NAMES,
    MOB_NAMES,
    as_int,
    name_of,
    number,
    sighting,
)

# 动作的回执要等模组受理，超时明显小于工具自身的 15s，否则模型在工具超时里干等。
ACTION_TIMEOUT_MS = 8000
# 看一眼周围等快照的超时。单人游戏按 Esc 暂停时它必然超时，这时要如实说。
LOOK_TIMEOUT_MS = 8000

LOOK_AROUND_TOOL = "maid_look_around"

# 带目标的动作与各自的默认目标。执行器只校验 JSON Schema 的一个子集、不认 enum，
# 所以合法值在这里兜底（模组那头还会再校验一次）。
TARGET_DEFAULTS = {"go_to": "player", "lie_down": "bed"}

# 受理了但还没做完时给模型的话：必须让它知道自己还没到位，否则它会当场宣布已经办到。
PENDING_TEXT = {
    "go_to": "正在走过去，到了会告诉你。",
    "lie_down": "正在走过去，躺下了会告诉你。",
}

# 受理即完成的动作给模型的话。
DONE_TEXT = {
    "sit": "已经坐下了。",
    "stand": "已经站起来了。",
    "look_at": "已经看向玩家了。",
}


def run(ctx):
    """工具入口。认不出的工具如实拒绝，不猜。"""
    tool = (ctx.get("tool_name") or "").strip()
    handler = TOOLS.get(tool)
    if handler is None:
        return {"ok": False, "error": f"这个工具不存在：{tool}"}
    return handler(ctx)


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
        return {"ok": False, "error": frame.get("reason") or "游戏拒绝了这次动作"}
    if frame.get("async"):
        return {"ok": True, "message": PENDING_TEXT.get(action, "正在做，好了会告诉你。")}
    return {"ok": True, "message": DONE_TEXT.get(action, "已经做完了。")}


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


def _look_around(ctx):
    """看一眼她周围：取一份快照翻成几句事实给模型。

    只回去给模型看，不写台词表、也不请谁开口——玩家正在跟她说话，这一轮本来就要回话，
    再写一条旁白就是重复。
    """
    maid, error = ready(ctx)
    if not maid:
        return {"ok": False, "error": error}

    response, error = ask(
        {"type": LOOK_AROUND_TOOL, "data": {"maid_id": maid}}, LOOK_TIMEOUT_MS
    )
    if error:
        return {"ok": False, "error": error}
    if response.get("type") == "error":
        # 老模组认不出这个帧，回的是统一错误帧（里面只有 message，没有业务字段）
        message = ((response.get("data") or {}).get("message") or "").strip()
        return {"ok": False, "error": f"游戏那边没能回答：{message or '这个模组版本还不支持查看周围'}"}
    return {"ok": True, "message": _look_around_text(response.get("data") or {})}


def _look_around_text(data):
    """她身边这一圈，说成四句事实：站位 → 正前方 → 玩家 → 附近的活物。"""
    dimension = name_of(DIMENSION_NAMES, data.get("dimension"), "某个世界")
    foot = name_of(BLOCK_NAMES, (data.get("standing_on") or {}).get("id"), "说不清的东西")
    roof = "头顶有遮挡，看不到天" if data.get("sheltered") else "头顶是露天的"
    lines = [f"你正站在{dimension}，脚下是{foot}，{roof}，光照 {as_int(data.get('light'), 0)}。"]

    look = data.get("looking_at") or {}
    look_kind = (look.get("kind") or "").strip()
    if look_kind == "block":
        block = name_of(BLOCK_NAMES, look.get("id"), "一个方块")
        lines.append(f"正前方 {number(look.get('distance'))} 格处是{block}。")
    elif look_kind == "entity":
        entity = name_of(MOB_NAMES, look.get("id"), "一个活物")
        lines.append(f"正前方 {number(look.get('distance'))} 格处是{entity}。")
    else:
        lines.append("正前方没有挡着的东西，视线是通的。")

    owner = data.get("owner") or {}
    if not owner.get("same_dimension"):
        lines.append("玩家不在你所在的这个世界里。")
    elif owner.get("visible"):
        lines.append(f"玩家就在你 {number(owner.get('distance'))} 格以内，你看得见他。")
    else:
        lines.append(f"玩家离你大约 {number(owner.get('distance'))} 格，但被什么挡着，你看不见他。")

    nearby = data.get("nearby") or []
    if not nearby:
        lines.append("附近没有别的活物。")
    else:
        described = "、".join(sighting(one) for one in nearby)
        lines.append(f"附近有 {len(nearby)} 个活物：{described}。")
        if data.get("truncated"):
            lines.append("更远的没列出来。")
    return "".join(lines)


TOOLS = {
    "maid_go_to": action_tool("go_to"),
    "maid_sit_down": action_tool("sit"),
    "maid_stand_up": action_tool("stand"),
    "maid_lie_down": action_tool("lie_down"),
    "maid_look_at": action_tool("look_at"),
    LOOK_AROUND_TOOL: _look_around,
}
