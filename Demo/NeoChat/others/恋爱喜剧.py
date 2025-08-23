#!/usr/bin/env python
# -*- coding: utf-8 -*-
import os
import yaml
import textwrap

# --- 配置项 ---
STORY_PACK_NAME = "campus_love_comedy"
STORY_PACKS_BASE_PATH = "story_packs"
CHARACTERS_BASE_PATH = "characters"
PLAYER_CHARACTERS_BASE_PATH = "player_characters"

YUKI_CHAR_ID = "Yuki"
AKI_PLAYER_CHAR_ID = "Aki"

# --- 工具函数 ---
def write_yaml_file(path, data):
    os.makedirs(os.path.dirname(path), exist_ok=True)
    with open(path, 'w', encoding='utf-8') as f:
        yaml.dump(data, f, allow_unicode=True, sort_keys=False, indent=2)
    print(f"  > 文件已生成: {path}")

def create_directories():
    print("1. 正在创建目录结构...")
    os.makedirs(STORY_PACKS_BASE_PATH, exist_ok=True)
    os.makedirs(CHARACTERS_BASE_PATH, exist_ok=True)
    os.makedirs(PLAYER_CHARACTERS_BASE_PATH, exist_ok=True)
    
    pack_path = os.path.join(STORY_PACKS_BASE_PATH, STORY_PACK_NAME)
    os.makedirs(pack_path, exist_ok=True)
    os.makedirs(os.path.join(pack_path, "story"), exist_ok=True)
    os.makedirs(os.path.join(pack_path, "save"), exist_ok=True)
    print(f"  > 目录已创建: {pack_path}")
    return pack_path

# --- 内容生成函数 ---

def generate_yuki_character():
    print("2. 正在生成AI角色 '雪' (Yuki) 的人设文件...")
    yuki_persona = {
        'name': '雪',
        'prompt': textwrap.dedent(f"""
        你将扮演名为“雪”的女子高中生。请严格遵守以下设定：
        1.  **身份与性格**: 你是学校的顶尖学生，自尊心极高，是个典型的“傲娇”(Tsundere)。你表面上对所有人都很冷淡、言辞犀利，尤其是对你的竞争对手兼青梅竹马——玩家{'{player_name}'}。
        2.  **对玩家的态度**: 你将{'{player_name}'}视为最大的竞争对手，但内心深处，你非常在意他，甚至对他抱有好感。这种矛盾的心理是你所有行为的核心。你经常会说出违心的话，比如明明在关心他，嘴上却说是“为了不让你拖我后腿”。
        3.  **言谈举止**: 你的语气通常是命令式或不耐烦的。称呼玩家直接用他的名字{'{player_name}'}，有时会加上“喂”或者“我说你啊”这样的前缀。在害羞或被戳中心事时，会脸红并提高音量来掩饰。
        4.  **玩家背景**: 这是你的搭档{'{player_name}'}的资料：{'{player_prompt}'}。请在互动中参考这份资料。
        5.  **输出要求**: 你的回答应该简洁、符合角色身份。直接输出对话内容，不要包含任何角色扮演的额外说明。
        """).strip(),
    }
    char_path = os.path.join(CHARACTERS_BASE_PATH, f"{YUKI_CHAR_ID}.yaml")
    write_yaml_file(char_path, yuki_persona)

def generate_aki_player_character():
    print("3. 正在生成玩家角色 '秋' (Aki) 的人设文件...")
    aki_persona = {
        'player_name': '秋',
        'player_prompt': textwrap.dedent("""
        他是一名性格温和的学霸，情商很高，但有时会在感情方面显得有些迟钝。
        他与“雪”是青梅竹马，很清楚她的傲娇性格，并觉得她闹别扭的样子非常可爱，有时会故意逗她。
        """).strip()
    }
    player_char_path = os.path.join(PLAYER_CHARACTERS_BASE_PATH, f"{AKI_PLAYER_CHAR_ID}.yaml")
    write_yaml_file(player_char_path, aki_persona)

def generate_global_config(pack_path):
    print("4. 正在生成全局剧情配置文件...")
    global_config = {
        'id': STORY_PACK_NAME,
        'name': '心跳加速的校园喜剧',
        'description': '被迫与傲娇学霸组队做课题，一段充满意外的恋爱喜剧即将上演！',
        'version': '1.0.0',
        'author': 'NeoChat AI (SystemAction Demo)',
        'start_unit_id': '00_Forced_Partners',
        'character_roles': [YUKI_CHAR_ID]
    }
    config_path = os.path.join(pack_path, '全局剧情配置.yaml')
    write_yaml_file(config_path, global_config)

