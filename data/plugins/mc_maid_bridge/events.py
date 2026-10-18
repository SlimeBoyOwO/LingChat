"""模组推来的一条游戏事件 → 一句只陈述事实的中文。

只写发生了什么，不写「该有什么情绪」——情绪由模型自己决定，留在台词表里的始终是
客观记录。哪一条算紧急（要当场请角色开口）由 `URGENT` 定，事件落地在 hooks.py；
每条值多少秒由 `EVENT_TTL` 定，宿主在角色真要开口时才判，过期的不进台词表。

**身边那一圈刻意不在这里登记**：附近有什么活物、玩家进出视线、头顶遮没遮住、脚下踩到
什么，这五类（模组的 `player_proximity_change` / `player_sight_change` / `living_near` /
`shelter_change` / `ground_change`）不再翻成旁白。她此刻的处境由玩家开口那一刻随消息
带过去的一份实时快照交代（见 `tools.carry_text`），逐条记下来只会让台词表积满
「附近有村民」——2026-10-16 真机上一个晚上 34 条事件旁白里 31 条是这类。模组那边照推
不误，这里不认就等于丢掉，是本次改动有意为之的「不认识就不出声」（见下）。

加一类事件：写一个函数，在 `FORMATTERS` 里登记一行；紧急的话再进 `URGENT`。
每个函数只收这一条事件的数据；「是不是玩家干的」由模组给的标记回答，不必拿配置里的用户名去比。
认不出的 event_type 一律不出声——模组先加了新事件、插件还没跟上时，这是刻意的向后兼容。
"""

from wording import (
    BIOME_NAMES,
    BLOCK_NAMES,
    DAMAGE_HINTS,
    DIMENSION_NAMES,
    MOB_NAMES,
    advancement_name,
    as_int,
    item_name,
    name_of,
    nearness_place,
    task_name,
    who_of,
)

# 挨了多重只说档位，不报具体掉了几点血——数字一进上下文，模型就会照着念
# 「一共掉了 0.2 点血」。档位对着血量尺度定：一点以内是擦破皮，五点上下是实打实的一下。
# 措辞不预设「是被打的」：摔伤、烧死、淹死也走这一条，说成「被打得有点疼」就不对了。
HURT_PHRASES = ((1.0, "受了点轻伤"), (5.0, "伤得有点疼"), (10.0, "伤得不轻"))
HURT_WORST = "伤得很重"

# 受理即完成的动作收尾时记进台词表的旁白。go_to 另有说法（看走到哪了）。
ACTION_DONE_PHRASES = {
    "sit": "你坐下了。",
    "stand": "你站起来了。",
    "lie_down": "你躺到床上睡着了。",
}


def format_event(data):
    """一条事件 → (中文, 是否紧急)。认不出的类型返回空文本，调用方据此什么都不做。"""
    kind = (data.get("event_type") or "").strip()
    formatter = FORMATTERS.get(kind)
    if formatter is None:
        return "", False
    # 玩家死在旁边不是背景动静，当场请她开口；别的生灵死了照旧走「攒够几条」那套。
    # 玩家丢下的东西被她收起来同理：那是给她的，该有反应；她自己捡到的杂物不必。
    # 外层那个 bool 不能省：`or` 会把右操作数原样返回，缺字段时就成了 None 而不是假。
    urgent = (
        kind in URGENT
        or bool(kind == "creature_death" and data.get("victim_owner"))
        or bool(kind == "item_pickup" and data.get("from_owner"))
    )
    return formatter(data), urgent


def event_ttl(kind):
    """这类事件值多少秒。"""
    return EVENT_TTL.get(kind, DEFAULT_EVENT_TTL)


def event_key(kind):
    """同类事件在队列里的合并键；不需要合并的返回空串。

    只给受伤用：连着挨几下是同一件事，留最重的那一次就够了，不必记成好几条旁白。
    """
    return "hurt" if kind == "maid_hurt" else ""


def event_severity(kind, data):
    """同键合并时比大小的严重度。受伤按掉血的百分之一计，取整免得浮点比较。"""
    if kind != "maid_hurt":
        return 0
    try:
        return int(round(float(data.get("damage") or 0) * 100))
    except (TypeError, ValueError):
        return 0


