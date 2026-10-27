"""世界这一层的事件：接上世界的第一帧、换群系/维度、天气、昼夜、血量告急、
上下载具、进出结构、玩家远近、她的姿势。"""

from events.spec import EventSpec
from naming import names
from wording import (
    BIOME_NAMES,
    BLOCK_NAMES,
    DIMENSION_NAMES,
    LIGHT_PLACES,
    OVERHEAD_PLACES,
    STRUCTURE_NAMES,
    UNCONSCIOUS_STATE_MARK,
    VEHICLE_NAMES,
    as_int,
    name_of,
    nearness_place,
)


def snapshot(data):
    """接上世界时的第一帧：把眼下的处境交代清楚，模型才知道自己在哪儿。

    旁白用第三人称，角色与玩家都叫名字：模型读到的每一句都像叙事，而不是
    「你正和玩家……」。名字来自宿主，没加载角色时退回「女仆」「主人」。

    血量不进这一条：数字说了模型就念，而且「快撑不住了」有专门那条（`low_health`，
    接上之后的下一次采样就会判）。
    """
    maid, player = names()
    clauses = [
        f"{maid}正和{player}在"
        f"{name_of(DIMENSION_NAMES, data.get('dimension'))}的{name_of(BIOME_NAMES, data.get('biome'))}"
    ]
    # 下界 / 末地没有天空，也就没有昼夜与天气这回事
    if data.get("sky"):
        clauses.append("现在是白天" if data.get("phase") == "day" else "现在是夜里")
        clauses.append(_weather_phrase(data))
    # 她可能正坐着、睡着、躺在地上或昏迷：接上的这一刻就得说清，否则模型默认她站着。
    # 昏迷不是睡着，不能写成「正睡着」。
    current = (data.get("posture") or "").strip()
    if current == "unconscious":
        clauses.append(f"{maid}{UNCONSCIOUS_STATE_MARK}，不省人事，只剩一口气")
    else:
        posture = {
            "sitting": f"{maid}正坐着",
            "sleeping": f"{maid}正睡着",
            "lying": f"{maid}正躺在地上",
        }.get(current)
        if posture:
            clauses.append(posture)
    return f"{'，'.join(clauses)}。"


def biome(data):
    """换群系。说成「跟着玩家走到了哪」，不要说成「周围的环境变了」——
    后者像是世界凭空变了个样，模型会以为发生了什么怪事，而不是两个人走了一路。
    """
    maid, player = names()
    return f"{maid}跟着{player}走到了{name_of(BIOME_NAMES, data.get('biome'))}。"


def dimension(data):
    maid, player = names()
    came_from = name_of(DIMENSION_NAMES, data.get("from_dimension"))
    went_to = name_of(DIMENSION_NAMES, data.get("to_dimension"))
    return f"{maid}跟着{player}从{came_from}来到了{went_to}。"


def time_phase(data):
    return "天黑了。" if data.get("phase") == "night" else "天亮了。"


def weather(data):
    if data.get("thundering"):
        return "变天了，外面在打雷。"
    if data.get("raining"):
        return "开始下雨了。"
    return "雨停了，天放晴了。"


def low_health(data):
    maid, player = names()
    who = maid if data.get("who") == "maid" else player
    return f"{who}伤得很重，再挨一下就撑不住了。"


def shelter_change(data):
    """头顶换了档：露天 / 半遮（树荫、屋檐、洞口）/ 屋内（房间、洞穴）。

    模组三档都推，这里只挑说法；老模组只给 `sheltered` 一个布尔，退回原来那两句。
    """
    maid, _ = names()
    kind = (data.get("overhead") or "").strip()
    if not kind:
        if data.get("sheltered"):
            return f"{maid}走进了有遮挡的地方，头顶看不到天了。"
        return f"{maid}走出了遮挡，头顶又见天了。"
    if kind == "open":
        return f"{maid}走出了遮挡，头顶又见天了。"
    cover = name_of(BLOCK_NAMES, data.get("overhead_cover"), "")
    place = OVERHEAD_PLACES.get((data.get("overhead_band") or "").strip(), "头顶")
    if kind == "partial":
        if cover:
            return f"{maid}走到{cover}下面，头顶只遮着一部分，还能看见天。"
        return f"{maid}走进一片遮挡里，头顶只遮着一部分，还能看见天。"
    if cover:
        return f"{maid}走到了一处头顶封着的地方，{place}是{cover}，看不到天了。"
    return f"{maid}走到了一处头顶封着的地方，看不到天了。"