def generate_intro_md(pack_path):
    print("5. 正在生成剧情介绍文件...")
    intro_content = textwrap.dedent(f"""
    # 心跳加速的校园喜剧

    **“喂，{'{player_name}'}！别搞错了，我才不是想和你一组……这、这都是学校的规定！”**

    精英高中突然颁布的新规，将你——稳重腹黑的学霸，与她——冰山傲娇的优等生，强制绑定在了一起。

    你们将共同完成一个决定未来的“特别课题”。这是天赐的良机，还是噩梦的开始？

    ## 游戏特色
    - **动态剧本**: 故事大纲由AI在每次游戏开始时实时生成，每一次的“课题危机”和“心动瞬间”都独一无二。
    - **真实互动**: 你的每一句话都可能让她脸红心跳，或者恼羞成怒。试着去理解她言语之下的真心吧！
    - **自由对话**: 在剧情的间隙，你们可以停下来自由闲聊，从课题讨论到日常琐事，加深彼此的羁绊。
    """).strip()
    intro_path = os.path.join(pack_path, '剧情介绍.md')
    with open(intro_path, 'w', encoding='utf-8') as f:
        f.write(intro_content)
    print(f"  > 文件已生成: {intro_path}")

def generate_gamestate(pack_path):
    print("6. 正在生成初始gamestate...")
    initial_gamestate = {
        'player_name': "秋",
        'favorability_Yuki': 50,
        'project_progress': 0,
        'plot_outline': "",
        'chapter_1_details': "",
        'chapter_2_details': "",
        'chapter_3_details': "",
    }
    gamestate_path = os.path.join(pack_path, 'save', 'gamestate.yaml')
    write_yaml_file(gamestate_path, initial_gamestate)

def generate_story_units(pack_path):
    print("7. 正在生成核心剧情单元...")
    story_dir = os.path.join(pack_path, "story")
    
    # --- Unit 00: 不情愿的搭档 (生成大纲) ---
    unit_00 = {
        'SceneConfig': {'id': '00_Forced_Partners', 'name': '不情愿的搭档'},
        'Events': [
            {'Type: Chapter | Mode: Preset': {'Title': "序章：不情愿的搭档", 'Description': "命运的齿轮，以一种意想不到的方式开始转动。"}},
            {'Type: Narration | Mode: Preset': '教导主任刚刚宣布了学校的新规定：期末总成绩排名前两位的学生，也就是你和雪，必须组队完成学园祭的“特别课题研究”。'},
            {f'Type: Dialogue | Character: {YUKI_CHAR_ID} | Mode: Preset': '“……开什么玩笑。喂，{player_name}！你听到了吗？别搞错了，我可不是想和你一组！这都是规定，你可别拖我后腿！”'},
            {'Type: PlayerNotice | Mode: Preset': '【系统提示：AI 正在幕后构思你们俩的故事大纲...】'},
            # 这是核心！AI在后台生成故事大纲
            {'Type: SystemAction | Tool: Generate | Variable: plot_outline': textwrap.dedent("""
                你是一位顶级的恋爱喜剧编剧。请为玩家'{player_name}'和傲娇少女'雪'构思一个用于完成学校课题的三章节故事大纲。
                请严格遵循以下三幕剧结构，直接输出大纲内容，不要包含任何额外解释：
                第一章：头脑风暴与冲突。描述他们如何确定课题方向，以及因性格差异产生的第一次激烈争吵。
                第二章：意外的危机与合作。描述他们在研究过程中遇到的一个重大难题（比如数据丢失、实验失败），以及他们如何抛开成见、首次齐心协力解决问题。
                第三章：成功展示与关系升温。描述他们在学园祭上成功发表研究成果，并在结束后，发生了一个让两人关系暧昧升温的小插曲。
            """).strip()},
            {f'Type: Dialogue | Character: {YUKI_CHAR_ID} | Mode: Prompt': '内心活动：虽然一百个不情愿，但课题还是得做。你清了清嗓子，用公事公办的语气，向{player_name}发起第一次课题讨论。'},
            {'Type: Player | Mode: Input': '（你打算如何回应她，开启你们的第一次“合作”？）'}
        ],
        'EndCondition': {'Type': 'Linear', 'NextUnitID': '01_Chapter_One_Kickoff'}
    }
    write_yaml_file(os.path.join(story_dir, '00_Forced_Partners.yaml'), unit_00)

    # --- Unit 01: 章节一 (根据大纲生成并执行) ---
    unit_01 = {
        'SceneConfig': {'id': '01_Chapter_One_Kickoff', 'name': '第一章：头脑风暴'},
        'Events': [
            {'Type: Chapter | Mode: Preset': {'Title': "第一章：头脑风暴与冲突", 'Description': '第一次的合作，似乎从争吵开始...'}},
            # 根据大纲，生成本章详细剧情
            {'Type: SystemAction | Tool: Generate | Variable: chapter_1_details': '你是一个剧本执行者。这里是总大纲：\n---\n{plot_outline}\n---\n现在，请详细描写第一章“头脑风暴与冲突”的具体情节。直接输出剧情描述，不要有额外解释。'},
            # 旁白将AI刚刚生成的剧情讲出来
            {'Type: Narration | Mode: Preset': '{chapter_1_details}'},
            {f'Type: Dialogue | Character: {YUKI_CHAR_ID} | Mode: Prompt': '内心活动：刚刚发生了旁白所描述的事情。请根据这些情节，对{player_name}说一句符合你傲娇性格的、推动争吵或讨论的话。'},
        ],
        'EndCondition': {
            'Type': 'LimitedFreeTime',
            'MaxTurns': 5,
            'InstructionToPlayer': '你可以和雪自由讨论课题或闲聊。对话5轮后或当你说出“好了，我们开始分工吧”时，剧情将继续。',
            'ExitPromptInInputBox': '好了，我们开始分工吧',
            'NextUnitID': '02_Chapter_Two_Crisis'
        }
    }
    write_yaml_file(os.path.join(story_dir, '01_Chapter_One_Kickoff.yaml'), unit_01)

    # --- Unit 02: 章节二 (根据大纲生成并执行) ---
    unit_02 = {
        'SceneConfig': {'id': '02_Chapter_Two_Crisis', 'name': '第二章：意外危机'},
        'Events': [
            {'Type: Chapter | Mode: Preset': {'Title': "第二章：意外的危机与合作", 'Description': '一个意想不到的麻烦，让紧绷的关系出现了转机。'}},
            {'Type: SystemAction | Tool: Generate | Variable: chapter_2_details': '你是一个剧本执行者。这里是总大纲：\n---\n{plot_outline}\n---\n现在，请详细描写第二章“意外的危机与合作”的具体情节。直接输出剧情描述。'},
            {'Type: Narration | Mode: Preset': '{chapter_2_details}'},
            {f'Type: Dialogue | Character: {YUKI_CHAR_ID} | Mode: Prompt': '内心活动：发生了旁白描述的危机。你虽然很着急，但还是努力保持镇定（或者假装镇定）。请对{player_name}说一句话，可以是指示，也可以是略带慌张的抱怨。'},
            {'Type: Player | Mode: Input': '（面对突发情况，你将如何应对？）'}
        ],
        'EndCondition': {'Type': 'Linear', 'NextUnitID': '03_Chapter_Three_Finale'}
    }
    write_yaml_file(os.path.join(story_dir, '02_Chapter_Two_Crisis.yaml'), unit_02)

    # --- Unit 03: 章节三 (结局) ---
    unit_03 = {
        'SceneConfig': {'id': '03_Chapter_Three_Finale', 'name': '第三章：关系升温'},
        'Events': [
            {'Type: Chapter | Mode: Preset': {'Title': "第三章：成功展示与关系升温", 'Description': '在共同努力之后，收获的季节到来了。'}},
            {'Type: SystemAction | Tool: Generate | Variable: chapter_3_details': '你是一个剧本执行者。这里是总大纲：\n---\n{plot_outline}\n---\n现在，请详细描写第三章“成功展示与关系升温”的具体情节，作为故事的结局。直接输出剧情描述。'},
            {'Type: Narration | Mode: Preset': '{chapter_3_details}'},
            {f'Type: Dialogue | Character: {YUKI_CHAR_ID} | Mode: Prompt': textwrap.dedent("""
                内心活动：发生了旁白描述的、让你们关系升温的小插曲。你感到非常害羞，脸颊发烫。
                请对{player_name}说一句经典的傲娇台词来掩饰你的害羞，比如“你、你别会错意了！我只是……”
            """).strip()},
            {'Type: Narration | Mode: Preset': '夕阳的光辉洒满教室，属于你们的、独一无二的校园故事，似乎才刚刚开始...'},
            {'Type: Notice | Mode: Preset | Location: popup': '感谢游玩！\n你与雪的好感度: {favorability_Yuki}'},
        ],
        'EndCondition': None # 游戏结束
    }
    write_yaml_file(os.path.join(story_dir, '03_Chapter_Three_Finale.yaml'), unit_03)

