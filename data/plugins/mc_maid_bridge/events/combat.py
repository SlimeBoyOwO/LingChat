"""战斗与威胁：女仆受伤、女仆自己死亡、附近生灵死亡、敌对生物靠近。"""

from events.spec import EventSpec
from naming import names
from wording import (
    DAMAGE_HINTS,
    DEAD_STATE_MARK,
    MOB_NAMES,
    RECOVERED_STATE_MARK,
    REVIVE_STATE_MARK,
    UNCONSCIOUS_STATE_MARK,
    as_int,
    name_of,
    nearness_place,
    who_of,
)

# 挨了多重只说档位，不报具体掉了几点血——数字一进上下文，模型就会照着念
# 「一共掉了 0.2 点血」。档位对着血量尺度定：一点以内是擦破皮，五点上下是实打实的一下。
# 措辞不预设「是被打的」：摔伤、烧死、淹死也走这一条，说成「被打得有点疼」就不对了。
HURT_PHRASES = ((1.0, "受了点轻伤"), (5.0, "伤得有点疼"), (10.0, "伤得不轻"))
HURT_WORST = "伤得很重"

# 挨完这一波之后的身体状态，按剩余血量比例切档。只给状态不给数字：数字一进上下文，
# 模型就会照着念「还剩 12 点血」。档位与模组的 lowHealthRatio（0.30）对得上。
HURT_STATES = ((0.0, "被这一下打死了"), (0.35, "已经奄奄一息了"), (0.7, "身体有点扛不住了"))
HURT_OK = "不过仍然感觉良好"


