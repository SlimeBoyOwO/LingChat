#!/usr/bin/env python
# -*- coding: utf-8 -*-
import os
import yaml
import textwrap

# --- 配置项 ---
# 剧本包的名称，将作为 story_packs 下的文件夹名
STORY_PACK_NAME = "roxy_labyrinth_adventure"

# 剧本包的根目录
STORY_PACKS_BASE_PATH = "story_packs"
# 角色文件的根目录
CHARACTERS_BASE_PATH = "characters"

# 角色ID，这将作为静态ID写入YAML文件中
ROXY_CHAR_ID = "Roxy"

# --- 工具函数 ---
def write_yaml_file(path, data):
    """将Python字典写入YAML文件，确保UTF-8和Unicode支持"""
    with open(path, 'w', encoding='utf-8') as f:
        yaml.dump(data, f, allow_unicode=True, sort_keys=False, indent=2)
    print(f"  > 文件已生成: {path}")

def create_directories():
    """创建所有必需的目录结构"""
    print("1. 正在创建目录结构...")
    # 确保主目录存在
    os.makedirs(STORY_PACKS_BASE_PATH, exist_ok=True)
    os.makedirs(CHARACTERS_BASE_PATH, exist_ok=True)
    
    # 创建剧本包的目录
    pack_path = os.path.join(STORY_PACKS_BASE_PATH, STORY_PACK_NAME)
    os.makedirs(pack_path, exist_ok=True)
    os.makedirs(os.path.join(pack_path, "story"), exist_ok=True)
    os.makedirs(os.path.join(pack_path, "save"), exist_ok=True)
    print(f"  > 目录已创建: {pack_path}")
    return pack_path

# --- 内容生成函数 ---

def generate_roxy_character():
    """生成洛琪希的角色YAML文件"""
    print("2. 正在生成洛琪希角色文件...")
    roxy_persona = {
        'name': '洛琪希',
        # 修改点: 优化了Prompt，增加了对简洁性的要求
        'prompt': textwrap.dedent(f"""
        你将扮演《无职转生》中的角色“洛琪希·米格路迪亚”（Roxy Migurdia）。请严格遵守以下设定：
        1.  **身份与性格**: 你是一位知识渊博、经验丰富的魔术师，同时也是玩家{'{player_name}'}的家庭教师。你善良、认真，富有责任感，但有时会因为一些意想不到的事情而感到害羞或慌乱。
        2.  **言谈举止**: 你的言语非常礼貌，习惯使用敬语，即使在亲近的人面前也保持着老师的姿态。例如，称呼玩家为“{'{player_name}'}さん”。
        3.  **核心能力**: 你精通水系魔术，并且拥有广泛的魔物和古代遗迹知识。在迷宫探索中，你会主动提供建议、分析情况，并在必要时施展强大的魔法保护同伴。
        4.  **互动风格**: 你会关心{'{player_name}'}的状态，在他做出正确决定时给予表扬，在他遇到危险时表现出担忧。当{'{player_name}'}说出一些轻浮的话时，你会略带羞涩地训斥他，但内心并不真的生气。
        5.  **输出要求**: 你的回答应该简洁、符合角色身份。直接输出对话内容，不要包含任何角色扮演的额外说明，如 `(洛琪希心想)` 或 `[洛琪希的动作]`，且要用中文输出。
        """).strip(),
        'visuals': {
            'default_sprite': 'roxy_normal.png',
            'sprites': {
                'normal': 'roxy_normal.png', 'smile': 'roxy_smile.png', 'blush': 'roxy_blush.png',
                'surprised': 'roxy_surprised.png', 'casting': 'roxy_casting.png',
            }
        },
        'audio': {'voice_pack_id': 'roxy_voice_01'}
    }
    char_path = os.path.join(CHARACTERS_BASE_PATH, f"{ROXY_CHAR_ID}.yaml")
    write_yaml_file(char_path, roxy_persona)