def light_change(data):
    """光线跨档：很黑 / 昏暗 / 明亮。

    旁白只说状态，不念 0-15 的数值；档位由模组判定，这里只挑中文说法。
    """
    maid, _ = names()
    place = LIGHT_PLACES.get((data.get("band") or "").strip())
    return f"{maid}现在在{place}。" if place else ""


# 远近档里算「走开了」的那一档：十几格外，或者不在这个世界上。几格、几十步的挪动
# 都不算变化——跟随状态下她就贴着他走，逐档报能报出一串没人要的报告。
FAR_BANDS = ("gone",)


def player_proximity(data):
    """玩家走远又回来，只在跨过「走开了」这条线时才说。

    几格之内怎么挪都不算变化——每句话都念一遍「玩家就在身边」会把模型教成复读机，
    而跟随状态下她就贴着他走，这类句子一天能攒上百条。
    """
    maid, player = names()
    try:
        distance = float(data.get("distance"))
    except (TypeError, ValueError):
        distance = -1.0
    if distance < 0:
        return f"{player}离开了{maid}所在的这个世界。"
    before = (data.get("from_proximity") or "").strip()
    now = (data.get("proximity") or "").strip()
    if now in FAR_BANDS and before not in FAR_BANDS:
        return f"{player}走远了，把{maid}落在后面。"
    if before in FAR_BANDS and now not in FAR_BANDS:
        return f"{player}回到了{maid}身边。"
    return ""


def ground_change(data):
    """脚下换了一种值得说的地。走在石头、泥土、木板上不报，所以这里可以直说。

    水里、岩浆里另走 `liquid_change`：那是「她泡进去」而不是「脚下踩着哪」。
    这里的水 / 岩浆两条只留给还没升级的老模组。
    """
    maid, _ = names()
    kind = (data.get("kind") or "").strip()
    phrases = {
        "water": f"{maid}踩进了水里。",
        "lava": f"{maid}踩进了岩浆里。",
        "fire": f"{maid}踩到了火里。",
        "snow": f"{maid}踩进了雪里。",
        "sand": f"{maid}走到了沙地上。",
        "farmland": f"{maid}踩进了耕地里。",
        "ice": f"{maid}踩到了冰面上。",
    }
    if kind in phrases:
        return phrases[kind]
    block = name_of(BLOCK_NAMES, data.get("block"), "另一种地面")
    return f"{maid}脚下现在是{block}。"


LIQUID_NAMES = {"minecraft:water": "水", "minecraft:lava": "岩浆"}


def liquid_change(data):
    """她泡进了液体里，或者从里面出来；头也埋进去就是游泳。

    模组看的是她自己那一格有没有液体，不看脚下：站在河里时脚下仍是河床，这里说的才是
    「她泡在水里」。液体 id 认不出时退回方块表，再认不出就说「液体」。
    """
    maid, _ = names()
    liquid = (data.get("liquid") or "").strip()
    if not liquid:
        came = _liquid_name(data.get("from_liquid"))
        return f"{maid}从{came}里出来了。" if came else f"{maid}离开了液体。"
    if data.get("swimming") and liquid == "minecraft:water":
        return f"{maid}整个人泡进了水里，正在游泳。"
    return f"{maid}泡进了{_liquid_name(liquid)}里。"


def _liquid_name(raw):
    liquid = (raw or "").strip()
    if not liquid:
        return ""
    return LIQUID_NAMES.get(liquid) or BLOCK_NAMES.get(liquid) or "液体"


def vehicle_change(data):
    """她坐上或离开了载具。坐在船、矿车里的时候脚下没有「地」可说，这件事单独记。"""
    maid, _ = names()
    vehicle = (data.get("vehicle") or "").strip()
    if vehicle:
        return f"{maid}坐进了{name_of(VEHICLE_NAMES, vehicle, '载具')}里。"
    from_vehicle = (data.get("from_vehicle") or "").strip()
    if not from_vehicle:
        return ""
    return f"{maid}从{name_of(VEHICLE_NAMES, from_vehicle, '载具')}上下来了。"


