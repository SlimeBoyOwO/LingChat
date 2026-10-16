"""把模组给的注册表 id、枚举串与数字翻成中文。

模组那一侧只给机器可读的东西：专用服务端上 `getName()` 只有英文（甚至裸翻译键），
中文只在客户端按玩家语言选得出来。所以中文表全在插件这一侧，事件的具体措辞在
events.py、工具的说法在 tools.py，这里只管「某个 id 叫什么」。
"""

# 常见维度。没收录的退回到 id 的后半段（英文），模型也认。
DIMENSION_NAMES = {
    "minecraft:overworld": "主世界",
    "minecraft:the_nether": "下界",
    "minecraft:the_end": "末地",
}

# 常见生物群系的中文名。
BIOME_NAMES = {
    "minecraft:plains": "平原",
    "minecraft:sunflower_plains": "向日葵平原",
    "minecraft:forest": "森林",
    "minecraft:flower_forest": "繁花森林",
    "minecraft:birch_forest": "白桦林",
    "minecraft:dark_forest": "黑森林",
    "minecraft:taiga": "针叶林",
    "minecraft:snowy_taiga": "积雪针叶林",
    "minecraft:old_growth_pine_taiga": "原始松木针叶林",
    "minecraft:snowy_plains": "雪原",
    "minecraft:ice_spikes": "冰刺之地",
    "minecraft:desert": "沙漠",
    "minecraft:savanna": "热带草原",
    "minecraft:jungle": "丛林",
    "minecraft:sparse_jungle": "稀疏丛林",
    "minecraft:swamp": "沼泽",
    "minecraft:mangrove_swamp": "红树林沼泽",
    "minecraft:badlands": "恶地",
    "minecraft:beach": "沙滩",
    "minecraft:stony_shore": "石岸",
    "minecraft:river": "河流",
    "minecraft:ocean": "海洋",
    "minecraft:frozen_ocean": "冻洋",
    "minecraft:warm_ocean": "暖海",
    "minecraft:mushroom_fields": "蘑菇岛",
    "minecraft:windswept_hills": "风袭丘陵",
    "minecraft:meadow": "草甸",
    "minecraft:cherry_grove": "樱花树林",
    "minecraft:nether_wastes": "下界荒地",
    "minecraft:soul_sand_valley": "灵魂沙峡谷",
    "minecraft:crimson_forest": "绯红森林",
    "minecraft:warped_forest": "诡异森林",
    "minecraft:basalt_deltas": "玄武岩三角洲",
    "minecraft:the_end": "末地",
    "minecraft:end_highlands": "末地高地",
    "minecraft:small_end_islands": "末地小岛",
}