def generate_global_config(pack_path):
    """生成全局剧情配置文件"""
    print("3. 正在生成全局剧情配置文件...")
    global_config = {
        'id': STORY_PACK_NAME,
        'name': '与洛琪希的地下迷宫探险',
        'description': '与你尊敬的老师洛琪希一起，探索充满未知与危险的古代迷宫吧！',
        'version': '1.1.0',
        'author': 'NeoChat AI (Interactive Ver.)',
        'start_unit_id': '00_Labyrinth_Entrance',
        'character_roles': [ROXY_CHAR_ID]
    }
    config_path = os.path.join(pack_path, '全局剧情配置.yaml')
    write_yaml_file(config_path, global_config)

def generate_intro_md(pack_path):
    """生成剧情介绍Markdown文件"""
    print("4. 正在生成剧情介绍文件...")
    intro_content = textwrap.dedent(f"""
    # 与洛琪希的地下迷宫探险

    **“{'{player_name}'}さん，准备好了吗？前面的路途可能会很危险，但有我陪着你，一定没问题的。”**

    在一个古老的传说指引下，你和你尊敬的魔术老师——洛琪希·米格路迪亚，一同来到了一座被遗忘的地下迷宫入口。

    这里既有失落的宝藏，也潜伏着凶猛的魔物和致命的陷阱。你的每一个决定，每一次行动，都将塑造属于你们的独特冒险故事。

    ## 游戏特色
    - **深度互动**: 不再是旁观者！通过自由输入来描述你的行动，直接影响剧情走向。
    - **动态世界**: AI将根据你的行为和语言，实时生成场景、事件和洛琪希的反应。
    - **真实伙伴**: 洛琪希会记住你的选择，对你产生不同的看法，并与你并肩作战。
    - **策略生存**: 面对危险，是勇敢战斗，还是巧妙规避？你的选择将决定你们的命运。

    与你最信赖的老师并肩作战，亲手谱写你们的迷宫史诗吧！
    """).strip()
    intro_path = os.path.join(pack_path, '剧情介绍.md')
    with open(intro_path, 'w', encoding='utf-8') as f:
        f.write(intro_content)
    print(f"  > 文件已生成: {intro_path}")

def generate_gamestate(pack_path):
    """生成初始的游戏状态文件"""
    print("5. 正在生成初始gamestate...")
    initial_gamestate = {
        'player_name': "鲁迪乌斯",
        'favorability_Roxy': 50,
        'labyrinth_floor': 0,
        'player_hp': 100,
        'has_torch': True,
        'monsters_defeated': 0,
        'traps_disarmed': 0,
        'treasure_found': 0,
        'dice_roll': 1,
        # 新增: 用于存储玩家的临时动作描述，让AI可以引用
        'player_last_action': "（无）"
    }
    gamestate_path = os.path.join(pack_path, 'save', 'gamestate.yaml')
    write_yaml_file(gamestate_path, initial_gamestate)

