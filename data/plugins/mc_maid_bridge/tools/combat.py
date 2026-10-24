"""攻击工具：让她去打指定的那个活物。"""

from bridge import ask, ready
from naming import names, narrate
from wording import MOB_NAMES, attack_style, attack_target, name_of

from tools.common import ACTION_TIMEOUT_MS

ATTACK_TOOL = "maid_attack"


def _attack(ctx):
    """让她去打一个活物。点谁由玩家那句话决定，插件只把说法翻成 id 或类别。"""
    maid, error = ready(ctx)
    if not maid:
        return {"ok": False, "error": error}

    maid_name, player = names()
    raw = (ctx.get("args") or {}).get("target")
    raw = raw.strip() if isinstance(raw, str) else ""
    target = attack_target(raw)
    if not target:
        return {
            "ok": False,
            "error": f"没听懂要打谁：{raw or '（空）'}。可以说活物的名字（僵尸、羊），"
            "另一个玩家的名字（照「看一眼周围」里的名字说），"
            "或者 hostile（最近的敌对生物）、animal（最近的动物）、other_player（另一个玩家）、"
            f"owner（主人自己，也就是{player}）。",
        }
    style = attack_style((ctx.get("args") or {}).get("style"))

    response, error = ask(
        {"type": ATTACK_TOOL, "data": {"maid_id": maid, "target": target, "style": style}}, ACTION_TIMEOUT_MS
    )
    if error:
        return {"ok": False, "error": error}
    frame = response.get("data") or {}
    if response.get("type") == "error":
        return {"ok": False, "error": narrate(frame.get("message")) or "游戏那边没能下达攻击"}
    if not frame.get("accepted"):
        return {"ok": False, "error": narrate(frame.get("reason")) or "游戏拒绝了这次攻击"}
    return {"ok": True, "message": _attack_text(frame, maid_name, player)}


def _attack_text(frame, maid, player):
    """攻击回执 → 一句话。目标只说名字，不给格数。

    追打是常驻的：受理之后她自己会一直打到对方倒下，不需要模型再叫一次。
    回执必须把这件事说死——原先写「打了一下，还在战斗」，模型会读成「再去叫一次」，
    于是同一轮里把同一个目标反复叫上七八遍，每次都把追打重启一遍。

    重复调用同一个目标时模组只回执「已经在打」，这里也说清不用再叫。
    轻轻一下是另一回事：只出一手、伤害固定，打完就收手。
    """
    target = frame.get("target") or {}
    if target.get("owner"):
        who = player
    else:
        fallback = (target.get("name") or "").strip() or "那个活物"
        who = name_of(MOB_NAMES, target.get("type"), fallback)
    if frame.get("style") == "light":
        return f"{maid}抬手轻轻拍了{who}一下，打完就收手了。"
    if frame.get("already_fighting"):
        return f"{maid}已经在追着{who}打了，不用再叫她；打到对方倒下她会自己收手。"
    text = f"{maid}提着家伙朝{who}冲了过去，这一仗她自己会打到对方倒下，过程中不用再叫她。"
    if not frame.get("weapon", True):
        text += f"{maid}手上没拿武器，只能空手打，落下去的力道很轻。"
    return text


TOOLS = {
    ATTACK_TOOL: _attack,
}