def snapshot(data):
    """接上世界时的第一帧：把眼下的处境交代清楚，模型才知道自己在哪儿。

    顺手把玩家在游戏里的名字写上：所有事件旁白里出现的都是这个名字，模型得先知道
    它指的就是玩家本人，才不会当成另一个人。名字来自模组，没配置也能说清。

    血量不进这一条：数字说了模型就念，而且「快撑不住了」有专门那条（`low_health`，
    接上之后的下一次采样就会判）。
    """
    player = (data.get("player_name") or "").strip()
    who = f"玩家（{player}）" if player else "玩家"
    clauses = [
        f"你正和{who}在"
        f"{name_of(DIMENSION_NAMES, data.get('dimension'))}的{name_of(BIOME_NAMES, data.get('biome'))}"
    ]
    # 下界 / 末地没有天空，也就没有昼夜与天气这回事
    if data.get("sky"):
        clauses.append("现在是白天" if data.get("phase") == "day" else "现在是夜里")
        clauses.append(_weather_phrase(data))
    return f"{'，'.join(clauses)}。"


def biome(data):
    return f"周围的环境变了，现在来到了{name_of(BIOME_NAMES, data.get('biome'))}。"


def dimension(data):
    came_from = name_of(DIMENSION_NAMES, data.get("from_dimension"))
    went_to = name_of(DIMENSION_NAMES, data.get("to_dimension"))
    return f"你跟着玩家从{came_from}来到了{went_to}。"


def time_phase(data):
    return "天黑了。" if data.get("phase") == "night" else "天亮了。"


def weather(data):
    if data.get("thundering"):
        return "变天了，外面在打雷。"
    if data.get("raining"):
        return "开始下雨了。"
    return "雨停了，天放晴了。"


def low_health(data):
    who = "你" if data.get("who") == "maid" else "玩家"
    return f"{who}伤得很重，再挨一下就撑不住了。"


def maid_hurt(data):
    """女仆受伤。模组会把一小段连续挨打合成一条，所以这里要照顾「几下」。

    只说伤得多重，不报掉了几点血、现在剩多少——那些数字模型会照抄进台词里。
    动手的是谁看两个标记：`attacker_owner` 是「她主人干的」（也就是玩家自己），
    `attacker_type` 是生物（说中文名），都没有才轮到伤害类型兜底。账号名一律不念。
    """
    if data.get("attacker_owner"):
        cause = "被玩家打了"
    elif data.get("attacker_type"):
        who = name_of(MOB_NAMES, data.get("attacker_type"), "什么东西")
        cause = f"被{who}打了"
    elif (data.get("attacker") or "").strip():
        cause = "被别的玩家打了"
    else:
        cause = DAMAGE_HINTS.get((data.get("damage_type") or "").strip(), "挨了打")
    count = as_int(data.get("count"), 1)
    times = "连着挨了几下，" if count > 1 else ""
    return f"你{cause}，{times}{_hurt_phrase(data.get('damage'))}。"


def creature_death(data):
    """附近死了个生灵：谁死了、怎么死的。

    因果是快照差分给不出的（它最多说「附近原来有村民，现在没有了」），只有事件说得清，
    所以这一条走 `LivingDeathEvent`。同类连着死时模组只报第一条，这里不必再判频率。
    """
    who = "玩家" if data.get("victim_owner") else who_of(data.get("category"), data.get("type"))
    return f"{nearness_place(data.get('distance'))}的{who}{_death_cause(data)}。"


def _death_cause(data):
    """怎么死的：一句动词短语。谁动的手看标记与类型 id，都不认识就按伤害类型说。

    `attacker_maid` 是「她自己动的手」，`attacker_owner` 是「她主人（玩家）动的手」。
    这两个标记必须排在 `attacker` 前面：她自己与各种生物也带一个显示名，拿名字去认
    「别的玩家」会把它们全说成玩家（2026-10-17 真机：玩家杀了两只羊，旁白写成「被你
    杀了」，角色于是以为自己动了手，一整天都在困惑）。
    """
    if data.get("attacker_maid"):
        return "被你杀了"
    if data.get("attacker_owner"):
        return "被玩家杀了"
    if (data.get("attacker_type") or "").strip():
        who = name_of(MOB_NAMES, data.get("attacker_type"), "什么东西")
        return f"被{who}杀了"
    if (data.get("attacker") or "").strip():
        return "被别的玩家杀了"
    return DAMAGE_HINTS.get((data.get("damage_type") or "").strip()) or "死了"