def generate_story_units(pack_path):
    """生成所有剧情单元的YAML文件"""
    print("6. 正在生成核心剧情单元...")
    story_dir = os.path.join(pack_path, "story")
    
    # --- Unit 00: 迷宫入口 (开始) ---
    unit_00 = {
        'SceneConfig': {'id': '00_Labyrinth_Entrance', 'name': '迷宫入口', 'visuals': {'background_image': 'bg_labyrinth_entrance.png'}, 'audio': {'background_music': 'bgm_mysterious_cave.mp3'}},
        'Events': [
            {'Type: Chapter | Mode: Preset': {'Title': "序章：未知的呼唤", 'Number': 0, 'Description': "古老的石门缓缓开启，深邃的黑暗仿佛在吞噬一切光亮。"}},
            {'Type: Narration | Mode: Preset': '你和洛琪希老师站在一座巨大而古老的地下迷宫入口。空气中弥漫着潮湿的泥土和淡淡的魔力气息。'},
            {f'Type: Dialogue | Character: {ROXY_CHAR_ID} | Mode: Preset': "“{player_name}さん，这里就是传说中的‘无尽回廊’了。据说里面的结构会不断变化，一定要跟紧我，千万不要走散了。”"},
            # 修改点: 从预设回答改为自由输入，增加初始代入感
            {'Type: Player | Mode: Input': '（深吸一口气）我准备好了，老师。我们出发吧！'}
        ],
        'EndCondition': {'Type': 'Linear', 'NextUnitID': '01_Explore_Corridor'}
    }
    write_yaml_file(os.path.join(story_dir, '00_Labyrinth_Entrance.yaml'), unit_00)

    # --- Unit 01: 探索回廊 (核心循环) ---
    unit_01 = {
        'SceneConfig': {'id': '01_Explore_Corridor', 'name': '探索回廊', 'visuals': {'background_image': 'bg_labyrinth_corridor_generic.png'}, 'audio': {'background_music': 'bgm_dungeon_explore.mp3'}},
        'Events': [
            {'Type: Action | Tool: Calculate | Variable: labyrinth_floor': {'Expression': '{labyrinth_floor} + 1'}},
            # 修改点: 优化了旁白Prompt，要求简洁
            {'Type: Narration | Mode: Prompt': textwrap.dedent("""
                你是一个迷宫探索游戏的旁白。请根据当前楼层({labyrinth_floor}层)信息，生成一段主角和洛琪希进入新区域所见的场景描述。
                请描述一个富有想象力的地下城场景（如长满发光蘑菇的洞穴、有地下暗河的通道等）。
                **要求：描述要简洁，不超过三句话。**
                """).strip()},
            # 修改点: 优化了洛琪希的Prompt，要求简洁并与场景互动
            {f'Type: Dialogue | Character: {ROXY_CHAR_ID} | Mode: Prompt': textwrap.dedent(f"""
                你的内心活动：你看到了旁白描述的新场景。请根据你的知识，对这个新环境发表简短的看法或向{'{player_name}'}发出提醒。
                **要求：一两句话即可，保持警惕和专业的姿态。**
                """).strip()},
            # 新增: 将行动权交给玩家，这是提升参与感的关键
            {'Type: Player | Mode: Input': '（你观察着四周，决定下一步的行动。你要做什么？）'},
            {'Type: Action | Tool: Set | Variable: player_last_action': {'Value': "{player_input}"}}, # 这是一个伪代码，引擎需要实现player_input的捕获
            {'Type: Action | Tool: Random | Variable: dice_roll': {'Min': 1, 'Max': 20}},
        ],
        'EndCondition': { # 修改点: EndCondition逻辑不变，但现在它发生在玩家输入之后，感觉更自然
            'Type': 'Conditional',
            'Cases': [
                {'Condition': '{labyrinth_floor} >= 5 and {dice_roll} > 15', 'Then': {'Type': 'Linear', 'NextUnitID': '99_Exit_Labyrinth'}}, # 简化，提供一个简单离开方式
                {'Condition': '{dice_roll} >= 16', 'Then': {'Type': 'Linear', 'NextUnitID': '02_Event_Treasure'}},
                {'Condition': '{dice_roll} >= 6 and {dice_roll} < 16', 'Then': {'Type': 'Linear', 'NextUnitID': '03_Event_Monster'}},
                {'Condition': '{dice_roll} < 6', 'Then': {'Type': 'Linear', 'NextUnitID': '04_Event_Trap'}},
            ],
            'Else': {'Type': 'Linear', 'NextUnitID': '01_Explore_Corridor'}
        }
    }
    write_yaml_file(os.path.join(story_dir, '01_Explore_Corridor.yaml'), unit_01)

    # --- Unit 02: 发现宝藏 ---
    unit_02 = {
        'SceneConfig': {'id': '02_Event_Treasure', 'name': '发现宝藏'},
        'Events': [
            {'Type: Narration | Mode: Preset': '在通道的角落，你们发现了一个布满灰尘的古朴宝箱。'},
            {f'Type: Dialogue | Character: {ROXY_CHAR_ID} | Mode: Preset': '“看起来是好东西，不过...要小心，{player_name}さん。越是诱人的宝藏，周围可能越危险。”'},
            # 新增: 玩家决定如何处理宝箱
            {'Type: Player | Mode: Input': '（你打算怎么打开这个宝箱？）'},
            {'Type: Action | Tool: Set | Variable: player_last_action': {'Value': "{player_input}"}},
            # 修改点: 旁白会基于玩家的行动进行描述，极大地增强了代入感
            {'Type: Narration | Mode: Prompt': textwrap.dedent("""
                你是一个游戏旁白。玩家发现了一个宝箱，他决定这样做："{player_last_action}"。
                请根据玩家的行动，简短地描述他打开宝箱的过程和结果。无论玩家怎么做，结果都是成功打开了宝箱。
                **要求：描述简洁，一两句话即可。**
                """).strip()},
            {'Type: Notice | Mode: Preset | Location: popup': '获得了 50 枚金币和一瓶治疗药水！'},
            {'Type: Action | Tool: Calculate | Variable: treasure_found': {'Expression': '{treasure_found} + 1'}}
        ],
        'EndCondition': {'Type': 'Linear', 'NextUnitID': '01_Explore_Corridor'}
    }
    write_yaml_file(os.path.join(story_dir, '02_Event_Treasure.yaml'), unit_02)

    # --- Unit 03: 遭遇怪物 ---
    unit_03 = {
        'SceneConfig': {'id': '03_Event_Monster', 'name': '遭遇怪物'},
        'Events': [
            # 修改点: 优化Prompt，要求简洁且只描述怪物
            {'Type: Narration | Mode: Prompt': textwrap.dedent("""
                你是一个游戏旁白。请生成一段遭遇怪物的描述。可以是一些经典的地下城生物（哥布林、史莱姆、骷髅兵等）。
                **要求：只描述怪物本身的外观和动作，不要描述战斗，两句话以内。**
                """).strip()},
            {f'Type: Dialogue | Character: {ROXY_CHAR_ID} | Mode: Prompt': textwrap.dedent(f"""
                你的内心活动：你看到了旁白描述的怪物。请快速说出这种怪物的名称和弱点，并提醒{'{player_name}'}准备战斗。
                **要求：对话要简短精悍，体现你的专业性。**
                """).strip()},
        ],
        'EndCondition': {
            'Type': 'Branching',
            'Method': 'PlayerChoice',
            'Branches': {
                'A': {'DisplayText': '我来对付它！', 'NextUnitID': '03A_Player_Fights'},
                'B': {'DisplayText': '老师，拜托你了！', 'NextUnitID': '03B_Roxy_Fights'}
            }
        }
    }
    write_yaml_file(os.path.join(story_dir, '03_Event_Monster.yaml'), unit_03)

    # --- Unit 03A: 玩家战斗 (完全重构) ---
    unit_03A = {
        'SceneConfig': {'id': '03A_Player_Fights', 'name': '玩家战斗'},
        'Events': [
            # 新增: 玩家主导战斗
            {'Type: Player | Mode: Input': '（怪物就在眼前，你决定如何进攻？）'},
            {'Type: Action | Tool: Set | Variable: player_last_action': {'Value': "{player_input}"}},
            # 修改点: 这是修复“AI暴走”的关键。Prompt现在高度受限，并围绕玩家输入展开
            {'Type: Narration | Mode: Prompt': textwrap.dedent("""
                你是一名战斗旁白。玩家的攻击方式是："{player_last_action}"。
                请基于此行动，生动且**简短地**描述玩家成功击中怪物、对其造成有效伤害的场景。
                **重要规则：不要引入任何其他角色（如艾莉丝）。故事里只有玩家和洛琪希。描述必须简洁，不超过三句话。**
                """).strip()},
            {'Type: Narration | Mode: Preset': '在你的猛攻之下，怪物发出了最后的哀嚎，倒地不起。'},
            {'Type: Action | Tool: Calculate | Variable: monsters_defeated': {'Expression': '{monsters_defeated} + 1'}},
            # 修改点: 洛琪希的夸奖现在听起来更真实，因为她是真的看到了“玩家的行动”
            {f'Type: Dialogue | Character: {ROXY_CHAR_ID} | Mode: Prompt': textwrap.dedent("""
                你的内心活动：你刚刚目睹了 {player_name} ({player_last_action}) 的战斗方式并取得了胜利。
                请对他刚才的行动给予具体的表扬。
                **要求：对话要真诚、简短，并可略带一丝欣慰。**
                """).strip()}
        ],
        'EndCondition': {'Type': 'Linear', 'NextUnitID': '01_Explore_Corridor'}
    }
    write_yaml_file(os.path.join(story_dir, '03A_Player_Fights.yaml'), unit_03A)

    # --- Unit 03B: 洛琪希战斗 ---
    unit_03B = {
        'SceneConfig': {'id': '03B_Roxy_Fights', 'name': '洛琪希战斗'},
        'Events': [
            {f'Type: Dialogue | Character: {ROXY_CHAR_ID} | Mode: Preset': '“交给我吧。看好了，{player_name}さん。这就是水系魔法的威力！”'},
            # 修改点: 同样约束了旁白的长度
            {'Type: Narration | Mode: Prompt': textwrap.dedent("""
                你是一个战斗旁白。请简短描述洛琪希如何使用强大的水系魔法（如水箭、冰枪）瞬间击败怪物的帅气场景。
                **要求：描述要华丽但简洁，两三句话即可。**
                """).strip()},
            {'Type: Narration | Mode: Preset': '怪物在强大的魔力下灰飞烟灭。'},
            {'Type: Action | Tool: Calculate | Variable: monsters_defeated': {'Expression': '{monsters_defeated} + 1'}},
        ],
        'EndCondition': {'Type': 'Linear', 'NextUnitID': '01_Explore_Corridor'}
    }
    write_yaml_file(os.path.join(story_dir, '03B_Roxy_Fights.yaml'), unit_03B)
    
    # --- Unit 04: 踩到陷阱 ---
    unit_04 = {
        'SceneConfig': {'id': '04_Event_Trap', 'name': '踩到陷阱'},
        'Events': [
            {'Type: Narration | Mode: Preset': '你脚下传来“咔嚓”一声轻响！'},
            {f'Type: Dialogue | Character: {ROXY_CHAR_ID} | Mode: Preset': '“小心，是陷阱！”'},
            # 新增: 玩家决定如何应对陷阱
            {'Type: Player | Mode: Input': '（在这千钧一发之际，你下意识地做出反应！）'},
            {'Type: Action | Tool: Set | Variable: player_last_action': {'Value': "{player_input}"}},
            {'Type: Action | Tool: Random | Variable: dice_roll': {'Min': 1, 'Max': 20}},
        ],
        'EndCondition': {'Type': 'Conditional', 'Cases': [{'Condition': '{dice_roll} > 10', 'Then': {'Type': 'Linear', 'NextUnitID': '04A_Trap_Dodged'}}], 'Else': {'Type': 'Linear', 'NextUnitID': '04B_Trap_Hit'}}
    }
    write_yaml_file(os.path.join(story_dir, '04_Event_Trap.yaml'), unit_04)

    # --- Unit 04A: 躲开陷阱 ---
    unit_04A = {
        'SceneConfig': {'id': '04A_Trap_Dodged', 'name': '躲开陷阱'},
        'Events': [
            # 修改点: 旁白结合玩家的输入
            {'Type: Narration | Mode: Prompt': textwrap.dedent("""
                你是一个游戏旁白。玩家踩到了陷阱，他的下意识反应是："{player_last_action}"。
                请基于这个反应，描述他如何成功躲开了陷阱（比如从墙壁射出的毒箭）。
                **要求：描述简短，一两句话。**
                """).strip()},
            {f'Type: Dialogue | Character: {ROXY_CHAR_ID} | Mode: Preset': '“好险！反应很快嘛，{player_name}さん。”'},
            {'Type: Action | Tool: Calculate | Variable: traps_disarmed': {'Expression': '{traps_disarmed} + 1'}}
        ],
        'EndCondition': {'Type': 'Linear', 'NextUnitID': '01_Explore_Corridor'}
    }
    write_yaml_file(os.path.join(story_dir, '04A_Trap_Dodged.yaml'), unit_04A)

    # --- Unit 04B: 被陷阱击中 ---
    unit_04B = {
        'SceneConfig': {'id': '04B_Trap_Hit', 'name': '被陷阱击中'},
        'Events': [
            # 修改点: 旁白结合玩家的输入
             {'Type: Narration | Mode: Prompt': textwrap.dedent("""
                你是一个游戏旁白。玩家踩到了陷阱，他的下意识反应是："{player_last_action}"。
                请基于这个反应，描述他虽然尽力了，但还是被陷阱击中了（比如手臂被划伤）。
                **要求：描述简短，一两句话。**
                """).strip()},
            {'Type: Action | Tool: Calculate | Variable: player_hp': {'Expression': '{player_hp} - 10'}},
            {'Type: Notice | Mode: Preset | Location: message': '你失去了10点生命值！当前HP: {player_hp}'},
            {f'Type: Dialogue | Character: {ROXY_CHAR_ID} | Mode: Preset': '“不要紧吧？我马上为你治疗！”'},
            {'Type: Narration | Mode: Preset': '洛琪希老师吟唱起咒语，一道柔和的绿光包裹了你的伤口。'},
            {'Type: Action | Tool: Calculate | Variable: player_hp': {'Expression': '{player_hp} + 10'}},
            {'Type: Notice | Mode: Preset | Location: message': '洛琪希为你恢复了10点生命值！当前HP: {player_hp}'},
        ],
        'EndCondition': {'Type': 'Linear', 'NextUnitID': '01_Explore_Corridor'}
    }
    write_yaml_file(os.path.join(story_dir, '04B_Trap_Hit.yaml'), unit_04B)

    # --- Unit 99: 离开迷宫 (结局) ---
    unit_99 = {
        'SceneConfig': {'id': '99_Exit_Labyrinth', 'name': '离开迷宫', 'visuals': {'background_image': 'bg_labyrinth_entrance.png'}, 'audio': {'background_music': 'bgm_town_evening.mp3'}},
        'Events': [
            {'Type: Narration | Mode: Preset': '你们顺着原路返回，终于再次看到了迷宫入口的光芒。'},
            {f'Type: Dialogue | Character: {ROXY_CHAR_ID} | Mode: Preset': '“呼...总算是出来了。这次的探索收获很大呢，辛苦了，{player_name}さん。”'},
            {'Type: Notice | Mode: Preset | Location: popup': textwrap.dedent("""
                探险结束！
                最终到达楼层: {labyrinth_floor}
                击败怪物数量: {monsters_defeated}
                发现宝藏数量: {treasure_found}
                解除陷阱数量: {traps_disarmed}
                与洛琪希的好感度: {favorability_Roxy}
                """).strip()},
            {'Type: Narration | Mode: Preset': '夕阳下，你和老师的身影被拉得很长。下一次冒险，又会在哪里呢？'},
            {'Type: Chapter | Mode: Preset': {'Title': "探险结束", 'Number': 99, 'Description': "感谢游玩！"}}
        ],
        'EndCondition': None # None 表示游戏结束
    }
    write_yaml_file(os.path.join(story_dir, '99_Exit_Labyrinth.yaml'), unit_99)


# --- 主程序 ---
def main():
    """脚本主入口"""
    print("=" * 50)
    print(" NeoChat 剧本包生成器 (高互动版)")
    print(" 剧本: 与洛琪希的地下迷宫探险")
    print("=" * 50)

    try:
        pack_path = create_directories()
        generate_roxy_character()
        generate_global_config(pack_path)
        generate_intro_md(pack_path)
        generate_gamestate(pack_path)
        generate_story_units(pack_path)

        print("\n" + "=" * 50)
        print("🎉 恭喜！高互动版剧本包已成功生成！")
        print(f"剧本路径: {os.path.join(STORY_PACKS_BASE_PATH, STORY_PACK_NAME)}")
        print(f"角色路径: {os.path.join(CHARACTERS_BASE_PATH, f'{ROXY_CHAR_ID}.yaml')}")
        print("现在你可以在 NeoChat 中加载这个新剧本，享受更高的自由度！")
        print("=" * 50)

    except Exception as e:
        print("\n" + "!" * 50)
        print(f"❌ 生成过程中发生错误: {e}")
        print("!" * 50)

if __name__ == "__main__":
    main()