def posture_change(data):
    """她换了姿势：坐下、站起、就地躺下、上床睡着、醒过来。

    这是她身上最容易被模型搞错的一件事：以为她还站着，就会让她「走过来」而不是「爬起来」。
    旁白只陈述换了什么姿势，不替她解释为什么。
    """
    maid, _ = names()
    posture = (data.get("posture") or "").strip()
    before = (data.get("from_posture") or "").strip()
    if posture == "sitting":
        return f"{maid}坐了下来。"
    if posture == "lying":
        return f"{maid}躺到了地上。"
    if posture == "sleeping":
        return f"{maid}躺下睡着了。"
    if posture == "unconscious":
        return f"{maid}昏了过去。"
    if posture == "standing":
        if before in ("sleeping", "unconscious"):
            return f"{maid}醒了过来。"
        return f"{maid}站了起来。"
    return ""


def structure_change(data):
    """她走进或离开一处结构（村庄、矿井、要塞这类）。"""
    maid, _ = names()
    structure = (data.get("structure") or "").strip()
    if structure:
        return f"{maid}走进了一处{name_of(STRUCTURE_NAMES, structure, '结构')}。"
    from_structure = (data.get("from_structure") or "").strip()
    if not from_structure:
        return ""
    return f"{maid}离开了一处{name_of(STRUCTURE_NAMES, from_structure, '结构')}。"


def hazard_near(data):
    """她视野里新出现了岩浆、火这类危险的东西。

    只说大概多远、是什么，不报方位与格数——模型会照着旁白复述「左前方三格有岩浆」，
    而这些细节对玩家没用；她该知道的只是那边有危险。脚下的岩浆另走 `ground_change`，
    同一次不会说两遍。
    """
    block = name_of(BLOCK_NAMES, data.get("block"), "危险的东西")
    return f"{nearness_place(data.get('distance'))}有{block}。"


def _weather_phrase(data):
    if data.get("thundering"):
        return "外面在打雷"
    if data.get("raining"):
        return "外面在下雨"
    return "天气晴朗"


SPECS = {
    "state_snapshot": EventSpec(snapshot, ttl=600),
    "biome_change": EventSpec(biome, ttl=600),
    "dimension_change": EventSpec(dimension, ttl=600),
    "time_phase_change": EventSpec(time_phase, ttl=300),
    "weather_change": EventSpec(weather, ttl=300),
    "low_health": EventSpec(low_health, urgent=True, ttl=60, snapshot=True),
    "shelter_change": EventSpec(
        shelter_change,
        ttl=90,
        # 一会儿进树荫一会儿出来，两句排在一起读着像来回折腾：同键合并，留最后那次
        merge_key="shelter",
        severity=lambda data: as_int(data.get("tick"), 0),
    ),
    # 光线跨档：进洞、天黑、点火把都会遇到。模组要连续两次采样才认，洞口来回不刷屏
    "light_change": EventSpec(light_change, ttl=300),
    # 玩家远近只在跨过「走开了」这条线时说一句，几格之内的挪动不算变化。
    # 走了又回来时只留最后一句——他已经回来了，前一句再念一遍只会让模型困惑。
    "player_proximity_change": EventSpec(player_proximity, ttl=30, merge_key="proximity"),
    # 上下载具与进出结构都很少见，记一笔就够，让角色自己决定要不要提
    "vehicle_change": EventSpec(vehicle_change, ttl=120),
    "structure_change": EventSpec(structure_change, ttl=300),
    # 换姿势是离散动作，不紧急；连着坐下又站起来时同键合并，只留最后那一次
    "posture_change": EventSpec(posture_change, ttl=120, merge_key="posture"),
    # 岩浆和火踩上去就在掉血，值得当场提醒；水、雪、沙这类只记一笔
    "ground_change": EventSpec(
        ground_change,
        ttl=90,
        urgent_when=lambda data: (data.get("kind") or "") in ("lava", "fire"),
    ),
    # 泡进液体里与埋头游泳：模组看她自己那一格有没有液体，一分钟最多一条。
    # 岩浆烧在身上，值得当场开口
    "liquid_change": EventSpec(
        liquid_change,
        ttl=90,
        urgent_when=lambda data: (data.get("liquid") or "") == "minecraft:lava",
    ),
    # 边上的岩浆与火：不紧急（她还没踩进去），同键合并时留最近的那一处。
    # 模组只在「这一种原来不在她眼里」时报，沿着岩浆湖走不会一步一句
    "hazard_near": EventSpec(
        hazard_near,
        ttl=60,
        merge_key="hazard",
        severity=lambda data: 100 - as_int(data.get("distance"), 100),
    ),
}