def item_gift(data):
    """主人把手里的东西丢在她身边。模组把一小段连续投掷合成一条。

    只记「给了什么」，不写数字之外的推测。她未必会去捡（拾物模式没开、背包没格就捡不了），
    捡没捡另有 `item_pickup` 那条。
    """
    place = "脚边" if _close(data.get("distance")) else "身边"
    return f"玩家往你{place}丢下了{_item_list(data.get('items'))}。"


def item_pickup(data):
    """她把地上的东西收进了背包。玩家丢的那份值得当场开口，其余只是记一笔。"""
    items = _item_list([data])
    if data.get("from_owner"):
        return f"你把玩家丢给你的{items} 收进了背包。"
    return f"你捡起了{items}。"


def item_received(data):
    """背包里多出东西，多半是玩家在女仆界面里放进去的。界面那条路没有事件可挂，
    模组只能前后比一次，所以这里只说「多了什么」，不硬认是谁放的。"""
    return f"你的背包里多了{_item_list(data.get('items'))}。"


def item_fed(data):
    """玩家喂她吃了东西（潜行右键食物）。连喂几口由模组合成一条。"""
    return f"玩家喂你吃了{_item_list(data.get('items'))}。"


def maid_ate(data):
    """她自己从背包里找东西吃了（车万女仆的进食系统）。"""
    return f"你自己吃了{_item_list(data.get('items'))}。"


def maid_carried(data):
    """被 Carry On 抱了起来。她主人才能抱她（那个模组只允许主人动驯服生物）。"""
    return "玩家把你抱了起来。"


def maid_dropped(data):
    """抱了一会儿又放下来了。"""
    return "玩家把你放下来了。"


def player_advancement(data):
    """玩家拿到了成就（原版叫「进度」）。挑战类另说一句，语气不一样。"""
    name = advancement_name(data.get("advancement"), data.get("title"))
    if (data.get("frame") or "") == "challenge":
        return f"玩家完成了挑战「{name}」。"
    return f"玩家获得了成就「{name}」。"


def maid_task_change(data):
    """她自己换了工作模式（农场、钓鱼、空闲……）。

    只说换成了什么，不催她开工：她现在会不会去干、在哪儿干，工具那次回执里已经说过了，
    而这条旁白要留在台词表里，管的是过一会儿她还记不记得自己在干什么。
    """
    return f"你换成了工作模式「{task_name(data.get('task'), data.get('summary'))}」。"


def _item_list(items):
    """[{item,count}, ...] → 「熟牛肉 ×2、面包」。不认识的 id 退回英文后半段，模型也认。"""
    parts = []
    for one in items or []:
        if not isinstance(one, dict):
            continue
        name = item_name(one.get("item"))
        count = as_int(one.get("count"), 1)
        parts.append(f"{name} ×{count}" if count > 1 else name)
    return "、".join(parts) or "东西"


def _close(distance):
    """丢的东西是不是就在脚边（三格内）。给不出距离就当脚边。"""
    try:
        return float(distance) <= 3.0
    except (TypeError, ValueError):
        return True


def threat_near(data):
    """敌对生物靠近。只说是什么东西靠过来了。

    身边那一层只剩这一条还在写旁白：它是唯一「得当场叫醒她」的观察，其余几类都由
    开口时那份快照带过去。方位与距离都不说——模型会照着旁白复述「左前方有僵尸，
    离我不远」，而这些细节对玩家没用，她该知道的只是「有危险在靠近」。
    """
    who = name_of(MOB_NAMES, data.get("type"), "什么东西")
    if not data.get("visible"):
        # 看不见的只剩声音，而且是模组筛过的近处（默认 6 格）；不说「靠过来了」
        return f"听到{who}的声音，就在附近。"
    return f"有{who}靠过来了。"


