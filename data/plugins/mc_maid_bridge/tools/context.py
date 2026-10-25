"""她身边那一圈：按需快照，以及事件旁白里的处境措辞。

工具「看一眼周围」与「出事那一刻顺手补一份」共用这一条路：帧、超时口径、老模组
认不出这个帧时的说法都只有一处。profile 决定模组用哪一档：auto 是出事时顺手带的
近处快照（看得见按工作范围、看不见只听很近的声音），query 是玩家明确问周围有什么
（看得见放宽到 32 格，并附方块与背包清单）。

处境信息不往玩家那句话后面追加：玩家的发言原样送进对话，感知一律走 `events/` 的旁白入口。
"""

from bridge import ask
from naming import names, narrate
from wording import MOB_NAMES, heard_sounds, name_of, sighting, who_of

# 出事时顺手取快照的超时：给得很短，那条旁白不能被一次查询拖住（游戏暂停时
# 模组答不上来，这一轮就不带，旁白照常进队列）。
NEARBY_TIMEOUT_MS = 1500

# 玩家离她这么近才算「在身边」。与模组远近档里「走开了」那条线（16 格）对齐：
# 两边口径不一样的话，同一个距离上工具说「就在身边」、旁白却说「走远了」。
OWNER_NEAR = 16.0

# 旁白与工具回执里最多念几只活物。模组两档都只给最近 3 个，这里再兜一层。
CREATURE_LIMIT = 3

LOOK_AROUND_TOOL = "maid_look_around"


def snapshot(maid_id, timeout_ms, profile="auto", detail="brief"):
    """现取一份她身边的快照。返回 (数据, 错误说明)；出错时数据是 None。

    detail = map 时模组会多画一张近细远粗的小图；默认 brief，只有玩家问路才要。
    """
    data = {"maid_id": maid_id, "profile": profile, "detail": detail}
    response, error = ask({"type": LOOK_AROUND_TOOL, "data": data}, timeout_ms)
    if error:
        return None, error
    if response.get("type") == "error":
        # 老模组认不出这个帧，回的是统一错误帧（里面只有 message，没有业务字段）
        message = narrate((response.get("data") or {}).get("message"))
        return None, f"游戏那边没能回答：{message or '这个模组版本还不支持查看周围'}"
    return response.get("data") or {}, ""


def creature_phrase(data):
    """附近有哪些活物，一句话的中间那半截（「附近有近处的僵尸」）。

    出事那一刻（她挨打、血量告急）跟着事件旁白一起给她——怪物另记一条有 2 分钟防抖
    挡着，但「她刚挨了一下，旁边站着谁」是当下最要紧的处境，不该等到下次采样。

    看得见的按距离档报；看不见的不报「被挡着」，只汇总成「听到僵尸和蜘蛛的声音」——
    隔墙知道具体是哪一只、离多远，读起来像透视，也不像她真有的感官。

    只看最近几只：按需查询现在能列十几只，全念出来会把那句旁白挤成一份清单。
    """
    nearby = (data.get("nearby") or [])[:CREATURE_LIMIT]
    if not nearby:
        return ""
    seen = [one for one in nearby if one.get("visible")]
    sounds = heard_sounds([one for one in nearby if not one.get("visible")])
    parts = []
    if seen:
        parts.append(f"附近有{'、'.join(sighting(one) for one in seen)}")
    if sounds:
        parts.append(f"{'还能' if seen else '附近能'}听到{sounds}")
    return "；".join(parts)


def creature_text(data):
    """同上，但成句：没有活物就什么都说不出来（空串）。"""
    phrase = creature_phrase(data)
    return f"{phrase}。" if phrase else ""


def remembered_text(ones):
    """刚才在附近见过、现在不在了的活物（模组的两分钟记忆）。

    用来回答「刚才那只僵尸呢」：说清它已经不在了，以及死在谁手里。只当过去的事说，
    不并进此刻的视野——活着的那些本来就会出现在附近那份清单里。
    """
    maid, player = names()
    parts = []
    for one in ones:
        who = who_of(one.get("category"), one.get("type"))
        if not one.get("dead"):
            parts.append(f"{who}已经不在附近了")
            continue
        by = (one.get("killed_by") or "").strip()
        if by == "maid":
            parts.append(f"{who}已经被{maid}打倒了")
        elif by == "owner":
            parts.append(f"{who}已经被{player}打死了")
        elif by == "other":
            raw = (one.get("killed_by_type") or "").strip()
            killer = "另一个人" if raw == "minecraft:player" else name_of(MOB_NAMES, raw, "别的生物")
            parts.append(f"{who}已经被{killer}打死了")
        else:
            parts.append(f"{who}已经死了")
    return f"刚才在附近见过、现在不在了的：{'、'.join(parts)}。" if parts else ""


def owner_phrase(owner, maid=None, player=None):
    """主人在不在她身边（`OWNER_NEAR` 之内），不带句号。

    只说在不在，不给格数，也不问视线：工具回执共用这一句，说法只有一处。
    调用方已经取过名字时传进来，省一次宿主往返。
    """
    if maid is None or player is None:
        maid, player = names()
    if not owner.get("same_dimension"):
        return f"{player}不在{maid}所在的这个世界里"
    try:
        distance = float(owner.get("distance"))
    except (TypeError, ValueError):
        return f"{maid}身边没有{player}"
    if 0 <= distance <= OWNER_NEAR:
        return f"{player}就在{maid}身边"
    return f"{player}不在{maid}身边"
