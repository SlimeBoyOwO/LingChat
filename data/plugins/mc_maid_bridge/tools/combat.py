"""攻击工具：让她去打指定的那个活物，可以一次点好几只。"""

from bridge import ask, ready
from naming import names, narrate
from wording import MOB_NAMES, as_int, attack_style, attack_target, name_of

from tools.common import ACTION_TIMEOUT_MS

ATTACK_TOOL = "maid_attack"
# 一次最多点几只：再多她也顾不过来，模型多半也把同一只想成了一群
MAX_TARGETS = 8


def _attack(ctx):
    """让她去打几个活物。点谁由玩家那句话决定，插件只把说法翻成 id 或类别。"""
    maid, error = ready(ctx)
    if not maid:
        return {"ok": False, "error": error}

    maid_name, player = names()
    args = ctx.get("args") or {}
    raw = args.get("target")
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
    style = attack_style(args.get("style"))
    count = _count(args.get("count"))

    response, error = ask(
        {
            "type": ATTACK_TOOL,
            "data": {"maid_id": maid, "target": target, "style": style, "count": count},
        },
        ACTION_TIMEOUT_MS,
    )
    if error:
        return {"ok": False, "error": error}
    frame = response.get("data") or {}
    if response.get("type") == "error":
        return {"ok": False, "error": narrate(frame.get("message")) or "游戏那边没能下达攻击"}
    if not frame.get("accepted"):
        return {"ok": False, "error": _attack_error(frame, raw, maid_name, player)}
    return {"ok": True, "message": _attack_text(frame, maid_name, player)}


def _count(raw):
    """要打几只；给不出时按一只算，最多 MAX_TARGETS。"""
    return max(1, min(MAX_TARGETS, as_int(raw, 1)))


def _attack_error(frame, raw, maid, player):
    """模组拒绝时说什么：目标已经不在了，理由照原因码讲清楚。

    模组查过它记得的活物：是先被谁打死了，还是刚才还在、现在找不到了。照原因码说人话，
    认不出才退回模组给的那句。说死「不用再叫」，否则模型会换个说法一遍遍重试。
    """
    who = raw or "那个活物"
    code = (frame.get("reason_code") or "").strip()
    if code == "target_killed_by_maid":
        return f"{who}已经被{maid}自己打倒了，不用再叫。"
    if code == "target_killed_by_owner":
        return f"{who}已经被{player}打死了，不用再叫。"
    if code == "target_killed_by_other":
        return f"{who}已经被别的生物打死了，不用再叫。"
    if code == "target_dead":
        return f"{who}已经死了，不用再打。"
    if code == "target_gone":
        return f"{who}已经不在{maid}身边了：刚才还在附近，现在找不到。"
    return narrate(frame.get("reason")) or "游戏拒绝了这次攻击"


def _attack_text(frame, maid, player):
    """攻击回执 → 一句话。目标只说名字，不给格数。

    追打是常驻的：受理之后她自己会一直打到对方倒下，不需要模型再叫一次。
    回执必须把这件事说死——原先写「打了一下，还在战斗」，模型会读成「再去叫一次」，
    于是同一轮里把同一个目标反复叫上七八遍，每次都把追打重启一遍。

    重复调用同一个目标时模组只回执「已经在打」，这里也说清不用再叫。
    轻轻一下是另一回事：只出一手、伤害固定，打完就收手。
    """
    target = frame.get("target") or {}
    targets = frame.get("targets")
    targets = targets if isinstance(targets, list) and targets else [target]
    if frame.get("style") == "light":
        if len(targets) > 1:
            return f"{maid}抬手把附近的{len(targets)}个活物各轻轻拍了一下，打完就收手了。"
        return f"{maid}抬手轻轻拍了{_who(target, player)}一下，打完就收手了。"
    if frame.get("already_fighting"):
        return f"{maid}已经在追着{_who(target, player)}打了，不用再叫她；打到对方倒下她会自己收手。"
    if len(targets) > 1:
        text = f"{maid}提着家伙朝附近的{len(targets)}个活物冲了过去。"
        if as_int(frame.get("requested"), len(targets)) > len(targets):
            text += "附近只找到这么多。"
    else:
        text = f"{maid}提着家伙朝{_who(target, player)}冲了过去。"
    if not frame.get("weapon", True):
        text += f"{maid}手上没拿武器，只能空手打。"
    return text


def _who(target, player):
    """目标怎么称呼：主人用名字，别的玩家用游戏里的名字，其余用生物名。"""
    if target.get("owner"):
        return player
    if (target.get("category") or "") == "player":
        name = (target.get("name") or "").strip()
        return f"另一个玩家 {name}" if name else "另一个玩家"
    return name_of(MOB_NAMES, target.get("type"), "那个活物")


TOOLS = {
    ATTACK_TOOL: _attack,
}
