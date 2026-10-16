"""模组推来的一条游戏事件 → 一句只陈述事实的中文。

只写发生了什么，不写「该有什么情绪」——情绪由模型自己决定，留在台词表里的始终是
客观记录。哪一条算紧急（要当场请角色开口）由 `_URGENT` 定，事件落地在 hooks.py。

加一类事件：写一个函数，在 `FORMATTERS` 里登记一行；紧急的话再进 `_URGENT`。
每个函数收 `(data, player_name)`，只有受伤那条用得上名字（打她的可能是玩家本人）。
认不出的 event_type 一律不出声——模组先加了新事件、插件还没跟上时，这是刻意的向后兼容。
"""

from wording import (
    BIOME_NAMES,
    DAMAGE_HINTS,
    DIMENSION_NAMES,
    MOB_NAMES,
    as_int,
    direction_text,
    name_of,
    number,
    who_of,
)

# 与主人的远近档。gone 只可能是他去了别的维度、或者根本没绑定主人。
PROXIMITY_PHRASES = {
    "close": "走到了你身边",
    "near": "退到了几步之外",
    "far": "走远了一些",
    "gone": "离开了你的世界",
}

# 脚下值得说的几种地，与模组侧那份白名单一一对应。
FOOTING_PHRASES = {
    "water": "你踩进水里了。",
    "lava": "你踩进岩浆里了。",
    "fire": "你踩进火里了。",
    "snow": "你走到雪地上了。",
    "sand": "你踩到沙子上了。",
    "farmland": "你踩到耕地上了。",
    "ice": "你踩到冰面上了。",
}

# 受理即完成的动作收尾时记进台词表的旁白。go_to 另有说法（看走到哪了）。
ACTION_DONE_PHRASES = {
    "sit": "你坐下了。",
    "stand": "你站起来了。",
    "lie_down": "你躺到床上睡着了。",
    "look_at": "你转头看向了玩家。",
}


def format_event(data, player_name=""):
    """一条事件 → (中文, 是否紧急)。认不出的类型返回空文本，调用方据此什么都不做。"""
    kind = (data.get("event_type") or "").strip()
    formatter = FORMATTERS.get(kind)
    if formatter is None:
        return "", False
    return formatter(data, player_name), kind in URGENT


def snapshot(data, _player):
    """接上世界时的第一帧：把眼下的处境交代清楚，模型才知道自己在哪儿。

    顺手把玩家在游戏里的名字写上：所有事件旁白里出现的都是这个名字，模型得先知道
    它指的就是玩家本人，才不会当成另一个人。名字来自模组，没配置也能说清。
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
    health = f"你的血量 {number(data.get('maid_health'))}/{number(data.get('maid_max_health'))}"
    if as_int(data.get("player_max_health"), 0) > 0:
        health += f"，玩家 {number(data.get('player_health'))}/{number(data.get('player_max_health'))}"
    return f"{'，'.join(clauses)}。{health}。"


def biome(data, _player):
    return f"周围的环境变了，现在来到了{name_of(BIOME_NAMES, data.get('biome'))}。"


def dimension(data, _player):
    came_from = name_of(DIMENSION_NAMES, data.get("from_dimension"))
    went_to = name_of(DIMENSION_NAMES, data.get("to_dimension"))
    return f"你跟着玩家从{came_from}来到了{went_to}。"


def time_phase(data, _player):
    return "天黑了。" if data.get("phase") == "night" else "天亮了。"


def weather(data, _player):
    if data.get("thundering"):
        return "变天了，外面在打雷。"
    if data.get("raining"):
        return "开始下雨了。"
    return "雨停了，天放晴了。"


def low_health(data, _player):
    who = "你" if data.get("who") == "maid" else "玩家"
    now = number(data.get("health"))
    most = number(data.get("max_health"))
    return f"{who}的血量只剩 {now}/{most}，快撑不住了。"


def maid_hurt(data, player_name):
    """女仆受伤。模组会把一小段连续挨打合成一条，所以这里要照顾「几下」。"""
    attacker = (data.get("attacker") or "").strip()
    if attacker and player_name and attacker == player_name:
        cause = "被玩家打了"
    elif attacker:
        cause = f"被{attacker}打了"
    else:
        cause = DAMAGE_HINTS.get((data.get("damage_type") or "").strip(), "挨了打")
    count = as_int(data.get("count"), 1)
    times = f"连续 {count} 下，" if count > 1 else ""
    damage = number(data.get("damage"))
    now = number(data.get("health"))
    most = number(data.get("max_health"))
    return f"你{cause}，{times}一共掉了 {damage} 点血，现在是 {now}/{most}。"


def player_proximity_change(data, _player):
    band = (data.get("proximity") or "").strip()
    return f"玩家{PROXIMITY_PHRASES.get(band, '离你的距离变了')}。"


def player_sight_change(data, _player):
    if data.get("visible"):
        return "玩家又出现在你能看到的地方了。"
    return "玩家走出了你的视线。"


def threat_near(data, _player):
    """敌对生物靠近。说清是什么、多远、在哪个方向；还没看见它就补一句。"""
    who = name_of(MOB_NAMES, data.get("type"), "什么东西")
    where = direction_text(data.get("direction"))
    text = f"有{who}靠过来了，大约 {number(data.get('distance'))} 格，在你{where}。"
    if not data.get("visible"):
        text += "你还没看到它。"
    return text


def living_near(data, _player):
    """身边来了别的活物。玩家与其他女仆用类别称呼，其余用生物名。"""
    who = who_of(data.get("category"), data.get("type"))
    where = direction_text(data.get("direction"))
    return f"附近有{who}，在你{where}，大约 {number(data.get('distance'))} 格。"


def shelter_change(data, _player):
    if data.get("sheltered"):
        return "你走进了有遮挡的地方，头顶看不到天了。"
    return "你走到露天的空地上了，头顶又能看到天。"


def ground_change(data, _player):
    kind = (data.get("kind") or "").strip()
    return FOOTING_PHRASES.get(kind, "你脚下踩的东西变了。")


def action_done(data, _player):
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


def _weather_phrase(data):
    if data.get("thundering"):
        return "外面在打雷"
    if data.get("raining"):
        return "外面在下雨"
    return "天气晴朗"


FORMATTERS = {
    "state_snapshot": snapshot,
    "biome_change": biome,
    "dimension_change": dimension,
    "time_phase_change": time_phase,
    "weather_change": weather,
    "low_health": low_health,
    "maid_hurt": maid_hurt,
    "player_proximity_change": player_proximity_change,
    "player_sight_change": player_sight_change,
    "threat_near": threat_near,
    "living_near": living_near,
    "shelter_change": shelter_change,
    "ground_change": ground_change,
    "action_done": action_done,
}

# 这几类当场请角色开口（挨打、血量告急、敌对生物靠近、动作收尾）；
# 其余走「攒够几条」那套，不然每一次散步都会变成一次生成。
URGENT = frozenset({"low_health", "maid_hurt", "threat_near", "action_done"})
