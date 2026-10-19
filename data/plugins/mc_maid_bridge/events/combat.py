"""战斗与威胁：女仆受伤、附近生灵死亡、敌对生物靠近。"""

from events.spec import EventSpec
from naming import names
from wording import DAMAGE_HINTS, MOB_NAMES, as_int, name_of, nearness_place, who_of

# 挨了多重只说档位，不报具体掉了几点血——数字一进上下文，模型就会照着念
# 「一共掉了 0.2 点血」。档位对着血量尺度定：一点以内是擦破皮，五点上下是实打实的一下。
# 措辞不预设「是被打的」：摔伤、烧死、淹死也走这一条，说成「被打得有点疼」就不对了。
HURT_PHRASES = ((1.0, "受了点轻伤"), (5.0, "伤得有点疼"), (10.0, "伤得不轻"))
HURT_WORST = "伤得很重"


def maid_hurt(data):
    """女仆受伤。模组会把一小段连续挨打合成一条，所以这里要照顾「几下」。

    只说伤得多重，不报掉了几点血、现在剩多少——那些数字模型会照抄进台词里。
    动手的是谁看两个标记：`attacker_owner` 是「她主人干的」（也就是玩家自己），
    `attacker_type` 是生物（说中文名），都没有才轮到伤害类型兜底。账号名一律不念。
    """
    maid, player = names()
    if data.get("attacker_owner"):
        cause = f"被{player}打了"
    elif data.get("attacker_type"):
        who = name_of(MOB_NAMES, data.get("attacker_type"), "什么东西")
        cause = f"被{who}打了"
    elif (data.get("attacker") or "").strip():
        cause = "被另一个人打了"
    else:
        cause = DAMAGE_HINTS.get((data.get("damage_type") or "").strip(), "挨了打")
    count = as_int(data.get("count"), 1)
    times = "连着挨了几下，" if count > 1 else ""
    return f"{maid}{cause}，{times}{_hurt_phrase(data.get('damage'))}。"


def creature_death(data):
    """附近死了个生灵：谁死了、怎么死的。

    因果是快照差分给不出的（它最多说「附近原来有村民，现在没有了」），只有事件说得清，
    所以这一条走 `LivingDeathEvent`。同类连着死时模组只报第一条，这里不必再判频率。
    """
    _, player = names()
    who = player if data.get("victim_owner") else who_of(data.get("category"), data.get("type"))
    return f"{nearness_place(data.get('distance'))}的{who}{_death_cause(data)}。"


def threat_near(data):
    """敌对生物靠近。只说是什么东西靠过来了。

    身边那一层只剩这一条还在写旁白：它是唯一「得当场叫醒她」的观察，其余几类都由
    开口时那份快照带过去。方位与距离都不说——模型会照着旁白复述「左前方有僵尸，
    离我不远」，而这些细节对玩家没用，她该知道的只是「有危险在靠近」。
    """
    maid, _ = names()
    who = name_of(MOB_NAMES, data.get("type"), "什么东西")
    if not data.get("visible"):
        # 看不见的只剩声音，而且是模组筛过的近处（默认 6 格）；不说「靠过来了」
        return f"听到{who}的声音，就在{maid}附近。"
    return f"有{who}正朝{maid}这边靠近。"


def _death_cause(data):
    """怎么死的：一句动词短语。谁动的手看标记与类型 id，都不认识就按伤害类型说。

    `attacker_maid` 是「她自己动的手」，`attacker_owner` 是「她主人（玩家）动的手」。
    这两个标记必须排在 `attacker` 前面：她自己与各种生物也带一个显示名，拿名字去认
    「别的玩家」会把它们全说成玩家（2026-10-17 真机：玩家杀了两只羊，旁白写成「被你
    杀了」，角色于是以为自己动了手，一整天都在困惑）。
    """
    maid, player = names()
    if data.get("attacker_maid"):
        return f"被{maid}杀了"
    if data.get("attacker_owner"):
        return f"被{player}杀了"
    if (data.get("attacker_type") or "").strip():
        who = name_of(MOB_NAMES, data.get("attacker_type"), "什么东西")
        return f"被{who}杀了"
    if (data.get("attacker") or "").strip():
        return "被另一个人杀了"
    return DAMAGE_HINTS.get((data.get("damage_type") or "").strip()) or "死了"


def _hurt_phrase(damage):
    """掉了多少血 → 「受了点轻伤」这类说法。数字给不出来时说「挨了一下」。"""
    try:
        lost = float(damage)
    except (TypeError, ValueError):
        return "挨了一下"
    for threshold, phrase in HURT_PHRASES:
        if lost <= threshold:
            return phrase
    return HURT_WORST


def _hurt_severity(data):
    """同键合并时比大小的严重度。受伤按掉血的百分之一计，取整免得浮点比较。"""
    try:
        return int(round(float(data.get("damage") or 0) * 100))
    except (TypeError, ValueError):
        return 0


SPECS = {
    "maid_hurt": EventSpec(
        maid_hurt,
        urgent=True,
        ttl=60,
        snapshot=True,
        merge_key="hurt",
        severity=_hurt_severity,
    ),
    "creature_death": EventSpec(
        creature_death,
        ttl=45,
        snapshot=True,
        urgent_when=lambda data: bool(data.get("victim_owner")),
    ),
    "threat_near": EventSpec(threat_near, urgent=True, ttl=60, snapshot=True),
}
