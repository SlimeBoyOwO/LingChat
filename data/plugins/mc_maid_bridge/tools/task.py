"""工作模式工具：切她接下来会自己去干什么活。"""

from bridge import ask, ready
from naming import names, narrate
from wording import task_choices, task_id, task_name

from tools.common import ACTION_TIMEOUT_MS

SET_TASK_TOOL = "maid_set_task"


def _set_task(ctx):
    """换她的工作模式。切的就是她自己界面里那个任务，我们这边不留副本。"""
    maid, error = ready(ctx)
    if not maid:
        return {"ok": False, "error": error}

    raw = (ctx.get("args") or {}).get("task")
    raw = raw.strip() if isinstance(raw, str) else ""
    task = task_id(raw)
    if not task:
        return {"ok": False, "error": f"没有这个工作模式：{raw or '（空）'}。能用的有：{task_choices()}。"}

    response, error = ask({"type": SET_TASK_TOOL, "data": {"maid_id": maid, "task": task}}, ACTION_TIMEOUT_MS)
    if error:
        return {"ok": False, "error": error}
    frame = response.get("data") or {}
    if response.get("type") == "error":
        # 模组没就绪，或者这个模组版本还不认这条帧
        return {"ok": False, "error": narrate(frame.get("message")) or "游戏那边没能换工作模式"}
    if not frame.get("accepted"):
        return {"ok": False, "error": narrate(frame.get("reason")) or "游戏拒绝了这次切换"}
    return {"ok": True, "message": _task_text(frame)}


def _task_text(frame):
    """换工作的回执 → 给模型的一句话：成没成、缺不缺东西、现在会不会真去干。

    后两件事模型自己看不见，而它们决定玩家会不会白等：她的日程表此刻可能不在工作时间；
    跟随状态下有活时，玩家走远了不会把她拽回来，活会接着干，叫一声就能回来。
    """
    maid, player = names()
    name = task_name(frame.get("task"), frame.get("summary"))
    if frame.get("changed"):
        parts = [f"{maid}把活计换成了「{name}」。"]
    else:
        parts = [f"{maid}本来就在干「{name}」。"]
    switch = (frame.get("switch_result") or "").strip()
    if switch == "MISSING_REQUIRED_ITEM":
        parts.append(f"只是{maid}手上和背包里都没有要用的家伙（锄头、剪刀、钓竿这类），得先给她。")
    elif switch == "PARTIAL_OK":
        parts.append("工具不全，只能干其中一部分。")
    if frame.get("work_time") is False:
        parts.append(f"现在正赶上{maid}的休息时段，到了工作时间才会动手。")
    elif frame.get("home_mode") is False:
        parts.append(
            f"不过{maid}还跟着{player}：干活的时候{player}走远了不会把{maid}拽回来，"
            "活会接着干，喊一声就回去。"
        )
    return "".join(parts)


TOOLS = {
    SET_TASK_TOOL: _set_task,
}
