"""世界这一层的事件：接上世界的第一帧、换群系/维度、天气、昼夜、血量告急。"""

from events.spec import EventSpec
from naming import names
from wording import BIOME_NAMES, BLOCK_NAMES, DIMENSION_NAMES, name_of


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
    return f"{'，'.join(clauses)}。"


def biome(data):
    maid, _ = names()
    return f"{maid}周围的环境变了，眼前是一片{name_of(BIOME_NAMES, data.get('biome'))}。"


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
    """头顶从露天变成有遮挡，或反过来。模组只在这件事真的翻转时推。"""
    maid, _ = names()
    if data.get("sheltered"):
        return f"{maid}走进了有遮挡的地方，头顶看不到天了。"
    return f"{maid}走出了遮挡，头顶又见天了。"


def ground_change(data):
    """脚下换了一种值得说的地。走在石头、泥土、木板上不报，所以这里可以直说。"""
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
    return f"{maid}脚下换成了{block}。"


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
    "shelter_change": EventSpec(shelter_change, ttl=90),
    # 岩浆和火踩上去就在掉血，值得当场提醒；水、雪、沙这类只记一笔
    "ground_change": EventSpec(
        ground_change,
        ttl=90,
        urgent_when=lambda data: (data.get("kind") or "") in ("lava", "fire"),
    ),
}
