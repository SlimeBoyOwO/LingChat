"""她身边那一圈：按需快照，以及随发言 / 事件带过去的处境措辞。

工具「看一眼周围」与「玩家开口时顺手带一份」共用这一条路：帧、超时口径、老模组
认不出这个帧时的说法都只有一处。profile 决定模组用哪一档：auto 是开口时顺手带的
近处快照（看得见按工作范围、看不见只听很近的声音），query 是玩家明确问周围有什么
（看得见放宽到 32 格，并附方块与背包清单）。
"""

from bridge import ask
from naming import names, narrate
from wording import BLOCK_NAMES, heard_sounds, name_of, sighting

# 玩家说话时顺手取快照的超时：给得很短，玩家那句话不能被一次查询拖住（游戏暂停时
# 模组答不上来，这一轮就不带，照常把话送进对话）。
CARRY_TIMEOUT_MS = 1500

# 玩家离她这么近才算「在身边」。只判距离，不问「她看不看得见他」——视线那条会让模型
# 去编「我看见你了」，而真正要紧的只是主人有没有走远。
OWNER_NEAR = 30.0

# 「某角色此刻：附近有…」里最多念几只活物。模组两档都只给最近 3 个，这里再兜一层。
CARRY_CREATURE_LIMIT = 3

LOOK_AROUND_TOOL = "maid_look_around"


def snapshot(maid_id, timeout_ms, profile="auto"):
    """现取一份她身边的快照。返回 (数据, 错误说明)；出错时数据是 None。"""
    data = {"maid_id": maid_id, "profile": profile}
    response, error = ask({"type": LOOK_AROUND_TOOL, "data": data}, timeout_ms)
    if error:
        return None, error
    if response.get("type") == "error":
        # 老模组认不出这个帧，回的是统一错误帧（里面只有 message，没有业务字段）
        message = narrate((response.get("data") or {}).get("message"))
        return None, f"游戏那边没能回答：{message or '这个模组版本还不支持查看周围'}"
    return response.get("data") or {}, ""


def carry_text(data):
    """一份快照 → 一句「角色此刻的处境」，附在玩家的发言后面。

    只挑此刻要紧的说：脚下与头顶、玩家在不在身边、附近有什么活物。正前方那一眼留给
    工具那句——玩家问起来才需要那么细，每句话都带上就成了噪音。
    帧里没有 owner 字段说明这压根不是快照（多半是别的东西回了信），那就什么都不带。
    """
    if "owner" not in data:
        return ""
    maid, player = names()
    parts = []
    foot = _footing_phrase(data)
    if foot:
        parts.append(foot)
    parts.append(owner_phrase(data.get("owner") or {}, maid, player))
    creatures = creature_phrase(data)
    if creatures:
        parts.append(creatures)
    return f"（{maid}此刻：" + "；".join(parts) + "）"


def creature_phrase(data):
    """附近有哪些活物，一句话的中间那半截（「附近有近处的僵尸」）。

    出事那一刻（她挨打、血量告急）跟着事件旁白一起给她——怪物另记一条有 2 分钟防抖
    挡着，但「她刚挨了一下，旁边站着谁」是当下最要紧的处境，不该等到玩家下次说话。

    看得见的按距离档报；看不见的不报「被挡着」，只汇总成「听到僵尸和蜘蛛的声音」——
    隔墙知道具体是哪一只、离多远，读起来像透视，也不像她真有的感官。

    只看最近几只：按需查询现在能列十几只，全念出来会把玩家那句话挤成一份清单。
    """
    nearby = (data.get("nearby") or [])[:CARRY_CREATURE_LIMIT]
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


def _footing_phrase(data):
    """脚下踩着哪。「近/中/远」的档位在这句里没意义，所以单独拎出来。"""
    foot = name_of(BLOCK_NAMES, (data.get("standing_on") or {}).get("id"), "")
    if not foot:
        return ""
    # 下界与末地没有天空，「头顶有没有遮挡」在那里无从谈起
    roof = ""
    if data.get("has_sky"):
        roof = "，头顶有遮挡" if data.get("sheltered") else "，头顶是露天的"
    return f"脚下是{foot}{roof}"


def owner_phrase(owner, maid=None, player=None):
    """主人在不在她身边（`OWNER_NEAR` 之内），不带句号。

    只说在不在，不给格数，也不问视线：两条路（工具与开口时携带）共用这一句，
    说法只有一处。调用方已经取过名字时传进来，省一次宿主往返。
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
