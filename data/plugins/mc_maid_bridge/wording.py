"""把模组给的注册表 id、枚举串与数字翻成中文。

模组那一侧只给机器可读的东西：专用服务端上 `getName()` 只有英文（甚至裸翻译键），
中文只在客户端按玩家语言选得出来。所以中文表全在插件这一侧，事件的具体措辞在
events/、工具的说法在 tools/，这里只管「某个 id 叫什么」。
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

# 结构的中文名。她走进一片村庄、矿井、要塞时才会用到；表里没有的退回 id 后半段。
STRUCTURE_NAMES = {
    "minecraft:village_plains": "平原村庄",
    "minecraft:village_desert": "沙漠村庄",
    "minecraft:village_savanna": "热带草原村庄",
    "minecraft:village_snowy": "雪原村庄",
    "minecraft:village_taiga": "针叶林村庄",
    "minecraft:mineshaft": "废弃矿井",
    "minecraft:mineshaft_mesa": "恶地废弃矿井",
    "minecraft:stronghold": "要塞",
    "minecraft:monument": "海底神殿",
    "minecraft:mansion": "林地府邸",
    "minecraft:desert_pyramid": "沙漠神殿",
    "minecraft:jungle_temple": "丛林神庙",
    "minecraft:swamp_hut": "女巫小屋",
    "minecraft:igloo": "雪屋",
    "minecraft:ocean_ruin_cold": "海底废墟",
    "minecraft:ocean_ruin_warm": "海底废墟",
    "minecraft:shipwreck": "沉船",
    "minecraft:shipwreck_beached": "搁浅的沉船",
    "minecraft:buried_treasure": "埋藏的宝藏",
    "minecraft:pillager_outpost": "掠夺者前哨站",
    "minecraft:ancient_city": "远古城市",
    "minecraft:trail_ruins": "古迹废墟",
    "minecraft:trial_chambers": "试炼密室",
    "minecraft:ruined_portal": "废弃传送门",
    "minecraft:ruined_portal_nether": "废弃传送门",
    "minecraft:ruined_portal_desert": "废弃传送门",
    "minecraft:ruined_portal_jungle": "废弃传送门",
    "minecraft:ruined_portal_swamp": "废弃传送门",
    "minecraft:ruined_portal_mountain": "废弃传送门",
    "minecraft:ruined_portal_ocean": "废弃传送门",
    "minecraft:nether_fortress": "下界要塞",
    "minecraft:bastion_remnant": "堡垒遗迹",
    "minecraft:end_city": "末地城",
}

# 载具的中文名。她坐进船、矿车这类东西时用；坐在生物身上（马、骆驼）也算。
VEHICLE_NAMES = {
    # 不分木种的裸 id：有些整合包的船实体就叫 boat，说「船」比念 id 强
    "minecraft:boat": "船",
    "minecraft:chest_boat": "运输船",
    "minecraft:oak_boat": "橡木船",
    "minecraft:spruce_boat": "云杉木船",
    "minecraft:birch_boat": "白桦木船",
    "minecraft:jungle_boat": "丛林木船",
    "minecraft:acacia_boat": "金合欢木船",
    "minecraft:dark_oak_boat": "深色橡木船",
    "minecraft:mangrove_boat": "红树木船",
    "minecraft:cherry_boat": "樱花木船",
    "minecraft:oak_chest_boat": "橡木运输船",
    "minecraft:spruce_chest_boat": "云杉木运输船",
    "minecraft:birch_chest_boat": "白桦木运输船",
    "minecraft:jungle_chest_boat": "丛林木运输船",
    "minecraft:acacia_chest_boat": "金合欢木运输船",
    "minecraft:dark_oak_chest_boat": "深色橡木运输船",
    "minecraft:mangrove_chest_boat": "红树木运输船",
    "minecraft:cherry_chest_boat": "樱花木运输船",
    "minecraft:bamboo_raft": "竹筏",
    "minecraft:bamboo_chest_raft": "竹制运输筏",
    "minecraft:minecart": "矿车",
    "minecraft:chest_minecart": "运输矿车",
    "minecraft:furnace_minecart": "动力矿车",
    "minecraft:hopper_minecart": "漏斗矿车",
    "minecraft:tnt_minecart": "TNT 矿车",
    "minecraft:horse": "马",
    "minecraft:donkey": "驴",
    "minecraft:mule": "骡",
    "minecraft:camel": "骆驼",
    "minecraft:pig": "猪",
    "minecraft:strider": "炽足兽",
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

# 原版「进度」的中文名（玩家平时就叫成就）。按注册表 id 收录，认不出时退回
# 模组给的游戏内标题，再不行退回 id 后半段——模型看得懂英文，但不该只看到 id。
ADVANCEMENT_NAMES = {
    "minecraft:adventure/adventuring_time": "探索的时光",
    "minecraft:adventure/arbalistic": "劲弩手",
    "minecraft:adventure/avoid_vibration": "潜行100级",
    "minecraft:adventure/blowback": "逆风翻盘",
    "minecraft:adventure/brush_armadillo": "这不是鳞甲么？",
    "minecraft:adventure/bullseye": "正中靶心",
    "minecraft:adventure/craft_decorated_pot_using_only_sherds": "精修细补",
    "minecraft:adventure/crafters_crafting_crafters": "合成器合成合成器",
    "minecraft:adventure/fall_from_world_height": "上天入地",
    "minecraft:adventure/heart_transplanter": "移心接木",
    "minecraft:adventure/hero_of_the_village": "村庄英雄",
    "minecraft:adventure/honey_block_slide": "胶着状态",
    "minecraft:adventure/kill_a_mob": "怪物猎人",
    "minecraft:adventure/kill_all_mobs": "资深怪物猎人",
    "minecraft:adventure/kill_mob_near_sculk_catalyst": "它蔓延了",
    "minecraft:adventure/lighten_up": "铜光焕发",
    "minecraft:adventure/lightning_rod_with_villager_no_fire": "电涌保护器",
    "minecraft:adventure/minecraft_trials_edition": "Minecraft：试炼版",
    "minecraft:adventure/ol_betsy": "扣下悬刀",
    "minecraft:adventure/overoverkill": "天赐良击",
    "minecraft:adventure/play_jukebox_in_meadows": "音乐之声",
    "minecraft:adventure/read_power_from_chiseled_bookshelf": "知识就是力量",
    "minecraft:adventure/revaulting": "宝经磨炼",
    "minecraft:adventure/root": "冒险",
    "minecraft:adventure/salvage_sherd": "探古寻源",
    "minecraft:adventure/shoot_arrow": "瞄准目标",
    "minecraft:adventure/sleep_in_bed": "甜蜜的梦",
    "minecraft:adventure/sniper_duel": "狙击手的对决",
    "minecraft:adventure/spear_many_mobs": "生物串串香",
    "minecraft:adventure/spyglass_at_dragon": "那是飞机吗？",
    "minecraft:adventure/spyglass_at_ghast": "那是气球吗？",
    "minecraft:adventure/spyglass_at_parrot": "那是鸟吗？",
    "minecraft:adventure/summon_iron_golem": "招募援兵",
    "minecraft:adventure/throw_trident": "抖包袱",
    "minecraft:adventure/totem_of_undying": "超越生死",
    "minecraft:adventure/trade": "成交！",
    "minecraft:adventure/trade_at_world_height": "星际商人",
    "minecraft:adventure/trim_with_all_exclusive_armor_patterns": "匠心独具",
    "minecraft:adventure/trim_with_any_armor_pattern": "旧貌锻新颜",
    "minecraft:adventure/two_birds_one_arrow": "一箭双雕",
    "minecraft:adventure/under_lock_and_key": "珍藏密敛",
    "minecraft:adventure/use_lodestone": "天涯共此石",
    "minecraft:adventure/very_very_frightening": "魔女审判",
    "minecraft:adventure/voluntary_exile": "自我放逐",
    "minecraft:adventure/walk_on_powder_snow_with_leather_boots": "轻功雪上飘",
    "minecraft:adventure/who_needs_rockets": "还要啥火箭啊？",
    "minecraft:adventure/whos_the_pillager_now": "现在谁才是掠夺者？",
    "minecraft:end/dragon_breath": "你需要来点薄荷糖",
    "minecraft:end/dragon_egg": "下一世代",
    "minecraft:end/elytra": "天空即为极限",
    "minecraft:end/enter_end_gateway": "远程折跃",
    "minecraft:end/find_end_city": "在游戏尽头的城市",
    "minecraft:end/kill_dragon": "解放末地",
    "minecraft:end/levitate": "这上面的风景不错",
    "minecraft:end/respawn_dragon": "结束了…再一次…",
    "minecraft:end/root": "末地",
    "minecraft:husbandry/allay_deliver_cake_to_note_block": "生日快乐歌",
    "minecraft:husbandry/allay_deliver_item_to_player": "找到一个好朋友",
    "minecraft:husbandry/axolotl_in_a_bucket": "最萌捕食者",
    "minecraft:husbandry/balanced_diet": "均衡饮食",
    "minecraft:husbandry/breed_all_animals": "成双成对",
    "minecraft:husbandry/breed_an_animal": "我从哪儿来？",
    "minecraft:husbandry/complete_catalogue": "百猫全书",
    "minecraft:husbandry/feed_snifflet": "小小嗅探兽",
    "minecraft:husbandry/fishy_business": "腥味十足的生意",
    "minecraft:husbandry/froglights": "相映生辉！",
    "minecraft:husbandry/kill_axolotl_target": "友谊的治愈力！",
    "minecraft:husbandry/leash_all_frog_variants": "呱呱队出动",
    "minecraft:husbandry/make_a_sign_glow": "眼前一亮！",
    "minecraft:husbandry/netherite_hoe": "终极奉献",
    "minecraft:husbandry/obtain_sniffer_egg": "怪味蛋",
    "minecraft:husbandry/place_dried_ghast_in_water": "补水保湿！",
    "minecraft:husbandry/plant_any_sniffer_seed": "播种往事",
    "minecraft:husbandry/plant_seed": "开荒垦地",
    "minecraft:husbandry/remove_wolf_armor": "华丽一剪",
    "minecraft:husbandry/repair_wolf_armor": "完好如初",
    "minecraft:husbandry/ride_a_boat_with_a_goat": "羊帆起航！",
    "minecraft:husbandry/root": "农牧业",
    "minecraft:husbandry/safely_harvest_honey": "与蜂共舞",
    "minecraft:husbandry/silk_touch_nest": "举巢搬迁",
    "minecraft:husbandry/tactical_fishing": "战术性钓鱼",
    "minecraft:husbandry/tadpole_in_a_bucket": "蚪到桶里来",
    "minecraft:husbandry/tame_an_animal": "永恒的伙伴",
    "minecraft:husbandry/uh_oh": "坏了",
    "minecraft:husbandry/wax_off": "脱蜡",
    "minecraft:husbandry/wax_on": "涂蜡",
    "minecraft:husbandry/whole_pack": "群狼聚首",
    "minecraft:nether/all_effects": "为什么会变成这样呢？",
    "minecraft:nether/all_potions": "狂乱的鸡尾酒",
    "minecraft:nether/brew_potion": "本地酿造厂",
    "minecraft:nether/charge_respawn_anchor": "锚没有九条命",
    "minecraft:nether/create_beacon": "带信标回家",
    "minecraft:nether/create_full_beacon": "信标工程师",
    "minecraft:nether/distract_piglin": "金光闪闪！",
    "minecraft:nether/explore_nether": "热门景点",
    "minecraft:nether/fast_travel": "曲速泡",
    "minecraft:nether/find_bastion": "光辉岁月",
    "minecraft:nether/find_fortress": "阴森的要塞",
    "minecraft:nether/get_wither_skull": "惊悚恐怖骷髅头",
    "minecraft:nether/loot_bastion": "战猪",
    "minecraft:nether/netherite_armor": "残骸裹身",
    "minecraft:nether/obtain_ancient_debris": "深藏不露",
    "minecraft:nether/obtain_blaze_rod": "与火共舞",
    "minecraft:nether/obtain_crying_obsidian": "谁在切洋葱？",
    "minecraft:nether/return_to_sender": "见鬼去吧",
    "minecraft:nether/ride_strider": "画船添足",
    "minecraft:nether/ride_strider_in_overworld_lava": "温暖如家",
    "minecraft:nether/root": "下界",
    "minecraft:nether/summon_wither": "凋零山庄",
    "minecraft:nether/uneasy_alliance": "脆弱的同盟",
    "minecraft:nether/use_lodestone": "天涯共此石",
    "minecraft:story/cure_zombie_villager": "僵尸科医生",
    "minecraft:story/deflect_arrow": "不吃这套，谢谢",
    "minecraft:story/enchant_item": "附魔师",
    "minecraft:story/enter_the_end": "结束了？",
    "minecraft:story/enter_the_nether": "勇往直下",
    "minecraft:story/follow_ender_eye": "隔墙有眼",
    "minecraft:story/form_obsidian": "冰桶挑战",
    "minecraft:story/iron_tools": "这不是铁镐么？",
    "minecraft:story/lava_bucket": "热腾腾的",
    "minecraft:story/mine_diamond": "钻石！",
    "minecraft:story/mine_stone": "石器时代",
    "minecraft:story/obtain_armor": "整装上阵",
    "minecraft:story/root": "Minecraft",
    "minecraft:story/shiny_gear": "钻石护体",
    "minecraft:story/smelt_iron": "来硬的",
    "minecraft:story/upgrade_tools": "获得升级",
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
    "minecraft:sandstone": "砂岩",
    "minecraft:cut_sandstone": "切制砂岩",
    "minecraft:smooth_sandstone": "平滑砂岩",
    "minecraft:chiseled_sandstone": "錾制砂岩",
    "minecraft:red_sandstone": "红砂岩",
    "minecraft:cut_red_sandstone": "切制红砂岩",
    "minecraft:smooth_red_sandstone": "平滑红砂岩",
    "minecraft:chiseled_red_sandstone": "錾制红砂岩",
    "minecraft:gravel": "砂砾",
    "minecraft:dirt": "泥土",
    "minecraft:coarse_dirt": "砂土",
    "minecraft:grass_block": "草方块",
    "minecraft:stone": "石头",
    "minecraft:cobblestone": "圆石",
    "minecraft:coal_ore": "煤矿石",
    "minecraft:iron_ore": "铁矿石",
    "minecraft:copper_ore": "铜矿石",
    "minecraft:gold_ore": "金矿石",
    "minecraft:diamond_ore": "钻石矿石",
    "minecraft:redstone_ore": "红石矿石",
    "minecraft:lapis_ore": "青金石矿石",
    "minecraft:emerald_ore": "绿宝石矿石",
    "minecraft:deepslate_coal_ore": "深层煤矿石",
    "minecraft:deepslate_iron_ore": "深层铁矿石",
    "minecraft:deepslate_copper_ore": "深层铜矿石",
    "minecraft:deepslate_gold_ore": "深层金矿石",
    "minecraft:deepslate_diamond_ore": "深层钻石矿石",
    "minecraft:deepslate_redstone_ore": "深层红石矿石",
    "minecraft:deepslate_lapis_ore": "深层青金石矿石",
    "minecraft:deepslate_emerald_ore": "深层绿宝石矿石",
    "minecraft:nether_quartz_ore": "下界石英矿石",
    "minecraft:ancient_debris": "远古残骸",
    "minecraft:bedrock": "基岩",
    "minecraft:deepslate": "深板岩",
    "minecraft:granite": "花岗岩",
    "minecraft:diorite": "闪长岩",
    "minecraft:andesite": "安山岩",
    "minecraft:ice": "冰",
    "minecraft:packed_ice": "浮冰",
    "minecraft:farmland": "耕地",
    "minecraft:dirt_path": "土径",
    "#minecraft:logs": "原木",
    "minecraft:oak_log": "橡木原木",
    "minecraft:spruce_log": "云杉原木",
    "minecraft:birch_log": "白桦原木",
    "minecraft:jungle_log": "丛林原木",
    "minecraft:acacia_log": "金合欢原木",
    "minecraft:dark_oak_log": "深色橡木原木",
    "minecraft:mangrove_log": "红树原木",
    "minecraft:cherry_log": "樱花原木",
    "minecraft:crimson_stem": "绯红菌柄",
    "minecraft:warped_stem": "诡异菌柄",
    "minecraft:oak_planks": "橡木木板",
    "minecraft:spruce_planks": "云杉木板",
    "minecraft:oak_door": "橡木门",
    "minecraft:oak_fence": "橡木栅栏",
    "minecraft:glass": "玻璃",
    "minecraft:glass_pane": "玻璃板",
    "minecraft:oak_leaves": "橡树树叶",
    "minecraft:rose_bush": "玫瑰丛",
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

# 中文名反查 id：模型说「挖石头」时用。
_BLOCK_BY_NAME = {name: ident for ident, name in BLOCK_NAMES.items()}

# 常见物品的中文名。玩家丢过来、喂过来的东西大多在这张表里；没收录的退回 id 后半段，
# 方块类还能借上面那张方块表（丢过来的「方块」在物品栏里只是一个物品）。
ITEM_NAMES = {
    "minecraft:apple": "苹果",
    "minecraft:golden_apple": "金苹果",
    "minecraft:enchanted_golden_apple": "附魔金苹果",
    "minecraft:bread": "面包",
    "minecraft:cookie": "曲奇",
    "minecraft:cake": "蛋糕",
    "minecraft:pumpkin_pie": "南瓜派",
    "minecraft:carrot": "胡萝卜",
    "minecraft:golden_carrot": "金胡萝卜",
    "minecraft:potato": "马铃薯",
    "minecraft:baked_potato": "烤马铃薯",
    "minecraft:poisonous_potato": "毒马铃薯",
    "minecraft:beetroot": "甜菜根",
    "minecraft:beetroot_soup": "甜菜汤",
    "minecraft:melon_slice": "西瓜片",
    "minecraft:sweet_berries": "甜浆果",
    "minecraft:glow_berries": "发光浆果",
    "minecraft:honey_bottle": "蜂蜜瓶",
    "minecraft:mushroom_stew": "蘑菇煲",
    "minecraft:rabbit_stew": "兔肉煲",
    "minecraft:suspicious_stew": "迷之炖菜",
    "minecraft:beef": "生牛肉",
    "minecraft:cooked_beef": "熟牛肉",
    "minecraft:porkchop": "生猪排",
    "minecraft:cooked_porkchop": "熟猪排",
    "minecraft:chicken": "生鸡肉",
    "minecraft:cooked_chicken": "熟鸡肉",
    "minecraft:mutton": "生羊肉",
    "minecraft:cooked_mutton": "熟羊肉",
    "minecraft:rabbit": "兔肉",
    "minecraft:cooked_rabbit": "熟兔肉",
    "minecraft:cod": "生鳕鱼",
    "minecraft:cooked_cod": "熟鳕鱼",
    "minecraft:salmon": "生鲑鱼",
    "minecraft:cooked_salmon": "熟鲑鱼",
    "minecraft:tropical_fish": "热带鱼",
    "minecraft:pufferfish": "河豚",
    "minecraft:bone": "骨头",
    "minecraft:leather": "皮革",
    "minecraft:string": "线",
    "minecraft:feather": "羽毛",
    "minecraft:flint": "燧石",
    "minecraft:gunpowder": "火药",
    "minecraft:slime_ball": "黏液球",
    "minecraft:clay_ball": "黏土球",
    "minecraft:brick": "红砖",
    "minecraft:nether_brick": "下界砖",
    "minecraft:paper": "纸",
    "minecraft:book": "书",
    "minecraft:written_book": "成书",
    "minecraft:name_tag": "命名牌",
    "minecraft:saddle": "鞍",
    "minecraft:shears": "剪刀",
    "minecraft:flint_and_steel": "打火石",
    "minecraft:fishing_rod": "钓鱼竿",
    "minecraft:compass": "指南针",
    "minecraft:clock": "钟",
    "minecraft:recovery_compass": "追溯指针",
    "minecraft:map": "地图",
    "minecraft:filled_map": "地图",
    "minecraft:spyglass": "望远镜",
    "minecraft:brush": "刷子",
    "minecraft:coal": "煤炭",
    "minecraft:charcoal": "木炭",
    "minecraft:raw_iron": "粗铁",
    "minecraft:iron_ingot": "铁锭",
    "minecraft:raw_copper": "粗铜",
    "minecraft:copper_ingot": "铜锭",
    "minecraft:raw_gold": "粗金",
    "minecraft:gold_ingot": "金锭",
    "minecraft:diamond": "钻石",
    "minecraft:emerald": "绿宝石",
    "minecraft:lapis_lazuli": "青金石",
    "minecraft:redstone": "红石",
    "minecraft:quartz": "下界石英",
    "minecraft:amethyst_shard": "紫水晶碎片",
    "minecraft:netherite_ingot": "下界合金锭",
    "minecraft:netherite_scrap": "下界合金碎片",
    "minecraft:echo_shard": "回响碎片",
    "minecraft:stick": "木棍",
    "minecraft:bowl": "碗",
    "minecraft:bucket": "桶",
    "minecraft:water_bucket": "水桶",
    "minecraft:lava_bucket": "岩浆桶",
    "minecraft:milk_bucket": "奶桶",
    "minecraft:powder_snow_bucket": "细雪桶",
    "minecraft:ender_pearl": "末影珍珠",
    "minecraft:ender_eye": "末影之眼",
    "minecraft:blaze_rod": "烈焰棒",
    "minecraft:blaze_powder": "烈焰粉",
    "minecraft:ghast_tear": "恶魂之泪",
    "minecraft:magma_cream": "岩浆膏",
    "minecraft:honeycomb": "蜜脾",
    "minecraft:turtle_scute": "海龟壳",
    "minecraft:phantom_membrane": "幻翼膜",
    "minecraft:rabbit_foot": "兔子脚",
    "minecraft:rabbit_hide": "兔子皮",
    "minecraft:iron_nugget": "铁粒",
    "minecraft:gold_nugget": "金粒",
    "minecraft:wooden_sword": "木剑",
    "minecraft:stone_sword": "石剑",
    "minecraft:iron_sword": "铁剑",
    "minecraft:golden_sword": "金剑",
    "minecraft:diamond_sword": "钻石剑",
    "minecraft:netherite_sword": "下界合金剑",
    "minecraft:wooden_pickaxe": "木镐",
    "minecraft:stone_pickaxe": "石镐",
    "minecraft:iron_pickaxe": "铁镐",
    "minecraft:golden_pickaxe": "金镐",
    "minecraft:diamond_pickaxe": "钻石镐",
    "minecraft:netherite_pickaxe": "下界合金镐",
    "minecraft:wooden_axe": "木斧",
    "minecraft:stone_axe": "石斧",
    "minecraft:iron_axe": "铁斧",
    "minecraft:golden_axe": "金斧",
    "minecraft:diamond_axe": "钻石斧",
    "minecraft:netherite_axe": "下界合金斧",
    "minecraft:wooden_shovel": "木锹",
    "minecraft:stone_shovel": "石锹",
    "minecraft:iron_shovel": "铁锹",
    "minecraft:golden_shovel": "金锹",
    "minecraft:diamond_shovel": "钻石锹",
    "minecraft:netherite_shovel": "下界合金锹",
    "minecraft:wooden_hoe": "木锄",
    "minecraft:stone_hoe": "石锄",
    "minecraft:iron_hoe": "铁锄",
    "minecraft:golden_hoe": "金锄",
    "minecraft:diamond_hoe": "钻石锄",
    "minecraft:netherite_hoe": "下界合金锄",
    "minecraft:bow": "弓",
    "minecraft:crossbow": "弩",
    "minecraft:arrow": "箭",
    "minecraft:spectral_arrow": "光灵箭",
    "minecraft:shield": "盾牌",
    "minecraft:trident": "三叉戟",
    "minecraft:totem_of_undying": "不死图腾",
    "minecraft:elytra": "鞘翅",
    "minecraft:firework_rocket": "烟花火箭",
    "minecraft:leather_helmet": "皮革帽子",
    "minecraft:leather_chestplate": "皮革外套",
    "minecraft:leather_leggings": "皮革裤子",
    "minecraft:leather_boots": "皮革靴子",
    "minecraft:chainmail_helmet": "锁链头盔",
    "minecraft:chainmail_chestplate": "锁链胸甲",
    "minecraft:chainmail_leggings": "锁链护腿",
    "minecraft:chainmail_boots": "锁链靴子",
    "minecraft:iron_helmet": "铁头盔",
    "minecraft:iron_chestplate": "铁胸甲",
    "minecraft:iron_leggings": "铁护腿",
    "minecraft:iron_boots": "铁靴子",
    "minecraft:golden_helmet": "金头盔",
    "minecraft:golden_chestplate": "金胸甲",
    "minecraft:golden_leggings": "金护腿",
    "minecraft:golden_boots": "金靴子",
    "minecraft:diamond_helmet": "钻石头盔",
    "minecraft:diamond_chestplate": "钻石胸甲",
    "minecraft:diamond_leggings": "钻石护腿",
    "minecraft:diamond_boots": "钻石靴子",
    "minecraft:netherite_helmet": "下界合金头盔",
    "minecraft:netherite_chestplate": "下界合金胸甲",
    "minecraft:netherite_leggings": "下界合金护腿",
    "minecraft:netherite_boots": "下界合金靴子",
    "minecraft:turtle_helmet": "海龟壳",
    "minecraft:wheat": "小麦",
    "minecraft:wheat_seeds": "小麦种子",
    "minecraft:melon_seeds": "西瓜种子",
    "minecraft:pumpkin_seeds": "南瓜种子",
    "minecraft:beetroot_seeds": "甜菜种子",
    "minecraft:bone_meal": "骨粉",
    "minecraft:sugar": "糖",
    "minecraft:egg": "鸡蛋",
    "minecraft:snowball": "雪球",
    "minecraft:cocoa_beans": "可可豆",
    "minecraft:ink_sac": "墨囊",
    "minecraft:glow_ink_sac": "荧光墨囊",
    "minecraft:rotten_flesh": "腐肉",
    "minecraft:spider_eye": "蜘蛛眼",
    "minecraft:fermented_spider_eye": "发酵蜘蛛眼",
    "minecraft:experience_bottle": "附魔之瓶",
    "minecraft:nether_star": "下界之星",
    "minecraft:heart_of_the_sea": "海洋之心",
    "minecraft:nautilus_shell": "鹦鹉螺壳",
}

# 车万女仆的工作模式（模组只给注册表 id，中文名在这里，取的是她界面里那一排的写法）。
# 顺序照她界面里的排列；枪械那条只有整合包里装了枪械模组时才存在。
TASK_NAMES = {
    "touhou_little_maid:idle": "空闲",
    "touhou_little_maid:attack": "攻击",
    "touhou_little_maid:ranged_attack": "弓兵",
    "touhou_little_maid:crossbow_attack": "弩兵",
    "touhou_little_maid:danmaku_attack": "弹幕攻击",
    "touhou_little_maid:trident_attack": "三叉戟攻击",
    "touhou_little_maid:gun_attack": "枪械",
    "touhou_little_maid:farm": "农场",
    "touhou_little_maid:sugar_cane": "甘蔗",
    "touhou_little_maid:melon": "瓜类",
    "touhou_little_maid:cocoa": "可可",
    "touhou_little_maid:honey": "采蜜",
    "touhou_little_maid:grass": "花草",
    "touhou_little_maid:snow": "清雪",
    "touhou_little_maid:feed": "喂食",
    "touhou_little_maid:shears": "剪刀",
    "touhou_little_maid:milk": "牛奶",
    "touhou_little_maid:torch": "火把",
    "touhou_little_maid:feed_animal": "繁殖动物",
    "touhou_little_maid:fishing": "钓鱼",
    "touhou_little_maid:extinguishing": "灭火",
    "touhou_little_maid:board_games": "游戏",
}

# 模型嘴里未必照抄上面那些名字（玩家说的是「种菜」「打怪」「别干活了」）。
# 同义词收在这里，只收真正常见的那几个；名字本身由 TASK_NAMES 兜住。
TASK_ALIASES = {
    "待命": "touhou_little_maid:idle",
    "休息": "touhou_little_maid:idle",
    "停工": "touhou_little_maid:idle",
    "别干活": "touhou_little_maid:idle",
    "种地": "touhou_little_maid:farm",
    "种田": "touhou_little_maid:farm",
    "种菜": "touhou_little_maid:farm",
    "农活": "touhou_little_maid:farm",
    "打怪": "touhou_little_maid:attack",
    "近战": "touhou_little_maid:attack",
    "战斗": "touhou_little_maid:attack",
    "弓箭": "touhou_little_maid:ranged_attack",
    "瓜": "touhou_little_maid:melon",
    "西瓜": "touhou_little_maid:melon",
    "南瓜": "touhou_little_maid:melon",
    "养蜂": "touhou_little_maid:honey",
    "蜂蜜": "touhou_little_maid:honey",
    "除草": "touhou_little_maid:grass",
    "拔草": "touhou_little_maid:grass",
    "扫雪": "touhou_little_maid:snow",
    "铲雪": "touhou_little_maid:snow",
    "投喂": "touhou_little_maid:feed",
    "喂我": "touhou_little_maid:feed",
    "剪羊毛": "touhou_little_maid:shears",
    "羊毛": "touhou_little_maid:shears",
    "挤奶": "touhou_little_maid:milk",
    "插火把": "touhou_little_maid:torch",
    "点火把": "touhou_little_maid:torch",
    "照明": "touhou_little_maid:torch",
    "养动物": "touhou_little_maid:feed_animal",
    "繁殖": "touhou_little_maid:feed_animal",
    "救火": "touhou_little_maid:extinguishing",
    "下棋": "touhou_little_maid:board_games",
    "五子棋": "touhou_little_maid:board_games",
    "捕鱼": "touhou_little_maid:fishing",
}

# 中文名反查 id：模型多半直接说「农场」「钓鱼」，而不是英文短名。
_TASK_BY_NAME = {name: ident for ident, name in TASK_NAMES.items()}

# 攻击目标的口语说法。owner 是主人自己——她可以真的打玩家，这是有意开放的。
ATTACK_ALIASES = {
    "owner": "owner",
    "master": "owner",
    "主人": "owner",
    "other_player": "other_player",
    "player": "other_player",
    "玩家": "other_player",
    "另一个玩家": "other_player",
    "另一个人": "other_player",
    "其他玩家": "other_player",
    "别的玩家": "other_player",
    "别人": "other_player",
    "hostile": "hostile",
    "monster": "hostile",
    "敌对": "hostile",
    "怪物": "hostile",
    "敌对生物": "hostile",
    "animal": "animal",
    "动物": "animal",
    "牲畜": "animal",
}

_MOB_BY_NAME = {name: ident for ident, name in MOB_NAMES.items()}

# 「挖这块」这类说法：不指定方块时挖玩家正看着的那一块。
MINE_LOOKED = frozenset(
    {"looked", "this", "here", "target", "这块", "这个", "这里", "这块方块", "面前", "看着的", "看着的那块"}
)

# 方块的口语说法。BLOCK_NAMES 里的中文名已经能反查，这里补矿石、原木这类常用但
# 不在「身边一圈」表里的说法。认不出的英文短名按原版方块放行，模组会如实报「附近没有」。
BLOCK_ALIASES = {
    # 泛称走方块标签：哪片林子就砍哪种原木，不再只认橡木
    "木头": "#minecraft:logs",
    "原木": "#minecraft:logs",
    "树干": "#minecraft:logs",
    "树木": "#minecraft:logs",
    "树": "#minecraft:logs",
    "橡木": "minecraft:oak_log",
    "云杉木": "minecraft:spruce_log",
    "白桦木": "minecraft:birch_log",
    "木板": "minecraft:oak_planks",
    "树叶": "minecraft:oak_leaves",
    "煤矿": "minecraft:coal_ore",
    "煤矿石": "minecraft:coal_ore",
    "铁矿": "minecraft:iron_ore",
    "铁矿石": "minecraft:iron_ore",
    "铜矿": "minecraft:copper_ore",
    "铜矿石": "minecraft:copper_ore",
    "金矿": "minecraft:gold_ore",
    "金矿石": "minecraft:gold_ore",
    "钻石矿": "minecraft:diamond_ore",
    "钻石矿石": "minecraft:diamond_ore",
    "钻石": "minecraft:diamond_ore",
    "红石矿": "minecraft:redstone_ore",
    "红石矿石": "minecraft:redstone_ore",
    "红石": "minecraft:redstone_ore",
    "青金石矿": "minecraft:lapis_ore",
    "青金石": "minecraft:lapis_ore",
    "绿宝石矿": "minecraft:emerald_ore",
    "绿宝石": "minecraft:emerald_ore",
    "深层铁矿": "minecraft:deepslate_iron_ore",
    "深层金矿": "minecraft:deepslate_gold_ore",
    "深层钻石矿": "minecraft:deepslate_diamond_ore",
    "下界石英": "minecraft:nether_quartz_ore",
    "石英矿": "minecraft:nether_quartz_ore",
    "远古残骸": "minecraft:ancient_debris",
    "基岩": "minecraft:bedrock",
}

# 攻击力度。默认是追着打倒；「轻轻打一下」只出一手、伤害固定，用来逗她抬手。
ATTACK_STYLES = {
    "light": "light",
    "tap": "light",
    "gentle": "light",
    "轻轻": "light",
    "轻轻打": "light",
    "轻打": "light",
    "打一下": "light",
    "拍一下": "light",
    "碰一下": "light",
    "fight": "fight",
    "full": "fight",
    "追着打": "fight",
    "打倒": "fight",
}

# 死亡与昏迷状态的标记。写在状态旁白里，插件靠台词表里最后一条这样的旁白判断她还在不在
# 这个状态（插件没有跨调用状态，历史本身就是状态）。昏迷不是死亡，醒来也不等于复活。
DEAD_STATE_MARK = "现在在MC里处于死亡状态"
REVIVE_STATE_MARK = "已经在MC里复活了"
UNCONSCIOUS_STATE_MARK = "现在在MC里处于昏迷状态"
RECOVERED_STATE_MARK = "已经从昏迷中醒过来了"

# 「近」与「中」的分界（格）。再远一律说「远」。
NEAR_CLOSE = 5.0
NEAR_MID = 10.0

# 附近那只算哪一类。有自己名字的生物直接用名字，所以 living 不带称呼。
CATEGORY_NAMES = {"player": "另一个人", "maid": "另一位女仆", "living": ""}

# 说不清是谁打的时候，用伤害类型兜个底（模组给的是伤害类型的 msg id）。
# 措辞接在名字后面读：主人在旁边、生物点名都轮不上时才用得到这一张表。
DAMAGE_HINTS = {
    # 环境
    "fall": "从高处摔了下来",
    "fly_into_wall": "撞到了墙上",
    "in_wall": "被墙磕了一下",
    "cramming": "被挤得喘不上气",
    "lava": "碰到了岩浆",
    "hot_floor": "踩到了岩浆块",
    "in_fire": "被火烧到了",
    "on_fire": "身上着火了",
    "campfire": "被营火烧了",
    "drown": "溺水了",
    "starve": "饿得不行",
    "cactus": "被仙人掌扎了",
    "sweet_berry_bush": "被甜浆果丛扎了",
    "freeze": "被冻伤了",
    "lightning_bolt": "被雷劈了",
    "out_of_world": "掉进了虚空",
    "outside_border": "被世界边界推挤",
    # 掉落物与投掷物
    "falling_block": "被掉下来的方块砸了",
    "falling_anvil": "被铁砧砸了",
    "falling_stalactite": "被钟乳石砸了",
    "stalagmite": "被石笋扎了",
    "arrow": "中了箭",
    "trident": "被三叉戟扎了",
    "thrown": "被丢过来的东西砸了",
    "fireball": "被火球砸了",
    "fireworks": "被烟花炸了",
    "sting": "被蜂蛰了",
    "explosion": "被炸了",
    "explosion.player": "被炸了",
    "bad_respawn_point": "被炸了",
    # 魔法与特殊攻击
    "magic": "被魔法击中",
    "indirect_magic": "被魔法击中",
    "wither": "中了凋零",
    "wither_skull": "被凋零头炸了",
    "dragon_breath": "被龙息烧了",
    "sonic_boom": "被音波轰了",
    "thorns": "被反伤扎了",
}


def name_of(names, raw, default="未知的地方"):
    """游戏 id → 中文名；没收录的退回到 id 的后半段（英文），模型也认。"""
    identifier = (raw or "").strip()
    if not identifier:
        return default
    return names.get(identifier) or identifier.split(":")[-1].replace("_", " ")


def blocker_phrase(one):
    """关系里的「隔着两块石头」：挡路的块数与第一种挡路的方块；没挡着给空串。"""
    count = as_int(one.get("blockers"), 0)
    if count <= 0:
        return ""
    blocker = name_of(BLOCK_NAMES, one.get("blocker"), "方块")
    return f"隔着{count}块{blocker}"


def item_name(raw):
    """物品 id → 中文名。方块表也兜一层：丢过来的方块在物品栏里只是一个物品。"""
    identifier = (raw or "").strip()
    if not identifier:
        return "一件东西"
    fallback = identifier.split(":")[-1].replace("_", " ")
    return ITEM_NAMES.get(identifier) or BLOCK_NAMES.get(identifier) or fallback


def advancement_name(raw, title=""):
    """成就 id → 中文名。

    原版的按 id 查表；模组加的先用它自己给的游戏内标题（英文也认），实在没有才退回
    id 后半段。标题若是没被翻译的裸键（advancements.xxx.title）就不算数——那比 id 还难读。
    """
    identifier = (raw or "").strip()
    known = ADVANCEMENT_NAMES.get(identifier)
    if known:
        return known
    text = (title or "").strip()
    if text and not text.startswith("advancements."):
        return text
    return identifier.split("/")[-1].replace("_", " ") or "新成就"


def who_of(category, type_id):
    """附近那只算谁：类别有专门称呼的（另一个玩家、别的女仆）用它，其余用生物名。"""
    return CATEGORY_NAMES.get(category or "") or name_of(MOB_NAMES, type_id, "什么东西")


def task_id(raw):
    """工作模式：中文名、同义词或注册表 id 都换成 id；认不出给空串。

    带命名空间的写法原样放行——模组或附属包加的模式不在这张表里，有没有交给游戏判，
    插件不在这一层替它拦。
    """
    text = (raw or "").strip()
    if not text:
        return ""
    lowered = text.lower()
    if lowered in TASK_NAMES:
        return lowered
    short = "touhou_little_maid:" + lowered
    if short in TASK_NAMES:
        return short
    alias = TASK_ALIASES.get(text) or _TASK_BY_NAME.get(text)
    if alias:
        return alias
    if ":" in lowered and " " not in lowered:
        return lowered
    return ""


def task_name(raw, summary=""):
    """工作模式 id → 中文名。表里没有的用模组给的英文摘要兜底，再不行退回 id 后半段。"""
    identifier = (raw or "").strip().lower()
    known = TASK_NAMES.get(identifier) or TASK_NAMES.get("touhou_little_maid:" + identifier)
    if known:
        return known
    text = (summary or "").strip()
    if text and not text.startswith("task."):
        return text
    return identifier.split(":")[-1].replace("_", " ") or "新工作"


def task_choices():
    """认不出工作模式时列给模型看的那一串：中文名（短 id）。"""
    return "、".join(f"{name}（{ident.split(':')[-1]}）" for ident, name in TASK_NAMES.items())


def attack_style(raw):
    """攻击力度：认不出的按默认（追着打）。轻轻一下得玩家明说。"""
    text = (raw or "").strip().lower()
    return ATTACK_STYLES.get(text, "fight")


def attack_target(raw):
    """攻击目标：活物的中文名、英文短名、完整 id、另一个玩家的名字，
    或 owner / other_player / hostile / animal。

    owner 是她主人（她可以真的打玩家）；other_player 是另一个玩家，hostile / animal
    让模组按生物类别挑最近的一只。另一个玩家在感知里只有名字，所以这里不把认不出的
    英文写法硬补成 minecraft: ——模组会先按实体 id 认，再按玩家名认。
    认不出的给空串，由工具报错——瞎猜一个目标打出去比报错更糟。
    """
    text = (raw or "").strip()
    if not text:
        return ""
    lowered = text.lower()
    alias = ATTACK_ALIASES.get(lowered) or ATTACK_ALIASES.get(text)
    if alias:
        return alias
    # 模型可能把感知里的整段称呼抄回来：「另一个玩家 Layeray」只取名字
    for prefix in ("另一个玩家", "别的玩家", "其他玩家", "玩家"):
        if text.startswith(prefix) and text[len(prefix):].strip():
            text = text[len(prefix):].strip()
            lowered = text.lower()
            break
    known = _MOB_BY_NAME.get(text)
    if known:
        return known
    if ":" in lowered and " " not in lowered:
        return lowered
    if lowered.isascii() and lowered.replace("_", "").isalnum():
        # 英文短名与玩家名都原样交给模组：它先试 minecraft:<名字>，再试玩家名
        return lowered
    return ""


# 模型可能把「看一眼周围」里的「中文名（id）」整段抄回来，也可能把 id 夹在解释里。
# 认方块名之前先抠一次 id，比让它整段报错好。这里手写扫一遍而不用 re：插件脚本跑在
# 2MB 栈的阻塞线程上，沿 bridge → wording → re 再进 re 自己的导入，层数一深就爆栈。
_ID_CHARS = frozenset("#abcdefghijklmnopqrstuvwxyz0123456789_/:")


def _identifier_in(text):
    """从一段话里抠出第一个 `namespace:id`（可带 `#` 标签前缀），找不到给空串。"""
    run = []
    for ch in text + " ":
        if ch in _ID_CHARS:
            run.append(ch)
            continue
        token = "".join(run)
        run = []
        namespace, _, path = token.partition(":")
        if namespace and path:
            return token
    return ""


def mine_target(raw):
    """挖掘目标：方块的中文名、英文 id、方块标签，或 looked（玩家准星指着的方向）。

    空字符串按 looked 处理：玩家说「把这块挖了」时模型多半不给 target。认不出的中文
    说法给空串，由工具报错并列出几个例子——瞎猜一个方块挖出去比报错更糟。
    """
    text = (raw or "").strip()
    lowered = text.lower()
    if not text or lowered in MINE_LOOKED or text in MINE_LOOKED:
        return "looked"
    alias = BLOCK_ALIASES.get(text) or BLOCK_ALIASES.get(lowered)
    if alias:
        return alias
    known = _BLOCK_BY_NAME.get(text)
    if known:
        return known
    found = _identifier_in(lowered)
    if found:
        return found
    if ":" in lowered and " " not in lowered:
        return lowered
    spaced = lowered.replace(" ", "_").replace("-", "_")
    if spaced.isascii() and spaced.replace("_", "").isalnum():
        # 英文短名按原版方块认；空格与连字符按 id 的写法归一（rose bush → rose_bush）
        return "minecraft:" + spaced
    return ""


def mine_choices():
    """认不出方块时列给模型看的例子。"""
    return "石头、圆石、泥土、沙子、木头（任意原木）、铁矿（浅层与深层都算）、钻石矿，或者 looked"


_ITEM_BY_NAME = {name: ident for ident, name in ITEM_NAMES.items()}


def item_target(raw):
    """要交给玩家的物品：中文名、英文 id，或夹在说明里的 id。认不出给空串。

    方块名也认（石头、橡木原木这类在物品栏里就是方块本身）；只有标签（木头）不行，
    交东西得是具体某一件。
    """
    text = (raw or "").strip()
    lowered = text.lower()
    if not text:
        return ""
    known = _ITEM_BY_NAME.get(text)
    if not known:
        block = _BLOCK_BY_NAME.get(text)
        known = block if block and not block.startswith("#") else ""
    if known:
        return known
    found = _identifier_in(lowered)
    if found:
        return found
    if ":" in lowered and " " not in lowered:
        return lowered
    spaced = lowered.replace(" ", "_").replace("-", "_")
    if spaced.isascii() and spaced.replace("_", "").isalnum():
        return "minecraft:" + spaced
    return ""


def item_choices():
    """认不出物品时列给模型看的例子。"""
    return "苹果、面包、熟牛肉、铁镐、钻石剑、火把"


def sighting(one):
    """一份看得见的附近实体记录 → 「近处的僵尸」。

    只说是什么、大概多远，不说方位，也不给格数：模型会把旁白里的方位与数字照样
    复述出来（「左前方 10.5 格有只蜘蛛」），而玩家并不需要知道这些。
    被挡住的实体不走这句：看不见就只剩声音，统一由调用方汇总成「听到…的声音」。
    """
    who = nearby_name(one)
    return f"{nearness_place(one.get('distance'))}的{who}"


def nearby_name(one):
    """身边那只活物叫什么：玩家用他在 MC 里的名字（她只叫得出这个），其余用生物名。

    别的活物的显示名在专用服务端上是翻译键，所以协议里只有玩家带 name 字段。玩家的名字
    前面缀一句「另一个玩家」，模型才分得清这是人而不是某种生物。
    """
    if (one.get("category") or "") == "player":
        name = (one.get("name") or "").strip()
        if name:
            return f"另一个玩家 {name}"
    return who_of(one.get("category"), one.get("type"))


def heard_sounds(ones):
    """看不见的活物 → 「僵尸和蜘蛛的声音」；同一类只念一次。

    隔墙报「被挡着的僵尸」既生硬又像透视；她真正拥有的信息只是「那边有声音」。
    认不出的类型退回 id 后半段，与别处一致。
    """
    names = []
    for one in ones:
        name = nearby_name(one)
        if name not in names:
            names.append(name)
    return f"{'和'.join(names)}的声音" if names else ""


def nearness(distance):
    """距离 → 「近」/「中」/「远」。档位对着模组的感知半径（默认 12 格）定：
    贴身与一臂之内算近，走到跟前要几步算中，剩下那一圈算远。给不出数字就当「中」。"""
    try:
        value = float(distance)
    except (TypeError, ValueError):
        return "中"
    if value <= NEAR_CLOSE:
        return "近"
    if value <= NEAR_MID:
        return "中"
    return "远"


def nearness_place(distance):
    """同上，但给的是「近处」/「不远处」/「远处」这种能直接塞进句子的说法。"""
    return {"近": "近处", "中": "不远处", "远": "远处"}[nearness(distance)]


# 相对她身体朝向的方位说法。模组给 front / front_left 这类 token，这里只翻中文。
DIRECTION_NAMES = {
    "front": "正前方",
    "front_right": "右前方",
    "right": "右边",
    "back_right": "右后方",
    "back": "背后",
    "back_left": "左后方",
    "left": "左边",
    "front_left": "左前方",
}


# 光线跨档的旁白说法。档位由模组判定（SurroundingsDiff.lightBand），这里只负责中文。
LIGHT_PLACES = {"dark": "很黑的地方", "dim": "光线昏暗的地方", "bright": "光线明亮的地方"}

# 头顶那块遮挡物离她多高。模组只给 low / high 两档（OverheadScan），不报具体格数。
OVERHEAD_PLACES = {"low": "头顶", "high": "头顶很高处"}


def as_int(value, default=0):
    """配置与事件里的数字可能是浮点（前端表单、JSON）或字符串。"""
    try:
        return int(float(value))
    except (TypeError, ValueError):
        return default


# ============================================================
# 情绪 → 颜文字
# ============================================================

# 宿主给的情绪是分类器的 19 个标签，提示词放开时还会出现「尴尬 / 无语 / 情动」这类写法，
# 所以表比分类器宽一档；认不出的情绪退回「正常」。
#
# 挑哪一条交给模组随机（它有自己的随机源，插件这边不依赖 random 模块）。这里只做映射。
KAOMOJI = {
    "正常": ["(・_・)", "(-_-)", "(´・ω・`)"],
    "平静": ["(￣ー￣)", "(・ω・)", "(˘ω˘)"],
    "高兴": ["(๑•̀ㅂ•́)و✧", "ヽ(✿ﾟ▽ﾟ)ノ", "(≧▽≦)"],
    "兴奋": ["٩(๑>◡<๑)۶", "o(≧▽≦)o", "(๑>؂<๑)"],
    "心动": ["(⁄ ⁄•⁄ω⁄•⁄ ⁄)", "(〃▽〃)", "(*/ω＼*)"],
    "情动": ["(⁄ ⁄•⁄ω⁄•⁄ ⁄)", "(〃▽〃)", "(*/ω＼*)"],
    "害羞": ["(*/ω＼*)", "(〃▽〃)", "(灬ºωº灬)"],
    "难为情": ["(⁄ ⁄>⁄ ▽ ⁄<⁄ ⁄)", "(´,,•ω•,,)", "(〃＞＿＜;〃)"],
    "生气": ["(╬ Ò﹏Ó)", "(＃`Д´)", "(╯°□°）╯"],
    "厌恶": ["(￣ヘ￣)", "(눈_눈)", "(-_-メ)"],
    "无语": ["(ーー;)", "(・_・;)", "(￣ω￣;)"],
    "无奈": ["(´-ω-`)", "(￣▽￣)", "(°ー°〃)"],
    "尴尬": ["(￣▽￣;)", "(・_・;)", "(ー_ーゞ"],
    "慌张": ["(°Д°)", "Σ(°△°|||)", "(ﾟДﾟ;)"],
    "紧张": ["(；´Д`)", "(>_<)", "(ﾉД`)"],
    "担心": ["(´・_・`)", "(；・∀・)", "(´；ω；`)"],
    "害怕": ["(ﾟдﾟ;)", "((ﾟДﾟ;))", "(；ﾟДﾟ)"],
    "惊讶": ["(°ロ°)", "(⊙_⊙)", "Σ(ﾟДﾟ)"],
    "疑惑": ["(・_・?)", "(￣～￣;)", "(⊙_⊙)?"],
    "认真": ["(｀・ω・´)", "(•̀ᴗ•́)و", "(￣^￣)"],
    "自信": ["(￣^￣)ゞ", "(・∀・)ノ", "(๑•̀ㅂ•́)و"],
    "调皮": ["(≖ᴗ≖)✧", "(￣ω￣)", "(๑•̀ㅂ•́)✧"],
    "哭泣": ["(´；ω；`)", "｡ﾟ(ﾟ´ω`ﾟ)ﾟ｡", "(T_T)"],
    "伤心": ["(´；ω；`)", "｡ﾟ(ﾟ´ω`ﾟ)ﾟ｡", "(T_T)"],
}


def kaomoji_for(emotion):
    """某个情绪的颜文字候选；认不出的情绪按「正常」给。返回给模组去随机挑一条。"""
    return KAOMOJI.get(emotion) or KAOMOJI["正常"]