# 常见生物的中文名。
MOB_NAMES = {
    "minecraft:zombie": "僵尸",
    "minecraft:skeleton": "骷髅",
    "minecraft:creeper": "苦力怕",
    "minecraft:spider": "蜘蛛",
    "minecraft:cave_spider": "洞穴蜘蛛",
    "minecraft:enderman": "末影人",
    "minecraft:witch": "女巫",
    "minecraft:slime": "史莱姆",
    "minecraft:magma_cube": "岩浆怪",
    "minecraft:drowned": "溺尸",
    "minecraft:husk": "尸壳",
    "minecraft:stray": "流浪者",
    "minecraft:bogged": "沼骸",
    "minecraft:phantom": "幻翼",
    "minecraft:blaze": "烈焰人",
    "minecraft:ghast": "恶魂",
    "minecraft:piglin": "猪灵",
    "minecraft:piglin_brute": "猪灵蛮兵",
    "minecraft:hoglin": "疣猪兽",
    "minecraft:zoglin": "僵尸疣猪兽",
    "minecraft:zombified_piglin": "僵尸猪灵",
    "minecraft:wither_skeleton": "凋灵骷髅",
    "minecraft:zombie_villager": "僵尸村民",
    "minecraft:pillager": "掠夺者",
    "minecraft:vindicator": "卫道士",
    "minecraft:evoker": "唤魔者",
    "minecraft:ravager": "劫掠兽",
    "minecraft:vex": "恼鬼",
    "minecraft:silverfish": "蠹虫",
    "minecraft:endermite": "末影螨",
    "minecraft:guardian": "守卫者",
    "minecraft:elder_guardian": "远古守卫者",
    "minecraft:shulker": "潜影贝",
    "minecraft:warden": "监守者",
    "minecraft:breeze": "旋风人",
    "minecraft:wither": "凋灵",
    "minecraft:ender_dragon": "末影龙",
    "minecraft:cow": "牛",
    "minecraft:pig": "猪",
    "minecraft:sheep": "羊",
    "minecraft:chicken": "鸡",
    "minecraft:rabbit": "兔子",
    "minecraft:horse": "马",
    "minecraft:donkey": "驴",
    "minecraft:llama": "羊驼",
    "minecraft:wolf": "狼",
    "minecraft:cat": "猫",
    "minecraft:ocelot": "豹猫",
    "minecraft:fox": "狐狸",
    "minecraft:bee": "蜜蜂",
    "minecraft:bat": "蝙蝠",
    "minecraft:parrot": "鹦鹉",
    "minecraft:villager": "村民",
    "minecraft:wandering_trader": "流浪商人",
    "minecraft:iron_golem": "铁傀儡",
    "minecraft:snow_golem": "雪傀儡",
    "minecraft:allay": "悦灵",
    "minecraft:axolotl": "美西螈",
    "minecraft:frog": "青蛙",
    "minecraft:squid": "鱿鱼",
    "minecraft:glow_squid": "发光鱿鱼",
    "minecraft:dolphin": "海豚",
    "minecraft:turtle": "海龟",
    "minecraft:panda": "熊猫",
    "minecraft:polar_bear": "北极熊",
    "minecraft:goat": "山羊",
    "minecraft:strider": "炽足兽",
    "minecraft:camel": "骆驼",
    "minecraft:sniffer": "嗅探兽",
    "minecraft:armadillo": "犰狳",
    "touhou_little_maid:maid": "别的女仆",
}

# 常见方块的中文名，只给「你周围有什么」那份描述用。
BLOCK_NAMES = {
    "minecraft:water": "水",
    "minecraft:lava": "岩浆",
    "minecraft:fire": "火",
    "minecraft:soul_fire": "灵魂火",
    "minecraft:snow": "雪",
    "minecraft:snow_block": "雪块",
    "minecraft:powder_snow": "细雪",
    "minecraft:sand": "沙子",
    "minecraft:red_sand": "红沙",
    "minecraft:gravel": "砂砾",
    "minecraft:dirt": "泥土",
    "minecraft:coarse_dirt": "砂土",
    "minecraft:grass_block": "草方块",
    "minecraft:stone": "石头",
    "minecraft:cobblestone": "圆石",
    "minecraft:deepslate": "深板岩",
    "minecraft:granite": "花岗岩",
    "minecraft:diorite": "闪长岩",
    "minecraft:andesite": "安山岩",
    "minecraft:ice": "冰",
    "minecraft:packed_ice": "浮冰",
    "minecraft:farmland": "耕地",
    "minecraft:dirt_path": "土径",
    "minecraft:oak_log": "橡木原木",
    "minecraft:spruce_log": "云杉原木",
    "minecraft:birch_log": "白桦原木",
    "minecraft:oak_planks": "橡木木板",
    "minecraft:spruce_planks": "云杉木板",
    "minecraft:oak_door": "橡木门",
    "minecraft:oak_fence": "橡木栅栏",
    "minecraft:glass": "玻璃",
    "minecraft:glass_pane": "玻璃板",
    "minecraft:oak_leaves": "橡树树叶",
    "minecraft:torch": "火把",
    "minecraft:crafting_table": "工作台",
    "minecraft:chest": "箱子",
    "minecraft:furnace": "熔炉",
    "minecraft:white_bed": "床",
    "minecraft:white_wool": "白色羊毛",
    "minecraft:white_carpet": "地毯",
    "minecraft:stone_bricks": "石砖",
    "minecraft:bricks": "砖块",
    "minecraft:bookshelf": "书架",
    "minecraft:enchanting_table": "附魔台",
    "minecraft:pumpkin": "南瓜",
    "minecraft:melon": "西瓜",
    "minecraft:wheat": "小麦",
    "minecraft:sugar_cane": "甘蔗",
    "minecraft:magma_block": "岩浆块",
    "minecraft:obsidian": "黑曜石",
    "minecraft:netherrack": "下界岩",
    "minecraft:soul_sand": "灵魂沙",
    "minecraft:end_stone": "末地石",
    "minecraft:iron_block": "铁块",
    "minecraft:gold_block": "金块",
    "minecraft:diamond_block": "钻石块",
    "minecraft:glowstone": "萤石",
}