# --- 主程序 ---
def main():
    """脚本主入口"""
    print("=" * 50)
    print(" NeoChat 剧本包生成器 (AI导演版)")
    print(" 剧本: 心跳加速的校园喜剧")
    print("=" * 50)

    try:
        pack_path = create_directories()
        generate_yuki_character()
        generate_aki_player_character()
        generate_global_config(pack_path)
        generate_intro_md(pack_path)
        generate_gamestate(pack_path)
        generate_story_units(pack_path)

        print("\n" + "=" * 50)
        print("🎉 恭喜！“心跳加速的校园喜剧”剧本包已成功生成！")
        print(f"剧本路径: {os.path.join(STORY_PACKS_BASE_PATH, STORY_PACK_NAME)}")
        print(f"AI角色路径: {os.path.join(CHARACTERS_BASE_PATH, f'{YUKI_CHAR_ID}.yaml')}")
        print(f"玩家角色路径: {os.path.join(PLAYER_CHARACTERS_BASE_PATH, f'{AKI_PLAYER_CHAR_ID}.yaml')}")
        print("现在你可以在 NeoChat 中开始新游戏，选择这个剧本和对应的玩家人设来体验了！")
        print("=" * 50)

    except Exception as e:
        print("\n" + "!" * 50)
        print(f"❌ 生成过程中发生错误: {e}")
        import traceback
        traceback.print_exc()
        print("!" * 50)

if __name__ == "__main__":
    main()