def action_done(data):
    """动作收尾。三种结局都要说：被顶掉也要说，否则角色会一直以为还在做。"""
    action = (data.get("action") or "").strip()
    status = (data.get("status") or "").strip()
    if status == "done":
        if action == "go_to":
            if (data.get("target") or "") == "bed":
                return "你走到了床边。"
            return "你走到了玩家身边。"
        return ACTION_DONE_PHRASES.get(action, "这件事你做完了。")
    if status == "cancelled":
        return "刚才那件事被新的事顶掉了，没做成。"
    reason = (data.get("reason") or "").strip()
    return f"刚才那件事没做成：{reason}" if reason else "刚才那件事没做成。"


def maid_mine_done(data):
    """挖掘收尾：挖到多少、还差多少、为什么停。

    工具受理只代表开始挖了，模型很容易把「正过去挖」当成「已经挖到了」；这条旁白才是
    唯一能让它改口的依据。走不到、工具不对也在这里说清楚，不留悬案。
    """
    name = name_of(BLOCK_NAMES, data.get("block"), "方块")
    mined = as_int(data.get("mined"), 0)
    wanted = as_int(data.get("wanted"), mined)
    status = (data.get("status") or "").strip()
    if status == "done":
        return f"你挖掉了一块{name}。" if mined == 1 else f"你挖掉了{mined}块{name}。"
    reason = (data.get("reason") or "").strip()
    if mined > 0:
        text = f"你挖掉了{mined}块{name}"
        if wanted > mined:
            text += f"，还差{wanted - mined}块没挖完"
        return text + (f"：{reason}。" if reason else "。")
    return f"没能挖成：{reason}。" if reason else "没能挖成。"


def _weather_phrase(data):
    if data.get("thundering"):
        return "外面在打雷"
    if data.get("raining"):
        return "外面在下雨"
    return "天气晴朗"


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


FORMATTERS = {
    "state_snapshot": snapshot,
    "biome_change": biome,
    "dimension_change": dimension,
    "time_phase_change": time_phase,
    "weather_change": weather,
    "low_health": low_health,
    "maid_hurt": maid_hurt,
    "threat_near": threat_near,
    "creature_death": creature_death,
    "action_done": action_done,
    "maid_mine_done": maid_mine_done,
    "item_gift": item_gift,
    "item_pickup": item_pickup,
    "item_received": item_received,
    "item_fed": item_fed,
    "maid_ate": maid_ate,
    "maid_carried": maid_carried,
    "maid_dropped": maid_dropped,
    "player_advancement": player_advancement,
    "maid_task_change": maid_task_change,
}

# 一条事件从发生到「还值得让角色知道」有多久（秒）。宿主在角色真要开口时按它截断：
# 过期的条目直接丢掉、不进台词表。捡起东西这种小事过几秒就没人关心；受伤、被围住
# 能管一分钟；换地形、换工作模式这类背景信息留得久一些。认不出的类型按下面那档。
EVENT_TTL = {
    "item_pickup": 5,
    "item_received": 30,
    "item_fed": 30,
    "maid_ate": 60,
    "item_gift": 60,
    "maid_hurt": 60,
    "low_health": 60,
    "threat_near": 60,
    "creature_death": 45,
    "action_done": 60,
    "maid_carried": 30,
    "maid_dropped": 30,
    "maid_mine_done": 120,
    "player_advancement": 300,
    "maid_task_change": 300,
    "time_phase_change": 300,
    "weather_change": 300,
    "state_snapshot": 600,
    "biome_change": 600,
    "dimension_change": 600,
}

DEFAULT_EVENT_TTL = 120

# 这几类当场请角色开口（挨打、血量告急、敌对生物靠近、动作收尾、挖完了、收到东西、
# **被抱起来**、玩家拿到成就）；其余走「攒够几件」那套，不然每一次散步都会变成一次生成。
# `item_pickup` 只有「捡的是玩家丢的」才当场开口，判在 `format_event` 里。
# 放下不算紧急：抱起来那一下要当场反应，放下来只是接着上一件事往下走。
URGENT = frozenset(
    {
        "low_health",
        "maid_hurt",
        "threat_near",
        "action_done",
        "maid_mine_done",
        "item_received",
        "item_fed",
        "maid_carried",
        "player_advancement",
    }
)