# 八向方位。以女仆的身体朝向为准，而不是她正在看的方向。
DIRECTION_NAMES = {
    "front": "正前方",
    "front_right": "右前方",
    "right": "右侧",
    "back_right": "右后方",
    "back": "背后",
    "back_left": "左后方",
    "left": "左侧",
    "front_left": "左前方",
}

# 附近那只算哪一类。有自己名字的生物直接用名字，所以 living 不带称呼。
CATEGORY_NAMES = {"player": "另一个玩家", "maid": "别的女仆", "living": ""}

# 说不清是谁打的时候，用伤害类型兜个底（模组给的是伤害类型的 msg id）。
DAMAGE_HINTS = {
    "fall": "从高处摔了下来",
    "lava": "碰到了岩浆",
    "in_fire": "被火烧到了",
    "on_fire": "身上着火了",
    "drown": "溺水了",
    "starve": "饿得不行",
    "cactus": "被仙人掌扎了",
    "freeze": "被冻伤了",
    "lightning_bolt": "被雷劈了",
    "magic": "被魔法击中",
    "wither": "中了凋零",
    "hot_floor": "踩到了岩浆块",
    "sweet_berry_bush": "被甜浆果丛扎了",
    "fly_into_wall": "撞到了墙上",
    "out_of_world": "掉进了虚空",
}


def name_of(names, raw, default="未知的地方"):
    """游戏 id → 中文名；没收录的退回到 id 的后半段（英文），模型也认。"""
    identifier = (raw or "").strip()
    if not identifier:
        return default
    return names.get(identifier) or identifier.split(":")[-1].replace("_", " ")


def direction_text(raw):
    """方位 token → 中文；认不出的说「附近」，别让句子缺一块。"""
    return name_of(DIRECTION_NAMES, raw, "附近")


def who_of(category, type_id):
    """附近那只算谁：类别有专门称呼的（另一个玩家、别的女仆）用它，其余用生物名。"""
    return CATEGORY_NAMES.get(category or "") or name_of(MOB_NAMES, type_id, "什么东西")


def sighting(one):
    """一份附近实体的记录 → 「右前方 6.2 格的僵尸（被挡着）」。"""
    where = direction_text(one.get("direction"))
    who = who_of(one.get("category"), one.get("type"))
    hidden = "" if one.get("visible") else "（被挡着）"
    return f"{where} {number(one.get('distance'))} 格的{who}{hidden}"


def number(value):
    """血量与距离这类数字：整数不带小数点，其余留一位；给不出来就留个问号。"""
    try:
        as_float = float(value)
    except (TypeError, ValueError):
        return "?"
    if as_float == int(as_float):
        return str(int(as_float))
    return f"{as_float:.1f}"


def as_int(value, default=0):
    """配置与事件里的数字可能是浮点（前端表单、JSON）或字符串。"""
    try:
        return int(float(value))
    except (TypeError, ValueError):
        return default