def maid_hurt(data):
    """女仆受伤。模组会把一小段连续挨打合成一条，所以这里要照顾「几下」。

    只说伤得多重、现在身体怎么样，不报掉了多少血、还剩多少——那些数字模型会照抄进
    台词里。`health` 是这一波最后一下之后的血量，拿它与上限算比例来说状态。
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
    body = f"{maid}{cause}，{times}{_hurt_phrase(data.get('damage'))}"
    state = _health_phrase(data)
    if state:
        body = f"{body}，{state}"
    return f"{body}。"


def creature_death(data):
    """附近死了个生灵：谁死了、怎么死的。

    因果是快照差分给不出的（它最多说「附近原来有村民，现在没有了」），只有事件说得清，
    所以这一条走 `LivingDeathEvent`。同类连着死时模组只报第一条，这里不必再判频率。
    """
    _, player = names()
    who = player if data.get("victim_owner") else who_of(data.get("category"), data.get("type"))
    return f"{nearness_place(data.get('distance'))}的{who}{_death_cause(data)}。"


def owner_death(data):
    """主人死了。她自己没死，会跟着复活的主人回到复活点。

    这条不看距离与维度：她可能被留在家里，但主人复活后她还是会跟过去，模型得知道
    自己现在该往哪儿去。居家模式的她哪也不去，那不跟过去这一点要说清楚。
    """
    maid, player = names()
    cause = _death_cause(data)
    if data.get("home_mode"):
        return (
            f"{player}{cause}。"
            f"{player}会在复活点复活，{maid}留在原地（居家模式），不会跟过去。"
        )
    return (
        f"{player}{cause}。"
        f"{player}会在复活点复活，{maid}也会跟着回到{player}身边。"
    )


def maid_unconscious(data):
    """她伤重昏过去了：不是死，但叫不醒；玩家要把她放到床上等一分钟。

    和死亡分开说：死亡是灵魂状态、要靠神龛与胶片；昏迷还有实体，只是动不了。
    复活办法写在这里，模型才知道该怎么回答玩家。
    """
    maid, _ = names()
    cause = _down_cause(data)
    if data.get("on_bed"):
        return (
            f"{maid}{cause}，昏倒在床上，只剩一口气，身边开始冒出爱心。"
            f"在床上连续躺满一分钟，{maid}就会醒过来。"
            f"{maid}{UNCONSCIOUS_STATE_MARK}。"
        )
    return (
        f"{maid}{cause}，倒在地上昏了过去，只剩一口气。"
        f"把{maid}抱到床上（普通床或女仆床）连续躺一分钟，{maid}就会醒过来。"
        f"{maid}{UNCONSCIOUS_STATE_MARK}。"
    )


def maid_recovering(data):
    """她被放到床上，开始计时恢复。"""
    maid, _ = names()
    return f"{maid}躺到了床上，呼吸慢慢平稳下来，身边冒起了爱心。"


def maid_recovered(data):
    """一分钟到了，她醒过来。醒来只有一点血，身体还很虚。"""
    maid, _ = names()
    return (
        f"{maid}在床上睁开了眼睛，醒了过来，但只剩一点力气。"
        f"{maid}{RECOVERED_STATE_MARK}。"
    )


def maid_death(data):
    """她自己死了：怎么死的、遗物在哪儿、玩家要怎么把她换回来。

    她倒下之后模组查不到她，所有指令都只会回「找不到女仆」——这一条是插件唯一知道
    发生了什么的地方，所以插件会当场请角色开口。旁白里带上死亡状态标记：之后每一轮
    都靠它判断她还在不在死亡状态（见 hooks._remind_state）。

    复活办法也写在这里：玩家要知道该去哪儿、拿什么换。墓碑里有她的遗物和胶片，
    胶片放进神龛后空手点神龛，会拿玩家的命换她回来（生命值不到一半时复活不了）。
    """
    maid, player = names()
    return (
        f"{maid}{_death_cause(data)}。"
        f"她的遗物和胶片留在墓碑里；把胶片放进神龛，再空手点神龛，"
        f"{player}会掉到只剩一口气，{maid}就能在神龛旁复活"
        f"（{player}的生命值不到一半时复活不了）。"
        f"{maid}{DEAD_STATE_MARK}，在复活之前没法行动，进入灵魂状态。"
    )


def maid_revive(data):
    """她被胶片复活了。死亡是持久状态，这一条把它收回来。"""
    maid, _ = names()
    return f"{maid}{REVIVE_STATE_MARK}。"


def threat_near(data):
    """敌对生物靠近。只说是什么东西靠过来了。

    身边这一层里最主要的威胁观察；脚下与头顶另有变化事件（踩进岩浆或火里也会当场
    提醒），其余几类由开口时那份快照带过去。方位与距离都不说——模型会照着旁白复述
    「左前方有僵尸，离我不远」，而这些细节对玩家没用，她该知道的只是「有危险在靠近」。
    """
    maid, _ = names()
    who = name_of(MOB_NAMES, data.get("type"), "什么东西")
    if not data.get("visible"):
        # 看不见的只剩声音，而且是模组筛过的近处（默认 6 格）；不说「靠过来了」
        return f"听到{who}的声音，就在{maid}附近。"
    return f"有{who}正朝{maid}这边靠近。"


def _down_cause(data):
    """怎么倒下的：一句动词短语，比死亡那句轻一档。

    摔伤、烧到、饿晕也走这一条，所以不预设「被打倒」；有人动手时才点名。
    """
    maid, player = names()
    if data.get("attacker_maid"):
        return "被自己弄伤了"
    if data.get("attacker_owner"):
        return f"被{player}打倒了"
    if (data.get("attacker_type") or "").strip():
        who = name_of(MOB_NAMES, data.get("attacker_type"), "什么东西")
        return f"被{who}打倒了"
    if (data.get("attacker") or "").strip():
        return "被另一个人打倒了"
    return DAMAGE_HINTS.get((data.get("damage_type") or "").strip()) or "伤得太重"


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


def _health_phrase(data):
    """挨完这一波还剩多少 → 「不过仍然感觉良好」这类状态。比例算不出来时不给这一句。

    老版本模组、或者实体没有血量上限时这两个字段缺一个，旁白就只保留伤势那句。
    """
    try:
        health = float(data.get("health"))
        max_health = float(data.get("max_health"))
    except (TypeError, ValueError):
        return ""
    if max_health <= 0:
        return ""
    ratio = health / max_health
    for threshold, phrase in HURT_STATES:
        if ratio <= threshold:
            return phrase
    return HURT_OK


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
    # 昏迷不是死亡：它同样要当场开口，并且一直留到玩家下一次说话。床上开始计时与
    # 醒来各有一条，避免模型把「昏迷」误当成「已经死了」。
    "maid_unconscious": EventSpec(maid_unconscious, urgent=True, ttl=3600),
    "maid_recovering": EventSpec(maid_recovering, ttl=120),
    "maid_recovered": EventSpec(maid_recovered, urgent=True, ttl=3600),
    # 她自己的死没有可合并的同键，也不该被静默：当场开口。死亡是持久状态，复活办法
    # 也在这条里——主动开口被拒时，这条得一直等玩家下一次说话，别几分钟就过期了。
    "maid_death": EventSpec(maid_death, urgent=True, ttl=3600),
    # 主人死了同样当场开口：她得知道接下来要跟着主人回复活点
    "owner_death": EventSpec(owner_death, urgent=True, ttl=120),
    # 复活同样重要：她得知道自己又能动了，才不会继续按「已经死了」说话
    "maid_revive": EventSpec(maid_revive, urgent=True, ttl=120),
    "threat_near": EventSpec(threat_near, urgent=True, ttl=60, snapshot=True),
}
